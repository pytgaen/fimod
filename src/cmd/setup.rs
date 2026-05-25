//! Top-level `fimod setup <category> defaults` subcommands.
//!
//! Installs recommended defaults for each category:
//! - `registry`  → community registries (same as legacy `fimod registry setup`).
//! - `sandbox`   → recommended `~/.config/fimod/sandbox.toml`.
//! - `all`       → runs registry then sandbox, failing at the first error.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};

use crate::registry::{
    self,
    config::{load_config, save_config},
};

const EXAMPLES_NAME: &str = "examples";
const EXAMPLES_URL: &str = "https://github.com/pytgaen/fimod/tree/main/molds";
const POWERED_NAME: &str = "fimod-powered";
const POWERED_URL: &str = "https://github.com/pytgaen/fimod-powered/tree/main/molds";

/// Canonical sandbox config, kept minimal and conservative by design.
const SANDBOX_RECOMMENDED: &str = r#"# fimod sandbox policy — recommended defaults.
# See `fimod shape --help` and docs for field reference.
[sandbox]
allow_clock  = true
max_duration = "2m"   # same as hard-coded default
max_memory   = "1GB"  # same as hard-coded default
allow_env    = []     # opt in per-key as needed
"#;

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

/// Write the recommended sandbox policy to `~/.config/fimod/sandbox.toml`.
///
/// - Refuses to overwrite an existing file unless `force` is set.
/// - `yes` skips the confirmation prompt (required in non-TTY contexts).
pub fn sandbox_defaults(options: SetupOptions) -> Result<()> {
    let path = sandbox_config_path()?;

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
            "This will {} {} with the recommended preset:",
            if path.exists() { "overwrite" } else { "create" },
            path.display()
        );
        println!();
        for line in SANDBOX_RECOMMENDED.lines().filter(|l| !l.trim().is_empty()) {
            println!("  {line}");
        }
        println!();
    }

    let default_yes = options.if_needed && !path.exists();
    if !apply_pref(pref, "Continue?", default_yes)? {
        println!("Skipped. Run 'fimod setup sandbox defaults --if-needed' at any time.");
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;
    }
    std::fs::write(&path, SANDBOX_RECOMMENDED)
        .with_context(|| format!("Failed to write sandbox config: {}", path.display()))?;

    println!("✓ Wrote {}", path.display());
    Ok(())
}

/// Run `registry_defaults` then `sandbox_defaults`, stopping at the first failure.
pub fn all_defaults(options: SetupOptions) -> Result<()> {
    registry_defaults(options)?;
    println!();
    sandbox_defaults(options)?;
    Ok(())
}

fn sandbox_config_path() -> Result<PathBuf> {
    Ok(fimod::paths::config_dir()?.join("sandbox.toml"))
}
