use std::borrow::Cow;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use monty::{
    ExcType, LimitedTracker, MontyDate, MontyDateTime, MontyException, MontyObject, MontyRun,
    NameLookupResult, OsFunction, PrintWriter, PrintWriterCallback, ResourceLimits, RunProgress,
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
use crate::sandbox::SandboxPolicy;
use crate::template;

/// Exit code used when the sandbox aborts execution (time or memory limit exceeded).
/// 128 + 9 (SIGKILL-ish); mirrors OOM-killer convention and stands out from generic failures.
pub const SANDBOX_EXPLODED_EXIT_CODE: i32 = 137;

/// Error returned when a sandbox limit is exceeded. Carries the stderr message the CLI should print.
#[derive(Debug)]
pub struct SandboxLimitExceeded {
    pub message: String,
}

impl std::fmt::Display for SandboxLimitExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SandboxLimitExceeded {}

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

/// Runtime options for mold execution.
pub struct MoldOptions<'a> {
    pub extra_args: &'a [(String, String)],
    pub env_value: &'a Value,
    pub headers_value: &'a Value,
    pub debug: bool,
    pub msg_level: u8,
    pub mold_base_dir: Option<&'a str>,
    pub policy: &'a SandboxPolicy,
}

/// Internal runtime context — extends MoldOptions with mutable shared state
/// for exit code, format override, and output file.
struct MoldContext<'a> {
    debug: bool,
    msg_level: u8,
    mold_base_dir: Option<&'a str>,
    exit_code: Arc<Mutex<Option<i32>>>,
    format_override: Arc<Mutex<Option<String>>>,
    output_file: Arc<Mutex<Option<String>>>,
    policy: &'a SandboxPolicy,
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
        || template::EXTERNAL_FUNCTIONS.contains(&name)
}

