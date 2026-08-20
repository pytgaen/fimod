use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

/// Prompt the user with a yes/no question. Returns `false` in non-interactive contexts.
/// `default_yes` controls the default when the user presses Enter without typing.
pub fn confirm(prompt: &str, default_yes: bool) -> Result<bool> {
    use std::io::{BufRead, IsTerminal, Write};

    fn ask<R: BufRead, W: Write>(
        input: &mut R,
        output: &mut W,
        prompt: &str,
        default_yes: bool,
    ) -> Result<bool> {
        let hint = if default_yes { "[Y/n]" } else { "[y/N]" };
        write!(output, "{prompt} {hint} ")?;
        output.flush()?;
        let mut answer = String::new();
        input.read_line(&mut answer)?;
        let answer = answer.trim().to_lowercase();
        if answer.is_empty() {
            Ok(default_yes)
        } else {
            Ok(answer == "y" || answer == "yes")
        }
    }

    if !std::io::stdin().is_terminal() {
        #[cfg(unix)]
        {
            if let Ok(tty) = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open("/dev/tty")
            {
                let mut output = tty.try_clone()?;
                let mut input = std::io::BufReader::new(tty);
                return ask(&mut input, &mut output, prompt, default_yes);
            }
        }
        return Ok(false);
    }

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();
    ask(&mut input, &mut output, prompt, default_yes)
}

// ── config path ───────────────────────────────────────────────────────────────

pub(super) fn config_path() -> Result<PathBuf> {
    Ok(crate::paths::config_dir()?.join("sources.toml"))
}

// ── data model ────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    Local,
    Github,
    Gitlab,
    Http,
}

impl fmt::Display for SourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local => write!(f, "local"),
            Self::Github => write!(f, "github"),
            Self::Gitlab => write!(f, "gitlab"),
            Self::Http => write!(f, "http"),
        }
    }
}

impl SourceType {
    pub(super) fn detect_from_url(url: &str) -> Self {
        if url.contains("github.com") {
            Self::Github
        } else if url.contains("gitlab") {
            Self::Gitlab
        } else {
            Self::Http
        }
    }

    pub(super) fn default_token_env(&self) -> Option<&'static str> {
        match self {
            Self::Github => Some("GITHUB_TOKEN"),
            Self::Gitlab => Some("GITLAB_TOKEN"),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Source {
    #[serde(rename = "type")]
    pub kind: SourceType,
    /// Absolute filesystem path (local sources only).
    pub path: Option<String>,
    /// Base URL (remote sources only).
    pub url: Option<String>,
    /// Override the default env var used for authentication.
    pub token_env: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct SourcesConfig {
    /// Legacy field — migrated to `priority[name] = 0` on load. Kept for deserialization compat.
    #[serde(default, skip_serializing)]
    default: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub priority: BTreeMap<String, u32>,
    #[serde(default)]
    pub sources: BTreeMap<String, Source>,
}

// ── persistence ───────────────────────────────────────────────────────────────

pub fn load_config() -> Result<SourcesConfig> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(SourcesConfig::default());
    }
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read registry: {}", path.display()))?;
    let mut cfg: SourcesConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse registry: {}", path.display()))?;

    // Migrate legacy `default = "name"` → `priority.name = 0`
    if let Some(name) = cfg.default.take() {
        if cfg.sources.contains_key(&name) && !cfg.priority.contains_key(&name) {
            cfg.priority.insert(name, 0);
        }
        save_config(&cfg)?;
    }

    Ok(cfg)
}

pub fn save_config(cfg: &SourcesConfig) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }
    let content = toml::to_string_pretty(cfg).context("Failed to serialize registry")?;
    fs::write(&path, content)
        .with_context(|| format!("Failed to write registry: {}", path.display()))?;
    Ok(())
}

/// Compute the effective priority for each source in the config.
///
/// Returns `(name, source, priority_rank)` sorted by rank.
/// - `default` is always P0 (if set and exists in sources).
/// - Entries in `[priority]` get their assigned rank.
/// - Remaining sources come after all prioritized ones, in file order.
pub(super) fn ordered_sources(cfg: &SourcesConfig) -> Vec<(&str, &Source, Option<u32>)> {
    let mut ranked: Vec<(&str, &Source, Option<u32>)> = Vec::new();

    for (name, source) in &cfg.sources {
        let rank = cfg.priority.get(name).copied();
        ranked.push((name.as_str(), source, rank));
    }

    // Sort: ranked entries first (by rank), then unranked (preserve insertion order)
    ranked.sort_by(|a, b| match (a.2, b.2) {
        (Some(ra), Some(rb)) => ra.cmp(&rb),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });

    ranked
}

