//! Top-level `fimod setup <category>` subcommands.
//!
//! Installs recommended defaults for each category:
//! - `registry`  → community registries (same as legacy `fimod registry setup`).
//! - `sandbox`   → recommended `~/.config/fimod/sandbox.toml`.
//! - `all`       → runs registry then sandbox, failing at the first error.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::cli::{SetupSandboxKey, SetupSandboxPreset};
use crate::registry::{
    self,
    config::{load_config, save_config},
};

const EXAMPLES_NAME: &str = "examples";
const EXAMPLES_URL: &str = "https://github.com/pytgaen/fimod/tree/main/molds";
const POWERED_NAME: &str = "fimod-powered";
const POWERED_URL: &str = "https://github.com/pytgaen/fimod-powered/tree/main/molds";

const SANDBOX_DEFAULT_MAX_DURATION: &str = "10m";
const SANDBOX_DEFAULT_MAX_MEMORY: &str = "2GB";

#[derive(Debug, Clone)]
struct SandboxConfig {
    allow_clock: bool,
    max_duration: String,
    max_memory: String,
    allow_env: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SandboxFile {
    sandbox: Option<SandboxTable>,
}

#[derive(Debug, Deserialize, Default)]
struct SandboxTable {
    allow_clock: Option<bool>,
    max_duration: Option<String>,
    max_memory: Option<String>,
    allow_env: Option<Vec<String>>,
}

impl SandboxConfig {
    fn runtime_defaults() -> Self {
        Self {
            allow_clock: false,
            max_duration: SANDBOX_DEFAULT_MAX_DURATION.to_string(),
            max_memory: SANDBOX_DEFAULT_MAX_MEMORY.to_string(),
            allow_env: Vec::new(),
        }
    }

    fn preset(preset: SetupSandboxPreset) -> Self {
        match preset {
            SetupSandboxPreset::Recommended => Self {
                allow_clock: true,
                max_duration: SANDBOX_DEFAULT_MAX_DURATION.to_string(),
                max_memory: SANDBOX_DEFAULT_MAX_MEMORY.to_string(),
                allow_env: Vec::new(),
            },
            SetupSandboxPreset::Strict => Self {
                allow_clock: false,
                max_duration: "30s".to_string(),
                max_memory: "512MB".to_string(),
                allow_env: Vec::new(),
            },
            SetupSandboxPreset::Permissive => Self {
                allow_clock: true,
                max_duration: "30m".to_string(),
                max_memory: "4GB".to_string(),
                allow_env: vec![
                    "LANG".to_string(),
                    "LC_*".to_string(),
                    "TZ".to_string(),
                    "USER".to_string(),
                    "HOME".to_string(),
                ],
            },
        }
    }

    fn from_table(table: SandboxTable) -> Self {
        let defaults = Self::runtime_defaults();
        Self {
            allow_clock: table.allow_clock.unwrap_or(defaults.allow_clock),
            max_duration: table.max_duration.unwrap_or(defaults.max_duration),
            max_memory: table.max_memory.unwrap_or(defaults.max_memory),
            allow_env: table.allow_env.unwrap_or(defaults.allow_env),
        }
    }

