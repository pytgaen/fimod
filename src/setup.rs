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

/// Canonical sandbox config, kept minimal and conservative by design.
const SANDBOX_RECOMMENDED: &str = r#"# fimod sandbox policy — recommended defaults.
# See `fimod shape --help` and docs for field reference.
[sandbox]
allow_clock  = true
max_duration = "2m"   # same as hard-coded default
max_memory   = "1GB"  # same as hard-coded default
allow_env    = []     # opt in per-key as needed
"#;

/// Install recommended community registries.
///
/// `force` forces fimod to take ownership of `fimod-powered` and `examples`
/// entries: if they already exist (under any name pointing to the canonical
/// URLs), they are removed first so the canonical name + URL + priority is
/// re-applied.
///
/// Behaviour:
/// - Already present (by URL) → prints a message and exits cleanly.
/// - Fresh install (no default yet) → adds as default, no prompt needed.
/// - Default already set, `--force` absent → adds without overriding default (asks first unless `--yes`).
/// - Default already set, `--force` present → adds and promotes to default (asks first unless `--yes`).
pub fn registry_defaults(yes: bool, force: bool) -> Result<()> {
    const EXAMPLES_NAME: &str = "examples";
    const EXAMPLES_URL: &str = "https://github.com/pytgaen/fimod/tree/main/molds";
    const POWERED_NAME: &str = "fimod-powered";
    const POWERED_URL: &str = "https://github.com/pytgaen/fimod-powered/tree/main/molds";

    let mut cfg = load_config()?;

    // --force: take ownership of the canonical URLs by removing any existing
    // entry pointing to them, so they get reinstalled with the canonical
    // name and priority below.
    if force {
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
    if let Some(source) = cfg.sources.get("official") {
        if source.url.as_deref() == Some(EXAMPLES_URL) {
            let do_migrate = yes || registry::confirm(
                "Registry 'official' detected — this name has been renamed to 'examples'. Migrate?",
                true,
            )?;
            if do_migrate {
                let source = cfg.sources.remove("official").unwrap();
                cfg.sources.insert(EXAMPLES_NAME.to_string(), source);
                cfg.priority.remove("official");
                cfg.priority.insert(EXAMPLES_NAME.to_string(), 99);
                save_config(&cfg)?;
                println!("Migrated registry 'official' → 'examples' (P99).");
                cfg = load_config()?;
            }
        }
    }

    // ── Detect which registries are already installed (by URL) ──
    let has_powered = cfg
        .sources
        .values()
        .any(|s| s.url.as_deref() == Some(POWERED_URL));
    let has_examples = cfg
        .sources
        .values()
        .any(|s| s.url.as_deref() == Some(EXAMPLES_URL));

    if has_powered && has_examples {
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
    if !yes {
        println!("\nAdd community registries?");
        for entry in &to_install {
            println!(
                "  • {:<16} ({})    P{}",
                entry.name, entry.label, entry.priority
            );
        }
        println!();
        if !registry::confirm("Install?", true)? {
            println!("Skipped.");
            return Ok(());
        }
    }

    // ── Install each registry ──
    for entry in &to_install {
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
pub fn sandbox_defaults(yes: bool, force: bool) -> Result<()> {
    let path = sandbox_config_path()?;

    if path.exists() && !force {
        bail!(
            "{} already exists — use --force to overwrite",
            path.display()
        );
    }

    if !yes {
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
        if !registry::confirm("Continue?", false)? {
            println!("Skipped.");
            return Ok(());
        }
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
pub fn all_defaults(yes: bool, force: bool) -> Result<()> {
    registry_defaults(yes, force)?;
    println!();
    sandbox_defaults(yes, force)?;
    Ok(())
}

fn sandbox_config_path() -> Result<PathBuf> {
    Ok(crate::paths::config_dir()?.join("sandbox.toml"))
}