/// Format a priority label for display.
pub(super) fn priority_label(cfg: &SourcesConfig, name: &str) -> String {
    if let Some(&p) = cfg.priority.get(name) {
        format!("P{p}")
    } else {
        String::new()
    }
}

// ── registry commands ─────────────────────────────────────────────────────────

/// Add a named source (local directory or remote URL).
pub fn add(name: &str, location: &str, token_env: Option<&str>) -> Result<()> {
    let mut cfg = load_config()?;

    if cfg.sources.contains_key(name) {
        bail!("Registry '{name}' already exists (use 'fimod registry remove {name}' first)");
    }

    let source = if crate::http::is_url(location) {
        let kind = SourceType::detect_from_url(location);
        if let Some((existing_name, _)) = cfg
            .sources
            .iter()
            .find(|(_, s)| s.url.as_deref() == Some(location))
        {
            bail!(
                "URL already registered as '{existing_name}' (use 'fimod registry remove {existing_name}' first)"
            );
        }
        Source {
            kind,
            path: None,
            url: Some(location.to_string()),
            token_env: token_env.map(|s| s.to_string()),
        }
    } else {
        let abs =
            fs::canonicalize(location).with_context(|| format!("Path not found: {location}"))?;
        if !abs.is_dir() {
            bail!("Local registry must be a directory: {}", abs.display());
        }
        let abs_str = abs.to_string_lossy();
        if let Some((existing_name, _)) = cfg
            .sources
            .iter()
            .find(|(_, s)| s.path.as_deref() == Some(abs_str.as_ref()))
        {
            bail!(
                "Path already registered as '{existing_name}' (use 'fimod registry remove {existing_name}' first)"
            );
        }
        Source {
            kind: SourceType::Local,
            path: Some(abs_str.into_owned()),
            url: None,
            token_env: None,
        }
    };

    let location_display = source
        .path
        .clone()
        .or_else(|| source.url.clone())
        .unwrap_or_else(|| location.to_string());
    cfg.sources.insert(name.to_string(), source);

    save_config(&cfg)?;
    println!("Added registry '{name}' → {location_display}");
    Ok(())
}

/// Remove a named source.
pub fn remove(name: &str) -> Result<()> {
    let mut cfg = load_config()?;
    if cfg.sources.remove(name).is_none() {
        bail!("Registry '{name}' not found");
    }
    cfg.priority.remove(name);
    save_config(&cfg)?;
    println!("Removed registry '{name}'");
    Ok(())
}

/// List all registered sources.
pub fn list(output_format: &str) -> Result<()> {
    let cfg = load_config()?;
    let env_entries = super::resolve::parse_env_registries();

    if output_format == "json" {
        #[derive(Serialize)]
        struct RegistryInfo<'a> {
            name: &'a str,
            kind: &'a SourceType,
            location: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            priority: Option<u32>,
            #[serde(skip_serializing_if = "std::ops::Not::not")]
            from_env: bool,
        }
        let mut entries: Vec<RegistryInfo> = ordered_sources(&cfg)
            .iter()
            .map(|(name, source, rank)| RegistryInfo {
                name,
                kind: &source.kind,
                location: source
                    .path
                    .as_deref()
                    .or(source.url.as_deref())
                    .unwrap_or("?"),
                priority: *rank,
                from_env: false,
            })
            .collect();
        let mut anon_index = 0;
        for entry in &env_entries {
            let display_name = super::resolve::env_display_name(entry, &mut anon_index);
            let name_ref: &str = Box::leak(display_name.into_boxed_str());
            entries.push(RegistryInfo {
                name: name_ref,
                kind: &entry.source.kind,
                location: entry
                    .source
                    .path
                    .as_deref()
                    .or(entry.source.url.as_deref())
                    .unwrap_or("?"),
                priority: Some(0),
                from_env: true,
            });
        }
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    if cfg.sources.is_empty() && env_entries.is_empty() {
        println!("No registries configured.");
        println!("Use 'fimod registry add <name> <path-or-url>' to add one.");
        return Ok(());
    }
    // Show FIMOD_REGISTRY entries first (they are always P0)
    let mut anon_index = 0;
    for entry in &env_entries {
        let display_name = super::resolve::env_display_name(entry, &mut anon_index);
        let marker = if display_name == "env-default" {
            "P0 (FIMOD_REGISTRY)"
        } else {
            "(FIMOD_REGISTRY)"
        };
        let location = entry
            .source
            .path
            .as_deref()
            .or(entry.source.url.as_deref())
            .unwrap_or("?");
        println!(
            "{:20} [{:6}] {:44} {}",
            display_name, entry.source.kind, location, marker
        );
    }
    // Then sources.toml entries in priority order
    for (name, source, _) in ordered_sources(&cfg) {
        let label = priority_label(&cfg, name);
        let location = source
            .path
            .as_deref()
            .or(source.url.as_deref())
            .unwrap_or("?");
        println!("{:20} [{:6}] {:44} {}", name, source.kind, location, label);
    }
    Ok(())
}

