use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::mold::MoldSource;

// ── config path ───────────────────────────────────────────────────────────────

fn config_path() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .context("HOME environment variable not set")?;
    Ok(Path::new(&home)
        .join(".config")
        .join("fimod")
        .join("sources.toml"))
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
    fn detect_from_url(url: &str) -> Self {
        if url.contains("github.com") {
            Self::Github
        } else if url.contains("gitlab") {
            Self::Gitlab
        } else {
            Self::Http
        }
    }

    fn default_token_env(&self) -> Option<&'static str> {
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
    pub default: Option<String>,
    #[serde(default)]
    pub sources: BTreeMap<String, Source>,
}

// ── persistence ───────────────────────────────────────────────────────────────

fn load_config() -> Result<SourcesConfig> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(SourcesConfig::default());
    }
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read registry: {}", path.display()))?;
    toml::from_str(&content)
        .with_context(|| format!("Failed to parse registry: {}", path.display()))
}

fn save_config(cfg: &SourcesConfig) -> Result<()> {
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

// ── registry commands ─────────────────────────────────────────────────────────

/// Add a named source (local directory or remote URL).
pub fn add(name: &str, location: &str, token_env: Option<&str>, set_default: bool) -> Result<()> {
    let mut cfg = load_config()?;

    if cfg.sources.contains_key(name) {
        bail!("Registry '{name}' already exists (use 'fimod registry remove {name}' first)");
    }

    let source = if location.starts_with("http://") || location.starts_with("https://") {
        let kind = SourceType::detect_from_url(location);
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
        Source {
            kind: SourceType::Local,
            path: Some(abs.to_string_lossy().into_owned()),
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

    let is_first = cfg.default.is_none();
    if set_default || is_first {
        cfg.default = Some(name.to_string());
    }

    save_config(&cfg)?;
    println!("Added registry '{}' → {}", name, &location_display);
    if set_default || is_first {
        println!("Set '{name}' as default registry");
    }
    Ok(())
}

/// Remove a named source.
pub fn remove(name: &str) -> Result<()> {
    let mut cfg = load_config()?;
    if cfg.sources.remove(name).is_none() {
        bail!("Registry '{name}' not found");
    }
    if cfg.default.as_deref() == Some(name) {
        cfg.default = None;
        eprintln!("warning: removed the default registry; use 'fimod registry set-default <name>'");
    }
    save_config(&cfg)?;
    println!("Removed registry '{name}'");
    Ok(())
}

/// List all registered sources.
pub fn list(output_format: &str) -> Result<()> {
    let cfg = load_config()?;
    let env_entries = parse_env_registries();

    if output_format == "json" {
        #[derive(Serialize)]
        struct RegistryInfo<'a> {
            name: &'a str,
            kind: &'a SourceType,
            location: &'a str,
            default: bool,
            #[serde(skip_serializing_if = "std::ops::Not::not")]
            from_env: bool,
        }
        let mut entries: Vec<RegistryInfo> = cfg
            .sources
            .iter()
            .map(|(name, source)| RegistryInfo {
                name,
                kind: &source.kind,
                location: source
                    .path
                    .as_deref()
                    .or(source.url.as_deref())
                    .unwrap_or("?"),
                default: cfg.default.as_deref() == Some(name),
                from_env: false,
            })
            .collect();
        let mut anon_index = 0;
        for entry in &env_entries {
            let display_name = env_display_name(entry, &mut anon_index);
            // Leak the string so we can store &str in the struct.
            // This is fine — list() runs once then exits.
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
                default: false,
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
    let has_env_anon = env_has_anonymous(&env_entries);
    for (name, source) in &cfg.sources {
        let default_marker = if cfg.default.as_deref() == Some(name) {
            if has_env_anon {
                " (fallback default)"
            } else {
                " (default)"
            }
        } else {
            ""
        };
        let location = source
            .path
            .as_deref()
            .or(source.url.as_deref())
            .unwrap_or("?");
        println!(
            "{:20} [{:6}] {}{}",
            name, source.kind, location, default_marker
        );
    }
    let mut anon_index = 0;
    for entry in &env_entries {
        let display_name = env_display_name(entry, &mut anon_index);
        let marker = if display_name == "env-default" {
            "(default FIMOD_REGISTRY)"
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
            "{:20} [{:6}] {} {}",
            display_name, entry.source.kind, location, marker
        );
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
    if cfg.default.as_deref() == Some(name) {
        println!("Default: yes");
    }
    Ok(())
}

/// Set which source is used when no registry prefix is given.
pub fn set_default(name: &str) -> Result<()> {
    let mut cfg = load_config()?;
    if !cfg.sources.contains_key(name) {
        bail!("Registry '{name}' not found");
    }
    cfg.default = Some(name.to_string());
    save_config(&cfg)?;
    println!("Set '{name}' as default registry");
    Ok(())
}

// ── FIMOD_REGISTRY env var ────────────────────────────────────────────────────

/// A parsed FIMOD_REGISTRY entry: either named (`ci=/path`) or anonymous (`/path`).
struct EnvRegistry {
    name: Option<String>,
    source: Source,
}

/// Build a Source from a location string (path or URL).
fn source_from_location(location: &str) -> Source {
    if location.starts_with("http://") || location.starts_with("https://") {
        let kind = SourceType::detect_from_url(location);
        Source {
            kind,
            path: None,
            url: Some(location.to_string()),
            token_env: None,
        }
    } else {
        Source {
            kind: SourceType::Local,
            path: Some(location.to_string()),
            url: None,
            token_env: None,
        }
    }
}

/// Check if a string is a valid registry name (`[a-zA-Z0-9_-]+`).
fn is_registry_name(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Parse the `FIMOD_REGISTRY` environment variable into a list of entries.
///
/// The variable is comma-separated. Each entry can be:
/// - `name=/path` or `name=https://...` → named registry (supports `@name/mold`)
/// - `/path` or `https://...`           → anonymous registry (bare `@mold` only)
///
/// A named entry is detected when the part before the first `=` is a simple
/// identifier (`[a-zA-Z0-9_-]+`). Otherwise the whole string is the location.
fn parse_env_registries() -> Vec<EnvRegistry> {
    let Ok(val) = std::env::var("FIMOD_REGISTRY") else {
        return Vec::new();
    };
    val.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|entry| {
            if let Some(eq_pos) = entry.find('=') {
                let left = &entry[..eq_pos];
                if is_registry_name(left) {
                    let location = &entry[eq_pos + 1..];
                    return EnvRegistry {
                        name: Some(left.to_string()),
                        source: source_from_location(location),
                    };
                }
            }
            EnvRegistry {
                name: None,
                source: source_from_location(entry),
            }
        })
        .collect()
}

/// Generate a display name for an anonymous FIMOD_REGISTRY entry.
///
/// The first anonymous entry is `env-default` (it has priority for bare `@mold`),
/// subsequent ones are `env-1`, `env-2`, etc.
fn env_anonymous_name(anon_index: usize) -> String {
    if anon_index == 0 {
        "env-default".to_string()
    } else {
        format!("env-{anon_index}")
    }
}

/// Generate a display name for a FIMOD_REGISTRY entry.
fn env_display_name(entry: &EnvRegistry, anon_index: &mut usize) -> String {
    match &entry.name {
        Some(n) => n.clone(),
        None => {
            let name = env_anonymous_name(*anon_index);
            *anon_index += 1;
            name
        }
    }
}

/// Returns true if FIMOD_REGISTRY contains at least one anonymous entry
/// (which takes priority over the sources.toml default for bare `@mold`).
fn env_has_anonymous(entries: &[EnvRegistry]) -> bool {
    entries.iter().any(|e| e.name.is_none())
}

// ── mold resolution ───────────────────────────────────────────────────────────

/// Try resolving a mold name against a single source.
fn resolve_source(source: &Source, mold_name: &str, no_cache: bool) -> Result<MoldSource> {
    let token = effective_token(source);
    match &source.kind {
        SourceType::Local => resolve_local(source, mold_name),
        SourceType::Github => resolve_github(source, mold_name, token, no_cache),
        SourceType::Gitlab => resolve_gitlab(source, mold_name, token, no_cache),
        SourceType::Http => resolve_http(source, mold_name, token, no_cache),
    }
}

/// Resolve an `@spec` reference to a MoldSource.
///
/// `spec` is the part after the leading `@`:
/// - `"moldname"`              → FIMOD_REGISTRY (anonymous) first, then sources.toml default
/// - `"registryname/moldname"` → FIMOD_REGISTRY (named) first, then sources.toml
///
/// FIMOD_REGISTRY takes priority over sources.toml because env vars are explicit
/// overrides (typical Unix convention: env > config file).
pub fn resolve(spec: &str, no_cache: bool) -> Result<MoldSource> {
    let cfg = load_config()?;
    let env_entries = parse_env_registries();

    // Explicit registry prefix: @registry/mold
    if let Some(pos) = spec.find('/') {
        let source_name = &spec[..pos];
        let mold_name = &spec[pos + 1..];

        // Try named FIMOD_REGISTRY entries first
        for entry in &env_entries {
            if entry.name.as_deref() == Some(source_name) {
                return resolve_source(&entry.source, mold_name, no_cache);
            }
        }

        // Then sources.toml
        let source = cfg.sources.get(source_name).ok_or_else(|| {
            anyhow::anyhow!(
                "Registry '{source_name}' not found. Use 'fimod registry list' to see available registries."
            )
        })?;
        return resolve_source(source, mold_name, no_cache);
    }

    // Bare @name — try FIMOD_REGISTRY anonymous entries first (env overrides config)
    let mold_name = spec;
    for entry in &env_entries {
        if let Ok(result) = resolve_source(&entry.source, mold_name, no_cache) {
            return Ok(result);
        }
    }

    // Fallback: try sources.toml default registry
    if let Some(default_name) = cfg.default.as_deref() {
        if let Some(source) = cfg.sources.get(default_name) {
            if let Ok(result) = resolve_source(source, mold_name, no_cache) {
                return Ok(result);
            }
        }
    }

    // Nothing found — produce a helpful error
    if cfg.default.is_none() && env_entries.is_empty() {
        bail!(
            "No default registry configured and FIMOD_REGISTRY not set. \
             Use 'fimod registry add' or set FIMOD_REGISTRY."
        );
    }
    bail!(
        "Mold '{mold_name}' not found in any configured registry. \
         Use 'fimod mold list' to see available molds."
    );
}

/// Determine the effective auth token for a source.
///
/// Priority: explicit `token_env` > default env var for the source type.
fn effective_token(source: &Source) -> Option<String> {
    if let Some(env_var) = &source.token_env {
        return std::env::var(env_var).ok();
    }
    // Default env var by source type, then generic fallback for Http
    source
        .kind
        .default_token_env()
        .and_then(|var| std::env::var(var).ok())
        .or_else(|| {
            if source.kind == SourceType::Http {
                std::env::var("FIMOD_DL_AUTH_TOKEN").ok()
            } else {
                None
            }
        })
}

/// Resolve the sources to iterate: a single named registry or all configured registries.
fn select_sources<'a>(
    cfg: &'a SourcesConfig,
    registry_name: Option<&'a str>,
) -> Result<Vec<(&'a str, &'a Source)>> {
    if let Some(name) = registry_name {
        let source = cfg.sources.get(name).ok_or_else(|| {
            anyhow::anyhow!(
                "Registry '{name}' not found. Use 'fimod registry list' to see configured registries."
            )
        })?;
        Ok(vec![(name, source)])
    } else {
        Ok(cfg.sources.iter().map(|(n, s)| (n.as_str(), s)).collect())
    }
}

/// Build Bearer authorization headers for a source, if a token is available.
fn auth_headers(source: &Source) -> Vec<String> {
    effective_token(source)
        .map(|t| format!("Authorization: Bearer {t}"))
        .into_iter()
        .collect()
}

/// Determine the auth token to use for a direct URL (no named source).
///
/// Priority:
/// 1. `$GITHUB_TOKEN` for github.com / raw.githubusercontent.com URLs
/// 2. `$GITLAB_TOKEN` for gitlab URLs
/// 3. `$FIMOD_DL_AUTH_TOKEN` as a generic fallback (Gitea, Forgejo, private hosts, …)
pub fn token_for_url(url: &str) -> Option<String> {
    if url.contains("github.com") || url.contains("raw.githubusercontent.com") {
        std::env::var("GITHUB_TOKEN").ok()
    } else if url.contains("gitlab") {
        std::env::var("GITLAB_TOKEN").ok()
    } else {
        std::env::var("FIMOD_DL_AUTH_TOKEN").ok()
    }
}

// ── per-type resolution helpers ───────────────────────────────────────────────

fn resolve_local(source: &Source, mold_name: &str) -> Result<MoldSource> {
    let base = source
        .path
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Local registry has no path configured"))?;
    let base = Path::new(base);

    // 1. base/mold_name.py
    let direct = base.join(format!("{mold_name}.py"));
    if direct.is_file() {
        return Ok(MoldSource::File(direct.to_string_lossy().into_owned()));
    }

    // 2. base/mold_name/<last_segment>.py
    let last = mold_name.split('/').next_back().unwrap_or(mold_name);
    let named = base.join(mold_name).join(format!("{last}.py"));
    if named.is_file() {
        return Ok(MoldSource::File(named.to_string_lossy().into_owned()));
    }

    // 3. base/mold_name/__main__.py
    let main = base.join(mold_name).join("__main__.py");
    if main.is_file() {
        return Ok(MoldSource::File(main.to_string_lossy().into_owned()));
    }

    bail!(
        "Mold '{}' not found in registry '{}' (tried {}.py, {}/{}.py, {}/__main__.py)",
        mold_name,
        base.display(),
        mold_name,
        mold_name,
        last,
        mold_name
    )
}

/// Fetch the relative path (and optional content hash) for a mold from the remote catalog.
///
/// Returns:
/// - `Ok(Some((path, hash)))` — mold found in catalog
/// - `Ok(None)`               — catalog does not exist (HTTP 404); caller falls back to convention
/// - `Err(_)`                 — catalog exists but is broken (network error, bad TOML, missing path)
fn remote_catalog_entry(
    source: &Source,
    mold_name: &str,
    no_cache: bool,
) -> Result<Option<(String, Option<String>)>> {
    let catalog = match fetch_catalog(source, no_cache)? {
        Some(c) => c,
        None => return Ok(None),
    };
    let catalog_url = catalog_url_for(source).unwrap_or_else(|_| "(unknown)".to_string());
    let entry = catalog
        .molds
        .get(mold_name)
        .ok_or_else(|| anyhow::anyhow!("Mold '{mold_name}' not found in catalog: {catalog_url}"))?;
    let path = entry
        .path
        .clone()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Mold '{mold_name}' has no 'path' field in catalog: {catalog_url}\n\
                 Hint: regenerate the catalog with 'fimod registry build-catalog'"
            )
        })?;
    Ok(Some((path, entry.hash.clone())))
}

fn resolve_github(
    source: &Source,
    mold_name: &str,
    token: Option<String>,
    no_cache: bool,
) -> Result<MoldSource> {
    let base_url = source
        .url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("GitHub registry has no URL configured"))?;
    let raw_base = github_to_raw(base_url)?;
    resolve_via_catalog(source, mold_name, &raw_base, token, no_cache)
}

