//! Top-level `fimod setup <category> defaults` subcommands.
//!
//! Installs recommended defaults for each category:
//! - `registry`  → community registries (same as legacy `fimod registry setup`).
//! - `sandbox`   → recommended `~/.config/fimod/sandbox.toml`.
//! - `all`       → runs registry then sandbox, failing at the first error.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};

use crate::registry;

/// Canonical sandbox config, kept minimal and conservative by design.
const SANDBOX_RECOMMENDED: &str = r#"# fimod sandbox policy — recommended defaults.
# See `fimod shape --help` and docs for field reference.
[sandbox]
allow_clock  = true
max_duration = "2m"   # same as hard-coded default
max_memory   = "1GB"  # same as hard-coded default
allow_env    = []     # opt in per-key as needed
"#;

/// Install recommended community registries. Thin wrapper over `registry::setup`.
pub fn registry_defaults(yes: bool) -> Result<()> {
    registry::setup(yes)
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
    registry_defaults(yes)?;
    println!();
    sandbox_defaults(yes, force)?;
    Ok(())
}

fn sandbox_config_path() -> Result<PathBuf> {
    Ok(crate::paths::config_dir()?.join("sandbox.toml"))
}
