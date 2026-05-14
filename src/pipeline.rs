use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use monty::MontyObject;
use serde_json::Value;

pub use crate::engine::PipelineMetadata;

use crate::engine::{MoldExecResult, MoldResult, PendingOp};
use crate::format::{CsvOptions, DataFormat};
use crate::mold::{MoldSource, MoldStep, StepOrigin};
use crate::sandbox::SandboxPolicy;
use crate::{convert, engine, format, http, mold};

/// A single pipeline step reference — either a mold path/name or an inline expression.
#[derive(Debug, Clone)]
pub enum ScriptRef {
    Mold(String),
    Expr(String),
}

/// Outcome of a CLI invocation. Either completed normally, or the pipeline
/// requested the process to exit with a specific code (`set_exit()` in a mold,
/// `--check` mode, or a sandbox kill).
///
/// Returning this through `Result` instead of calling `process::exit()`
/// directly lets `pipeline.rs` stay free of side effects so it can be used as
/// a library; only `main()` is allowed to actually exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "CliResult::Exit must be propagated to main() so the process exits with the right code"]
pub enum CliResult {
    Done,
    Exit(i32),
}

fn debug_phase(debug: bool, label: &str, start: Instant) {
    if debug {
        eprintln!("[debug] {label}: {:.3}s", start.elapsed().as_secs_f64());
    }
}

/// Determine if a JSON value is "truthy" for --check mode.
/// Falsy: null, false, 0, "", [], {}
/// Everything else is truthy.
pub fn is_truthy(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i != 0
            } else if let Some(f) = n.as_f64() {
                f != 0.0
            } else {
                true
            }
        }
        serde_json::Value::String(s) => !s.is_empty(),
        serde_json::Value::Array(a) => !a.is_empty(),
        serde_json::Value::Object(o) => !o.is_empty(),
    }
}

/// Build the list of mold steps from an ordered sequence of script references.
///
/// Returns at least one step, or an error if the list is empty.
pub fn build_scripts(refs: &[ScriptRef], no_cache: bool) -> Result<Vec<MoldStep>> {
    if refs.is_empty() {
        bail!("Either -m/--mold or -e/--expression is required");
    }
    let mut steps = Vec::new();
    for r in refs {
        match r {
            ScriptRef::Expr(e) => {
                let source = MoldSource::Inline(e.clone());
                let script = source.load(no_cache)?;
                steps.push(MoldStep {
                    source,
                    script,
                    defaults: mold::MoldDefaults::default(),
                    runtime_args: None,
                    origin: StepOrigin::Cli,
                });
            }
            ScriptRef::Mold(m) => {
                let source = MoldSource::from_mold_str(m, no_cache)?;
                let is_inline = matches!(source, MoldSource::Inline(_));
                let script = source.load(no_cache)?;
                let defaults = if !is_inline {
                    mold::parse_mold_defaults(&script)
                } else {
                    mold::MoldDefaults::default()
                };
                steps.push(MoldStep {
                    source,
                    script,
                    defaults,
                    runtime_args: None,
                    origin: StepOrigin::Cli,
                });
            }
        }
    }
    Ok(steps)
}

/// Cross-cutting execution concerns shared by every step in a chain.
/// (`headers_value` is intentionally separate — it's derived per-input from CSV
/// state right before `execute_chain` runs, and doesn't belong in a context
/// constructed earlier by the caller.)
pub struct ChainExecCtx<'a> {
    pub extra_args: &'a [(String, String)],
    pub env_value: &'a Value,
    pub policy: &'a SandboxPolicy,
    pub debug: bool,
    pub msg_level: u8,
}

