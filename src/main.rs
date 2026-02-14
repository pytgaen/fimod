mod convert;
mod dotpath;
mod engine;
mod env_helpers;
mod exit_control;
mod format;
mod format_control;
mod gatekeeper;
mod hash;
mod http;
mod iter_helpers;
mod mold;
mod msg;
mod regex;
mod registry;
mod test_runner;

/// Monty engine version — keep in sync with the `tag` in Cargo.toml when upgrading monty.
const MONTY_VERSION: &str = "0.0.8";

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process;

use engine::MoldResult;
use mold::MoldSource;

use anyhow::{bail, Context, Result};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use monty::MontyObject;
use serde_json::Value;

use format::{CsvOptions, DataFormat};

/// Verbosity level for `msg_*` functions in mold scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum MsgLevel {
    /// Show msg_verbose() output in addition to defaults
    Verbose,
    /// Show msg_verbose() and msg_trace() output
    Trace,
}

/// Transform structured data using Python scripts via Monty.
///
/// No system Python required.
#[derive(Parser, Debug)]
#[command(name = "fimod", about, long_about)]
// keep "0.0.8" in sync with MONTY_VERSION const and the tag in Cargo.toml
#[command(version = concat!(env!("CARGO_PKG_VERSION"), " (Monty engine: v0.0.8)"))]
#[command(after_help = "\
EXAMPLES:
  fimod shape -i data.json -e 'data[\"name\"].upper()'
  fimod s -i data.json -m transform.py -o out.yaml
  fimod s -i data.csv -e '[r for r in data if int(r[\"age\"]) > 30]' --output-format json-compact
  cat data.json | fimod s -e '{\"count\": len(data)}' --output-format txt
  fimod s -i users.json --arg min_age=30 -e '[u for u in data if u[\"age\"] > int(args[\"min_age\"])]'
  fimod s -i data.json --env 'HOME,USER' -e 'env[\"HOME\"]'
  fimod s -i data.json -e 'data[\"users\"]' -e '[u for u in data if u[\"active\"]]'
  fimod s -i a.json b.json -m cleanup.py -o cleaned/
  fimod registry add my ./my-molds/
  fimod registry add official https://github.com/org/fimod-molds
  fimod s -m @cleanup
  fimod s -m @my/toto")]
struct Cli {
    /// Generate shell completion script and exit
    #[arg(long, value_name = "SHELL")]
    completions: Option<Shell>,

    #[command(subcommand)]
    command: Option<Commands>,
}

/// Shape args: all flags for the transform pipeline
#[derive(Args, Debug)]
struct ShapeArgs {
    /// Input file(s) — supports multiple files for batch mode (shell glob expansion)
    #[arg(short, long, num_args = 1..)]
    input: Vec<String>,

    /// Mold scripts applied in order (repeatable; mutually exclusive with -e)
    #[arg(short, long, conflicts_with = "expression")]
    mold: Vec<String>,

    /// Inline Python expressions applied in order (repeatable; mutually exclusive with -m)
    #[arg(short = 'e', long = "expression", conflicts_with = "mold")]
    expression: Vec<String>,

    /// Output file or directory (writes to stdout if not provided; directory required for batch)
    #[arg(short, long)]
    output: Option<String>,

    /// Modify input file(s) in-place (requires -i, incompatible with -o)
    #[arg(long = "in-place")]
    in_place: bool,

    /// Use the filename from the input URL as the output filename (like curl -O)
    #[arg(short = 'O', long = "url-filename", conflicts_with_all = ["output", "in_place"])]
    url_filename: bool,

