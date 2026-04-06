#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process;

use fimod::pipeline::{
    build_env, build_scripts, execute_chain, is_truthy, output_result, parse_input_entry,
    path_stem, process_single_input, read_and_parse_for_slurp, read_input_list, url_filename,
    HttpOptions, ScriptRef,
};
use fimod::MONTY_VERSION;
use fimod::{convert, format, http, registry, test_runner};

use anyhow::{bail, Context, Result};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::engine::{ArgValueCandidates, ArgValueCompleter, CompletionCandidate};
use clap_complete::CompleteEnv;
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

/// fimod - the data shaper CLI.
///
/// Transform structured data with embedded Python. No system Python required.
#[derive(Parser, Debug)]
#[command(name = "fimod", about, long_about)]
#[cfg_attr(feature = "reqwest", command(version = concat!(env!("CARGO_PKG_VERSION"), " standard (Monty engine: v", env!("MONTY_VERSION"), ")")))]
#[cfg_attr(not(feature = "reqwest"), command(version = concat!(env!("CARGO_PKG_VERSION"), " slim (Monty engine: v", env!("MONTY_VERSION"), ")")))]
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
  fimod registry add examples https://github.com/org/fimod-molds
  fimod s -m @cleanup
  fimod s -m @my/toto")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

/// Shape args: all flags for the transform pipeline
#[derive(Args, Debug)]
struct ShapeArgs {
    /// Input file(s) — supports multiple files for batch mode (shell glob expansion)
    #[arg(short, long, num_args = 1..)]
    input: Vec<String>,

    /// Mold scripts applied in order (repeatable, can be mixed with -e)
    #[arg(short, long, add = ArgValueCompleter::new(complete_molds))]
    mold: Vec<String>,

    /// Inline Python expressions applied in order (repeatable, can be mixed with -m)
    #[arg(short = 'e', long = "expression")]
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
    #[arg(long, value_name = "FORMAT", add = ArgValueCandidates::new(format_candidates))]
    input_format: Option<String>,

    /// Output format (defaults to input format if not specified)
    #[arg(long, value_name = "FORMAT", add = ArgValueCandidates::new(format_candidates))]
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

    /// Bypass the local cache for remote catalogs and molds (always fetch fresh)
    #[arg(long = "no-cache")]
    no_cache: bool,
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
    /// Monty Python engine utilities
    Monty {
        #[command(subcommand)]
        action: MontyAction,
    },
    /// Show how to enable shell completions
    Completions {
        /// Shell to generate instructions for
        #[arg(value_enum)]
        shell: CompletionShell,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    Elvish,
    Powershell,
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
        #[arg(add = ArgValueCompleter::new(complete_sources))]
        registry: Option<String>,
        /// Output format
        #[arg(long = "output-format", value_name = "FORMAT", default_value = "text")]
        output_format: registry::MoldListFormat,
    },
    /// Run tests for a mold against *.input.* / *.expected.* file pairs
    Test {
        /// Mold script to test
        mold: String,
        /// Directory containing test cases
        tests_dir: String,
    },
    /// Show metadata and defaults for a mold
    Show {
        /// Mold name (use @registry/name to disambiguate)
        name: String,
        /// Registry to search (searches all registries if not specified)
        #[arg(short, long, add = ArgValueCompleter::new(complete_sources))]
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
    },
    /// Show details of a registry
    Show {
        /// Name of the registry
        #[arg(add = ArgValueCompleter::new(complete_sources))]
        name: String,
    },
    /// Remove a registry
    Remove {
        /// Name of the registry to remove
        #[arg(add = ArgValueCompleter::new(complete_sources))]
        name: String,
    },
    /// Set the priority rank for a registry
    ///
    /// Registries are searched in priority order (P0 first) when resolving bare @mold references.
    /// By default, swaps ranks if both registries already have a priority;
    /// cascades (shifts others down) if the source had no prior rank.
    /// Use --cascade to force cascade behavior.
    SetPriority {
        /// Name of the registry
        #[arg(add = ArgValueCompleter::new(complete_sources))]
        name: String,
        /// Priority rank (0, 1, 2, …)
        rank: Option<u32>,
        /// Clear the priority for this registry
        #[arg(long)]
        clear: bool,
        /// Force cascade: shift existing entries down instead of swapping
        #[arg(long)]
        cascade: bool,
    },
    /// Build or rebuild catalog.toml from a directory or registered registry
    BuildCatalog {
        /// Path to a molds directory
        #[arg(required_unless_present = "registry")]
        path: Option<String>,
        /// Use a registered registry name instead of a path
        #[arg(long, add = ArgValueCompleter::new(complete_sources))]
        registry: Option<String>,
    },
    /// Set up the fimod example molds registry
    ///
    /// Adds the example molds registry (P99) if not already present.
    /// Migrates the legacy 'official' registry to 'examples' if detected.
    Setup {
        /// Answer yes to all prompts (non-interactive / CI use)
        #[arg(short, long)]
        yes: bool,
    },
    /// Manage the local cache for remote catalogs and molds
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },
}

#[derive(Subcommand, Debug)]
enum CacheAction {
    /// Remove cached catalogs and molds
    Clear {
        /// Clear cache for a specific mold only (@name or @registry/name)
        name: Option<String>,
    },
    /// Show cache directory location and disk usage
    Info,
}