/// Execute a chain of mold scripts sequentially, with dynamic step insertion.
///
/// Molds can inject new steps via `pipeline.insert_next()` / `pipeline.append()`,
/// and mutate future steps via `pipeline.step(i)['key'] = value`.
pub fn execute_chain(
    steps: &[MoldStep],
    initial_data: MontyObject,
    metadata: &PipelineMetadata<'_>,
    headers_value: &Value,
    ctx: &ChainExecCtx<'_>,
) -> MoldResult {
    let mut steps: Vec<MoldStep> = steps.to_vec();
    let mut data = initial_data;
    let mut last_exit = None;
    // Pending mutations keyed by absolute step index → {field → value}.
    let mut mutations: HashMap<usize, HashMap<String, Value>> = HashMap::new();

    // Step-level format overrides resolve against these as their fallback.
    let base_input_format = metadata.input_format;
    let base_output_format = metadata.output_format;

    let mut i = 0;
    while i < steps.len() {
        let step_script = steps[i].script.clone();
        let step_base_dir = steps[i].base_dir();
        // Pending `set('args', ...)` mutation overrides the static spec for this step.
        let step_runtime_args = mutations
            .get(&i)
            .and_then(|m| m.get("args"))
            .cloned()
            .or_else(|| steps[i].runtime_args.clone());

        if ctx.debug {
            eprintln!("[debug] {}", steps[i].error_context(i, steps.len()));
        }

        // Apply pending format mutations for this step.
        let step_input_format = mutations
            .get(&i)
            .and_then(|m| m.get("input_format"))
            .and_then(Value::as_str)
            .or(base_input_format);
        let step_output_format_mutation = mutations
            .get(&i)
            .and_then(|m| m.get("output_format"))
            .and_then(Value::as_str)
            .map(String::from);
        let step_output_format = step_output_format_mutation
            .as_deref()
            .or(base_output_format);
        let step_output_file_override = mutations
            .get(&i)
            .and_then(|m| m.get("output_file"))
            .and_then(Value::as_str)
            .map(String::from);

        // Build remaining_steps specs so pipeline.step(j) works for j > i.
        // Pending `set('args', ...)` mutations targeting future steps are overlaid
        // here so that a later `step(j).get('args')` reflects the mutation.
        let remaining_steps: Vec<Value> = steps[i + 1..]
            .iter()
            .enumerate()
            .map(|(offset, step)| {
                let abs_idx = i + 1 + offset;
                let mut spec = step_to_remaining_spec(step);
                if let Some(args_mut) = mutations.get(&abs_idx).and_then(|m| m.get("args")) {
                    if let Some(obj) = spec.as_object_mut() {
                        obj.insert("args".into(), args_mut.clone());
                    }
                }
                spec
            })
            .collect();

        let opts = engine::MoldOptions {
            extra_args: ctx.extra_args,
            env_value: ctx.env_value,
            headers_value,
            debug: ctx.debug,
            msg_level: ctx.msg_level,
            mold_base_dir: step_base_dir.as_deref(),
            policy: ctx.policy,
            metadata,
            current_step_idx: i,
            total_steps: steps.len(),
            remaining_steps,
            input_format: step_input_format,
            output_format: step_output_format,
            output_file_override: step_output_file_override,
            format_override_init: step_output_format_mutation.clone(),
            step_args: step_runtime_args,
        };
        let exec = engine::execute_mold(&step_script, data, &opts)
            .with_context(|| format!("in {}", steps[i].error_context(i, steps.len())))?;

        // Accumulate pending mutations from this execution.
        for m in exec.pending_mutations {
            mutations
                .entry(m.step_idx)
                .or_default()
                .insert(m.key, m.value);
        }

        // Apply pending exit mutations for THIS step (from prior steps' mutations).
        if let Some(step_mutations) = mutations.remove(&i) {
            for (key, val) in step_mutations {
                if key == "exit" {
                    if let Some(code) = val.as_i64() {
                        last_exit = Some(code as i32);
                    }
                }
            }
        }

        // Inject new steps requested by this mold.
        let mut insert_at = i + 1;
        for ps in exec.pending_steps {
            let new_step = resolve_pending(ps.spec, i)
                .with_context(|| format!("while resolving step injected by step {}", i + 1))?;
            match ps.op {
                PendingOp::InsertNext => {
                    steps.insert(insert_at, new_step);
                    insert_at += 1;
                }
                PendingOp::Append => {
                    steps.push(new_step);
                }
            }
        }

        let result = exec.value;
        let fmt_override = exec.format_override;
        let out_file = exec.output_file;

        if let Some(c) = exec.exit_code {
            last_exit = Some(c);
        }

        let is_last = i == steps.len() - 1;

        if let Some(ref fmt_name) = fmt_override {
            if is_last {
                return Ok(MoldExecResult {
                    value: result,
                    exit_code: last_exit,
                    format_override: fmt_override,
                    output_file: out_file,
                    pending_steps: vec![],
                    pending_mutations: vec![],
                });
            }
            let step_ctx = steps[i].error_context(i, steps.len());
            let target_fmt = format::parse_format_name(fmt_name).with_context(|| {
                format!("after {step_ctx}: invalid set_input_format({fmt_name:?})")
            })?;
            if target_fmt == DataFormat::Raw {
                bail!(
                    "in {step_ctx}: set_output_format(\"raw\") can only be used in the final step of a mold chain"
                );
            }
            if ctx.debug {
                eprintln!("[debug] set_input_format(\"{fmt_name}\") — re-parsing between steps");
            }
            // set_input_format always re-parses from a serialized form, so we
            // pay the MontyObject → Value conversion only here (not every step).
            let result_value = convert::monty_to_json(result).with_context(|| {
                format!(
                    "after {step_ctx}: failed to convert result for set_input_format re-parsing"
                )
            })?;
            let as_string = match &result_value {
                Value::String(s) => s.clone(),
                other => serde_json::to_string(other).with_context(|| {
                    format!("after {step_ctx}: failed to serialize result for set_input_format re-parsing")
                })?,
            };
            let reparsed = if target_fmt == DataFormat::Csv {
                let (val, _) = format::parse_csv(&as_string, &CsvOptions::default())
                    .with_context(|| format!("after {step_ctx}: re-parsing as csv failed"))?;
                val
            } else {
                target_fmt
                    .parse(&as_string)
                    .with_context(|| format!("after {step_ctx}: re-parsing as {fmt_name} failed"))?
            };
            data = convert::json_into_monty(reparsed);
        } else if is_last {
            return Ok(MoldExecResult {
                value: result,
                exit_code: last_exit,
                format_override: None,
                output_file: out_file,
                pending_steps: vec![],
                pending_mutations: vec![],
            });
        } else {
            // Hot path: thread MontyObject straight into the next step, no
            // round-trip through serde_json::Value.
            data = result;
        }

        i += 1;
    }
    Ok(MoldExecResult {
        value: MontyObject::None,
        exit_code: last_exit,
        format_override: None,
        output_file: None,
        pending_steps: vec![],
        pending_mutations: vec![],
    })
}

