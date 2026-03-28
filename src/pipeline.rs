use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process;

use anyhow::{bail, Context, Result};
use monty::MontyObject;
use serde_json::Value;

use crate::engine::MoldResult;
use crate::format::{CsvOptions, DataFormat};
use crate::mold::MoldSource;
use crate::{convert, engine, format, http, mold};

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

/// Build the list of (display, compiled_script, defaults) from CLI molds or expressions.
///
/// Returns at least one step, or an error if neither -m nor -e was provided.
pub fn build_scripts(
    molds: &[String],
    expressions: &[String],
    no_cache: bool,
) -> Result<Vec<(String, String, mold::MoldDefaults)>> {
    if !expressions.is_empty() {
        let mut steps = Vec::new();
        for e in expressions {
            let source = MoldSource::Inline(e.clone());
            let display = source.to_string();
            let script = source.load(no_cache)?;
            steps.push((display, script, mold::MoldDefaults::default()));
        }
        Ok(steps)
    } else if !molds.is_empty() {
        let mut steps = Vec::new();
        for m in molds {
            let source = MoldSource::from_mold_str(m, no_cache)?;
            let is_inline = matches!(source, MoldSource::Inline(_));
            let display = source.to_string();
            let script = source.load(no_cache)?;
            let defaults = if !is_inline {
                mold::parse_mold_defaults(&script)
            } else {
                mold::MoldDefaults::default()
            };
            steps.push((display, script, defaults));
        }
        Ok(steps)
    } else {
        bail!("Either -m/--mold or -e/--expression is required")
    }
}

