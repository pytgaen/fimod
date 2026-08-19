use std::path::Path;

use anyhow::{bail, Result};

use super::catalog::{catalog_url_for, fetch_catalog, github_to_raw};
use super::config::{load_config, ordered_sources, Source, SourceType, SourcesConfig};
use crate::mold::MoldSource;

// ── FIMOD_REGISTRY env var ────────────────────────────────────────────────────

/// A parsed FIMOD_REGISTRY entry: either named (`ci=/path`) or anonymous (`/path`).
pub(super) struct EnvRegistry {
    name: Option<String>,
    pub(super) source: Source,
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
pub(super) fn parse_env_registries() -> Vec<EnvRegistry> {
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
pub(super) fn env_display_name(entry: &EnvRegistry, anon_index: &mut usize) -> String {
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

/// Why a single registry did not yield the mold.
///
/// The distinction drives the final error message: a registry that answered and
/// does not carry the mold is evidence of absence, while one that could not be
/// queried at all says nothing about whether the mold exists.
pub(super) enum SourceMiss {
    /// The registry was queried successfully and does not carry this mold.
    Absent(anyhow::Error),
    /// The registry could not be queried: network failure, TLS problem,
    /// misconfiguration, or a catalog that could not be read.
    Unqueryable(anyhow::Error),
}

impl SourceMiss {
    /// The underlying error, whatever the reason.
    fn into_error(self) -> anyhow::Error {
        match self {
            SourceMiss::Absent(e) | SourceMiss::Unqueryable(e) => e,
        }
    }

    /// Short one-line reason for the per-registry breakdown.
    fn reason(&self) -> String {
        match self {
            SourceMiss::Absent(_) => "queried, no match".to_string(),
            SourceMiss::Unqueryable(e) => format!("unreachable: {e}"),
        }
    }

    fn is_unqueryable(&self) -> bool {
        matches!(self, SourceMiss::Unqueryable(_))
    }
}

/// Try resolving a mold name against a single source.
fn resolve_source(
    source: &Source,
    mold_name: &str,
    no_cache: bool,
) -> std::result::Result<MoldSource, SourceMiss> {
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
                return resolve_source(&entry.source, mold_name, no_cache)
                    .map_err(SourceMiss::into_error);
            }
        }

        // Then sources.toml
        let source = cfg.sources.get(source_name).ok_or_else(|| {
            anyhow::anyhow!(
                "Registry '{source_name}' not found. Use 'fimod registry list' to see available registries."
            )
        })?;
        return resolve_source(source, mold_name, no_cache).map_err(SourceMiss::into_error);
    }

    // Bare @name — try FIMOD_REGISTRY entries first (env overrides config).
    // Every miss is recorded: a registry that could not be queried must not be
    // reported as proof that the mold does not exist.
    let mold_name = spec;
    let mut misses: Vec<(String, SourceMiss)> = Vec::new();
    let mut anon_index = 0usize;

    for entry in &env_entries {
        let label = env_display_name(entry, &mut anon_index);
        match resolve_source(&entry.source, mold_name, no_cache) {
            Ok(result) => return Ok(result),
            Err(miss) => misses.push((label, miss)),
        }
    }

    // Then try all sources.toml registries in priority order
    for (name, source, _) in ordered_sources(&cfg) {
        match resolve_source(source, mold_name, no_cache) {
            Ok(result) => return Ok(result),
            Err(miss) => misses.push((name.to_string(), miss)),
        }
    }

    // Nothing found — produce a helpful error
    if cfg.sources.is_empty() && env_entries.is_empty() {
        bail!(
            "No registry configured and FIMOD_REGISTRY not set. \
             Use 'fimod registry add' or set FIMOD_REGISTRY."
        );
    }
    Err(unresolved_error(mold_name, &misses))
}