/// Serialize a MoldStep into a minimal spec Value for `pipeline.step(i)`.
fn step_to_remaining_spec(step: &MoldStep) -> Value {
    let mut map = serde_json::Map::new();
    if let Some(ref fmt) = step.defaults.input_format {
        map.insert("input_format".into(), Value::String(fmt.clone()));
    }
    if let Some(ref fmt) = step.defaults.output_format {
        map.insert("output_format".into(), Value::String(fmt.clone()));
    }
    if let Some(ref args) = step.runtime_args {
        map.insert("args".into(), args.clone());
    }
    Value::Object(map)
}

/// Resolve a pending step spec (from `insert_next`/`append`) into a `MoldStep`.
/// `parent_step` is the 0-based index of the mold that injected this step.
fn resolve_pending(spec: Value, parent_step: usize) -> Result<MoldStep> {
    let script_ref = if let Some(m) = spec.get("mold").and_then(Value::as_str) {
        ScriptRef::Mold(m.to_string())
    } else if let Some(e) = spec.get("expr").and_then(Value::as_str) {
        ScriptRef::Expr(e.to_string())
    } else {
        bail!("pending step spec must contain 'mold' or 'expr'");
    };
    let mut steps = build_scripts(&[script_ref], false)?;
    let mut step = steps.remove(0);
    step.origin = StepOrigin::Injected { parent_step };
    if let Some(fmt) = spec.get("input_format").and_then(Value::as_str) {
        step.defaults.input_format = Some(fmt.to_string());
    }
    if let Some(fmt) = spec.get("output_format").and_then(Value::as_str) {
        step.defaults.output_format = Some(fmt.to_string());
    }
    if let Some(args) = spec.get("args").filter(|v| v.is_object()) {
        step.runtime_args = Some(args.clone());
    }
    Ok(step)
}

/// HTTP options passed through the pipeline.
#[derive(Debug, Clone)]
pub struct HttpOptions {
    pub headers: Vec<String>,
    pub timeout: u64,
    pub no_follow: bool,
}

