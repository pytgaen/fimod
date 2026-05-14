use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::ValueEnum;

pub mod catalog;
pub mod config;
pub mod resolve;

pub(crate) use catalog::cache_base_dir;
pub use catalog::{build_catalog, cache_clear, cache_info};
use catalog::{
    effective_description, fetch_catalog, fetch_script_docs, scan_local_molds, CatalogEntry,
};
pub use config::{
    add, confirm, list, remove, set_priority, show, Source, SourceType, SourcesConfig,
};
use config::{load_config, ordered_sources, priority_label, save_config};
use resolve::select_sources;
pub(super) use resolve::{env_display_name, parse_env_registries};
pub use resolve::{resolve, token_for_url};

// ── mold commands ─────────────────────────────────────────────────────────────

/// Print molds for a single registry (name + source already resolved).
fn print_registry_molds(name: &str, source: &Source, prio_label: &str) -> Result<()> {
    let marker = if prio_label.is_empty() {
        String::new()
    } else {
        format!(" {prio_label}")
    };
    println!("{} [{}]{}", name, source.kind, marker);

    match &source.kind {
        SourceType::Local => {
            let base = source
                .path
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("Local registry '{name}' has no path configured"))?;
            let molds = scan_local_molds(Path::new(base));
            if molds.is_empty() {
                println!("  (no molds found)");
            } else {
                for (mold_name, desc, _rel) in &molds {
                    println!("  {:<20} {}", mold_name, desc.as_deref().unwrap_or(""));
                }
            }
        }
        SourceType::Github | SourceType::Gitlab | SourceType::Http => {
            let catalog = fetch_catalog(source, false)?.ok_or_else(|| {
                anyhow::anyhow!(
                    "Failed to fetch catalog for registry '{name}'. \
                         Hint: push a catalog.toml generated with \
                         'fimod registry build-catalog'."
                )
            })?;
            if catalog.molds.is_empty() {
                println!("  (no molds in catalog)");
            } else {
                for (mold_name, entry) in &catalog.molds {
                    println!(
                        "  {:<20} {}",
                        mold_name,
                        entry.description.as_deref().unwrap_or("")
                    );
                }
            }
        }
    }

    Ok(())
}

// ── shell completion helpers ──────────────────────────────────────────────────

/// List source names matching `prefix` for shell completion.
pub fn complete_source_names(prefix: &str) -> Vec<String> {
    let Ok(cfg) = load_config() else {
        return Vec::new();
    };
    cfg.sources
        .keys()
        .filter(|name| name.starts_with(prefix))
        .cloned()
        .collect()
}

/// List mold `@name` and `@source/name` references matching `prefix` for shell completion.
///
/// Returns `(completion, description)` pairs. Silently returns empty on errors.
pub fn complete_mold_names(prefix: &str) -> Vec<(String, Option<String>)> {
    let Ok(cfg) = load_config() else {
        return Vec::new();
    };
    let Ok(entries) = collect_all_molds(&cfg, None) else {
        return Vec::new();
    };

    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();

    for (reg_name, _prio, mold_name, desc) in &entries {
        // @source/name form
        let qualified = format!("@{reg_name}/{mold_name}");
        if qualified.starts_with(prefix) && seen.insert(qualified.clone()) {
            result.push((qualified, desc.clone()));
        }
        // @name bare form
        let bare = format!("@{mold_name}");
        if bare.starts_with(prefix) && seen.insert(bare.clone()) {
            result.push((bare, desc.clone()));
        }
    }

    result
}

/// `(registry_name, priority_label, mold_name, description)` entry from [`collect_all_molds`].
type MoldEntry = (String, String, String, Option<String>);