/// Execute a chain of mold scripts sequentially.
///
/// The output of each step becomes the input of the next.
/// `extra_args` (the `args` dict) is passed to every step.
/// `env_value` is the filtered environment dict (always passed).
/// `csv_headers` are passed as `headers` to every step (None if not CSV).
///
/// Returns `(result, optional_exit_code, optional_format_override)`.
/// The format override is from the last step's `set_input_format()` / `set_output_format()` call (if any).
/// Between steps, if `set_input_format()` was called, the result is serialized
/// and re-parsed with the requested format.
///
/// `initial_data` is taken as an owned `MontyObject` so the caller can pass
/// the result of `csv_to_monty` without an extra `json_to_monty` round-trip.
pub fn execute_chain(
    steps: &[(String, String, mold::MoldDefaults)],
    initial_data: MontyObject,
    extra_args: &[(String, String)],
    env_value: &Value,
    headers_value: &Value,
    debug: bool,
    msg_level: u8,
) -> MoldResult {
    let mut data = initial_data;
    let mut last_exit = None;
    let is_last_step = |i: usize| i == steps.len() - 1;

    for (i, (display, script, _defaults)) in steps.iter().enumerate() {
        if debug {
            eprintln!("[debug] mold: {display}");
        }
        let (result, exit_code, fmt_override, out_file) = engine::execute_mold(
            script,
            data,
            extra_args,
            env_value,
            headers_value,
            debug,
            msg_level,
        )?;

        if let Some(c) = exit_code {
            last_exit = Some(c);
        }

        // Handle set_input_format() between chain steps
        if let Some(ref fmt_name) = fmt_override {
            if is_last_step(i) {
                // Last step: pass format override and output_file to the caller
                return Ok((result, last_exit, fmt_override, out_file));
            }
            // "raw" cannot be used in an intermediate chain step
            if fmt_name == "raw" {
                bail!(
                    "set_output_format(\"raw\") can only be used in the final step of a mold chain"
                );
            }
            // Intermediate step: re-parse the result with the requested format
            if debug {
                eprintln!("[debug] set_input_format(\"{fmt_name}\") — re-parsing between steps");
            }
            let as_string = match &result {
                Value::String(s) => s.clone(),
                other => serde_json::to_string(other)
                    .context("Failed to serialize result for set_input_format re-parsing")?,
            };
            let target_fmt = format::parse_format_name(fmt_name)?;
            let reparsed = if target_fmt == DataFormat::Csv {
                let (val, _) = format::parse_csv(&as_string, &CsvOptions::default())?;
                val
            } else {
                target_fmt.parse(&as_string)?
            };
            data = convert::json_into_monty(reparsed);
        } else if is_last_step(i) {
            return Ok((result, last_exit, None, out_file));
        } else {
            // Intermediate step without set_input_format: convert result back to MontyObject
            data = convert::json_into_monty(result);
        }
    }
    Ok((Value::Null, last_exit, None, None))
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
#[allow(clippy::too_many_arguments)]
fn run_pipeline_core(
    input_path: Option<&str>,
    no_input: bool,
    slurp: bool,
    effective_input_format: Option<&str>,
    csv_opts: &CsvOptions,
    scripts: &[(String, String, mold::MoldDefaults)],
    extra_args: &[(String, String)],
    env_value: &Value,
    debug: bool,
    msg_level: u8,
    http_opts: &HttpOptions,
) -> Result<PipelineResult> {
    let mut csv_headers: Option<Vec<String>> = None;
    let mut http_raw_bytes: Option<Vec<u8>> = None;

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
            if effective_input_format == Some("http") {
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
            let in_fmt = if let Some(name) = effective_input_format {
                format::parse_format_name(name)?
            } else if let Some(ct_name) = ct_format {
                format::parse_format_name(ct_name)?
            } else if is_http {
                // For URLs, try extension from URL path, fallback to JSON
                let url_path = input_path.unwrap();
                // Extract path part from URL for extension detection
                let path_part = url_path
                    .split('?')
                    .next()
                    .unwrap_or(url_path)
                    .split('#')
                    .next()
                    .unwrap_or(url_path);
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

    // Execute the mold chain
    let (value, exit_code, format_override, output_file_override) = execute_chain(
        scripts,
        data,
        extra_args,
        env_value,
        &headers_value,
        debug,
        msg_level,
    )?;

    Ok(PipelineResult {
        value,
        exit_code,
        format_override,
        output_file_override,
        input_format: in_fmt,
        http_raw_bytes,
    })
}

// ---------------------------------------------------------------------------
// CLI wrapper: output writing + process::exit
// ---------------------------------------------------------------------------

/// Process a single input through the full pipeline: read → parse → execute chain → serialize → write.
///
/// This is the CLI-facing function that handles output writing and `process::exit()`.
/// For library usage, prefer `run_pipeline` which returns the result without side effects.
#[allow(clippy::too_many_arguments)]
pub fn process_single_input(
    input_path: Option<&str>,
    no_input: bool,
    slurp: bool,
    effective_input_format: Option<&str>,
    csv_opts: &CsvOptions,
    scripts: &[(String, String, mold::MoldDefaults)],
    extra_args: &[(String, String)],
    env_value: &Value,
    debug: bool,
    msg_level: u8,
    output_path: Option<&str>,
    effective_output_format: Option<&str>,
    check: bool,
    http_opts: &HttpOptions,
) -> Result<()> {
    let result = run_pipeline_core(
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
        http_opts,
    )?;

    // set_output_file() overrides the CLI -o path; otherwise fall back to CLI-provided path
    let actual_output = result.output_file_override.as_deref().or(output_path);

    // Binary pass-through: set_output_format("raw") signals that raw HTTP bytes should be written
    // directly, bypassing the normal serde serialization pipeline.
    if result.format_override.as_deref() == Some("raw") {
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
        match actual_output {
            Some(path) => {
                fs::write(path, &bytes)
                    .with_context(|| format!("Failed to write binary output to: {path}"))?;
            }
            None => {
                use std::io::Write;
                io::stdout()
                    .write_all(&bytes)
                    .context("Failed to write binary output to stdout")?;
            }
        }
        if let Some(code) = result.exit_code {
            process::exit(code);
        }
        return Ok(());
    }

    // If set_input_format() or set_output_format() was called (non-raw), it overrides the output format
    let effective_output_format = result.format_override.as_deref().or(effective_output_format);

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
        process::exit(code);
    }

    if check {
        let code = if is_truthy(&result.value) { 0 } else { 1 };
        process::exit(code);
    }

    output_result(
        &result.value,
        actual_output,
        effective_output_format,
        result.input_format,
        csv_opts,
        no_input,
        debug,
    )
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
/// Each pattern string may contain comma-separated segments.
/// Each segment is either:
/// - `*` → matches everything
/// - `PREFIX*` → matches names starting with PREFIX
/// - `EXACT` → exact match
pub fn env_pattern_matches(name: &str, patterns: &[String]) -> bool {
    for pat_str in patterns {
        for segment in pat_str.split(',') {
            let segment = segment.trim();
            if segment.is_empty() {
                continue;
            }
            if segment == "*" {
                return true;
            }
            if let Some(prefix) = segment.strip_suffix('*') {
                if name.starts_with(prefix) {
                    return true;
                }
            } else if name == segment {
                return true;
            }
        }
    }
    false
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

/// Derive a filename from a URL path (strip query/fragment, take last segment).
pub fn url_filename(url: &str) -> Result<String> {
    url.split('?')
        .next()
        .unwrap_or(url)
        .split('#')
        .next()
        .unwrap_or(url)
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("cannot determine filename from URL '{url}'"))
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
            let path_part = path
                .split('?')
                .next()
                .unwrap_or(path)
                .split('#')
                .next()
                .unwrap_or(path);
            DataFormat::from_extension(path_part)
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
    /// Mold script paths/references (mutually exclusive with `expressions`).
    pub molds: Vec<String>,
    /// Inline Python expressions (mutually exclusive with `molds`).
    pub expressions: Vec<String>,
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
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            molds: Vec::new(),
            expressions: Vec::new(),
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
/// cfg.expressions = vec!["data['name'].upper()".into()];
///
/// let result = run_pipeline(Some("data.json"), &cfg)?;
/// println!("{}", result.value);
/// ```
pub fn run_pipeline(input_path: Option<&str>, config: &PipelineConfig) -> Result<PipelineResult> {
    let scripts = build_scripts(&config.molds, &config.expressions, config.no_cache)?;
    let env_value = build_env(&config.env_patterns);

    let first_defaults = &scripts[0].2;
    let effective_input_format = config
        .input_format
        .as_deref()
        .or(first_defaults.input_format.as_deref());

    run_pipeline_core(
        input_path,
        config.no_input,
        config.slurp,
        effective_input_format,
        &config.csv_opts,
        &scripts,
        &config.args,
        &env_value,
        config.debug,
        config.msg_level,
        &config.http_opts,
    )
}

/// Parse a string into a `serde_json::Value` using the specified format.
///
/// This is the simplest entry point for format conversion (fimod-py use case).
///
/// # Examples
///
/// ```ignore
/// let val = fimod::pipeline::parse_data("name: Alice\nage: 30", "yaml")?;
/// ```
pub fn parse_data(content: &str, format_name: &str) -> Result<Value> {
    let fmt = format::parse_format_name(format_name)?;
    if fmt == DataFormat::Csv {
        let (val, _) = format::parse_csv(content, &CsvOptions::default())?;
        Ok(val)
    } else {
        fmt.parse(content)
    }
}

/// Parse a file into a `serde_json::Value`, auto-detecting the format from the extension.
pub fn parse_file(path: &str) -> Result<Value> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {path}"))?;
    let fmt = format::resolve_format(None, Some(path), DataFormat::Json)?;
    if fmt == DataFormat::Csv {
        let (val, _) = format::parse_csv(&content, &CsvOptions::default())?;
        Ok(val)
    } else {
        fmt.parse(&content)
    }
}

/// Serialize a `serde_json::Value` to a string in the specified format.
///
/// # Examples
///
/// ```ignore
/// let yaml = fimod::pipeline::serialize_data(&val, "yaml")?;
/// ```
pub fn serialize_data(value: &Value, format_name: &str) -> Result<String> {
    let fmt = format::parse_format_name(format_name)?;
    if fmt == DataFormat::Csv {
        format::serialize_csv(value, &CsvOptions::default())
    } else {
        fmt.serialize(value)
    }
}

/// Convert a file from one format to another, auto-detecting formats from extensions.
pub fn convert_file(input_path: &str, output_path: &str) -> Result<()> {
    let value = parse_file(input_path)?;
    let out_fmt = format::resolve_format(None, Some(output_path), DataFormat::Json)?;
    let output_str = if out_fmt == DataFormat::Csv {
        format::serialize_csv(&value, &CsvOptions::default())?
    } else {
        out_fmt.serialize(&value)?
    };
    fs::write(output_path, &output_str)
        .with_context(|| format!("Failed to write output file: {output_path}"))?;
    Ok(())
}
