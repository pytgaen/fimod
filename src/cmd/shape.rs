use std::fs;
use std::io::{self, Read};
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use fimod::format::{CsvOptions, DataFormat};
use fimod::pipeline::{
    build_env, build_scripts, execute_chain_to_value, is_truthy, output_result, parse_input_entry,
    path_stem, process_single_input, read_and_parse_for_slurp, read_input_list, url_filename,
    write_bytes_to, CliResult, HttpOptions, ScriptRef, SingleRunOptions,
};
use fimod::sandbox::SandboxPolicy;
use fimod::{convert, format, http};

use crate::cli::{MsgLevel, ShapeArgs};

pub fn build_script_refs(molds: &[String], expressions: &[String]) -> Vec<ScriptRef> {
    let args: Vec<String> = std::env::args().collect();
    let mut mold_iter = molds.iter();
    let mut expr_iter = expressions.iter();
    let mut refs = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if (arg == "-m" || arg == "--mold") && i + 1 < args.len() {
            if let Some(v) = mold_iter.next() {
                refs.push(ScriptRef::Mold(v.clone()));
            }
            i += 2;
        } else if arg.starts_with("--mold=")
            || (arg.starts_with("-m") && arg.len() > 2 && !arg.starts_with("-m-"))
        {
            if let Some(v) = mold_iter.next() {
                refs.push(ScriptRef::Mold(v.clone()));
            }
            i += 1;
        } else if (arg == "-e" || arg == "--expression") && i + 1 < args.len() {
            if let Some(v) = expr_iter.next() {
                refs.push(ScriptRef::Expr(v.clone()));
            }
            i += 2;
        } else if arg.starts_with("--expression=")
            || (arg.starts_with("-e") && arg.len() > 2 && !arg.starts_with("-e-"))
        {
            if let Some(v) = expr_iter.next() {
                refs.push(ScriptRef::Expr(v.clone()));
            }
            i += 1;
        } else {
            i += 1;
        }
    }

    refs
}

pub fn run_shape(mut shape: ShapeArgs) -> Result<CliResult> {
    // Parse output format once so the rest of the function can dispatch on
    // the typed `OutputMode` enum instead of string-comparing to "raw".
    let output_mode = shape
        .output_format
        .as_deref()
        .map(format::OutputMode::parse)
        .transpose()?;
    let is_raw_output = output_mode == Some(format::OutputMode::Raw);

    // Validate --watch combos before any input resolution
    if shape.watch {
        if shape.in_place {
            bail!("--watch is not compatible with --in-place");
        }
        if shape.no_input {
            bail!("--watch is not compatible with --no-input");
        }
        if shape.input_list.is_some() {
            bail!("--watch is not compatible with --input-list");
        }
        if is_raw_output {
            bail!("--watch is not compatible with --output-format raw");
        }
        if shape.input.is_empty() {
            bail!("--watch requires -i/--input (cannot watch stdin)");
        }
        if shape.input.len() > 1 {
            bail!("--watch is not compatible with batch mode (multiple -i)");
        }
        if shape.input.iter().any(|p| http::is_url(p)) {
            bail!("--watch is not supported for HTTP inputs");
        }
    }

    // Resolve --input-list into shape.input before any other processing
    if let Some(ref source) = shape.input_list.clone() {
        shape.input = read_input_list(source)?;
        if shape.input.is_empty() {
            bail!("--input-list: no inputs found in '{source}'");
        }
    }

    let policy = fimod::sandbox::SandboxPolicy::resolve(shape.sandbox_file.as_deref())?;

    let debug = shape.debug;
    let msg_level: u8 = if shape.quiet {
        0
    } else {
        match shape.msg_level {
            None => 1,
            Some(MsgLevel::Verbose) => 2,
            Some(MsgLevel::Trace) => 3,
        }
    };
    let is_batch = shape.input.len() > 1;
    // Multi-file slurp: -s with multiple -i combines all files into one data structure.
    // This intercepts before the regular batch loop.
    let is_multi_slurp = is_batch && shape.slurp;

    validate_post_input_list(&shape, is_batch, is_multi_slurp)?;

    if is_raw_output {
        return run_raw_passthrough(shape, debug, is_batch);
    }

    if shape.watch {
        #[cfg(feature = "watch")]
        return crate::watch::run_watch(&shape, &policy, debug, msg_level);
        #[cfg(not(feature = "watch"))]
        bail!("--watch is not available in this build (compiled without the 'watch' feature)");
    }

    run_shape_pipeline(&shape, &policy, debug, msg_level)
}