/// Result of a pipeline execution.
pub struct PipelineResult {
    /// The transformed data.
    pub value: Value,
    /// Exit code requested by the mold via `set_exit()`.
    pub exit_code: Option<i32>,
    /// Output format override requested by the mold via `set_output_format()`.
    pub format_override: Option<String>,
    /// Output file override requested by the mold via `set_output_file()`.
    pub output_file_override: Option<String>,
    /// Detected input format (needed by CLI to determine output format fallback).
    pub input_format: DataFormat,
    /// Raw HTTP bytes for binary pass-through via `set_output_format("raw")`.
    pub http_raw_bytes: Option<Vec<u8>>,
}

// ---------------------------------------------------------------------------
// Core pipeline: single source of truth for read → parse → execute
// ---------------------------------------------------------------------------

/// Core pipeline logic: read input → parse → execute mold chain → return result.
///
/// This is the single source of truth. Both `process_single_input` (CLI) and
/// `run_pipeline` (library API) delegate to this function.
fn run_pipeline_core(
    metadata: &PipelineMetadata<'_>,
    effective_input_format: Option<&str>,
    csv_opts: &CsvOptions,
    scripts: &[MoldStep],
    http_opts: &HttpOptions,
    ctx: &ChainExecCtx<'_>,
) -> Result<PipelineResult> {
    let input_path = metadata.input;
    let no_input = metadata.no_input;
    let slurp = metadata.slurp;
    let debug = ctx.debug;
    let parse_start = Instant::now();
    let mut csv_headers: Option<Vec<String>> = None;
    let mut http_raw_bytes: Option<Vec<u8>> = None;

    let parsed_input_fmt = effective_input_format
        .map(format::parse_format_name)
        .transpose()?;

    let (in_fmt, data) = if no_input {
        if debug {
            eprintln!("[debug] no-input mode: data = None");
        }
        (DataFormat::Json, MontyObject::None)
    } else {
        let is_http = input_path.is_some_and(http::is_url);

        // For HTTP inputs, Content-Type can influence format detection
        let (input_content, ct_format, http_shortcircuit) = if is_http {
            let url = input_path.unwrap();
            let resp = http::fetch_url(
                url,
                &http_opts.headers,
                http_opts.timeout,
                http_opts.no_follow,
                debug,
            )?;

            let ct_fmt = resp
                .content_type
                .as_deref()
                .and_then(http::content_type_to_format);

            // If --input-format http, build the HTTP dict directly and skip normal parsing
            if parsed_input_fmt == Some(DataFormat::Http) {
                // Detect binary content: no known text format and not a text/ type
                let is_binary = ct_fmt.is_none()
                    && resp
                        .content_type
                        .as_deref()
                        .is_some_and(|ct| !ct.starts_with("text/"));

                http_raw_bytes = Some(resp.body_bytes.clone());

                let mut headers_map = serde_json::Map::new();
                for (k, v) in &resp.headers {
                    headers_map.insert(k.clone(), Value::String(v.clone()));
                }
                let body_val = if is_binary {
                    Value::Null
                } else {
                    Value::String(resp.body.clone())
                };
                let http_data = serde_json::json!({
                    "status": resp.status,
                    "headers": Value::Object(headers_map),
                    "body": body_val,
                    "body_size": resp.body_bytes.len(),
                    "content_type": resp.content_type.as_deref().unwrap_or(""),
                    "url": url,
                });
                // Short-circuit: skip normal content parsing
                (String::new(), ct_fmt, Some((DataFormat::Http, http_data)))
            } else {
                (resp.body, ct_fmt, None)
            }
        } else {
            let content = match input_path {
                Some(path) => {
                    if debug {
                        eprintln!("[debug] input file: {path}");
                    }
                    fs::read_to_string(path)
                        .with_context(|| format!("Failed to read input file: {path}"))?
                }
                None => {
                    if debug {
                        eprintln!("[debug] input: stdin");
                    }
                    let mut buf = String::new();
                    io::stdin()
                        .read_to_string(&mut buf)
                        .context("Failed to read from stdin")?;
                    buf
                }
            };
            (content, None, None)
        };

        // If Http format was already resolved (--input-format http), use pre-built data
        if let Some((fmt, val)) = http_shortcircuit {
            (fmt, convert::json_into_monty(val))
        } else {
            // Format resolution: --input-format > Content-Type > extension > JSON
            let in_fmt = if let Some(fmt) = parsed_input_fmt {
                fmt
            } else if let Some(ct_name) = ct_format {
                format::parse_format_name(ct_name)?
            } else if is_http {
                let path_part = url_path_only(input_path.unwrap());
                DataFormat::from_extension(path_part).unwrap_or(DataFormat::Json)
            } else {
                format::resolve_format(None, input_path, DataFormat::Json)?
            };

            if debug {
                eprintln!("[debug] input format: {in_fmt}");
            }

            // For CSV without debug: build MontyObject directly, skipping the Value intermediate.
            // For all other cases (including CSV+debug): parse to Value first for debug printing.
            let data = if in_fmt == DataFormat::Csv && !debug {
                let (monty, headers) = format::csv_to_monty(&input_content, csv_opts)?;
                csv_headers = headers;
                monty
            } else {
                // Parse input to Value (needed for debug display, or non-CSV formats)
                let value = if slurp && in_fmt == DataFormat::Json {
                    let mut values = Vec::new();
                    let deserializer = serde_json::Deserializer::from_str(&input_content);
                    for result in deserializer.into_iter::<serde_json::Value>() {
                        values.push(result.context("Failed to parse JSON value in slurp mode")?);
                    }
                    serde_json::Value::Array(values)
                } else if in_fmt == DataFormat::Csv {
                    let (value, headers) = format::parse_csv(&input_content, csv_opts)?;
                    csv_headers = headers;
                    value
                } else {
                    let parsed = in_fmt.parse(&input_content)?;
                    if slurp && in_fmt != DataFormat::Ndjson {
                        serde_json::Value::Array(vec![parsed])
                    } else {
                        parsed
                    }
                };

                if debug {
                    eprintln!("[debug] input data:");
                    if let Ok(pretty) = serde_json::to_string_pretty(&value) {
                        for line in pretty.lines() {
                            eprintln!("  {line}");
                        }
                    }
                }

                convert::json_into_monty(value)
            };

            (in_fmt, data)
        }
    };

    // Build headers value for CSV (or None)
    let headers_value = match csv_headers {
        Some(ref hdrs) => serde_json::Value::Array(
            hdrs.iter()
                .map(|h| serde_json::Value::String(h.clone()))
                .collect(),
        ),
        None => serde_json::Value::Null,
    };

    debug_phase(ctx.debug, "parse", parse_start);

    // Execute the mold chain
    let exec_start = Instant::now();
    let exec = execute_chain(scripts, data, metadata, &headers_value, ctx)?;
    debug_phase(ctx.debug, "execute", exec_start);

    // Chain exit boundary: convert the final MontyObject back to a Value so
    // the rest of the pipeline can serialize it through the I/O layer.
    let value = convert::monty_to_json(exec.value)
        .context("Failed to convert mold chain result to JSON")?;

    Ok(PipelineResult {
        value,
        exit_code: exec.exit_code,
        format_override: exec.format_override,
        output_file_override: exec.output_file,
        input_format: in_fmt,
        http_raw_bytes,
    })
}

