//! Clap CLI definitions: top-level `Cli`, `ShapeArgs`, all subcommand enums,
//! and the completion candidate helpers referenced by their `#[arg]` attributes.

use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::engine::{ArgValueCandidates, ArgValueCompleter, CompletionCandidate};

use fimod::registry;

/// Verbosity level for `msg_*` functions in mold scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MsgLevel {
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
#[cfg_attr(
    feature = "fast",
    command(version = concat!(env!("CARGO_PKG_VERSION"), " fast (Monty engine: v", env!("MONTY_VERSION"), ")"))
)]
#[cfg_attr(
    all(not(feature = "fast"), feature = "reqwest"),
    command(version = concat!(env!("CARGO_PKG_VERSION"), " standard (Monty engine: v", env!("MONTY_VERSION"), ")"))
)]
#[cfg_attr(
    all(not(feature = "fast"), not(feature = "reqwest")),
    command(version = concat!(env!("CARGO_PKG_VERSION"), " slim (Monty engine: v", env!("MONTY_VERSION"), ")"))
)]
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
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Shape args: all flags for the transform pipeline
#[derive(Args, Debug)]
pub struct ShapeArgs {
    /// Input file(s) — supports multiple files for batch mode (shell glob expansion)
    #[arg(short, long, num_args = 1.., value_hint = clap::ValueHint::FilePath)]
    pub input: Vec<String>,

    /// Mold scripts applied in order (repeatable, can be mixed with -e)
    #[arg(short, long, add = ArgValueCompleter::new(complete_molds))]
    pub mold: Vec<String>,

    /// Inline Python expressions applied in order (repeatable, can be mixed with -m)
    #[arg(short = 'e', long = "expression")]
    pub expression: Vec<String>,

    /// Output file or directory (writes to stdout if not provided; directory required for batch)
    #[arg(short, long, value_hint = clap::ValueHint::AnyPath)]
    pub output: Option<String>,

    /// Modify input file(s) in-place (requires -i, incompatible with -o)
    #[arg(long = "in-place")]
    pub in_place: bool,

    /// Re-run the transform when -i or -m files change (single input, file-based only)
    #[arg(long, short = 'w')]
    pub watch: bool,

    /// Use the filename from the input URL as the output filename (like curl -O)
    #[arg(short = 'O', long = "url-filename", conflicts_with_all = ["output", "in_place"])]
    pub url_filename: bool,

    /// Read input paths/URLs from FILE or stdin (-), one per line
    #[arg(
        short = 'I',
        long = "input-list",
        value_name = "FILE|-",
        value_hint = clap::ValueHint::FilePath,
        conflicts_with = "input"
    )]
    pub input_list: Option<String>,

    /// Input format (auto-detected from extension if not specified)
    #[arg(long, value_name = "FORMAT", add = ArgValueCandidates::new(format_candidates))]
    pub input_format: Option<String>,

    /// Output format (defaults to input format if not specified)
    #[arg(long, value_name = "FORMAT", add = ArgValueCandidates::new(format_candidates))]
    pub output_format: Option<String>,

    /// Pass a named string variable to the mold (can be repeated): --arg name=value
    #[arg(long = "arg", value_name = "NAME=VALUE", action = clap::ArgAction::Append)]
    pub args: Vec<String>,

    /// Show debug info on stderr (script, input/output data, formats)
    #[arg(short = 'd', long = "debug")]
    pub debug: bool,

    /// Suppress all msg_* output except msg_error()
    #[arg(long = "quiet", conflicts_with = "msg_level")]
    pub quiet: bool,

    /// Verbosity level for msg_* functions (verbose: +msg_verbose, trace: +msg_verbose+msg_trace)
    #[arg(long = "msg-level", value_name = "LEVEL", conflicts_with = "quiet")]
    pub msg_level: Option<MsgLevel>,

    /// CSV delimiter character (default: ',', use '\t' for tab)
    #[arg(long, default_value = ",")]
    pub csv_delimiter: String,

    /// CSV output delimiter (defaults to --csv-delimiter)
    #[arg(long)]
    pub csv_output_delimiter: Option<String>,

    /// CSV: input file has no header line (columns named col0, col1, ...)
    #[arg(long)]
    pub csv_no_input_header: bool,

    /// CSV: don't write header line in output
    #[arg(long)]
    pub csv_no_output_header: bool,

    /// CSV: explicit column names for input (comma-separated, implies no header in file)
    #[arg(long, value_name = "COLS")]
    pub csv_header: Option<String>,

    /// Slurp: read multiple JSON values into a single array
    #[arg(short = 's', long = "slurp")]
    pub slurp: bool,

    /// No input data (data = None in Python)
    #[arg(long = "no-input")]
    pub no_input: bool,

    /// Check mode: no stdout, exit 0 if result is truthy, 1 if falsy
    #[arg(long = "check")]
    pub check: bool,

    /// Filter environment variables into the `env` parameter (glob pattern, repeatable)
    ///
    /// Examples: --env '*' (all), --env 'HOME,PATH', --env 'GITHUB_*'
    #[arg(long = "env", value_name = "PATTERN", action = clap::ArgAction::Append)]
    pub env_patterns: Vec<String>,

    /// Custom HTTP header (repeatable): --http-header "Authorization: Bearer xxx"
    #[arg(long = "http-header", num_args = 1, action = clap::ArgAction::Append)]
    pub http_header: Vec<String>,

    /// HTTP request timeout in seconds (default: 30)
    #[arg(long, default_value = "30")]
    pub timeout: u64,

    /// Don't follow HTTP redirects
    #[arg(long = "no-follow")]
    pub no_follow: bool,

    /// Bypass the local cache for remote catalogs and molds (always fetch fresh)
    #[arg(long = "no-cache")]
    pub no_cache: bool,

    /// Sandbox policy file (TOML). Empty (`--sandbox-file=""`) forces zero authorization.
    #[arg(long = "sandbox-file", value_name = "PATH", value_hint = clap::ValueHint::FilePath)]
    pub sandbox_file: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
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
    /// Install recommended defaults for a fimod category
    Setup {
        #[command(subcommand)]
        category: SetupCategory,
    },
}

