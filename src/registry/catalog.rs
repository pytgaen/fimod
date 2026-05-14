use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use super::config::{load_config, Source, SourceType};
use super::molds::format_defaults_options;
use super::resolve::auth_headers;

// ── catalog data model ────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Default)]
pub(crate) struct Catalog {
    #[serde(default)]
    pub(crate) molds: BTreeMap<String, CatalogEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub(crate) struct CatalogEntry {
    pub(crate) description: Option<String>,
    /// Free-form documentation extracted from the mold's module-level docstring.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) docs: Option<String>,
    /// Relative path to the mold script from the registry base (e.g. `gh_latest/gh_latest.py`).
    /// Stored in catalog.toml to avoid probing multiple URL patterns at resolution time.
    pub(crate) path: Option<String>,
    /// Relative path to the mold's README from the registry base (e.g. `gh_latest/README.md`).
    /// Only present when the README exists at catalog build time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) readme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) input_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) output_format: Option<String>,
    /// Options like `no-follow`, `csv-delimiter=,` etc.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) options: Vec<String>,
    /// Documented --arg parameters: name → description (empty string if undocumented).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) args: BTreeMap<String, String>,
    /// Documented ENV variables: name → description (empty string if undocumented).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) envs: BTreeMap<String, String>,
    /// Deterministic content hash of the mold directory (SHA-256, truncated to 16 hex chars).
    /// Computed by `build-catalog`; used by the client cache to detect mold changes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) hash: Option<String>,
    /// Companion files (templates, data, etc.) relative to the registry base.
    /// Downloaded alongside the main script into the mold cache directory.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) files: Vec<String>,
}

// ── local mold scanning ───────────────────────────────────────────────────────

/// Returns the effective description for a mold: explicit `description=` directive first,
/// falling back to the first line of the docstring (trimming trailing period).
/// Fetch a remote mold script and extract its docstring.
/// Returns `None` silently on any error (network, parse, etc.).
#[cfg(feature = "reqwest")]
pub(crate) fn fetch_script_docs(url: &str) -> Option<String> {
    let resp = crate::http::fetch_url(url, &[], 30, false, false).ok()?;
    let defaults = crate::mold::parse_mold_defaults(&resp.body);
    defaults.docs
}

#[cfg(not(feature = "reqwest"))]
pub(crate) fn fetch_script_docs(_url: &str) -> Option<String> {
    None
}

pub(crate) fn effective_description(d: &crate::mold::MoldDefaults) -> Option<String> {
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
pub(crate) fn scan_local_molds(base: &Path) -> Vec<(String, Option<String>, String)> {
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

pub(crate) fn catalog_url_for(source: &Source) -> Result<String> {
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
    let hash = crate::paths::sha256_hex(catalog_url.as_bytes());
    cache_base_dir().join("catalog").join(&hash[..16])
}

/// TTL for cached catalogs: skip HTTP entirely if the cache file is younger than this.
const CATALOG_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(60);

/// Fetch (with ETag caching + TTL) and parse a remote catalog.
///
/// Returns:
/// - `Ok(Some(catalog))` — catalog found and parsed
/// - `Ok(None)`          — catalog does not exist (HTTP 404)
/// - `Err(_)`            — network error, bad TOML, etc.
pub(crate) fn fetch_catalog(source: &Source, no_cache: bool) -> Result<Option<Catalog>> {
    let catalog_url = catalog_url_for(source)?;
    let mut headers = auth_headers(source);

    let cache_dir = catalog_cache_dir(&catalog_url);
    let cached_catalog_path = cache_dir.join("catalog.toml");
    let cached_etag_path = cache_dir.join("etag");

    // TTL fast path: if the cached catalog is fresh enough, use it without any HTTP.
    if !no_cache {
        let is_fresh = fs::metadata(&cached_catalog_path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|m| m.elapsed().ok())
            .is_some_and(|age| age < CATALOG_CACHE_TTL);
        if is_fresh {
            if let Ok(body) = fs::read_to_string(&cached_catalog_path) {
                let catalog: Catalog =
                    toml::from_str(&body).context("Failed to parse cached catalog.toml")?;
                return Ok(Some(catalog));
            }
        }
    }

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

    // 304 Not Modified — use cached catalog and refresh mtime for TTL.
    if resp.status == 304 {
        if let Ok(body) = fs::read_to_string(&cached_catalog_path) {
            // Touch the file so TTL resets from now.
            let _ = fs::write(&cached_catalog_path, &body);
            let catalog: Catalog =
                toml::from_str(&body).context("Failed to parse cached catalog.toml")?;
            return Ok(Some(catalog));
        }
        // Cache file gone? Fall through to re-fetch without ETag.
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

/// Compute a deterministic content hash for a mold.
///
/// - Flat file (`name.py`): SHA-256 of the file content.
/// - Directory (`name/`): collect all files recursively, sort paths alphabetically,
///   build `path:{sha256(content)}|…`, SHA-256 the concatenation.
///
/// Returns a hex string truncated to 16 characters.
fn compute_mold_hash(base: &Path, rel_path: &str) -> Result<String> {
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
        let digest = crate::paths::sha256_hex(&content);
        return Ok(digest[..32].to_string());
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
        let file_hash = crate::paths::sha256_hex(content);
        if !combined.is_empty() {
            combined.push('|');
        }
        combined.push_str(&format!("{path}:{file_hash}"));
    }

    let digest = crate::paths::sha256_hex(combined.as_bytes());
    Ok(digest[..32].to_string())
}

/// Recursively collect files under `dir`, storing paths relative to `prefix`.
fn collect_companion_files(dir: &Path, prefix: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let rel = prefix.join(entry.file_name());
        if path.is_dir() {
            collect_companion_files(&path, &rel, out);
        } else {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// Build or rebuild `catalog.toml` for a local registry.
pub fn build_catalog(registry_name: Option<&str>, direct_path: Option<&str>) -> Result<()> {
    let base: String = if let Some(p) = direct_path {
        let path = Path::new(p);
        if !path.is_dir() {
            bail!("Path '{p}' is not a directory");
        }
        p.to_string()
    } else {
        let name = registry_name.expect("either name or --path must be provided");
        let cfg = load_config()?;

        let source = cfg.sources.get(name).ok_or_else(|| {
            anyhow::anyhow!(
                "Registry '{name}' not found. Use 'fimod registry list' to see configured registries."
            )
        })?;

        if source.kind != SourceType::Local {
            bail!(
                "Registry '{}' is of type '{}'; build-catalog only works for local registries.",
                name,
                source.kind
            );
        }

        source
            .path
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Local registry has no path configured"))?
            .to_string()
    };
    let base = &base;

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

        // Collect companion files (templates, data, etc.) — everything in the
        // mold directory except the main script and README.md.
        let files: Vec<String> = Path::new(rel_path)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|mold_dir| {
                let abs_dir = Path::new(base).join(mold_dir);
                let mut companion = Vec::new();
                collect_companion_files(&abs_dir, mold_dir, &mut companion);
                let script_rel = rel_path.replace('\\', "/");
                let readme_rel = readme.as_deref().unwrap_or("");
                companion.retain(|f| f != &script_rel && f != readme_rel);
                companion.sort();
                companion
            })
            .unwrap_or_default();

        catalog.molds.insert(
            name.clone(),
            CatalogEntry {
                description: effective_description(&defaults),
                docs: None,
                path: Some(rel_path.clone()),
                readme,
                input_format: defaults.input_format,
                output_format: defaults.output_format,
                options,
                args,
                envs,
                hash: mold_hash,
                files,
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
pub(crate) fn github_to_raw(url: &str) -> Result<String> {
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