// ---------------------------------------------------------------------------
// CLI wrapper: output writing + process::exit
// ---------------------------------------------------------------------------

/// Pre-computed inputs for `process_single_input`. Bundled into a struct so the
/// CLI-facing signature stays manageable as new pipeline-wide options are
/// added (the alternative was a 16-argument function).
pub struct SingleRunOptions<'a> {
    pub input_path: Option<&'a str>,
    pub no_input: bool,
    pub slurp: bool,
    pub effective_input_format: Option<&'a str>,
    pub csv_opts: &'a CsvOptions,
    pub scripts: &'a [MoldStep],
    pub extra_args: &'a [(String, String)],
    pub env_value: &'a Value,
    pub debug: bool,
    pub msg_level: u8,
    pub output_path: Option<&'a str>,
    pub effective_output_format: Option<&'a str>,
    pub in_place: bool,
    pub check: bool,
    pub http_opts: &'a HttpOptions,
    pub policy: &'a SandboxPolicy,
}

/// Process a single input through the full pipeline: read → parse → execute chain → serialize → write.
///
/// Handles output writing and returns a `CliResult` describing whether the
/// invocation completed normally or requests the process to exit with a code
/// (from `set_exit()` or `--check`). For library usage, prefer `run_pipeline`,
/// which returns the result without writing or signalling exits.
pub fn process_single_input(opts: SingleRunOptions<'_>) -> Result<CliResult> {
    let SingleRunOptions {
        input_path,
        no_input,
        slurp,
        effective_input_format,
        csv_opts,
        scripts,
        extra_args,
        env_value,
        debug,
        msg_level,
        output_path,
        effective_output_format,
        in_place,
        check,
        http_opts,
        policy,
    } = opts;
    let total_start = Instant::now();
    let metadata = PipelineMetadata {
        input: input_path,
        output: output_path,
        input_format: effective_input_format,
        output_format: effective_output_format,
        in_place,
        slurp,
        no_input,
    };
    let exec_ctx = ChainExecCtx {
        extra_args,
        env_value,
        policy,
        debug,
        msg_level,
    };
    let result = run_pipeline_core(
        &metadata,
        effective_input_format,
        csv_opts,
        scripts,
        http_opts,
        &exec_ctx,
    )?;

    // set_output_file() overrides the CLI -o path; otherwise fall back to CLI-provided path
    let actual_output = result.output_file_override.as_deref().or(output_path);

    // Binary pass-through: set_output_format("raw") signals that raw HTTP bytes should be written
    // directly, bypassing the normal serde serialization pipeline.
    let format_override_parsed = result
        .format_override
        .as_deref()
        .map(format::parse_format_name)
        .transpose()?;
    if format_override_parsed == Some(DataFormat::Raw) {
        let bytes = result.http_raw_bytes.ok_or_else(|| {
            anyhow::anyhow!(
                "set_output_format(\"raw\") requires --input-format http (no raw bytes available)"
            )
        })?;
        if debug {
            eprintln!("[debug] raw binary output: {} bytes", bytes.len());
            if let Some(path) = actual_output {
                eprintln!("[debug] writing to: {path}");
            }
        }
        write_bytes_to(actual_output, &bytes)?;
        if let Some(code) = result.exit_code {
            return Ok(CliResult::Exit(code));
        }
        return Ok(CliResult::Done);
    }

    // If set_input_format() or set_output_format() was called (non-raw), it overrides the output format
    let effective_output_format = result
        .format_override
        .as_deref()
        .or(effective_output_format);

    // Handle set_exit and --check
    if let Some(code) = result.exit_code {
        if !check {
            output_result(
                &result.value,
                actual_output,
                effective_output_format,
                result.input_format,
                csv_opts,
                no_input,
                debug,
            )?;
        }
        return Ok(CliResult::Exit(code));
    }

    if check {
        let code = if is_truthy(&result.value) { 0 } else { 1 };
        return Ok(CliResult::Exit(code));
    }

    output_result(
        &result.value,
        actual_output,
        effective_output_format,
        result.input_format,
        csv_opts,
        no_input,
        debug,
    )?;

    debug_phase(debug, "total", total_start);

    Ok(CliResult::Done)
}