/// Validation pass after `--input-list` has been resolved into `shape.input`.
/// Checks `--no-input`, `--in-place`, and batch-mode constraints.
fn validate_post_input_list(shape: &ShapeArgs, is_batch: bool, is_multi_slurp: bool) -> Result<()> {
    if shape.no_input {
        if shape.in_place {
            bail!("--no-input is incompatible with --in-place");
        }
        if !shape.input.is_empty() {
            bail!("--no-input is incompatible with -i/--input");
        }
        if shape.input_format.is_some() {
            bail!("--no-input is incompatible with --input-format");
        }
    }

    if shape.in_place {
        if shape.input.is_empty() {
            bail!("--in-place requires -i/--input (cannot modify stdin)");
        }
        if shape.output.is_some() {
            bail!("--in-place is incompatible with -o/--output");
        }
        if shape.input.iter().any(|p| http::is_url(p)) {
            bail!("--in-place is incompatible with HTTP URLs");
        }
    }

    // Batch validation is skipped for multi-file slurp (it has its own rules).
    if is_batch && !is_multi_slurp {
        if !shape.in_place && shape.output.is_none() {
            bail!("Batch mode requires -o/--output directory or --in-place");
        }
        if !shape.in_place {
            let out = shape
                .output
                .as_ref()
                .expect("invariant: batch mode without --in-place requires -o (validated above)");
            if Path::new(out).exists() && !Path::new(out).is_dir() {
                bail!("Batch mode output must be a directory: {out}");
            }
        }
    }

    Ok(())
}

/// `--output-format raw` short-circuit: bypass the transform pipeline and
/// stream input bytes (file or HTTP) straight to the output destination.
fn run_raw_passthrough(shape: ShapeArgs, debug: bool, is_batch: bool) -> Result<CliResult> {
    if !shape.mold.is_empty() || !shape.expression.is_empty() {
        bail!("--output-format raw is incompatible with -m/--mold and -e/--expression (raw bypasses the transform pipeline)");
    }
    if shape.no_input {
        bail!("--output-format raw requires input data");
    }
    let http_opts = HttpOptions {
        headers: shape.http_header,
        timeout: shape.timeout,
        no_follow: shape.no_follow,
    };

    let fetch_bytes = |path: &str| -> Result<Vec<u8>> {
        if http::is_url(path) {
            if debug {
                eprintln!("[debug] binary mode: HTTP fetch {path}");
            }
            http::fetch_url_bytes(
                path,
                &http_opts.headers,
                http_opts.timeout,
                http_opts.no_follow,
                debug,
            )
        } else {
            if debug {
                eprintln!("[debug] binary mode: reading file {path}");
            }
            fs::read(path).with_context(|| format!("Failed to read input file: {path}"))
        }
    };

    if is_batch {
        // Streaming multiple binaries to stdout doesn't work — must derive a filename.
        if !shape.url_filename {
            bail!("--output-format raw with multiple inputs requires -O (--url-filename)");
        }
        for input in &shape.input {
            let bytes = fetch_bytes(input)?;
            if debug {
                eprintln!("[debug] binary mode: {} bytes", bytes.len());
            }
            let filename = url_filename(input)?;
            write_bytes_to(Some(&filename), &bytes)?;
        }
        return Ok(CliResult::Done);
    }

    let input_path = shape.input.first().map(|s| s.as_str());
    let bytes = if let Some(path) = input_path {
        fetch_bytes(path)?
    } else {
        if debug {
            eprintln!("[debug] binary mode: reading stdin");
        }
        let mut buf = Vec::new();
        io::stdin()
            .read_to_end(&mut buf)
            .context("Failed to read from stdin")?;
        buf
    };

    if debug {
        eprintln!("[debug] binary mode: {} bytes", bytes.len());
    }

    let binary_output_path: Option<String> = if shape.url_filename {
        Some(url_filename(input_path.unwrap_or(""))?)
    } else {
        shape.output.clone()
    };

    write_bytes_to(binary_output_path.as_deref(), &bytes)?;

    Ok(CliResult::Done)
}