/// Collect all molds from the configured registries into a flat list.
///
/// Returns `(registry_name, priority_label, mold_name, description)` tuples.
fn collect_all_molds(cfg: &SourcesConfig, registry_name: Option<&str>) -> Result<Vec<MoldEntry>> {
    let sources = select_sources(cfg, registry_name)?;
    let env_entries = parse_env_registries();

    let mut result = Vec::new();

    for (reg_name, source) in sources {
        let label = priority_label(cfg, reg_name);
        collect_molds_from_source(reg_name, source, &label, &mut result);
    }

    // Include FIMOD_REGISTRY entries (only when listing all, not a specific registry)
    if registry_name.is_none() {
        let mut anon_index = 0;
        for entry in &env_entries {
            let display_name = env_display_name(entry, &mut anon_index);
            collect_molds_from_source(&display_name, &entry.source, "", &mut result);
        }
    }

    Ok(result)
}

/// Collect molds from a single source into the result vector.
fn collect_molds_from_source(
    reg_name: &str,
    source: &Source,
    prio_label: &str,
    result: &mut Vec<MoldEntry>,
) {
    match &source.kind {
        SourceType::Local => {
            let Some(base) = source.path.as_deref() else {
                return;
            };
            for (mold_name, desc, _rel) in scan_local_molds(Path::new(base)) {
                result.push((
                    reg_name.to_string(),
                    prio_label.to_string(),
                    mold_name,
                    desc,
                ));
            }
        }
        SourceType::Github | SourceType::Gitlab | SourceType::Http => {
            let Ok(Some(catalog)) = fetch_catalog(source, false) else {
                return;
            };
            for (mold_name, entry) in catalog.molds {
                result.push((
                    reg_name.to_string(),
                    prio_label.to_string(),
                    mold_name,
                    entry.description,
                ));
            }
        }
    }
}

/// Output format for `fimod mold list`.
#[derive(ValueEnum, Clone, Debug, Default)]
pub enum MoldListFormat {
    /// Human-readable table (default)
    #[default]
    Text,
    /// JSON array of objects
    Json,
    /// Tab-delimited `@registry/name\tdescription` lines (for scripting)
    Lines,
}

#[derive(ValueEnum, Clone, Debug, Default)]
pub enum MoldShowFormat {
    /// Human-readable text (default)
    #[default]
    Text,
    /// JSON object
    Json,
}