    /// Read input paths/URLs from FILE or stdin (-), one per line
    #[arg(
        short = 'I',
        long = "input-list",
        value_name = "FILE|-",
        conflicts_with = "input"
    )]
    input_list: Option<String>,

    /// Input format (auto-detected from extension if not specified)
    #[arg(long, value_name = "FORMAT")]
    input_format: Option<String>,

    /// Output format (defaults to input format if not specified)
    #[arg(long, value_name = "FORMAT")]
    output_format: Option<String>,

    /// Pass a named string variable to the mold (can be repeated): --arg name=value
    #[arg(long = "arg", value_name = "NAME=VALUE", action = clap::ArgAction::Append)]
    args: Vec<String>,

    /// Show debug info on stderr (script, input/output data, formats)
    #[arg(short = 'd', long = "debug")]
    debug: bool,

    /// Suppress all msg_* output except msg_error()
    #[arg(long = "quiet", conflicts_with = "msg_level")]
    quiet: bool,

    /// Verbosity level for msg_* functions (verbose: +msg_verbose, trace: +msg_verbose+msg_trace)
    #[arg(long = "msg-level", value_name = "LEVEL", conflicts_with = "quiet")]
    msg_level: Option<MsgLevel>,

    /// CSV delimiter character (default: ',', use '\t' for tab)
    #[arg(long, default_value = ",")]
    csv_delimiter: String,

    /// CSV output delimiter (defaults to --csv-delimiter)
    #[arg(long)]
    csv_output_delimiter: Option<String>,

    /// CSV: input file has no header line (columns named col0, col1, ...)
    #[arg(long)]
    csv_no_input_header: bool,

    /// CSV: don't write header line in output
    #[arg(long)]
    csv_no_output_header: bool,

    /// CSV: explicit column names for input (comma-separated, implies no header in file)
    #[arg(long, value_name = "COLS")]
    csv_header: Option<String>,

    /// Slurp: read multiple JSON values into a single array
    #[arg(short = 's', long = "slurp")]
    slurp: bool,

    /// No input data (data = None in Python)
    #[arg(long = "no-input")]
    no_input: bool,

    /// Check mode: no stdout, exit 0 if result is truthy, 1 if falsy
    #[arg(long = "check")]
    check: bool,

    /// Filter environment variables into the `env` parameter (glob pattern, repeatable)
    ///
    /// Examples: --env '*' (all), --env 'HOME,PATH', --env 'GITHUB_*'
    #[arg(long = "env", value_name = "PATTERN", action = clap::ArgAction::Append)]
    env_patterns: Vec<String>,

    /// Custom HTTP header (repeatable): --http-header "Authorization: Bearer xxx"
    #[arg(long = "http-header", num_args = 1, action = clap::ArgAction::Append)]
    http_header: Vec<String>,

    /// HTTP request timeout in seconds (default: 30)
    #[arg(long, default_value = "30")]
    timeout: u64,

    /// Don't follow HTTP redirects
    #[arg(long = "no-follow")]
    no_follow: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Transform structured data (reads, applies Python mold, outputs result)
    #[command(visible_alias = "s")]
    Shape(Box<ShapeArgs>),
    /// Manage mold registries (named collections of mold scripts)
    Registry {
        #[command(subcommand)]
        action: RegistryAction,
    },
    /// Browse molds available in a registry
    Mold {
        #[command(subcommand)]
        action: MoldAction,
    },
    /// Run tests for a mold against *.input.* / *.expected.* file pairs
    Test {
        /// Mold script to test
        mold: String,
        /// Directory containing test cases
        tests_dir: String,
    },
    /// Monty Python engine utilities
    Monty {
        #[command(subcommand)]
        action: MontyAction,
    },
}

#[derive(Subcommand, Debug)]
enum MontyAction {
    /// Start an interactive Monty Python REPL
    Repl,
}