fn resolve_gitlab(
    source: &Source,
    mold_name: &str,
    token: Option<String>,
    no_cache: bool,
) -> Result<MoldSource> {
    let base_url = source
        .url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("GitLab registry has no URL configured"))?;
    resolve_via_catalog(source, mold_name, base_url, token, no_cache)
}

fn resolve_http(
    source: &Source,
    mold_name: &str,
    token: Option<String>,
    no_cache: bool,
) -> Result<MoldSource> {
    let base_url = source
        .url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("HTTP registry has no URL configured"))?;
    resolve_via_catalog(source, mold_name, base_url, token, no_cache)
}

/// Shared resolution logic: try catalog first, warn and fall back to `{mold_name}.py` otherwise.
fn resolve_via_catalog(
    source: &Source,
    mold_name: &str,
    base: &str,
    token: Option<String>,
    no_cache: bool,
) -> Result<MoldSource> {
    let (rel, catalog_hash) = match remote_catalog_entry(source, mold_name, no_cache) {
        Ok(Some((path, hash))) => (path, hash),
        Ok(None) => {
            let catalog_url = catalog_url_for(source).unwrap_or_else(|_| "(unknown)".to_string());
            eprintln!(
                "warning: catalog not found (HTTP 404): {catalog_url}\n\
                 warning: falling back to '{mold_name}.py'"
            );
            (format!("{mold_name}.py"), None)
        }
        Err(e) => {
            eprintln!("warning: catalog lookup failed — {e:#}");
            eprintln!("warning: falling back to '{mold_name}.py'");
            (format!("{mold_name}.py"), None)
        }
    };
    let url = format!("{}/{rel}", base.trim_end_matches('/'));
    Ok(MoldSource::Url(url, token, catalog_hash))
}

