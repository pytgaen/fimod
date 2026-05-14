use std::path::Path;

use anyhow::{bail, Result};

use super::catalog::{catalog_url_for, fetch_catalog, github_to_raw};
use super::config::{load_config, ordered_sources, Source, SourceType, SourcesConfig};
use crate::mold::MoldSource;

// ── FIMOD_REGISTRY env var ────────────────────────────────────────────────────

/// A parsed FIMOD_REGISTRY entry: either named (`ci=/path`) or anonymous (`/path`).
pub(crate) struct EnvRegistry {
    name: Option<String>,
    pub(crate) source: Source,
}

/// Build a Source from a location string (path or URL).
fn source_from_location(location: &str) -> Source {
    if crate::http::is_url(location) {
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
pub(crate) fn parse_env_registries() -> Vec<EnvRegistry> {
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
pub(crate) fn env_display_name(entry: &EnvRegistry, anon_index: &mut usize) -> String {
    match &entry.name {
        Some(n) => n.clone(),
        None => {
            let name = env_anonymous_name(*anon_index);
            *anon_index += 1;
            name
        }
    }
}

// ── mold resolution ───────────────────────────────────────────────────────────

/// Try resolving a mold name against a single source.
fn resolve_source(source: &Source, mold_name: &str, no_cache: bool) -> Result<MoldSource> {
    let token = effective_token(source);
    match &source.kind {
        SourceType::Local => resolve_local(source, mold_name),
        SourceType::Github | SourceType::Gitlab | SourceType::Http => {
            resolve_remote(source, mold_name, token, no_cache)
        }
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

    // Bare @name — try FIMOD_REGISTRY entries first (env overrides config)
    let mold_name = spec;
    for entry in &env_entries {
        if let Ok(result) = resolve_source(&entry.source, mold_name, no_cache) {
            return Ok(result);
        }
    }

    // Then try all sources.toml registries in priority order
    for (_, source, _) in ordered_sources(&cfg) {
        if let Ok(result) = resolve_source(source, mold_name, no_cache) {
            return Ok(result);
        }
    }

    // Nothing found — produce a helpful error
    if cfg.sources.is_empty() && env_entries.is_empty() {
        bail!(
            "No registry configured and FIMOD_REGISTRY not set. \
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
pub(super) fn select_sources<'a>(
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
pub(super) fn auth_headers(source: &Source) -> Vec<String> {
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
        let path = direct.to_string_lossy().into_owned();
        return Ok(MoldSource::file(path));
    }

    // 2. base/mold_name/<last_segment>.py
    let last = mold_name.split('/').next_back().unwrap_or(mold_name);
    let named = base.join(mold_name).join(format!("{last}.py"));
    if named.is_file() {
        let path = named.to_string_lossy().into_owned();
        return Ok(MoldSource::file(path));
    }

    // 3. base/mold_name/__main__.py
    let main = base.join(mold_name).join("__main__.py");
    if main.is_file() {
        let path = main.to_string_lossy().into_owned();
        return Ok(MoldSource::file(path));
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

/// `(script_rel_path, content_hash, companion_files)`
type CatalogLookup = (String, Option<String>, Vec<String>);

/// Fetch the relative path, content hash, and companion files for a mold from the remote catalog.
///
/// Returns:
/// - `Ok(Some(..))` — mold found in catalog
/// - `Ok(None)` — catalog does not exist (HTTP 404); caller falls back to convention
/// - `Err(_)` — catalog exists but mold not in it, or broken (network error, bad TOML)
fn remote_catalog_entry(
    source: &Source,
    mold_name: &str,
    no_cache: bool,
) -> Result<Option<CatalogLookup>> {
    let catalog = match fetch_catalog(source, no_cache)? {
        Some(c) => c,
        None => return Ok(None),
    };
    let catalog_url = catalog_url_for(source).unwrap_or_else(|_| "(unknown)".to_string());
    let entry = catalog
        .molds
        .get(mold_name)
        .ok_or_else(|| anyhow::anyhow!("Mold '{mold_name}' not found in catalog: {catalog_url}"))?;
    let path = entry.path.clone().ok_or_else(|| {
        anyhow::anyhow!(
            "Mold '{mold_name}' has no 'path' field in catalog: {catalog_url}\n\
                 Hint: regenerate the catalog with 'fimod registry build-catalog'"
        )
    })?;
    Ok(Some((path, entry.hash.clone(), entry.files.clone())))
}

/// Resolve a mold from a remote source (GitHub, GitLab, generic HTTP).
/// Applies the `github_to_raw` URL transform for GitHub before delegating to
/// `resolve_via_catalog`.
fn resolve_remote(
    source: &Source,
    mold_name: &str,
    token: Option<String>,
    no_cache: bool,
) -> Result<MoldSource> {
    let base_url = source
        .url
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("{} registry has no URL configured", source.kind))?;
    let resolved_base = match source.kind {
        SourceType::Github => github_to_raw(base_url)?,
        _ => base_url.to_string(),
    };
    resolve_via_catalog(source, mold_name, &resolved_base, token, no_cache)
}

/// Shared resolution logic: try catalog first, warn and fall back to `{mold_name}.py` otherwise.
fn resolve_via_catalog(
    source: &Source,
    mold_name: &str,
    base: &str,
    token: Option<String>,
    no_cache: bool,
) -> Result<MoldSource> {
    let (rel, catalog_hash, files) = match remote_catalog_entry(source, mold_name, no_cache) {
        Ok(Some((path, hash, files))) => (path, hash, files),
        Ok(None) => {
            let catalog_url = catalog_url_for(source).unwrap_or_else(|_| "(unknown)".to_string());
            eprintln!(
                "warning: catalog not found (HTTP 404): {catalog_url}\n\
                 warning: falling back to '{mold_name}.py'"
            );
            (format!("{mold_name}.py"), None, vec![])
        }
        Err(e) => {
            // Catalog exists but mold not in it — propagate error so the
            // caller can try the next registry in priority order.
            return Err(e);
        }
    };
    let base_trimmed = base.trim_end_matches('/');
    let url = format!("{base_trimmed}/{rel}");
    let companion_urls: Vec<String> = files
        .iter()
        .map(|f| format!("{base_trimmed}/{f}"))
        .collect();
    Ok(MoldSource::Url {
        display_ref: url.clone(),
        url,
        token,
        catalog_hash,
        companion_files: companion_urls,
    })
}