pub fn output_result(
    result: &serde_json::Value,
    output_path: Option<&str>,
    effective_output_format: Option<&str>,
    in_fmt: DataFormat,
    csv_opts: &CsvOptions,
    no_input: bool,
    debug: bool,
) -> Result<()> {
    let serialize_start = Instant::now();
    let output_fallback = if no_input || in_fmt == DataFormat::Http {
        DataFormat::Json
    } else {
        in_fmt
    };
    let out_fmt = format::resolve_format(effective_output_format, output_path, output_fallback)?;

    if out_fmt == DataFormat::Http {
        bail!("HTTP format is input-only and cannot be used for output");
    }

    if debug {
        eprintln!("[debug] output format: {out_fmt}");
        eprintln!("[debug] output data:");
        if let Ok(pretty) = serde_json::to_string_pretty(result) {
            for line in pretty.lines() {
                eprintln!("  {line}");
            }
        }
    }

    // Serialize output
    let output_str = if out_fmt == DataFormat::Csv {
        format::serialize_csv(result, csv_opts)?
    } else {
        out_fmt.serialize(result)?
    };

    // Write output
    match output_path {
        Some(path) => {
            fs::write(path, &output_str)
                .with_context(|| format!("Failed to write output file: {path}"))?;
        }
        None => {
            print!("{output_str}");
        }
    }

    debug_phase(debug, "serialize", serialize_start);

    Ok(())
}