// ── catalog data model ────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Default)]
struct Catalog {
    #[serde(default)]
    molds: BTreeMap<String, CatalogEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct CatalogEntry {
    description: Option<String>,
    /// Free-form documentation extracted from the mold's module-level docstring.
    #[serde(skip_serializing_if = "Option::is_none")]
    docs: Option<String>,
    /// Relative path to the mold script from the registry base (e.g. `gh_latest/gh_latest.py`).
    /// Stored in catalog.toml to avoid probing multiple URL patterns at resolution time.
    path: Option<String>,
    /// Relative path to the mold's README from the registry base (e.g. `gh_latest/README.md`).
    /// Only present when the README exists at catalog build time.
    #[serde(skip_serializing_if = "Option::is_none")]
    readme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_format: Option<String>,
    /// Options like `no-follow`, `csv-delimiter=,` etc.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    options: Vec<String>,
    /// Documented --arg parameters: name → description (empty string if undocumented).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    args: BTreeMap<String, String>,
    /// Documented ENV variables: name → description (empty string if undocumented).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    envs: BTreeMap<String, String>,
    /// Deterministic content hash of the mold directory (SHA-256, truncated to 16 hex chars).
    /// Computed by `build-catalog`; used by the client cache to detect mold changes.
    #[serde(skip_serializing_if = "Option::is_none")]
    hash: Option<String>,
}

// ── local mold scanning ───────────────────────────────────────────────────────

/// Returns the effective description for a mold: explicit `description=` directive first,
/// falling back to the first line of the docstring (trimming trailing period).
fn effective_description(d: &crate::mold::MoldDefaults) -> Option<String> {
    d.docs
        .as_deref()?
        .lines()
        .next()
        .map(|l| l.trim_end_matches('.').to_string())
}

/// Scan a local registry directory and return `(name, description, relative_path)` triples.
///
/// Recognises two layouts:
/// - `<base>/mold_name.py`            (flat file)
/// - `<base>/mold_name/mold_name.py`  (directory, named script)
/// - `<base>/mold_name/__main__.py`   (directory, __main__ script)
///
/// A name is only returned once (directory layout takes priority over a
/// same-named flat file if both exist, which should not happen in practice).
fn scan_local_molds(base: &Path) -> Vec<(String, Option<String>, String)> {
    let mut results = Vec::new();
    let mut seen = std::collections::BTreeSet::new();

    let Ok(entries) = fs::read_dir(base) else {
        return results;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let stem = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("py") {
            if seen.contains(&stem) {
                continue;
            }
            seen.insert(stem.clone());
            let desc = fs::read_to_string(&path)
                .ok()
                .map(|s| crate::mold::parse_mold_defaults(&s))
                .and_then(|d| effective_description(&d));
            let rel = format!("{stem}.py");
            results.push((stem, desc, rel));
        } else if path.is_dir() {
            if seen.contains(&stem) {
                continue;
            }
            let named = path.join(format!("{stem}.py"));
            let main = path.join("__main__.py");
            let script = if named.is_file() {
                Some((named, format!("{stem}/{stem}.py")))
            } else if main.is_file() {
                Some((main, format!("{stem}/__main__.py")))
            } else {
                None
            };
            if let Some((script, rel)) = script {
                seen.insert(stem.clone());
                let desc = fs::read_to_string(&script)
                    .ok()
                    .map(|s| crate::mold::parse_mold_defaults(&s))
                    .and_then(|d| effective_description(&d));
                results.push((stem, desc, rel));
            }
        }
    }

    results.sort_by(|a, b| a.0.cmp(&b.0));
    results
}

// ── catalog URL helper ────────────────────────────────────────────────────────

fn catalog_url_for(source: &Source) -> Result<String> {
    match &source.kind {
        SourceType::Github => {
            let raw_base = github_to_raw(source.url.as_deref().unwrap_or(""))?;
            Ok(format!("{}/catalog.toml", raw_base.trim_end_matches('/')))
        }
        SourceType::Gitlab | SourceType::Http => Ok(format!(
            "{}/catalog.toml",
            source.url.as_deref().unwrap_or("").trim_end_matches('/')
        )),
        SourceType::Local => unreachable!("catalog_url_for called for local registry"),
    }
}

// ── catalog cache (ETag) ─────────────────────────────────────────────────────

/// Base directory for all fimod caches: `~/.cache/fimod/` (respects `FIMOD_CACHE_DIR`).
pub(crate) fn cache_base_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("FIMOD_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".cache").join("fimod")
}

/// Catalog cache directory for a specific source URL.
fn catalog_cache_dir(catalog_url: &str) -> PathBuf {
    use sha2::{Digest, Sha256};
    let hash = hex::encode(Sha256::digest(catalog_url.as_bytes()));
    cache_base_dir().join("catalog").join(&hash[..16])
}

/// Fetch (with ETag caching) and parse a remote catalog.
///
/// Returns:
/// - `Ok(Some(catalog))` — catalog found and parsed
/// - `Ok(None)`          — catalog does not exist (HTTP 404)
/// - `Err(_)`            — network error, bad TOML, etc.
fn fetch_catalog(source: &Source, no_cache: bool) -> Result<Option<Catalog>> {
    let catalog_url = catalog_url_for(source)?;
    let mut headers = auth_headers(source);

    let cache_dir = catalog_cache_dir(&catalog_url);
    let cached_catalog_path = cache_dir.join("catalog.toml");
    let cached_etag_path = cache_dir.join("etag");

    // Add If-None-Match if we have a cached ETag.
    if !no_cache {
        if let Ok(etag) = fs::read_to_string(&cached_etag_path) {
            let etag = etag.trim().to_string();
            if !etag.is_empty() {
                headers.push(format!("If-None-Match: {etag}"));
            }
        }
    }

    let resp = match crate::http::fetch_url(&catalog_url, &headers, 30, false, false) {
        Ok(r) => r,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("404") {
                return Ok(None);
            }
            return Err(e.context(format!("Failed to fetch registry catalog: {catalog_url}")));
        }
    };

