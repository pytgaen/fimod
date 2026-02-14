use std::borrow::Cow;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use monty::{
    MontyException, MontyObject, MontyRun, NameLookupResult, NoLimitTracker, PrintWriter,
    PrintWriterCallback, RunProgress,
};
use serde_json::Value;

use crate::convert::{json_to_monty, monty_to_json};

/// Return type of mold execution: (output data, exit code, format override, output file override).
pub type MoldResult = Result<(Value, Option<i32>, Option<String>, Option<String>)>;
use crate::dotpath;
use crate::env_helpers;
use crate::exit_control;
use crate::format_control;
use crate::gatekeeper;
use crate::hash;
use crate::iter_helpers;
use crate::msg;
use crate::regex;

/// Custom PrintWriterCallback that redirects Monty's print() output to stderr.
/// Used in --debug mode so that Python print statements don't corrupt stdout.
struct StderrPrint;

impl PrintWriterCallback for StderrPrint {
    fn stdout_write(&mut self, output: Cow<'_, str>) -> Result<(), MontyException> {
        eprint!("{output}");
        Ok(())
    }

    fn stdout_push(&mut self, end: char) -> Result<(), MontyException> {
        eprint!("{end}");
        Ok(())
    }
}

/// Check whether a name is a known external function exposed to molds.
fn is_external_function(name: &str) -> bool {
    regex::EXTERNAL_FUNCTIONS.contains(&name)
        || dotpath::EXTERNAL_FUNCTIONS.contains(&name)
        || iter_helpers::EXTERNAL_FUNCTIONS.contains(&name)
        || hash::EXTERNAL_FUNCTIONS.contains(&name)
        || exit_control::EXTERNAL_FUNCTIONS.contains(&name)
        || format_control::EXTERNAL_FUNCTIONS.contains(&name)
        || msg::EXTERNAL_FUNCTIONS.contains(&name)
        || gatekeeper::EXTERNAL_FUNCTIONS.contains(&name)
        || env_helpers::EXTERNAL_FUNCTIONS.contains(&name)
}

/// Route an external function call to the correct module.
/// Takes `args` by value so each module can consume without cloning.
fn dispatch_external(
    name: &str,
    args: Vec<MontyObject>,
    exit_code: &Arc<Mutex<Option<i32>>>,
    format_override: &Arc<Mutex<Option<String>>>,
    output_file: &Arc<Mutex<Option<String>>>,
    msg_level: u8,
) -> Result<MontyObject> {
    if regex::EXTERNAL_FUNCTIONS.contains(&name) {
        regex::dispatch(name, args)
    } else if dotpath::EXTERNAL_FUNCTIONS.contains(&name) {
        dotpath::dispatch(name, args)
    } else if iter_helpers::EXTERNAL_FUNCTIONS.contains(&name) {
        iter_helpers::dispatch(name, args)
    } else if hash::EXTERNAL_FUNCTIONS.contains(&name) {
        hash::dispatch(name, args)
    } else if exit_control::EXTERNAL_FUNCTIONS.contains(&name) {
        exit_control::dispatch(name, args, exit_code)
    } else if format_control::EXTERNAL_FUNCTIONS.contains(&name) {
        format_control::dispatch(name, args, format_override, output_file)
    } else if msg::EXTERNAL_FUNCTIONS.contains(&name) {
        msg::dispatch(name, args, msg_level)
    } else if gatekeeper::EXTERNAL_FUNCTIONS.contains(&name) {
        gatekeeper::dispatch(name, args, exit_code)
    } else if env_helpers::EXTERNAL_FUNCTIONS.contains(&name) {
        env_helpers::dispatch(name, args)
    } else {
        anyhow::bail!("Unknown external function: {name}")
    }
}

