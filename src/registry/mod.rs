use anyhow::Result;

pub mod catalog;
pub mod config;
pub mod molds;
pub mod resolve;

pub(crate) use catalog::cache_base_dir;
pub use catalog::{build_catalog, cache_clear, cache_info};
pub use config::{
    add, confirm, list, remove, set_priority, show, Source, SourceType, SourcesConfig,
};
use config::{load_config, save_config};
pub use molds::{
    complete_mold_names, complete_source_names, list_molds, show_mold, show_mold_by_path,
    MoldListFormat, MoldShowFormat,
};
pub(super) use resolve::{env_display_name, parse_env_registries};
pub use resolve::{resolve, token_for_url};

/// Set up the fimod example molds registry interactively.
///
/// Behaviour:
/// - Already present (by URL) → prints a message and exits cleanly.
/// - Fresh install (no default yet) → adds as default, no prompt needed.
/// - Default already set, `--force` absent → adds without overriding default (asks first unless `--yes`).
/// - Default already set, `--force` present → adds and promotes to default (asks first unless `--yes`).
pub fn setup(yes: bool, force: bool) -> Result<()> {
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
            remove(name)?;
        }
        if !to_remove.is_empty() {
            cfg = load_config()?;
        }
    }

    // ── Migrate legacy "official" → "examples" ──
    if let Some(source) = cfg.sources.get("official") {
        if source.url.as_deref() == Some(EXAMPLES_URL) {
            let do_migrate = yes || confirm(
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
        if !confirm("Install?", true)? {
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
            add(&alt, entry.url, None)?;
            set_priority(&alt, Some(entry.priority), false, false)?;
            alt
        } else {
            add(entry.name, entry.url, None)?;
            set_priority(entry.name, Some(entry.priority), false, false)?;
            entry.name.to_string()
        };
        println!("✓ Added {} (P{})", name, entry.priority);
    }

    Ok(())
}