    fn validate(&self) -> Result<()> {
        fimod::sandbox::parse_duration(&self.max_duration)
            .with_context(|| format!("max_duration: {:?}", self.max_duration))?;
        fimod::sandbox::parse_size(&self.max_memory)
            .with_context(|| format!("max_memory: {:?}", self.max_memory))?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct SandboxSetOptions {
    pub sandbox_file: Option<String>,
    pub allow_clock: bool,
    pub deny_clock: bool,
    pub max_duration: Option<String>,
    pub max_memory: Option<String>,
    pub allow_env: Vec<String>,
    pub clear_env: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct SetupOptions {
    pub yes: bool,
    pub force: bool,
    pub if_needed: bool,
}

impl SetupOptions {
    pub fn new(yes: bool, force: bool, if_needed: bool) -> Self {
        Self {
            yes,
            force,
            if_needed,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum SetupBlock {
    Registry,
    Sandbox,
}

#[derive(Debug, Clone, Copy)]
enum SetupPref {
    Yes,
    No,
    Ask,
}

fn setup_pref(block: SetupBlock, options: SetupOptions) -> SetupPref {
    if options.yes {
        return SetupPref::Yes;
    }

    let specific = match block {
        SetupBlock::Registry => "FIMOD_SETUP_REGISTRY",
        SetupBlock::Sandbox => "FIMOD_SETUP_SANDBOX",
    };

    env_pref(specific)
        .or_else(|| env_pref("FIMOD_SETUP_ALL"))
        .unwrap_or(SetupPref::Ask)
}

fn env_pref(name: &str) -> Option<SetupPref> {
    let value = std::env::var(name).ok()?;
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" => Some(SetupPref::Yes),
        "no" => Some(SetupPref::No),
        _ => None,
    }
}

fn apply_pref(pref: SetupPref, prompt: &str, default_yes: bool) -> Result<bool> {
    match pref {
        SetupPref::Yes => Ok(true),
        SetupPref::No => Ok(false),
        SetupPref::Ask => registry::confirm(prompt, default_yes),
    }
}

fn has_registry_url(cfg: &registry::SourcesConfig, url: &str) -> bool {
    cfg.sources.values().any(|s| s.url.as_deref() == Some(url))
}

/// Install recommended community registries.
///
/// `force` forces fimod to take ownership of `fimod-powered` and `examples`
/// entries: if they already exist (under any name pointing to the canonical
/// URLs), they are removed first so the canonical name + URL + priority is
/// re-applied.
///
/// Behaviour:
/// - Already present (by URL) → prints a message and exits cleanly.
/// - Missing entries → asks once unless `--yes` or `FIMOD_SETUP_*` answers it.
/// - `--force` removes existing canonical URLs first, then reinstalls them.
/// - `--if-needed` skips already-configured entries without prompting.
pub fn registry_defaults(options: SetupOptions) -> Result<()> {
    let mut cfg = load_config()?;

    // --force: take ownership of the canonical URLs by removing any existing
    // entry pointing to them, so they get reinstalled with the canonical
    // name and priority below.
    if options.force {
        let to_remove: Vec<String> = cfg
            .sources
            .iter()
            .filter(|(_, s)| {
                let url = s.url.as_deref();
                url == Some(POWERED_URL) || url == Some(EXAMPLES_URL)
            })
            .map(|(name, _)| name.clone())
            .collect();
        for name in &to_remove {
            registry::remove(name)?;
        }
        if !to_remove.is_empty() {
            cfg = load_config()?;
        }
    }

    // ── Migrate legacy "official" → "examples" ──
    let needs_official_migration = cfg
        .sources
        .get("official")
        .is_some_and(|source| source.url.as_deref() == Some(EXAMPLES_URL))
        && !cfg.sources.contains_key(EXAMPLES_NAME);

    // ── Detect which registries are already installed (by URL) ──
    let has_powered = has_registry_url(&cfg, POWERED_URL);
    let has_examples = has_registry_url(&cfg, EXAMPLES_URL);

    if !needs_official_migration && has_powered && has_examples {
        println!("Community registries are already configured.");
        return Ok(());
    }

    // ── Build the list of registries to install ──
    struct Entry {
        name: &'static str,
        url: &'static str,
        priority: u32,
        label: &'static str,
    }
    let mut to_install: Vec<Entry> = Vec::new();
    if !has_powered {
        to_install.push(Entry {
            name: POWERED_NAME,
            url: POWERED_URL,
            priority: 10,
            label: "production-ready molds",
        });
    }
    if !has_examples {
        to_install.push(Entry {
            name: EXAMPLES_NAME,
            url: EXAMPLES_URL,
            priority: 99,
            label: "learning & demo molds",
        });
    }

    // ── Ask confirmation ──
    let pref = setup_pref(SetupBlock::Registry, options);
    if matches!(pref, SetupPref::Ask) {
        println!("\nAdd community registries?");
        if needs_official_migration {
            println!("  • migrate official → examples");
        }
        for entry in &to_install {
            println!(
                "  • {:<16} ({})    P{}",
                entry.name, entry.label, entry.priority
            );
        }
        println!();
    }
    if !apply_pref(pref, "Install?", true)? {
        println!("Skipped. Run 'fimod setup registry defaults --if-needed' at any time.");
        return Ok(());
    }

    if needs_official_migration {
        let source = cfg.sources.remove("official").unwrap();
        cfg.sources.insert(EXAMPLES_NAME.to_string(), source);
        cfg.priority.remove("official");
        cfg.priority.insert(EXAMPLES_NAME.to_string(), 99);
        save_config(&cfg)?;
        println!("Migrated registry 'official' → 'examples' (P99).");
        cfg = load_config()?;
    }

    // ── Install each registry ──
    for entry in &to_install {
        if has_registry_url(&cfg, entry.url) {
            continue;
        }
        let name = if cfg.sources.contains_key(entry.name) {
            let alt = format!("fimod-{}", entry.name);
            println!(
                "Note: registry name '{}' is already taken, using '{alt}' instead.",
                entry.name
            );
            registry::add(&alt, entry.url, None)?;
            registry::set_priority(&alt, Some(entry.priority), false, false)?;
            alt
        } else {
            registry::add(entry.name, entry.url, None)?;
            registry::set_priority(entry.name, Some(entry.priority), false, false)?;
            entry.name.to_string()
        };
        println!("✓ Added {} (P{})", name, entry.priority);
    }

    Ok(())
}

/// Write a sandbox preset to `~/.config/fimod/sandbox.toml` or an explicit file.
///
/// - Refuses to overwrite an existing file unless `force` is set.
/// - `yes` skips the confirmation prompt (required in non-TTY contexts).
pub fn sandbox_defaults(
    options: SetupOptions,
    preset: SetupSandboxPreset,
    sandbox_file: Option<String>,
) -> Result<()> {
    let path = sandbox_target_path(sandbox_file.as_deref())?;
    let config = SandboxConfig::preset(preset);
    config.validate()?;
    let rendered = render_sandbox_config(&config);

    if path.exists() && !options.force {
        if options.if_needed {
            println!("Sandbox policy already exists: {}", path.display());
            return Ok(());
        }
        bail!(
            "{} already exists — use --force to overwrite",
            path.display()
        );
    }

    let pref = setup_pref(SetupBlock::Sandbox, options);
    if matches!(pref, SetupPref::Ask) {
        println!(
            "This will {} {} with the {preset:?} preset:",
            if path.exists() { "overwrite" } else { "create" },
            path.display()
        );
        println!();
        for line in rendered.lines().filter(|l| !l.trim().is_empty()) {
            println!("  {line}");
        }
        println!();
    }

    let default_yes = options.if_needed && !path.exists();
    if !apply_pref(pref, "Continue?", default_yes)? {
        println!("Skipped. Run 'fimod setup sandbox defaults --if-needed' at any time.");
        return Ok(());
    }

    write_sandbox_config_with_content(&path, &rendered)?;

    println!("✓ Wrote {}", path.display());
    Ok(())
}

/// Print the normalized sandbox policy for a target file.
pub fn sandbox_show(sandbox_file: Option<String>) -> Result<()> {
    let path = sandbox_target_path(sandbox_file.as_deref())?;
    let config = read_sandbox_config_or_recommended(&path)?;
    print!("{}", render_sandbox_config(&config));
    Ok(())
}

/// Print one sandbox policy value.
pub fn sandbox_get(key: SetupSandboxKey, sandbox_file: Option<String>) -> Result<()> {
    let path = sandbox_target_path(sandbox_file.as_deref())?;
    let config = read_sandbox_config_or_recommended(&path)?;
    match key {
        SetupSandboxKey::AllowClock => println!("{}", config.allow_clock),
        SetupSandboxKey::MaxDuration => println!("{}", config.max_duration),
        SetupSandboxKey::MaxMemory => println!("{}", config.max_memory),
        SetupSandboxKey::AllowEnv => {
            for pattern in config.allow_env {
                println!("{pattern}");
            }
        }
    }
    Ok(())
}

/// Update sandbox policy values in the canonical file or an explicit file.
pub fn sandbox_set(options: SandboxSetOptions) -> Result<()> {
    if options.allow_clock && options.deny_clock {
        bail!("--allow-clock and --deny-clock cannot be used together");
    }
    if options.clear_env && !options.allow_env.is_empty() {
        bail!("--clear-env and --allow-env cannot be used together");
    }
    if !options.allow_clock
        && !options.deny_clock
        && options.max_duration.is_none()
        && options.max_memory.is_none()
        && options.allow_env.is_empty()
        && !options.clear_env
    {
        bail!("nothing to configure; pass at least one sandbox option");
    }

    let path = sandbox_target_path(options.sandbox_file.as_deref())?;
    let mut config = read_sandbox_config_or_recommended(&path)?;

    if options.allow_clock {
        config.allow_clock = true;
    }
    if options.deny_clock {
        config.allow_clock = false;
    }
    if let Some(max_duration) = options.max_duration {
        fimod::sandbox::parse_duration(&max_duration)
            .with_context(|| format!("--max-duration {max_duration:?}"))?;
        config.max_duration = max_duration;
    }
    if let Some(max_memory) = options.max_memory {
        fimod::sandbox::parse_size(&max_memory)
            .with_context(|| format!("--max-memory {max_memory:?}"))?;
        config.max_memory = max_memory;
    }
    if !options.allow_env.is_empty() {
        config.allow_env = normalize_env_patterns(options.allow_env);
    }
    if options.clear_env {
        config.allow_env.clear();
    }

    config.validate()?;
    write_sandbox_config(&path, &config)?;
    println!("✓ Wrote {}", path.display());
    Ok(())
}

/// Run `registry_defaults` then `sandbox_defaults`, stopping at the first failure.
pub fn all_defaults(options: SetupOptions, preset: SetupSandboxPreset) -> Result<()> {
    registry_defaults(options)?;
    println!();
    sandbox_defaults(options, preset, None)?;
    Ok(())
}

fn sandbox_config_path() -> Result<PathBuf> {
    Ok(fimod::paths::config_dir()?.join("sandbox.toml"))
}

fn sandbox_target_path(sandbox_file: Option<&str>) -> Result<PathBuf> {
    match sandbox_file {
        Some("") => bail!("--sandbox-file cannot be empty for setup commands"),
        Some(path) => Ok(PathBuf::from(path)),
        None => sandbox_config_path(),
    }
}

fn read_sandbox_config_or_recommended(path: &Path) -> Result<SandboxConfig> {
    if path.exists() {
        read_sandbox_config(path)
    } else {
        Ok(SandboxConfig::preset(SetupSandboxPreset::Recommended))
    }
}

fn read_sandbox_config(path: &Path) -> Result<SandboxConfig> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read sandbox config: {}", path.display()))?;
    let parsed: SandboxFile = toml::from_str(&content)
        .with_context(|| format!("Failed to parse sandbox TOML: {}", path.display()))?;
    let config = SandboxConfig::from_table(parsed.sandbox.unwrap_or_default());
    config.validate()?;
    Ok(config)
}

fn write_sandbox_config(path: &Path, config: &SandboxConfig) -> Result<()> {
    write_sandbox_config_with_content(path, &render_sandbox_config(config))
}

fn write_sandbox_config_with_content(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
    }
    std::fs::write(path, content)
        .with_context(|| format!("Failed to write sandbox config: {}", path.display()))
}

fn render_sandbox_config(config: &SandboxConfig) -> String {
    format!(
        "[sandbox]\nallow_clock  = {}\nmax_duration = \"{}\"\nmax_memory   = \"{}\"\nallow_env    = {}\n",
        config.allow_clock,
        toml_escape(&config.max_duration),
        toml_escape(&config.max_memory),
        render_toml_string_array(&config.allow_env)
    )
}

fn render_toml_string_array(values: &[String]) -> String {
    if values.is_empty() {
        return "[]".to_string();
    }
    let rendered = values
        .iter()
        .map(|value| format!("\"{}\"", toml_escape(value)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{rendered}]")
}

fn toml_escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            other => vec![other],
        })
        .collect()
}

fn normalize_env_patterns(patterns: Vec<String>) -> Vec<String> {
    patterns
        .into_iter()
        .map(|pattern| pattern.trim().to_string())
        .filter(|pattern| !pattern.is_empty())
        .collect()
}