pub fn run_shape_pipeline(
    shape: &ShapeArgs,
    policy: &SandboxPolicy,
    debug: bool,
    msg_level: u8,
) -> Result<CliResult> {
    let is_batch = shape.input.len() > 1;
    let is_multi_slurp = is_batch && shape.slurp;

    // Parse --arg name=value pairs
    let extra_args: Vec<(String, String)> = shape
        .args
        .iter()
        .map(|arg| {
            let (name, value) = arg.split_once('=').unwrap_or_else(|| {
                eprintln!("Warning: --arg '{arg}' missing '=' separator, treating as empty value");
                (arg.as_str(), "")
            });
            (name.to_string(), value.to_string())
        })
        .collect();

    // Build env dict from --env patterns (empty dict if no --env)
    let env_value = build_env(&shape.env_patterns);

    // Build scripts chain
    let script_refs = build_script_refs(&shape.mold, &shape.expression);
    let scripts = build_scripts(&script_refs, shape.no_cache)?;

    // First mold's defaults drive input options; last mold's defaults drive output options
    let first_defaults = &scripts[0].defaults;
    let last_defaults = &scripts[scripts.len() - 1].defaults;

    // Build CSV options from CLI args
    let output_delim = match &shape.csv_output_delimiter {
        Some(d) => Some(format::parse_delimiter(d)?),
        None => None,
    };
    let mut csv_opts = CsvOptions {
        delimiter: format::parse_delimiter(&shape.csv_delimiter)?,
        output_delimiter: output_delim,
        no_input_header: shape.csv_no_input_header || shape.csv_header.is_some(),
        no_output_header: shape.csv_no_output_header,
        header_names: shape
            .csv_header
            .as_ref()
            .map(|h| h.split(',').map(|s| s.trim().to_string()).collect()),
    };

    // Apply first mold defaults to CSV options (CLI explicit > mold defaults > code defaults)
    if let Some(ref delim) = first_defaults.csv_delimiter {
        if shape.csv_delimiter == "," {
            csv_opts.delimiter = format::parse_delimiter(delim)?;
        }
    }
    if first_defaults.csv_no_input_header
        && !shape.csv_no_input_header
        && shape.csv_header.is_none()
    {
        csv_opts.no_input_header = true;
    }
    if first_defaults.csv_no_output_header && !shape.csv_no_output_header {
        csv_opts.no_output_header = true;
    }
    if let Some(ref delim) = first_defaults.csv_output_delimiter {
        if csv_opts.output_delimiter.is_none() {
            csv_opts.output_delimiter = Some(format::parse_delimiter(delim)?);
        }
    }
    if let Some(ref header) = first_defaults.csv_header {
        if csv_opts.header_names.is_none() {
            csv_opts.no_input_header = true;
            csv_opts.header_names = Some(header.split(',').map(|s| s.trim().to_string()).collect());
        }
    }

    // Effective input format (CLI > first mold defaults, unless directive is forced)
    let effective_input_format = if first_defaults.forced.contains("input-format") {
        if debug {
            eprintln!(
                "[debug] mold forces input-format={}",
                first_defaults.input_format.as_deref().unwrap_or("?")
            );
        }
        first_defaults.input_format.as_deref()
    } else {
        shape
            .input_format
            .as_deref()
            .or(first_defaults.input_format.as_deref())
    };

    // Effective output format (CLI > last mold defaults, unless directive is forced)
    let effective_output_format = if last_defaults.forced.contains("output-format") {
        if debug {
            eprintln!(
                "[debug] mold forces output-format={}",
                last_defaults.output_format.as_deref().unwrap_or("?")
            );
        }
        last_defaults.output_format.as_deref()
    } else {
        shape
            .output_format
            .as_deref()
            .or(last_defaults.output_format.as_deref())
    };

    // Build HTTP options
    let http_opts = HttpOptions {
        headers: shape.http_header.clone(),
        timeout: shape.timeout,
        no_follow: shape.no_follow || first_defaults.no_follow,
    };

    // Multi-file slurp: combine all inputs into a single data structure, run mold once.
    if is_multi_slurp {
        // --in-place makes no sense when outputs are combined into one
        if shape.in_place {
            bail!("Multi-file slurp (-s with multiple -i) is incompatible with --in-place");
        }
        // -o must be a file, not a directory
        if let Some(ref out) = shape.output {
            if Path::new(out).is_dir() {
                bail!("Multi-file slurp (-s with multiple -i): -o must be a file, not a directory");
            }
        }

        // Parse alias suffixes from each -i entry
        let entries: Vec<(&str, Option<Option<&str>>)> = shape
            .input
            .iter()
            .map(|s| parse_input_entry(s.as_str()))
            .collect();

        let has_alias = entries.iter().any(|(_, a)| a.is_some());
        let all_alias = entries.iter().all(|(_, a)| a.is_some());

        if has_alias && !all_alias {
            bail!(
                "Multi-file slurp: all -i entries must use ':' alias syntax or none must (cannot mix)"
            );
        }

        // Parse each file and build the combined Value
        let combined: Value = if has_alias {
            // Named mode → Value::Object keyed by stem or explicit alias
            let mut map = serde_json::Map::new();
            for (path, alias_opt) in &entries {
                let alias = match alias_opt.as_ref().expect(
                    "invariant: has_alias && all_alias means every entry carries an alias_opt",
                ) {
                    Some(name) => name.to_string(),
                    None => path_stem(path),
                };
                if map.contains_key(&alias) {
                    bail!(
                        "Multi-file slurp: duplicate key '{alias}' — use explicit aliases to disambiguate"
                    );
                }
                let val = read_and_parse_for_slurp(
                    path,
                    effective_input_format,
                    &csv_opts,
                    &http_opts,
                    debug,
                )?;
                map.insert(alias, val);
            }
            Value::Object(map)
        } else {
            // List mode → Value::Array in input order
            let mut values = Vec::new();
            for (path, _) in &entries {
                let val = read_and_parse_for_slurp(
                    path,
                    effective_input_format,
                    &csv_opts,
                    &http_opts,
                    debug,
                )?;
                values.push(val);
            }
            Value::Array(values)
        };

        if debug {
            eprintln!(
                "[debug] multi-file slurp: {} files combined into {}",
                entries.len(),
                if has_alias { "object" } else { "array" }
            );
        }

        let data = convert::json_into_monty(combined);
        let slurp_metadata = fimod::pipeline::PipelineMetadata {
            input: None,
            output: shape.output.as_deref(),
            input_format: effective_input_format,
            output_format: effective_output_format,
            in_place: false,
            slurp: true,
            no_input: false,
        };
        let slurp_ctx = fimod::pipeline::ChainExecCtx {
            extra_args: &extra_args,
            env_value: &env_value,
            policy,
            debug,
            msg_level,
        };
        let slurp_out =
            execute_chain_to_value(&scripts, data, &slurp_metadata, &Value::Null, &slurp_ctx)?;
        let result = slurp_out.value;
        let opt_exit_code = slurp_out.exit_code;
        let fmt_override = slurp_out.format_override;
        let output_file_override = slurp_out.output_file_override;

        // set_output_file() overrides the CLI -o path for multi-file slurp output
        let actual_output = output_file_override.as_deref().or(shape.output.as_deref());
        let eff_out_fmt = fmt_override.as_deref().or(effective_output_format);

        if let Some(code) = opt_exit_code {
            if !shape.check {
                output_result(
                    &result,
                    actual_output,
                    eff_out_fmt,
                    DataFormat::Json,
                    &csv_opts,
                    false,
                    debug,
                )?;
            }
            return Ok(CliResult::Exit(code));
        }

        if shape.check {
            return Ok(CliResult::Exit(if is_truthy(&result) { 0 } else { 1 }));
        }

        output_result(
            &result,
            actual_output,
            eff_out_fmt,
            DataFormat::Json,
            &csv_opts,
            false,
            debug,
        )?;
        return Ok(CliResult::Done);
    }

    if is_batch {
        // Batch mode: create output directory if needed
        if let Some(ref dir) = shape.output {
            fs::create_dir_all(dir)
                .with_context(|| format!("Failed to create output directory: {dir}"))?;
        }

        for input_path in &shape.input {
            let per_file_output: String = if shape.in_place {
                input_path.clone()
            } else {
                let dir = shape
                    .output
                    .as_ref()
                    .expect("invariant: batch mode without --in-place requires -o");
                let filename = Path::new(input_path)
                    .file_name()
                    .context("Input path has no filename")?;
                Path::new(dir).join(filename).to_string_lossy().into_owned()
            };

            // Propagate the first per-file CliResult::Exit; otherwise continue.
            if let CliResult::Exit(code) = process_single_input(SingleRunOptions {
                input_path: Some(input_path.as_str()),
                no_input: false, // no_input always false in batch
                slurp: shape.slurp,
                effective_input_format,
                csv_opts: &csv_opts,
                scripts: &scripts,
                extra_args: &extra_args,
                env_value: &env_value,
                debug,
                msg_level,
                output_path: Some(per_file_output.as_str()),
                effective_output_format,
                in_place: shape.in_place,
                check: shape.check,
                http_opts: &http_opts,
                policy,
            })? {
                return Ok(CliResult::Exit(code));
            }
        }
        return Ok(CliResult::Done);
    }

    // Single-file (or stdin) mode
    let input_path = shape.input.first().map(|s| s.as_str());
    let url_derived_filename: Option<String> = if shape.url_filename {
        let url = input_path.unwrap_or("");
        if http::is_url(url) {
            Some(url_filename(url)?)
        } else {
            bail!("--url-filename requires an HTTP URL as input (-i)");
        }
    } else {
        None
    };

    let output_path = if shape.in_place {
        shape.input.first().map(|s| s.as_str())
    } else if let Some(ref name) = url_derived_filename {
        Some(name.as_str())
    } else {
        shape.output.as_deref()
    };

    process_single_input(SingleRunOptions {
        input_path,
        no_input: shape.no_input,
        slurp: shape.slurp,
        effective_input_format,
        csv_opts: &csv_opts,
        scripts: &scripts,
        extra_args: &extra_args,
        env_value: &env_value,
        debug,
        msg_level,
        output_path,
        effective_output_format,
        in_place: shape.in_place,
        check: shape.check,
        http_opts: &http_opts,
        policy,
    })
}