    // 304 Not Modified — use cached catalog.
    if resp.status == 304 {
        if let Ok(body) = fs::read_to_string(&cached_catalog_path) {
            let catalog: Catalog =
                toml::from_str(&body).context("Failed to parse cached catalog.toml")?;
            return Ok(Some(catalog));
        }
        // Cache file gone? Fall through to re-fetch without ETag.
        // This shouldn't normally happen — treat as a cache miss.
    }

    let body = &resp.body;

    // Save catalog + ETag to cache (best-effort).
    let _ = fs::create_dir_all(&cache_dir);
    let _ = fs::write(&cached_catalog_path, body);
    if let Some(etag) = resp.headers.get("etag") {
        let _ = fs::write(&cached_etag_path, etag);
    }

    let catalog: Catalog =
        toml::from_str(body).with_context(|| format!("Failed to parse catalog: {catalog_url}"))?;
    Ok(Some(catalog))
}

// ── cache management ─────────────────────────────────────────────────────────

/// Remove cached catalogs and molds.
///
/// - `None` → wipe the entire cache directory
/// - `Some(name)` → wipe a specific mold's cache (not yet implemented, clears all)
pub fn cache_clear(name: Option<&str>) -> Result<()> {
    let base = cache_base_dir();
    if let Some(_name) = name {
        // TODO: resolve name to URL hash and remove only that entry.
        // For now, clear everything.
        eprintln!("warning: per-mold cache clear not yet implemented, clearing all");
    }
    if base.exists() {
        fs::remove_dir_all(&base)
            .with_context(|| format!("Failed to remove cache directory: {}", base.display()))?;
        println!("Cache cleared: {}", base.display());
    } else {
        println!("Cache directory does not exist: {}", base.display());
    }
    Ok(())
}