/// List molds available in a registry (local scan or remote catalog.toml).
/// Without a registry name, lists all configured registries.
pub fn list_molds(registry_name: Option<&str>, output_format: MoldListFormat) -> Result<()> {
    let cfg = load_config()?;

    match output_format {
        MoldListFormat::Json => {
            let molds = collect_all_molds(&cfg, registry_name)?;
            let arr: Vec<serde_json::Value> = molds
                .into_iter()
                .map(|(reg, prio, name, desc)| {
                    serde_json::json!({
                        "name": name,
                        "registry": reg,
                        "priority": prio,
                        "description": desc.unwrap_or_default(),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&arr)?);
        }
        MoldListFormat::Lines => {
            let molds = collect_all_molds(&cfg, registry_name)?;
            for (reg, _prio, name, desc) in molds {
                println!("@{reg}/{name}\t{}", desc.unwrap_or_default());
            }
        }
        MoldListFormat::Text => {
            // text format — existing human-readable output
            let env_entries = parse_env_registries();
            if cfg.sources.is_empty() && env_entries.is_empty() {
                println!("No registries configured. Use 'fimod registry add' to add one.");
                return Ok(());
            }
            if let Some(name) = registry_name {
                let source = cfg.sources.get(name).ok_or_else(|| {
                    anyhow::anyhow!(
                        "Registry '{name}' not found. Use 'fimod registry list' to see configured registries."
                    )
                })?;
                let label = priority_label(&cfg, name);
                print_registry_molds(name, source, &label)?;
            } else {
                let mut first = true;
                for (name, source, _) in ordered_sources(&cfg) {
                    if !first {
                        println!();
                    }
                    first = false;
                    let label = priority_label(&cfg, name);
                    print_registry_molds(name, source, &label)?;
                }
                let mut anon_index = 0;
                for entry in &env_entries {
                    if !first {
                        println!();
                    }
                    first = false;
                    let base_name = env_display_name(entry, &mut anon_index);
                    let marker = if base_name == "env-default" {
                        "P0 (FIMOD_REGISTRY)"
                    } else {
                        "(FIMOD_REGISTRY)"
                    };
                    let display_name = format!("{base_name} {marker}");
                    print_registry_molds(&display_name, &entry.source, "")?;
                }
            }
        }
    }

    Ok(())
}

/// Find the script path for a named mold in a local registry directory.
fn find_local_mold_script(base: &Path, name: &str) -> Option<PathBuf> {
    let flat = base.join(format!("{name}.py"));
    if flat.is_file() {
        return Some(flat);
    }
    let named = base.join(name).join(format!("{name}.py"));
    if named.is_file() {
        return Some(named);
    }
    let main = base.join(name).join("__main__.py");
    if main.is_file() {
        return Some(main);
    }
    None
}

/// Format non-default MoldDefaults fields as a human-readable list of strings.
pub(super) fn format_defaults_options(d: &crate::mold::MoldDefaults) -> Vec<String> {
    let mut opts = Vec::new();
    if d.no_follow {
        opts.push("no-follow".to_string());
    }
    if let Some(delim) = &d.csv_delimiter {
        opts.push(format!("csv-delimiter={delim}"));
    }
    if let Some(delim) = &d.csv_output_delimiter {
        opts.push(format!("csv-output-delimiter={delim}"));
    }
    if d.csv_no_input_header {
        opts.push("csv-no-input-header".to_string());
    }
    if d.csv_no_output_header {
        opts.push("csv-no-output-header".to_string());
    }
    if let Some(hdr) = &d.csv_header {
        opts.push(format!("csv-header={hdr}"));
    }
    opts
}

enum MoldDetail {
    Local {
        script_path: PathBuf,
        defaults: crate::mold::MoldDefaults,
    },
    Remote {
        registry_url: String,
        entry: CatalogEntry,
    },
}

struct MoldMatch {
    reg_name: Option<String>,
    prio_label: String,
    detail: MoldDetail,
}

fn collect_mold_matches(
    cfg: &SourcesConfig,
    mold_name: &str,
    registry_name: Option<&str>,
) -> Result<Vec<MoldMatch>> {
    let sources = select_sources(cfg, registry_name)?;

    let mut matches: Vec<MoldMatch> = Vec::new();

    for (reg_name, source) in sources {
        let label = priority_label(cfg, reg_name);
        match &source.kind {
            SourceType::Local => {
                let base = source.path.as_deref().ok_or_else(|| {
                    anyhow::anyhow!("Local registry '{reg_name}' has no path configured")
                })?;
                let Some(script_path) = find_local_mold_script(Path::new(base), mold_name) else {
                    continue;
                };
                let script = fs::read_to_string(&script_path)
                    .with_context(|| format!("Cannot read {script_path:?}"))?;
                let defaults = crate::mold::parse_mold_defaults(&script);
                matches.push(MoldMatch {
                    reg_name: Some(reg_name.to_string()),
                    prio_label: label,
                    detail: MoldDetail::Local {
                        script_path,
                        defaults,
                    },
                });
            }
            SourceType::Github | SourceType::Gitlab | SourceType::Http => {
                let Ok(Some(catalog)) = fetch_catalog(source, false) else {
                    continue;
                };
                let Some(entry) = catalog.molds.get(mold_name).cloned() else {
                    continue;
                };
                matches.push(MoldMatch {
                    reg_name: Some(reg_name.to_string()),
                    prio_label: label,
                    detail: MoldDetail::Remote {
                        registry_url: source.url.clone().unwrap_or_default(),
                        entry,
                    },
                });
            }
        }
    }

    Ok(matches)
}

fn print_mold_match(mold_name: &str, m: &MoldMatch) {
    let marker = if m.prio_label.is_empty() {
        String::new()
    } else {
        format!(" {}", m.prio_label)
    };
    match &m.reg_name {
        Some(name) => println!("{mold_name}  [{name}]{marker}"),
        None => println!("{mold_name}"),
    }
    match &m.detail {
        MoldDetail::Local {
            script_path,
            defaults,
        } => {
            if defaults.docs.is_none() {
                if let Some(desc) = effective_description(defaults) {
                    println!("  Description:    {desc}");
                }
            }
            if let Some(docs) = &defaults.docs {
                for line in docs.lines() {
                    println!("  {line}");
                }
                println!();
            }
            println!("  Source:         {}", script_path.display());
            println!();
            if let Some(fmt) = &defaults.input_format {
                println!("  Input format:   {fmt}");
            }
            if let Some(fmt) = &defaults.output_format {
                println!("  Output format:  {fmt}");
            }
            let opts = format_defaults_options(defaults);
            if !opts.is_empty() {
                println!("  Options:        {}", opts.join(", "));
            }
            if !defaults.args.is_empty() {
                println!("  Args:");
                for (name, desc) in &defaults.args {
                    match desc {
                        Some(d) => println!("    {name:<16}  {d}"),
                        None => println!("    {name}"),
                    }
                }
            }
            if !defaults.envs.is_empty() {
                println!("  Environment:");
                for (name, desc) in &defaults.envs {
                    match desc {
                        Some(d) => println!("    {name:<16}  {d}"),
                        None => println!("    {name}"),
                    }
                }
            }
            if let Some(parent) = script_path.parent() {
                let readme = parent.join("README.md");
                if readme.exists() {
                    println!();
                    println!("  Readme:         {}", readme.display());
                }
            }
        }
        MoldDetail::Remote {
            registry_url,
            entry,
        } => {
            // Fetch the script to extract the full docstring (not stored in catalog)
            let remote_docs = entry.path.as_deref().and_then(|rel| {
                let url = format!("{}/{rel}", registry_url.trim_end_matches('/'));
                fetch_script_docs(&url)
            });
            if let Some(docs) = &remote_docs {
                for line in docs.lines() {
                    println!("  {line}");
                }
                println!();
            } else if let Some(desc) = &entry.description {
                println!("  Description:    {desc}");
            }
            if let Some(fmt) = &entry.input_format {
                println!("  Input format:   {fmt}");
            }
            if let Some(fmt) = &entry.output_format {
                println!("  Output format:  {fmt}");
            }
            if !entry.options.is_empty() {
                println!("  Options:        {}", entry.options.join(", "));
            }
            if !entry.args.is_empty() {
                println!("  Args:");
                for (name, desc) in &entry.args {
                    if desc.is_empty() {
                        println!("    {name}");
                    } else {
                        println!("    {name:<16}  {desc}");
                    }
                }
            }
            if !entry.envs.is_empty() {
                println!("  Environment:");
                for (name, desc) in &entry.envs {
                    if desc.is_empty() {
                        println!("    {name}");
                    } else {
                        println!("    {name:<16}  {desc}");
                    }
                }
            }
            println!();
            println!("  Registry:       {registry_url}");
            if let Some(readme) = &entry.readme {
                let base = registry_url.trim_end_matches('/');
                println!("  Readme:         {base}/{readme}");
            }
        }
    }
}

/// Show metadata and defaults for a named mold.
///
/// `mold_ref` supports `registry/name` syntax to target a specific registry.
pub fn show_mold(
    mold_ref: &str,
    registry_name: Option<&str>,
    output_format: MoldShowFormat,
) -> Result<()> {
    let cfg = load_config()?;

    // Parse "registry/name" or "@registry/name" syntax (strip leading @)
    let mold_ref = mold_ref.trim_start_matches('@');
    let (resolved_registry, mold_name) = if let Some(slash) = mold_ref.find('/') {
        (Some(&mold_ref[..slash]), &mold_ref[slash + 1..])
    } else {
        (registry_name, mold_ref)
    };

    let matches = collect_mold_matches(&cfg, mold_name, resolved_registry)?;

    if matches.is_empty() {
        if let Some(name) = resolved_registry {
            bail!("Mold '{mold_name}' not found in registry '{name}'.");
        }
        bail!("Mold '{mold_name}' not found in any configured registry.");
    }

    let m = &matches[0];

    match output_format {
        MoldShowFormat::Json => {
            let json = mold_match_to_json(mold_name, m);
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        MoldShowFormat::Text => {
            // When a specific registry was requested (or only one match), show it directly
            let explicit = resolved_registry.is_some();
            print_mold_match(mold_name, m);
            if !explicit && matches.len() > 1 {
                let others: Vec<String> = matches[1..]
                    .iter()
                    .map(|m| {
                        format!(
                            "fimod mold show {}/{mold_name}",
                            m.reg_name.as_deref().unwrap_or("")
                        )
                    })
                    .collect();
                println!();
                println!("  See also:       {}", others.join(", "));
            }
        }
    }
    Ok(())
}

pub fn show_mold_by_path(
    path: &Path,
    name_override: Option<&str>,
    output_format: MoldShowFormat,
) -> Result<()> {
    let script_path = path
        .canonicalize()
        .with_context(|| format!("Cannot resolve path: {}", path.display()))?;

    let script = fs::read_to_string(&script_path)
        .with_context(|| format!("Cannot read {}", script_path.display()))?;

    let defaults = crate::mold::parse_mold_defaults(&script);

    let mold_name = name_override.map(str::to_string).unwrap_or_else(|| {
        let stem = script_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if stem == "__main__" {
            script_path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or(stem)
                .to_string()
        } else {
            stem.to_string()
        }
    });

    let m = MoldMatch {
        reg_name: None,
        prio_label: String::new(),
        detail: MoldDetail::Local {
            script_path,
            defaults,
        },
    };

    match output_format {
        MoldShowFormat::Json => {
            let json = mold_match_to_json(&mold_name, &m);
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        MoldShowFormat::Text => {
            print_mold_match(&mold_name, &m);
        }
    }
    Ok(())
}

fn mold_match_to_json(mold_name: &str, m: &MoldMatch) -> serde_json::Value {
    match &m.detail {
        MoldDetail::Local {
            script_path,
            defaults,
        } => {
            let description = defaults
                .docs
                .clone()
                .or_else(|| effective_description(defaults));
            let readme_path = script_path
                .parent()
                .map(|p| p.join("README.md"))
                .filter(|p| p.exists())
                .map(|p| p.display().to_string());
            let args: Vec<serde_json::Value> = defaults.args.iter().map(|(name, desc)| {
                serde_json::json!({ "name": name, "description": desc.as_deref().unwrap_or("") })
            }).collect();
            serde_json::json!({
                "name": mold_name,
                "registry": m.reg_name,
                "description": description,
                "source_path": script_path.display().to_string(),
                "readme_path": readme_path,
                "input_format": defaults.input_format,
                "output_format": defaults.output_format,
                "args": args,
            })
        }
        MoldDetail::Remote {
            registry_url,
            entry,
        } => {
            let readme_path = entry
                .readme
                .as_ref()
                .map(|r| format!("{}/{r}", registry_url.trim_end_matches('/')));
            let description = entry.docs.clone().or_else(|| entry.description.clone());
            let args: Vec<serde_json::Value> = entry
                .args
                .iter()
                .map(|(name, desc)| serde_json::json!({ "name": name, "description": desc }))
                .collect();
            serde_json::json!({
                "name": mold_name,
                "registry": m.reg_name,
                "description": description,
                "source_path": null,
                "readme_path": readme_path,
                "input_format": entry.input_format,
                "output_format": entry.output_format,
                "args": args,
            })
        }
    }
}

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