/// Show details of a named source.
pub fn show(name: &str) -> Result<()> {
    let cfg = load_config()?;
    let source = cfg
        .sources
        .get(name)
        .ok_or_else(|| anyhow::anyhow!("Registry '{name}' not found"))?;
    println!("Name:    {name}");
    println!("Type:    {}", source.kind);
    if let Some(p) = &source.path {
        println!("Path:    {p}");
        println!(
            "Exists:  {}",
            if Path::new(p).is_dir() { "yes" } else { "no" }
        );
    }
    if let Some(u) = &source.url {
        println!("URL:     {u}");
    }
    if let Some(e) = &source.token_env {
        println!("Token:   ${e}");
    } else if let Some(default_env) = source.kind.default_token_env() {
        let set = std::env::var(default_env).is_ok();
        println!(
            "Token:   ${} (auto) — {}",
            default_env,
            if set { "set" } else { "not set" }
        );
    }
    let label = priority_label(&cfg, name);
    if !label.is_empty() {
        println!("Priority: {label}");
    }
    Ok(())
}

pub fn set_priority(name: &str, rank: Option<u32>, clear: bool, cascade: bool) -> Result<()> {
    let mut cfg = load_config()?;
    if !cfg.sources.contains_key(name) {
        bail!("Registry '{name}' not found");
    }
    if clear {
        if cfg.priority.remove(name).is_some() {
            save_config(&cfg)?;
            println!("Cleared priority for '{name}'");
        } else {
            println!("'{name}' has no priority set");
        }
        return Ok(());
    }
    let Some(rank) = rank else {
        bail!("Provide a priority rank, or use --clear to unset");
    };
    let old_rank = cfg.priority.get(name).copied();
    let use_cascade = cascade || old_rank.is_none();

    if use_cascade {
        // Cascade: shift existing entries at the requested rank or above
        let mut priorities: Vec<(String, u32)> = cfg
            .priority
            .iter()
            .filter(|(n, _)| n.as_str() != name)
            .map(|(n, &r)| (n.clone(), r))
            .collect();
        priorities.sort_by_key(|&(_, r)| r);

        let mut next_rank = rank;
        for entry in &mut priorities {
            if entry.1 == next_rank {
                entry.1 = next_rank + 1;
                next_rank += 1;
            }
        }

        cfg.priority.clear();
        for (n, r) in priorities {
            cfg.priority.insert(n, r);
        }
    } else {
        // Swap: exchange ranks with the occupant (if any)
        let occupant = cfg
            .priority
            .iter()
            .find(|(n, &r)| r == rank && n.as_str() != name)
            .map(|(n, _)| n.clone());
        if let Some(occupant) = occupant {
            if let Some(old) = old_rank {
                cfg.priority.insert(occupant, old);
            } else {
                cfg.priority.remove(&occupant);
            }
        }
    }

    cfg.priority.insert(name.to_string(), rank);

    save_config(&cfg)?;
    println!("Set '{name}' to P{rank}");
    Ok(())
}