/// Show cache directory location and disk usage.
pub fn cache_info() -> Result<()> {
    let base = cache_base_dir();
    println!("Cache directory: {}", base.display());

    if !base.exists() {
        println!("  (empty — no cached data)");
        return Ok(());
    }

    let mut catalog_count: usize = 0;
    let mut mold_count: usize = 0;

    let catalog_dir = base.join("catalog");
    if catalog_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&catalog_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    catalog_count += 1;
                }
            }
        }
    }

    let molds_dir = base.join("molds");
    if molds_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&molds_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    mold_count += 1;
                } else if path.is_file() {
                    // Legacy flat .py files
                    mold_count += 1;
                }
            }
        }
    }

    // Walk all files for total size.
    fn dir_size(dir: &Path) -> u64 {
        let mut size = 0u64;
        let Ok(entries) = fs::read_dir(dir) else {
            return 0;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                size += dir_size(&path);
            } else if let Ok(meta) = path.metadata() {
                size += meta.len();
            }
        }
        size
    }

    let total_size = dir_size(&base);

    let size_str = if total_size < 1024 {
        format!("{total_size} B")
    } else if total_size < 1024 * 1024 {
        format!("{:.1} KB", total_size as f64 / 1024.0)
    } else {
        format!("{:.1} MB", total_size as f64 / (1024.0 * 1024.0))
    };

    println!("  Catalogs: {catalog_count}");
    println!("  Molds:    {mold_count}");
    println!("  Size:     {size_str}");

    Ok(())
}