#[derive(Subcommand, Debug)]
pub enum SetupCategory {
    /// Install community mold registries (examples, fimod-powered)
    Registry {
        #[command(subcommand)]
        action: SetupDefaults,
    },
    /// Write recommended sandbox policy to ~/.config/fimod/sandbox.toml
    Sandbox {
        #[command(subcommand)]
        action: SetupDefaults,
    },
    /// Run registry and sandbox setup in order (stops at the first failure)
    All {
        #[command(subcommand)]
        action: SetupDefaults,
    },
    /// Print a shell completion script to stdout
    ///
    /// Use with `eval` in your shell rc:
    ///   eval "$(fimod setup completions --shell zsh)"
    /// If --shell is omitted, the shell is auto-detected from $SHELL.
    Completions {
        /// Target shell (auto-detected from $SHELL if omitted)
        #[arg(long, value_enum)]
        shell: Option<CompletionShell>,
    },
}

#[derive(Subcommand, Debug)]
pub enum SetupDefaults {
    /// Install the recommended defaults for this category
    Defaults {
        /// Skip all prompts (non-interactive / CI)
        #[arg(short, long)]
        yes: bool,
        /// Overwrite existing configuration (sandbox.toml)
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    Elvish,
    Powershell,
}

#[derive(Subcommand, Debug)]
pub enum MontyAction {
    /// Start an interactive Monty Python REPL
    Repl,
}

#[derive(Subcommand, Debug)]
pub enum MoldAction {
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
        /// Mold name (use @registry/name to disambiguate); derived from filename when --path is used
        #[arg(required_unless_present = "path")]
        name: Option<String>,
        /// Inspect a mold file directly by path, without registry lookup
        #[arg(long, value_name = "FILE", conflicts_with = "registry")]
        path: Option<std::path::PathBuf>,
        /// Registry to search (searches all registries if not specified)
        #[arg(short, long, add = ArgValueCompleter::new(complete_sources))]
        registry: Option<String>,
        /// Output format
        #[arg(long = "output-format", value_name = "FORMAT", default_value = "text")]
        output_format: registry::MoldShowFormat,
    },
}

#[derive(Subcommand, Debug)]
pub enum RegistryAction {
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
pub enum CacheAction {
    /// Remove cached catalogs and molds
    Clear {
        /// Clear cache for a specific mold only (@name or @registry/name)
        name: Option<String>,
    },
    /// Show cache directory location and disk usage
    Info,
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