#[derive(Subcommand, Debug)]
enum MoldAction {
    /// List molds available in a registry (local scan or remote catalog.toml)
    List {
        /// Registry name (lists all registries if not specified)
        registry: Option<String>,
        /// Output format
        #[arg(long = "output-format", value_name = "FORMAT", default_value = "text")]
        output_format: registry::MoldListFormat,
    },
    /// Show metadata and defaults for a mold
    Show {
        /// Mold name (use @registry/name to disambiguate)
        name: String,
        /// Registry to search (searches all registries if not specified)
        #[arg(short, long)]
        registry: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum RegistryAction {
    /// List all configured registries
    List {
        /// Output format: text (default) or json
        #[arg(long = "output-format", value_name = "FORMAT", default_value = "text")]
        output_format: String,
    },
    /// Add a registry (local directory or remote URL)
    Add {
        /// Name for the registry
        name: String,
        /// Local directory path or URL (github.com, gitlab, https://)
        location: String,
        /// Environment variable name for authentication token (overrides default GITHUB_TOKEN / GITLAB_TOKEN)
        #[arg(long = "token-env", value_name = "VAR")]
        token_env: Option<String>,
        /// Set this registry as the default
        #[arg(long)]
        default: bool,
    },
    /// Show details of a registry
    Show {
        /// Name of the registry
        name: String,
    },
    /// Remove a registry
    Remove {
        /// Name of the registry to remove
        name: String,
    },
    /// Set the default registry (used when no @registry/ prefix is given)
    SetDefault {
        /// Name of the registry to set as default
        name: String,
    },
    /// Build or rebuild catalog.toml for a local registry
    BuildCatalog {
        /// Name of the local registry
        name: String,
    },
    /// Set up the official fimod molds registry
    ///
    /// Adds the official registry if not already present.
    /// In a fresh install (no default registry yet) it becomes the default automatically.
    /// If a default already exists it is left unchanged unless --force is given.
    Setup {
        /// Answer yes to all prompts (non-interactive / CI use)
        #[arg(short, long)]
        yes: bool,
        /// Promote the official registry to default even if another default is already set
        #[arg(short, long)]
        force: bool,
    },
}

/// Determine if a JSON value is "truthy" for --check mode.
/// Falsy: null, false, 0, "", [], {}
/// Everything else is truthy.
fn is_truthy(v: &serde_json::Value) -> bool {
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
fn build_scripts(
    molds: &[String],
    expressions: &[String],
) -> Result<Vec<(String, String, mold::MoldDefaults)>> {
    if !expressions.is_empty() {
        let mut steps = Vec::new();
        for e in expressions {
            let source = MoldSource::Inline(e.clone());
            let display = source.to_string();
            let script = source.load()?;
            steps.push((display, script, mold::MoldDefaults::default()));
        }
        Ok(steps)
    } else if !molds.is_empty() {
        let mut steps = Vec::new();
        for m in molds {
            let source = MoldSource::from_mold_str(m)?;
            let is_inline = matches!(source, MoldSource::Inline(_));
            let display = source.to_string();
            let script = source.load()?;
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
fn execute_chain(
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
struct HttpOptions {
    headers: Vec<String>,
    timeout: u64,
    no_follow: bool,
}

/// Process a single input through the full pipeline: read → parse → execute chain → serialize → write.
#[allow(clippy::too_many_arguments)]
fn process_single_input(
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
    // Track CSV headers for injection into step 0
    let mut csv_headers: Option<Vec<String>> = None;
    // Raw bytes from HTTP response, available for binary pass-through via set_output_format("raw")
    let mut http_raw_bytes: Option<Vec<u8>> = None;

    let (in_fmt, data) = if no_input {
        if debug {
            eprintln!("[debug] no-input mode: data = None");
        }
        (DataFormat::Json, MontyObject::None)
    } else {
        // Check if input is a URL
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

            // Determine format hint from Content-Type
            let ct_fmt = resp
                .content_type
                .as_deref()
                .and_then(http::content_type_to_format);

            // If --input-format http, build the HTTP dict directly and skip normal parsing
            if effective_input_format == Some("http") {
                // Detect binary content: no known text format and not a text/ type
                let is_binary = resp
                    .content_type
                    .as_deref()
                    .map(|ct| {
                        http::content_type_to_format(ct).is_none() && !ct.starts_with("text/")
                    })
                    .unwrap_or(false);

                // Store raw bytes for potential binary pass-through (set_output_format("raw"))
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
    let (result, opt_exit_code, fmt_override, output_file_override) = execute_chain(
        scripts,
        data,
        extra_args,
        env_value,
        &headers_value,
        debug,
        msg_level,
    )?;

    // set_output_file() overrides the CLI -o path; otherwise fall back to CLI-provided path
    let actual_output = output_file_override.as_deref().or(output_path);

    // Binary pass-through: set_output_format("raw") signals that raw HTTP bytes should be written
    // directly, bypassing the normal serde serialization pipeline.
    if fmt_override.as_deref() == Some("raw") {
        let bytes = http_raw_bytes.ok_or_else(|| {
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
        if let Some(code) = opt_exit_code {
            process::exit(code);
        }
        return Ok(());
    }

    // If set_input_format() or set_output_format() was called (non-raw), it overrides the output format
    let effective_output_format = fmt_override.as_deref().or(effective_output_format);

    // Handle set_exit and --check
    if let Some(code) = opt_exit_code {
        if !check {
            output_result(
                &result,
                actual_output,
                effective_output_format,
                in_fmt,
                csv_opts,
                no_input,
                debug,
            )?;
        }
        process::exit(code);
    }

    if check {
        let code = if is_truthy(&result) { 0 } else { 1 };
        process::exit(code);
    }

    output_result(
        &result,
        actual_output,
        effective_output_format,
        in_fmt,
        csv_opts,
        no_input,
        debug,
    )
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle --completions
    if let Some(shell) = cli.completions {
        let mut cmd = Cli::command();
        generate(shell, &mut cmd, "fimod", &mut io::stdout());
        return Ok(());
    }

    match cli.command {
        Some(Commands::Shape(shape)) => run_shape(*shape),
        Some(Commands::Registry { action }) => match action {
            RegistryAction::List { output_format } => registry::list(&output_format),
            RegistryAction::Add {
                name,
                location,
                token_env,
                default,
            } => registry::add(&name, &location, token_env.as_deref(), default),
            RegistryAction::Show { name } => registry::show(&name),
            RegistryAction::Remove { name } => registry::remove(&name),
            RegistryAction::SetDefault { name } => registry::set_default(&name),
            RegistryAction::BuildCatalog { name } => registry::build_catalog(&name),
            RegistryAction::Setup { yes, force } => registry::setup(yes, force),
        },
        Some(Commands::Mold { action }) => match action {
            MoldAction::List {
                registry,
                output_format,
            } => registry::list_molds(registry.as_deref(), output_format),
            MoldAction::Show { name, registry } => registry::show_mold(&name, registry.as_deref()),
        },
        Some(Commands::Test { mold, tests_dir }) => test_runner::run(&mold, &tests_dir),
        Some(Commands::Monty { action }) => match action {
            MontyAction::Repl => run_monty_repl(),
        },
        None => {
            Cli::command().print_help()?;
            std::process::exit(2);
        }
    }
}

fn run_monty_repl() -> Result<()> {
    use monty::{detect_repl_continuation_mode, MontyRepl, NoLimitTracker, ReplContinuationMode};
    use rustyline::error::ReadlineError;
    use rustyline::DefaultEditor;

    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stdin());

    if is_tty {
        eprintln!(
            "Monty REPL v{MONTY_VERSION} — fimod v{} (exit or Ctrl+D to quit)",
            env!("CARGO_PKG_VERSION")
        );
    }

    let mut rl = DefaultEditor::new()?;
    let mut repl = MontyRepl::new("repl.py", NoLimitTracker);
    let mut pending_snippet = String::new();
    let mut continuation_mode = ReplContinuationMode::Complete;

    loop {
        let prompt = if continuation_mode == ReplContinuationMode::Complete {
            ">>> "
        } else {
            "... "
        };

        let line = match rl.readline(prompt) {
            Ok(l) => l,
            Err(ReadlineError::Interrupted) => continue,
            Err(ReadlineError::Eof) => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        let _ = rl.add_history_entry(&line);

        let snippet = line.trim_end();
        if continuation_mode == ReplContinuationMode::Complete && snippet.is_empty() {
            continue;
        }
        if continuation_mode == ReplContinuationMode::Complete && snippet == "exit" {
            return Ok(());
        }

        pending_snippet.push_str(snippet);
        pending_snippet.push('\n');

        if continuation_mode == ReplContinuationMode::IncompleteBlock && snippet.is_empty() {
            repl_feed(&mut repl, &pending_snippet);
            pending_snippet.clear();
            continuation_mode = ReplContinuationMode::Complete;
            continue;
        }

        let detected = detect_repl_continuation_mode(&pending_snippet);
        match detected {
            ReplContinuationMode::Complete => {
                if continuation_mode == ReplContinuationMode::IncompleteBlock {
                    continue;
                }
                repl_feed(&mut repl, &pending_snippet);
                pending_snippet.clear();
                continuation_mode = ReplContinuationMode::Complete;
            }
            ReplContinuationMode::IncompleteBlock => {
                continuation_mode = ReplContinuationMode::IncompleteBlock;
            }
            ReplContinuationMode::IncompleteImplicit => {
                if continuation_mode != ReplContinuationMode::IncompleteBlock {
                    continuation_mode = ReplContinuationMode::IncompleteImplicit;
                }
            }
        }
    }
}

fn repl_feed(repl: &mut monty::MontyRepl<monty::NoLimitTracker>, snippet: &str) {
    match repl.feed_run(snippet, vec![], monty::PrintWriter::Stdout) {
        Ok(output) => {
            if output != MontyObject::None {
                println!("{output}");
            }
        }
        Err(err) => eprintln!("error:\n{err}"),
    }
}

fn read_input_list(source: &str) -> Result<Vec<String>> {
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
fn env_pattern_matches(name: &str, patterns: &[String]) -> bool {
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
fn parse_input_entry(s: &str) -> (&str, Option<Option<&str>>) {
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

/// Extract the stem (filename without extension) from a path string.
fn path_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_string()
}

/// Read and parse a single file (or URL) as a `Value` for multi-file slurp mode.
fn read_and_parse_for_slurp(
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
            let path_part = path.split('?').next().unwrap_or(path);
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

fn run_shape(mut shape: ShapeArgs) -> Result<()> {
    // Resolve --input-list into shape.input before any other processing
    if let Some(ref source) = shape.input_list.clone() {
        shape.input = read_input_list(source)?;
        if shape.input.is_empty() {
            bail!("--input-list: no inputs found in '{source}'");
        }
    }

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

    // Validate --no-input
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

    // Validate --in-place
    if shape.in_place {
        if shape.input.is_empty() {
            bail!("--in-place requires -i/--input (cannot modify stdin)");
        }
        if shape.output.is_some() {
            bail!("--in-place is incompatible with -o/--output");
        }
        // Cannot modify a URL in-place
        if shape.input.iter().any(|p| http::is_url(p)) {
            bail!("--in-place is incompatible with HTTP URLs");
        }
    }

    // Validate batch mode (skipped for multi-file slurp which has its own rules)
    if is_batch && !is_multi_slurp {
        if !shape.in_place && shape.output.is_none() {
            bail!("Batch mode requires -o/--output directory or --in-place");
        }
        if !shape.in_place {
            let out = shape.output.as_ref().unwrap();
            if Path::new(out).exists() && !Path::new(out).is_dir() {
                bail!("Batch mode output must be a directory: {out}");
            }
        }
    }

    // --output-format raw: short-circuit the entire pipeline (binary pass-through)
    if shape.output_format.as_deref() == Some("raw") {
        // Validate: raw output is incompatible with molds/expressions
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

        // Helper: derive filename from a URL path (for -O)
        let derive_url_filename = |url: &str| -> Result<String> {
            url.split('?')
                .next()
                .unwrap_or(url)
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .ok_or_else(|| {
                    anyhow::anyhow!("--url-filename: cannot determine filename from '{url}'")
                })
        };

        // Helper: fetch bytes from a URL or read from a file
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
            // Multiple inputs from --input-list: -O required (can't stream multiple binaries to stdout)
            if !shape.url_filename {
                bail!("--output-format raw with multiple inputs requires -O (--url-filename)");
            }
            for input in &shape.input {
                let bytes = fetch_bytes(input)?;
                if debug {
                    eprintln!("[debug] binary mode: {} bytes", bytes.len());
                }
                let filename = derive_url_filename(input)?;
                fs::write(&filename, &bytes)
                    .with_context(|| format!("Failed to write output file: {filename}"))?;
            }
            return Ok(());
        }

        // Single input
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
            Some(derive_url_filename(input_path.unwrap_or(""))?)
        } else {
            shape.output.clone()
        };

        match binary_output_path.as_deref() {
            Some(path) => {
                fs::write(path, &bytes)
                    .with_context(|| format!("Failed to write output file: {path}"))?;
            }
            None => {
                use std::io::Write;
                io::stdout()
                    .write_all(&bytes)
                    .context("Failed to write to stdout")?;
            }
        }

        return Ok(());
    }

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
    let env_value: Value = if shape.env_patterns.is_empty() {
        Value::Object(serde_json::Map::new())
    } else {
        let map: serde_json::Map<String, Value> = std::env::vars()
            .filter(|(k, _)| env_pattern_matches(k, &shape.env_patterns))
            .map(|(k, v)| (k, Value::String(v)))
            .collect();
        Value::Object(map)
    };

    // Build scripts chain
    let scripts = build_scripts(&shape.mold, &shape.expression)?;

    // First mold's defaults drive input options; last mold's defaults drive output options
    let first_defaults = &scripts[0].2;
    let last_defaults = &scripts[scripts.len() - 1].2;

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

    // Effective input format (CLI > first mold defaults)
    let effective_input_format = shape
        .input_format
        .as_deref()
        .or(first_defaults.input_format.as_deref());

    // Effective output format (CLI > last mold defaults)
    let effective_output_format = shape
        .output_format
        .as_deref()
        .or(last_defaults.output_format.as_deref());

    // Build HTTP options
    let http_opts = HttpOptions {
        headers: shape.http_header,
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
                let alias = match alias_opt.as_ref().unwrap() {
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
        let (result, opt_exit_code, fmt_override, output_file_override) = execute_chain(
            &scripts,
            data,
            &extra_args,
            &env_value,
            &Value::Null,
            debug,
            msg_level,
        )?;

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
            process::exit(code);
        }

        if shape.check {
            process::exit(if is_truthy(&result) { 0 } else { 1 });
        }

        return output_result(
            &result,
            actual_output,
            eff_out_fmt,
            DataFormat::Json,
            &csv_opts,
            false,
            debug,
        );
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
                let dir = shape.output.as_ref().unwrap();
                let filename = Path::new(input_path)
                    .file_name()
                    .context("Input path has no filename")?;
                Path::new(dir).join(filename).to_string_lossy().into_owned()
            };

            process_single_input(
                Some(input_path.as_str()),
                false, // no_input always false in batch
                shape.slurp,
                effective_input_format,
                &csv_opts,
                &scripts,
                &extra_args,
                &env_value,
                debug,
                msg_level,
                Some(per_file_output.as_str()),
                effective_output_format,
                shape.check,
                &http_opts,
            )?;
        }
        return Ok(());
    }

    // Single-file (or stdin) mode
    let input_path = shape.input.first().map(|s| s.as_str());
    let url_derived_filename: Option<String> = if shape.url_filename {
        let url = input_path.unwrap_or("");
        if http::is_url(url) {
            let name = url
                .split('?')
                .next()
                .unwrap_or(url)
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            if name.is_none() {
                bail!("--url-filename: cannot determine filename from URL '{url}'");
            }
            name
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

    process_single_input(
        input_path,
        shape.no_input,
        shape.slurp,
        effective_input_format,
        &csv_opts,
        &scripts,
        &extra_args,
        &env_value,
        debug,
        msg_level,
        output_path,
        effective_output_format,
        shape.check,
        &http_opts,
    )
}

fn output_result(
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