// ── mold commands ─────────────────────────────────────────────────────────────

/// Print molds for a single registry (name + source already resolved).
fn print_registry_molds(name: &str, source: &Source, is_default: bool) -> Result<()> {
    let default_marker = if is_default { " (default)" } else { "" };
    println!("{} [{}]{}", name, source.kind, default_marker);

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
            let catalog = fetch_catalog(source, false)?
                .ok_or_else(|| {
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

/// `(registry_name, is_default, mold_name, description)` entry from [`collect_all_molds`].
type MoldEntry = (String, bool, String, Option<String>);

/// Collect all molds from the configured registries into a flat list.
///
/// Returns `(registry_name, is_default, mold_name, description)` tuples.
fn collect_all_molds(cfg: &SourcesConfig, registry_name: Option<&str>) -> Result<Vec<MoldEntry>> {
    let sources = select_sources(cfg, registry_name)?;
    let env_entries = parse_env_registries();

    let default_name = cfg.default.as_deref();
    let mut result = Vec::new();

    for (reg_name, source) in sources {
        let is_default = default_name == Some(reg_name);
        collect_molds_from_source(reg_name, source, is_default, &mut result);
    }

    // Include FIMOD_REGISTRY entries (only when listing all, not a specific registry)
    if registry_name.is_none() {
        let mut anon_index = 0;
        for entry in &env_entries {
            let display_name = env_display_name(entry, &mut anon_index);
            collect_molds_from_source(&display_name, &entry.source, false, &mut result);
        }
    }

    Ok(result)
}

/// Collect molds from a single source into the result vector.
fn collect_molds_from_source(
    reg_name: &str,
    source: &Source,
    is_default: bool,
    result: &mut Vec<MoldEntry>,
) {
    match &source.kind {
        SourceType::Local => {
            let Some(base) = source.path.as_deref() else {
                return;
            };
            for (mold_name, desc, _rel) in scan_local_molds(Path::new(base)) {
                result.push((reg_name.to_string(), is_default, mold_name, desc));
            }
        }
        SourceType::Github | SourceType::Gitlab | SourceType::Http => {
            let Ok(Some(catalog)) = fetch_catalog(source, false) else {
                return;
            };
            for (mold_name, entry) in catalog.molds {
                result.push((
                    reg_name.to_string(),
                    is_default,
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

/// List molds available in a registry (local scan or remote catalog.toml).
/// Without a registry name, lists all configured registries.
pub fn list_molds(registry_name: Option<&str>, output_format: MoldListFormat) -> Result<()> {
    let cfg = load_config()?;

    match output_format {
        MoldListFormat::Json => {
            let molds = collect_all_molds(&cfg, registry_name)?;
            let arr: Vec<serde_json::Value> = molds
                .into_iter()
                .map(|(reg, is_default, name, desc)| {
                    serde_json::json!({
                        "name": name,
                        "registry": reg,
                        "is_default": is_default,
                        "description": desc.unwrap_or_default(),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&arr)?);
        }
        MoldListFormat::Lines => {
            let molds = collect_all_molds(&cfg, registry_name)?;
            for (reg, _is_default, name, desc) in molds {
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
                let is_default = cfg.default.as_deref() == Some(name);
                print_registry_molds(name, source, is_default)?;
            } else {
                let mut first = true;
                for (name, source) in &cfg.sources {
                    if !first {
                        println!();
                    }
                    first = false;
                    let is_default = cfg.default.as_deref() == Some(name.as_str());
                    print_registry_molds(name, source, is_default)?;
                }
                let mut anon_index = 0;
                for entry in &env_entries {
                    if !first {
                        println!();
                    }
                    first = false;
                    let base_name = env_display_name(entry, &mut anon_index);
                    let marker = if base_name == "env-default" {
                        "(default FIMOD_REGISTRY)"
                    } else {
                        "(FIMOD_REGISTRY)"
                    };
                    let display_name = format!("{base_name} {marker}");
                    print_registry_molds(&display_name, &entry.source, false)?;
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
fn format_defaults_options(d: &crate::mold::MoldDefaults) -> Vec<String> {
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
    reg_name: String,
    is_default: bool,
    detail: MoldDetail,
}

fn collect_mold_matches(
    cfg: &SourcesConfig,
    mold_name: &str,
    registry_name: Option<&str>,
) -> Result<Vec<MoldMatch>> {
    let sources = select_sources(cfg, registry_name)?;

    let default_name = cfg.default.as_deref();
    let mut matches: Vec<MoldMatch> = Vec::new();

    for (reg_name, source) in sources {
        let is_default = default_name == Some(reg_name);
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
                    reg_name: reg_name.to_string(),
                    is_default,
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
                    reg_name: reg_name.to_string(),
                    is_default,
                    detail: MoldDetail::Remote {
                        registry_url: source.url.clone().unwrap_or_default(),
                        entry,
                    },
                });
            }
        }
    }

    // Default registry first, preserve original order otherwise
    matches.sort_by_key(|m| if m.is_default { 0usize } else { 1 });
    Ok(matches)
}

fn print_mold_match(mold_name: &str, m: &MoldMatch) {
    let default_marker = if m.is_default { " (default)" } else { "" };
    println!("{mold_name}  [{}]{default_marker}", m.reg_name);
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
            if entry.docs.is_none() {
                if let Some(desc) = &entry.description {
                    println!("  Description:    {desc}");
                }
            }
            if let Some(docs) = &entry.docs {
                for line in docs.lines() {
                    println!("  {line}");
                }
                println!();
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
pub fn show_mold(mold_ref: &str, registry_name: Option<&str>) -> Result<()> {
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

    // When a specific registry was requested (or only one match), show it directly
    let explicit = resolved_registry.is_some();
    if explicit || matches.len() == 1 {
        print_mold_match(mold_name, &matches[0]);
        return Ok(());
    }

    // Show the primary (default-first) match, then "See also" for the rest
    print_mold_match(mold_name, &matches[0]);
    let others: Vec<String> = matches[1..]
        .iter()
        .map(|m| format!("fimod mold show {}/{mold_name}", m.reg_name))
        .collect();
    if !others.is_empty() {
        println!();
        println!("  See also:       {}", others.join(", "));
    }
    Ok(())
}

/// Set up the official fimod molds registry interactively.
///
/// Behaviour:
/// - Already present (by URL) → prints a message and exits cleanly.
/// - Fresh install (no default yet) → adds as default, no prompt needed.
/// - Default already set, `--force` absent → adds without overriding default (asks first unless `--yes`).
/// - Default already set, `--force` present → adds and promotes to default (asks first unless `--yes`).
pub fn setup(yes: bool, force: bool) -> Result<()> {
    const OFFICIAL_NAME: &str = "official";
    const OFFICIAL_URL: &str = "https://github.com/pytgaen/fimod/tree/main/molds";

    let cfg = load_config()?;

    // Already present (check by URL, regardless of name)
    if cfg
        .sources
        .values()
        .any(|s| s.url.as_deref() == Some(OFFICIAL_URL))
    {
        println!("Official molds registry is already configured.");
        return Ok(());
    }

    let has_default = cfg.default.is_some();

    // In interactive mode, ask confirmation (unless --yes)
    if !yes {
        use std::io::IsTerminal;
        if !std::io::stdin().is_terminal() {
            // Non-interactive context: skip silently
            return Ok(());
        }
        if has_default && !force {
            print!(
                "Install the official fimod molds registry (without changing your current default)? [y/N] "
            );
        } else {
            print!("Install the official fimod molds registry as default? [y/N] ");
        }
        use std::io::Write;
        std::io::stdout().flush()?;
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        let answer = answer.trim().to_lowercase();
        if answer != "y" && answer != "yes" {
            println!("Skipped.");
            return Ok(());
        }
    }

    // Determine whether to set as default
    // - No existing default → always set as default (fresh install)
    // - Existing default + --force → override
    // - Existing default, no --force → add without touching default
    let set_as_default = !has_default || force;

    if has_default && !force {
        println!("Note: a default registry is already set; adding official registry without changing it.");
        println!("      Use --force to promote it to default.");
    }

    // Resolve name (handle unlikely collision where 'official' name is already taken)
    let name = if cfg.sources.contains_key(OFFICIAL_NAME) {
        let alt = "fimod-official";
        println!("Note: registry name 'official' is already taken, using '{alt}' instead.");
        alt
    } else {
        OFFICIAL_NAME
    };

    add(name, OFFICIAL_URL, None, set_as_default)
}

/// Compute a deterministic content hash for a mold.
///
/// - Flat file (`name.py`): SHA-256 of the file content.
/// - Directory (`name/`): collect all files recursively, sort paths alphabetically,
///   build `path:{sha256(content)}|…`, SHA-256 the concatenation.
///
/// Returns a hex string truncated to 16 characters.
fn compute_mold_hash(base: &Path, rel_path: &str) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::collections::BTreeSet;

    let script_path = base.join(rel_path);

    // Determine if this is a flat file or a directory mold.
    let mold_dir = script_path
        .parent()
        .filter(|p| *p != base) // flat file: parent == base
        .unwrap_or(script_path.as_path());

    if mold_dir == script_path.as_path() {
        // Flat file: hash the file directly.
        let content = fs::read(&script_path)
            .with_context(|| format!("Cannot read mold for hashing: {}", script_path.display()))?;
        let digest = hex::encode(Sha256::digest(&content));
        return Ok(digest[..16].to_string());
    }

    // Directory: collect all files recursively, sort, hash.
    fn collect_files(dir: &Path, prefix: &str, out: &mut BTreeSet<(String, Vec<u8>)>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let rel = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            if path.is_dir() {
                collect_files(&path, &rel, out);
            } else if path.is_file() {
                if let Ok(content) = fs::read(&path) {
                    out.insert((rel, content));
                }
            }
        }
    }

    let mut files = BTreeSet::new();
    collect_files(mold_dir, "", &mut files);

    let mut combined = String::new();
    for (path, content) in &files {
        let file_hash = hex::encode(Sha256::digest(content));
        if !combined.is_empty() {
            combined.push('|');
        }
        combined.push_str(&format!("{path}:{file_hash}"));
    }

    let digest = hex::encode(Sha256::digest(combined.as_bytes()));
    Ok(digest[..16].to_string())
}

/// Build or rebuild `catalog.toml` for a local registry.
pub fn build_catalog(registry_name: &str) -> Result<()> {
    let cfg = load_config()?;

    let source = cfg.sources.get(registry_name).ok_or_else(|| {
        anyhow::anyhow!(
            "Registry '{registry_name}' not found. Use 'fimod registry list' to see configured registries."
        )
    })?;

    if source.kind != SourceType::Local {
        bail!(
            "Registry '{}' is of type '{}'; build-catalog only works for local registries.",
            registry_name,
            source.kind
        );
    }

    let base = source
        .path
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Local registry has no path configured"))?;

    let molds = scan_local_molds(Path::new(base));

    let mut catalog = Catalog::default();
    for (name, _description, rel_path) in &molds {
        let script_path = Path::new(base).join(rel_path);
        let defaults = fs::read_to_string(&script_path)
            .map(|s| crate::mold::parse_mold_defaults(&s))
            .unwrap_or_default();

        let readme = Path::new(rel_path)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|dir| dir.join("README.md"))
            .filter(|readme_rel| Path::new(base).join(readme_rel).is_file())
            .and_then(|p| p.to_str().map(|s| s.replace('\\', "/")));

        let args = defaults
            .args
            .iter()
            .map(|(n, d)| (n.clone(), d.clone().unwrap_or_default()))
            .collect();
        let envs = defaults
            .envs
            .iter()
            .map(|(n, d)| (n.clone(), d.clone().unwrap_or_default()))
            .collect();

        let options = format_defaults_options(&defaults);
        let mold_hash = compute_mold_hash(Path::new(base), rel_path)
            .map(Some)
            .unwrap_or_else(|e| {
                eprintln!("[fimod] warning: could not hash mold '{name}': {e}");
                None
            });
        catalog.molds.insert(
            name.clone(),
            CatalogEntry {
                description: effective_description(&defaults),
                docs: defaults.docs,
                path: Some(rel_path.clone()),
                readme,
                input_format: defaults.input_format,
                output_format: defaults.output_format,
                options,
                args,
                envs,
                hash: mold_hash,
            },
        );
    }

    let content = toml::to_string_pretty(&catalog).context("Failed to serialize catalog")?;
    let catalog_path = Path::new(base).join("catalog.toml");
    fs::write(&catalog_path, &content)
        .with_context(|| format!("Failed to write catalog: {}", catalog_path.display()))?;

    println!("Scanned {} molds:", molds.len());
    for (name, desc, rel) in &molds {
        println!(
            "  {:<20} \"{}\"  [{}]",
            name,
            desc.as_deref().unwrap_or("(no description)"),
            rel
        );
    }
    println!("Wrote {}", catalog_path.display());

    Ok(())
}

/// Returns true if a ref looks like a version tag: `v1.0.0`, `2.3.4`, `v1.0.0-beta.1`, etc.
/// Used to decide between refs/tags/ and refs/heads/ when the ref type is ambiguous.
fn looks_like_version_tag(r: &str) -> bool {
    let r = r.strip_prefix('v').unwrap_or(r);
    let first = r.split(['.', '-']).next().unwrap_or("");
    !first.is_empty() && first.chars().all(|c| c.is_ascii_digit())
}

/// Convert a `https://github.com/org/repo[/tree/<branch>/<path>]` URL to a raw content base URL.
fn github_to_raw(url: &str) -> Result<String> {
    let url = url.trim_end_matches('/');
    for prefix in &["https://github.com/", "http://github.com/"] {
        if let Some(path) = url.strip_prefix(prefix) {
            // Handle /tree/<branch>/<rest> and /blob/<branch>/<rest>
            let segments: Vec<&str> = path.splitn(4, '/').collect();
            // segments: [owner, repo, "tree"|"blob", branch/path...]  (len >= 4)
            if segments.len() >= 4 && (segments[2] == "tree" || segments[2] == "blob") {
                // segments[3] contains "<branch-or-sha>/<subpath>"
                // For branch refs, use refs/heads/ explicitly to avoid CDN ambiguity
                // that can cause anonymous requests to return 404 on raw.githubusercontent.com.
                // SHA refs (40 hex chars) are used as-is.
                let ref_part = segments[3].split('/').next().unwrap_or(segments[3]);
                let raw_ref =
                    if ref_part.len() == 40 && ref_part.bytes().all(|b| b.is_ascii_hexdigit()) {
                        // Commit SHA — use as-is
                        segments[3].to_string()
                    } else if segments[3].starts_with("refs/") {
                        // Already a full ref (e.g. refs/heads/main, refs/tags/v1.0.0)
                        segments[3].to_string()
                    } else if looks_like_version_tag(ref_part) {
                        // Semver-like tag (v1.0.0, 2.3.4, v1.0.0-beta) → refs/tags/
                        format!("refs/tags/{}", segments[3])
                    } else {
                        // Branch name → refs/heads/ for reliable anonymous CDN access
                        format!("refs/heads/{}", segments[3])
                    };
                return Ok(format!(
                    "https://raw.githubusercontent.com/{}/{}/{raw_ref}",
                    segments[0], segments[1]
                ));
            }
            // Plain repo URL: https://github.com/org/repo
            return Ok(format!("https://raw.githubusercontent.com/{path}/HEAD"));
        }
    }
    // Already a raw URL or custom format — use as-is
    Ok(url.to_string())
}