/// Execute a mold Python script against input data using Monty.
///
/// The script must define a `transform(data, args, env, headers)` function.
/// All four parameters are always passed explicitly — no global variable injection.
///
/// Takes `data` as an owned `MontyObject` to avoid the json_to_monty conversion
/// when the caller has already built a MontyObject directly (e.g. csv_to_monty path).
///
/// Returns `(result_value, optional_exit_code, optional_format_override, optional_output_file)`.
pub fn execute_mold(
    script: &str,
    data: MontyObject,
    extra_args: &[(String, String)],
    env_value: &Value,
    headers_value: &Value,
    debug: bool,
    msg_level: u8,
) -> MoldResult {
    // Build the `args` dict as a MontyObject
    let args_dict = MontyObject::Dict(monty::DictPairs::from(
        extra_args
            .iter()
            .map(|(k, v)| {
                (
                    MontyObject::String(k.clone()),
                    MontyObject::String(v.clone()),
                )
            })
            .collect::<Vec<_>>(),
    ));

    // Build env and headers MontyObjects
    let env_obj = json_to_monty(env_value);
    let headers_obj = json_to_monty(headers_value);

    // Namespace: data + args + env + headers (passed as function parameters, not globals)
    let input_names = vec![
        "data".to_string(),
        "args".to_string(),
        "env".to_string(),
        "headers".to_string(),
    ];
    let inputs = vec![data, args_dict, env_obj, headers_obj];

    let full_script = format!("{script}\ntransform(data, args, env, headers)");

    if debug {
        eprintln!("[debug] script:");
        eprintln!("---");
        eprintln!("{}", full_script.trim_end());
        eprintln!("---");
    }

    let runner = MontyRun::new(full_script, "mold.py", input_names)
        .context("Failed to compile mold script")?;

    let exit_code = Arc::new(Mutex::new(None));
    let format_override = Arc::new(Mutex::new(None));
    let output_file = Arc::new(Mutex::new(None));

    let result = run_loop(
        runner,
        inputs,
        debug,
        &exit_code,
        &format_override,
        &output_file,
        msg_level,
    )?;

    let exit_val = exit_code.lock().unwrap().take();
    let fmt_val = format_override.lock().unwrap().take();
    let out_file_val = output_file.lock().unwrap().take();
    Ok((result, exit_val, fmt_val, out_file_val))
}

fn run_loop(
    runner: MontyRun,
    inputs: Vec<MontyObject>,
    debug: bool,
    exit_code: &Arc<Mutex<Option<i32>>>,
    format_override: &Arc<Mutex<Option<String>>>,
    output_file: &Arc<Mutex<Option<String>>>,
    msg_level: u8,
) -> Result<Value> {
    let mut sp = StderrPrint;
    let mut progress = runner
        .start(
            inputs,
            NoLimitTracker,
            if debug {
                PrintWriter::Callback(&mut sp)
            } else {
                PrintWriter::Stdout
            },
        )
        .map_err(|e| anyhow::anyhow!("Python error in mold:\n{e}"))?;

    loop {
        match progress {
            RunProgress::Complete(result) => {
                return monty_to_json(result).context("Failed to convert Monty result to JSON");
            }
            RunProgress::FunctionCall(mut call) => {
                let function_name = call.function_name.clone();
                let args = std::mem::take(&mut call.args);
                let result = dispatch_external(
                    &function_name,
                    args,
                    exit_code,
                    format_override,
                    output_file,
                    msg_level,
                )
                .map_err(|e| anyhow::anyhow!("External function '{function_name}' failed: {e}"))?;
                let mut sp2 = StderrPrint;
                progress = call
                    .resume(
                        result,
                        if debug {
                            PrintWriter::Callback(&mut sp2)
                        } else {
                            PrintWriter::Stdout
                        },
                    )
                    .map_err(|e| {
                        anyhow::anyhow!(
                            "Python error in mold (after calling '{function_name}'):\n{e}"
                        )
                    })?;
            }
            RunProgress::OsCall(call) => {
                let mut sp2 = StderrPrint;
                progress = call
                    .resume(
                        MontyObject::None,
                        if debug {
                            PrintWriter::Callback(&mut sp2)
                        } else {
                            PrintWriter::Stdout
                        },
                    )
                    .map_err(|e| anyhow::anyhow!("Python error in mold:\n{e}"))?;
            }
            RunProgress::NameLookup(lookup) => {
                let name = lookup.name.clone();
                let result = if is_external_function(&name) {
                    NameLookupResult::Value(MontyObject::Function {
                        name,
                        docstring: None,
                    })
                } else {
                    NameLookupResult::Undefined
                };
                let mut sp2 = StderrPrint;
                progress = lookup
                    .resume(
                        result,
                        if debug {
                            PrintWriter::Callback(&mut sp2)
                        } else {
                            PrintWriter::Stdout
                        },
                    )
                    .map_err(|e| anyhow::anyhow!("Python error in mold:\n{e}"))?;
            }
            RunProgress::ResolveFutures(_) => {
                anyhow::bail!("Async futures are not supported in fimod molds");
            }
        }
    }
}