/// Build an ordered vec of ScriptRef by scanning CLI args to preserve -m/-e ordering.
fn build_script_refs(molds: &[String], expressions: &[String]) -> Vec<ScriptRef> {
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
        } else if arg.starts_with("-m") && arg.len() > 2 && !arg.starts_with("-m-") {
            // -mFOO (no space)
            if let Some(v) = mold_iter.next() {
                refs.push(ScriptRef::Mold(v.clone()));
            }
            i += 1;
        } else if (arg == "-e" || arg == "--expression") && i + 1 < args.len() {
            if let Some(v) = expr_iter.next() {
                refs.push(ScriptRef::Expr(v.clone()));
            }
            i += 2;
        } else if arg.starts_with("-e") && arg.len() > 2 && !arg.starts_with("-e-") {
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

fn format_candidates() -> Vec<CompletionCandidate> {
    [
        ("json", "Pretty-printed JSON"),
        ("json-compact", "Single-line JSON"),
        ("ndjson", "Newline-delimited JSON"),
        ("jsonl", "Alias for ndjson"),
        ("yaml", "YAML"),
        ("yml", "Alias for yaml"),
        ("toml", "TOML"),
        ("csv", "CSV"),
        ("tsv", "Alias for csv (tab-separated)"),
        ("txt", "Plain text (bare string)"),
        ("lines", "One line per array element"),
        ("raw", "Binary pass-through (output only)"),
        ("http", "HTTP response dict (input only)"),
    ]
    .into_iter()
    .map(|(val, help)| CompletionCandidate::new(val).help(Some(help.into())))
    .collect()
}

fn complete_molds(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let prefix = current.to_str().unwrap_or("");
    if !prefix.starts_with('@') {
        return Vec::new();
    }
    registry::complete_mold_names(prefix)
        .into_iter()
        .map(|(name, desc): (String, Option<String>)| {
            let mut c = CompletionCandidate::new(name);
            if let Some(d) = desc {
                c = c.help(Some(d.into()));
            }
            c
        })
        .collect()
}

fn complete_sources(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let prefix = current.to_str().unwrap_or("");
    registry::complete_source_names(prefix)
        .into_iter()
        .map(CompletionCandidate::new)
        .collect()
}

fn print_completion_instructions(shell: CompletionShell) {
    let (shell_name, instruction) = match shell {
        CompletionShell::Bash => ("Bash", "echo 'source <(COMPLETE=bash fimod)' >> ~/.bashrc"),
        CompletionShell::Zsh => ("Zsh", "echo 'source <(COMPLETE=zsh fimod)' >> ~/.zshrc"),
        CompletionShell::Fish => (
            "Fish",
            "echo 'COMPLETE=fish fimod | source' >> ~/.config/fish/completions/fimod.fish",
        ),
        CompletionShell::Elvish => (
            "Elvish",
            "echo 'eval (E:COMPLETE=elvish fimod | slurp)' >> ~/.elvish/rc.elv",
        ),
        CompletionShell::Powershell => (
            "PowerShell",
            r#"echo '$env:COMPLETE = "powershell"; fimod | Out-String | Invoke-Expression; Remove-Item Env:\COMPLETE' >> $PROFILE"#,
        ),
    };
    eprintln!(
        "# {shell_name}: add this to your shell config, then restart your shell:\n{instruction}"
    );
}

fn main() -> Result<()> {
    CompleteEnv::with_factory(Cli::command).complete();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Shape(shape)) => run_shape(*shape),
        Some(Commands::Registry { action }) => match action {
            RegistryAction::List { output_format } => registry::list(&output_format),
            RegistryAction::Add {
                name,
                location,
                token_env,
            } => registry::add(&name, &location, token_env.as_deref()),
            RegistryAction::Show { name } => registry::show(&name),
            RegistryAction::Remove { name } => registry::remove(&name),
            RegistryAction::SetPriority {
                name,
                rank,
                clear,
                cascade,
            } => registry::set_priority(&name, rank, clear, cascade),
            RegistryAction::BuildCatalog { path, registry } => {
                registry::build_catalog(registry.as_deref(), path.as_deref())
            }
            RegistryAction::Setup { yes } => registry::setup(yes),
            RegistryAction::Cache { action } => match action {
                CacheAction::Clear { name } => registry::cache_clear(name.as_deref()),
                CacheAction::Info => registry::cache_info(),
            },
        },
        Some(Commands::Mold { action }) => match action {
            MoldAction::List {
                registry,
                output_format,
            } => registry::list_molds(registry.as_deref(), output_format),
            MoldAction::Show { name, registry } => registry::show_mold(&name, registry.as_deref()),
            MoldAction::Test { mold, tests_dir } => test_runner::run(&mold, &tests_dir),
        },
        Some(Commands::Monty { action }) => match action {
            MontyAction::Repl => run_monty_repl(),
        },
        Some(Commands::Completions { shell }) => {
            print_completion_instructions(shell);
            Ok(())
        }
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
                let filename = url_filename(input)?;
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
            Some(url_filename(input_path.unwrap_or(""))?)
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