/// Build the error for a bare `@mold` that no registry produced.
///
/// When no registry could be queried at all, the mold's existence is simply
/// unknown and saying "not found" would be a lie; the message says so and points
/// at connectivity. Otherwise at least one registry answered, "not found" holds,
/// and any registry that stayed silent is still listed so the user knows the
/// answer is partial.
fn unresolved_error(mold_name: &str, misses: &[(String, SourceMiss)]) -> anyhow::Error {
    let width = misses
        .iter()
        .map(|(name, _)| name.chars().count())
        .max()
        .unwrap_or(0);
    let breakdown = misses
        .iter()
        .map(|(name, miss)| format!("  {name:<width$}  {}", miss.reason(), width = width))
        .collect::<Vec<_>>()
        .join("\n");

    let all_unqueryable = misses.iter().all(|(_, miss)| miss.is_unqueryable());
    if all_unqueryable {
        anyhow::anyhow!(
            "Mold '{mold_name}' could not be resolved: no registry could be queried.\n\n\
             {breakdown}\n\n\
             None of them could answer, so the mold may well exist.\n\
             Fix the failures above, then retry."
        )
    } else {
        anyhow::anyhow!(
            "Mold '{mold_name}' not found in any configured registry.\n\n\
             {breakdown}\n\n\
             Use 'fimod mold list' to see available molds."
        )
    }
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

fn resolve_local(source: &Source, mold_name: &str) -> std::result::Result<MoldSource, SourceMiss> {
    let Some(base) = source.path.as_deref() else {
        return Err(SourceMiss::Unqueryable(anyhow::anyhow!(
            "Local registry has no path configured"
        )));
    };
    let base = Path::new(base);

    if let Some(path) = crate::mold::find_script(base, mold_name) {
        return Ok(MoldSource::file(path.to_string_lossy().into_owned()));
    }

    // A local directory that does not exist cannot be searched, so its silence
    // is not evidence that the mold is absent.
    if !base.is_dir() {
        return Err(SourceMiss::Unqueryable(anyhow::anyhow!(
            "local registry directory does not exist: {}",
            base.display()
        )));
    }

    let last = mold_name.split('/').next_back().unwrap_or(mold_name);
    Err(SourceMiss::Absent(anyhow::anyhow!(
        "Mold '{}' not found in registry '{}' (tried {}.py, {}/{}.py, {}/__main__.py)",
        mold_name,
        base.display(),
        mold_name,
        mold_name,
        last,
        mold_name
    )))
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
) -> std::result::Result<Option<CatalogLookup>, SourceMiss> {
    // A catalog that cannot be fetched or parsed leaves the registry unqueried.
    let catalog = match fetch_catalog(source, no_cache).map_err(SourceMiss::Unqueryable)? {
        Some(c) => c,
        None => return Ok(None),
    };
    let catalog_url = catalog_url_for(source).unwrap_or_else(|_| "(unknown)".to_string());
    let entry = catalog.molds.get(mold_name).ok_or_else(|| {
        SourceMiss::Absent(anyhow::anyhow!(
            "Mold '{mold_name}' not found in catalog: {catalog_url}"
        ))
    })?;
    // The mold is listed but the entry is unusable: the registry answered, yet
    // this is a broken catalog rather than an absent mold.
    let path = entry.path.clone().ok_or_else(|| {
        SourceMiss::Unqueryable(anyhow::anyhow!(
            "Mold '{mold_name}' has no 'path' field in catalog: {catalog_url}\n\
                 Hint: regenerate the catalog with 'fimod registry build-catalog'"
        ))
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
) -> std::result::Result<MoldSource, SourceMiss> {
    let Some(base_url) = source.url.as_deref() else {
        return Err(SourceMiss::Unqueryable(anyhow::anyhow!(
            "{} registry has no URL configured",
            source.kind
        )));
    };
    let resolved_base = match source.kind {
        SourceType::Github => github_to_raw(base_url).map_err(SourceMiss::Unqueryable)?,
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
) -> std::result::Result<MoldSource, SourceMiss> {
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