/// Route an external function call to the correct module.
/// Takes `args` by value so each module can consume without cloning.
fn dispatch_external(
    name: &str,
    args: Vec<MontyObject>,
    ctx: &MoldContext<'_>,
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
        exit_control::dispatch(name, args, &ctx.exit_code)
    } else if format_control::EXTERNAL_FUNCTIONS.contains(&name) {
        format_control::dispatch(name, args, &ctx.format_override, &ctx.output_file)
    } else if msg::EXTERNAL_FUNCTIONS.contains(&name) {
        msg::dispatch(name, args, ctx.msg_level)
    } else if gatekeeper::EXTERNAL_FUNCTIONS.contains(&name) {
        gatekeeper::dispatch(name, args, &ctx.exit_code)
    } else if env_helpers::EXTERNAL_FUNCTIONS.contains(&name) {
        env_helpers::dispatch(name, args)
    } else if template::EXTERNAL_FUNCTIONS.contains(&name) {
        template::dispatch(name, args, ctx.mold_base_dir)
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
pub fn execute_mold(script: &str, data: MontyObject, opts: &MoldOptions<'_>) -> MoldResult {
    // Build the `args` dict as a MontyObject
    let args_dict = MontyObject::Dict(monty::DictPairs::from(
        opts.extra_args
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
    let env_obj = json_to_monty(opts.env_value);
    let headers_obj = json_to_monty(opts.headers_value);

    // Namespace: data + args + env + headers (passed as function parameters, not globals)
    let input_names = vec![
        "data".to_string(),
        "args".to_string(),
        "env".to_string(),
        "headers".to_string(),
    ];
    let inputs = vec![data, args_dict, env_obj, headers_obj];

    let full_script = format!("{script}\ntransform(data, args=args, env=env, headers=headers)");

    if opts.debug {
        eprintln!("[debug] script:");
        eprintln!("---");
        eprintln!("{}", full_script.trim_end());
        eprintln!("---");
    }

    let runner = MontyRun::new(full_script, "mold.py", input_names)
        .context("Failed to compile mold script")?;

    let ctx = MoldContext {
        debug: opts.debug,
        msg_level: opts.msg_level,
        mold_base_dir: opts.mold_base_dir,
        exit_code: Arc::new(Mutex::new(None)),
        format_override: Arc::new(Mutex::new(None)),
        output_file: Arc::new(Mutex::new(None)),
        policy: opts.policy,
    };

    let result = run_loop(runner, inputs, &ctx)?;

    let exit_val = ctx.exit_code.lock().unwrap().take();
    let fmt_val = ctx.format_override.lock().unwrap().take();
    let out_file_val = ctx.output_file.lock().unwrap().take();
    Ok((result, exit_val, fmt_val, out_file_val))
}

fn run_loop(runner: MontyRun, inputs: Vec<MontyObject>, ctx: &MoldContext<'_>) -> Result<Value> {
    let mut sp = StderrPrint;
    let tracker = LimitedTracker::new(build_limits(ctx.policy));
    let mut progress = runner
        .start(
            inputs,
            tracker,
            if ctx.debug {
                PrintWriter::Callback(&mut sp)
            } else {
                PrintWriter::Stdout
            },
        )
        .map_err(|e| translate_monty_error(e, ctx.policy))?;

    loop {
        match progress {
            RunProgress::Complete(result) => {
                return monty_to_json(result).context("Failed to convert Monty result to JSON");
            }
            RunProgress::FunctionCall(mut call) => {
                let function_name = call.function_name.clone();
                let args = std::mem::take(&mut call.args);
                let result = dispatch_external(&function_name, args, ctx).map_err(|e| {
                    anyhow::anyhow!("External function '{function_name}' failed: {e}")
                })?;
                let mut sp2 = StderrPrint;
                progress = call
                    .resume(
                        result,
                        if ctx.debug {
                            PrintWriter::Callback(&mut sp2)
                        } else {
                            PrintWriter::Stdout
                        },
                    )
                    .map_err(|e| translate_monty_error(e, ctx.policy))?;
            }
            RunProgress::OsCall(call) => {
                let result = dispatch_os_call(&call.function, &call.args, ctx.policy);
                if ctx.debug {
                    eprintln!(
                        "[debug] OsCall {:?} -> {}",
                        call.function,
                        describe_os_result(&result)
                    );
                }
                let mut sp2 = StderrPrint;
                progress = call
                    .resume(
                        result,
                        if ctx.debug {
                            PrintWriter::Callback(&mut sp2)
                        } else {
                            PrintWriter::Stdout
                        },
                    )
                    .map_err(|e| translate_monty_error(e, ctx.policy))?;
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
                        if ctx.debug {
                            PrintWriter::Callback(&mut sp2)
                        } else {
                            PrintWriter::Stdout
                        },
                    )
                    .map_err(|e| translate_monty_error(e, ctx.policy))?;
            }
            RunProgress::ResolveFutures(_) => {
                anyhow::bail!("Async futures are not supported in fimod molds");
            }
        }
    }
}

/// Build `ResourceLimits` from a `SandboxPolicy`.
fn build_limits(policy: &SandboxPolicy) -> ResourceLimits {
    let mut limits = ResourceLimits::new();
    if let Some(d) = policy.max_duration {
        limits = limits.max_duration(d);
    }
    if let Some(m) = policy.max_memory {
        limits = limits.max_memory(m);
    }
    limits
}

/// Resolve an `OsCall` result according to the policy.
///
/// Capability-deny defaults follow Python ergonomics:
/// - Clock (`date.today`, `datetime.now`): explicit `PermissionError` with actionable hint when denied, because returning `None` would crash downstream `.isoformat()` calls.
/// - `os.getenv(key)`: returns `None` silently when `key` is not in `allow_env` — mirrors the standard Python behavior for unset vars.
/// - `os.environ`: returns an empty dict when denied (no raise).
/// - `Path.*`: returns `None` (legacy behavior; proper filesystem gating lands with `[[mount]]`).
fn dispatch_os_call(
    function: &OsFunction,
    args: &[MontyObject],
    policy: &SandboxPolicy,
) -> OsCallOutcome {
    match function {
        OsFunction::DateToday => {
            if policy.allow_clock {
                OsCallOutcome::Value(MontyObject::Date(current_date()))
            } else {
                OsCallOutcome::Error(permission_denied(clock_denied_message(function)))
            }
        }
        OsFunction::DateTimeNow => {
            if policy.allow_clock {
                OsCallOutcome::Value(MontyObject::DateTime(current_datetime()))
            } else {
                OsCallOutcome::Error(permission_denied(clock_denied_message(function)))
            }
        }
        OsFunction::Getenv => OsCallOutcome::Value(lookup_env(args, policy)),
        OsFunction::GetEnviron => OsCallOutcome::Value(empty_environ()),
        _ => OsCallOutcome::Value(MontyObject::None),
    }
}

fn clock_denied_message(function: &OsFunction) -> String {
    format!(
        "{function}() denied by sandbox policy — add `allow_clock = true` to your sandbox.toml (or run `fimod setup sandbox defaults`)"
    )
}

fn current_date() -> MontyDate {
    use chrono::{Datelike, Local};
    let now = Local::now().date_naive();
    MontyDate {
        year: now.year(),
        month: now.month() as u8,
        day: now.day() as u8,
    }
}

/// Naive datetime (no tz) matches Python's `datetime.now()` without args.
fn current_datetime() -> MontyDateTime {
    use chrono::{Datelike, Local, Timelike};
    let now = Local::now().naive_local();
    MontyDateTime {
        year: now.year(),
        month: now.month() as u8,
        day: now.day() as u8,
        hour: now.hour() as u8,
        minute: now.minute() as u8,
        second: now.second() as u8,
        microsecond: now.nanosecond() / 1_000,
        offset_seconds: None,
        timezone_name: None,
    }
}

/// Result of dispatching an `OsCall` — either a return value or a Python exception.
enum OsCallOutcome {
    Value(MontyObject),
    Error(MontyException),
}

impl From<OsCallOutcome> for monty::ExtFunctionResult {
    fn from(outcome: OsCallOutcome) -> Self {
        match outcome {
            OsCallOutcome::Value(v) => monty::ExtFunctionResult::Return(v),
            OsCallOutcome::Error(e) => monty::ExtFunctionResult::Error(e),
        }
    }
}

fn describe_os_result(outcome: &OsCallOutcome) -> String {
    match outcome {
        OsCallOutcome::Value(_) => "allowed".to_string(),
        OsCallOutcome::Error(_) => "denied".to_string(),
    }
}

fn permission_denied(msg: String) -> MontyException {
    MontyException::new(ExcType::PermissionError, Some(msg))
}

fn lookup_env(args: &[MontyObject], policy: &SandboxPolicy) -> MontyObject {
    let Some(MontyObject::String(key)) = args.first() else {
        return MontyObject::None;
    };
    if !policy.env_allowed(key) {
        return MontyObject::None;
    }
    match std::env::var(key) {
        Ok(v) => MontyObject::String(v),
        Err(_) => MontyObject::None,
    }
}

fn empty_environ() -> MontyObject {
    MontyObject::Dict(monty::DictPairs::from(
        Vec::<(MontyObject, MontyObject)>::new(),
    ))
}

/// Upgrades resource-limit exceptions (`TimeoutError`, `MemoryError`) into `SandboxLimitExceeded`
/// so the CLI can exit with 137.
fn translate_monty_error(err: MontyException, policy: &SandboxPolicy) -> anyhow::Error {
    match err.exc_type() {
        ExcType::TimeoutError => {
            limit_exceeded("max_duration", policy.max_duration.map(format_duration))
        }
        ExcType::MemoryError => limit_exceeded("max_memory", policy.max_memory.map(format_bytes)),
        _ => anyhow::anyhow!("Python error in mold:\n{err}"),
    }
}

fn limit_exceeded(kind: &str, limit: Option<String>) -> anyhow::Error {
    let limit = limit.unwrap_or_else(|| "n/a".to_string());
    anyhow::Error::new(SandboxLimitExceeded {
        message: format!("sandbox exploded: {kind} exceeded ({limit})"),
    })
}

fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs >= 3600 && secs % 3600 == 0 {
        format!("{}h", secs / 3600)
    } else if secs >= 60 && secs % 60 == 0 {
        format!("{}m", secs / 60)
    } else if secs > 0 {
        format!("{secs}s")
    } else {
        format!("{}ms", d.as_millis())
    }
}

fn format_bytes(b: usize) -> String {
    const KB: usize = 1_000;
    const MB: usize = 1_000_000;
    const GB: usize = 1_000_000_000;
    if b >= GB && b % GB == 0 {
        format!("{}GB", b / GB)
    } else if b >= MB && b % MB == 0 {
        format!("{}MB", b / MB)
    } else if b >= KB && b % KB == 0 {
        format!("{}KB", b / KB)
    } else {
        format!("{b}B")
    }
}