pub fn read_input_list(source: &str) -> Result<Vec<String>> {
    let content = if source == "-" {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .context("Failed to read input list from stdin")?;
        buf
    } else {
        fs::read_to_string(source)
            .with_context(|| format!("Failed to read input list file: {source}"))?
    };
    Ok(content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect())
}

/// Check if an environment variable name matches any of the --env patterns.
///
/// Each pattern string may contain comma-separated segments. Each segment uses
/// the same glob syntax as `sandbox::matches_glob` (`*`, `PREFIX*`, `*SUFFIX`,
/// `*INNER*`, exact).
pub fn env_pattern_matches(name: &str, patterns: &[String]) -> bool {
    patterns
        .iter()
        .flat_map(|p| p.split(','))
        .map(str::trim)
        .filter(|seg| !seg.is_empty())
        .any(|seg| crate::sandbox::matches_glob(seg, name))
}

/// Parse "path:alias" syntax from a single -i entry.
///
/// Returns `(path, alias_mode)` where:
/// - `None` → no colon found, list mode
/// - `Some(None)` → colon with empty alias, use file stem as key
/// - `Some(Some("alias"))` → explicit alias
///
/// URLs are never parsed for aliases (they contain `://`).
/// The alias part must not contain path separators to avoid false positives.
pub fn parse_input_entry(s: &str) -> (&str, Option<Option<&str>>) {
    if http::is_url(s) {
        return (s, None);
    }
    if let Some(colon_pos) = s.rfind(':') {
        let path = &s[..colon_pos];
        let alias = &s[colon_pos + 1..];
        // Reject if alias contains path separators (the ':' is part of the path)
        if !alias.contains('/') && !alias.contains('\\') {
            return if alias.is_empty() {
                (path, Some(None))
            } else {
                (path, Some(Some(alias)))
            };
        }
    }
    (s, None)
}

/// Strip `?query` and `#fragment` from a URL, returning just the path portion.
/// Used for extension-based format detection and filename derivation.
pub fn url_path_only(url: &str) -> &str {
    url.split('?')
        .next()
        .unwrap_or(url)
        .split('#')
        .next()
        .unwrap_or(url)
}

/// Derive a filename from a URL path (strip query/fragment, take last segment).
pub fn url_filename(url: &str) -> Result<String> {
    url_path_only(url)
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("cannot determine filename from URL '{url}'"))
}

/// Write `bytes` to either a file at `actual_output` or stdout when `None`.
/// Used by the binary / raw output paths in `pipeline` and `cmd::shape`.
pub fn write_bytes_to(actual_output: Option<&str>, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    match actual_output {
        Some(path) => fs::write(path, bytes)
            .with_context(|| format!("Failed to write binary output to: {path}")),
        None => io::stdout()
            .write_all(bytes)
            .context("Failed to write binary output to stdout"),
    }
}

/// Extract the stem (filename without extension) from a path string.
pub fn path_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_string()
}

/// Read and parse a single file (or URL) as a `Value` for multi-file slurp mode.
pub fn read_and_parse_for_slurp(
    path: &str,
    effective_input_format: Option<&str>,
    csv_opts: &CsvOptions,
    http_opts: &HttpOptions,
    debug: bool,
) -> Result<Value> {
    let is_url = http::is_url(path);
    let content: String;
    let detected_fmt: Option<DataFormat>;

    if is_url {
        let resp = http::fetch_url(
            path,
            &http_opts.headers,
            http_opts.timeout,
            http_opts.no_follow,
            debug,
        )?;
        let ct = resp
            .content_type
            .as_deref()
            .and_then(http::content_type_to_format);
        detected_fmt = if let Some(ct_name) = ct {
            format::parse_format_name(ct_name).ok()
        } else {
            DataFormat::from_extension(url_path_only(path))
        };
        content = resp.body;
    } else {
        if debug {
            eprintln!("[debug] slurp: reading {path}");
        }
        content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read input file: {path}"))?;
        detected_fmt = None;
    }

    let in_fmt = if let Some(name) = effective_input_format {
        format::parse_format_name(name)?
    } else if let Some(fmt) = detected_fmt {
        fmt
    } else {
        format::resolve_format(None, Some(path), DataFormat::Json)?
    };

    if debug {
        eprintln!("[debug] slurp: {path} → format: {in_fmt}");
    }

    let value = if in_fmt == DataFormat::Csv {
        let (val, _) = format::parse_csv(&content, csv_opts)?;
        val
    } else {
        in_fmt.parse(&content)?
    };

    Ok(value)
}

/// Build the filtered environment dict from --env patterns.
pub fn build_env(env_patterns: &[String]) -> Value {
    if env_patterns.is_empty() {
        Value::Object(serde_json::Map::new())
    } else {
        let map: serde_json::Map<String, Value> = std::env::vars()
            .filter(|(k, _)| env_pattern_matches(k, env_patterns))
            .map(|(k, v)| (k, Value::String(v)))
            .collect();
        Value::Object(map)
    }
}

// ---------------------------------------------------------------------------
// High-level public API
// ---------------------------------------------------------------------------

/// Configuration for a pipeline run.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Ordered pipeline steps (molds and/or expressions, applied in order).
    pub steps: Vec<ScriptRef>,
    /// Named arguments passed to the mold as `args["key"]`.
    pub args: Vec<(String, String)>,
    /// Environment variables exposed to the mold as `env["KEY"]`.
    pub env_patterns: Vec<String>,
    /// Override input format (e.g. "json", "yaml", "csv", "toml").
    pub input_format: Option<String>,
    /// Override output format.
    pub output_format: Option<String>,
    /// CSV-specific options.
    pub csv_opts: CsvOptions,
    /// HTTP options for URL inputs.
    pub http_opts: HttpOptions,
    /// Combine multiple JSON values into a single array.
    pub slurp: bool,
    /// No input data (`data = None` in the mold).
    pub no_input: bool,
    /// Print debug info to stderr.
    pub debug: bool,
    /// Message verbosity level (0=quiet, 1=default, 2=verbose, 3=trace).
    pub msg_level: u8,
    /// Bypass the local cache for remote catalogs and molds.
    pub no_cache: bool,
    /// Sandbox policy gating OS calls and enforcing limits. Defaults to zero authorization.
    pub sandbox: SandboxPolicy,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            steps: Vec::new(),
            args: Vec::new(),
            env_patterns: Vec::new(),
            input_format: None,
            output_format: None,
            csv_opts: CsvOptions::default(),
            http_opts: HttpOptions {
                headers: Vec::new(),
                timeout: 30,
                no_follow: false,
            },
            slurp: false,
            no_input: false,
            debug: false,
            msg_level: 1,
            no_cache: false,
            sandbox: SandboxPolicy::zero_authorization(),
        }
    }
}

/// Run the full transform pipeline: parse input → execute mold chain → return result.
///
/// This function does **not** write output or call `process::exit()` — it returns
/// the result for the caller to handle.
///
/// # Examples
///
/// ```ignore
/// use fimod::pipeline::{run_pipeline, PipelineConfig};
///
/// let mut cfg = PipelineConfig::default();
/// cfg.steps = vec![ScriptRef::Expr("data['name'].upper()".into())];
///
/// let result = run_pipeline(Some("data.json"), &cfg)?;
/// println!("{}", result.value);
/// ```
pub fn run_pipeline(input_path: Option<&str>, config: &PipelineConfig) -> Result<PipelineResult> {
    let scripts = build_scripts(&config.steps, config.no_cache)?;
    let env_value = build_env(&config.env_patterns);

    let first_defaults = &scripts[0].defaults;
    let effective_input_format = config
        .input_format
        .as_deref()
        .or(first_defaults.input_format.as_deref());

    let metadata = PipelineMetadata {
        input: input_path,
        output: None,
        input_format: effective_input_format,
        output_format: config.output_format.as_deref(),
        in_place: false,
        slurp: config.slurp,
        no_input: config.no_input,
    };
    let exec_ctx = ChainExecCtx {
        extra_args: &config.args,
        env_value: &env_value,
        policy: &config.sandbox,
        debug: config.debug,
        msg_level: config.msg_level,
    };

    run_pipeline_core(
        &metadata,
        effective_input_format,
        &config.csv_opts,
        &scripts,
        &config.http_opts,
        &exec_ctx,
    )
}
