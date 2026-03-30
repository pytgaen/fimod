use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};

/// A compiled mold step ready for execution.
pub struct MoldStep {
    pub display: String,
    pub script: String,
    pub defaults: MoldDefaults,
    /// Base directory for resolving relative paths (e.g., template files).
    /// `None` for inline expressions.
    pub base_dir: Option<String>,
}

/// Where the mold script comes from.
#[derive(Debug)]
pub enum MoldSource {
    /// Local file path.
    File(String),
    /// HTTP/HTTPS URL, with an optional Bearer token for authentication,
    /// and an optional catalog content hash for cache validation.
    ///
    /// The token is populated from:
    /// - A named registry's `token_env` override, or default `GITHUB_TOKEN`/`GITLAB_TOKEN`
    /// - Domain detection when the URL is passed directly via `-m https://...`
    ///
    /// The catalog hash is present only for registry-based molds; direct URLs have `None`.
    Url(String, Option<String>, Option<String>),
    /// Inline expression passed via `-e`.
    Inline(String),
}

impl std::fmt::Display for MoldSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MoldSource::File(path) => write!(f, "file({path})"),
            MoldSource::Url(url, _, _) => write!(f, "url({url})"),
            MoldSource::Inline(_) => write!(f, "inline(-e)"),
        }
    }
}

/// Resolve a directory path to a mold script.
///
/// Lookup order:
/// 1. `<dir>/<dirname>.py` (convention: script named after the mold directory)
/// 2. `<dir>/__main__.py`  (Python package convention)
fn resolve_directory_mold(dir: &Path) -> Result<String> {
    let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");

    // 1. <dirname>.py
    if !dir_name.is_empty() {
        let named = dir.join(format!("{dir_name}.py"));
        if named.is_file() {
            return Ok(named.to_string_lossy().into_owned());
        }
    }

    // 2. __main__.py
    let main = dir.join("__main__.py");
    if main.is_file() {
        return Ok(main.to_string_lossy().into_owned());
    }

    bail!("expected {dir_name}/{dir_name}.py or {dir_name}/__main__.py")
}

impl MoldSource {
    /// Resolve a single mold string to a MoldSource.
    ///
    /// Resolution rules (in priority order):
    /// - `@name`          → default registry lookup
    /// - `@source/name`   → named registry lookup
    /// - `http://...`     → URL (auto-injects `GITHUB_TOKEN`/`GITLAB_TOKEN` when applicable)
    /// - `https://...`    → URL (same)
    /// - `/abs/path`      → local file/directory
    /// - `./rel/path`     → local file/directory
    /// - `path`           → local file/directory
    pub fn from_mold_str(s: &str, no_cache: bool) -> Result<Self> {
        // Registry reference (@name or @source/name)
        if let Some(spec) = s.strip_prefix('@') {
            return crate::registry::resolve(spec, no_cache);
        }

        // Direct URL — auto-detect auth token from domain
        if s.starts_with("http://") || s.starts_with("https://") {
            let token = crate::registry::token_for_url(s);
            return Ok(Self::Url(s.to_string(), token, None));
        }

        // Local path or directory
        let path = Path::new(s);
        if path.is_dir() {
            let resolved = resolve_directory_mold(path)
                .with_context(|| format!("No mold script found in directory: {s}"))?;
            Ok(Self::File(resolved))
        } else {
            Ok(Self::File(s.to_string()))
        }
    }

    /// Resolve the mold source from CLI args.
    ///
    /// Exactly one of `mold` (`-m`) or `expression` (`-e`) must be provided.
    /// If `-m` starts with `http://` or `https://`, it's treated as a URL.
    #[cfg(test)]
    pub fn resolve(mold: Option<&str>, expression: Option<&str>) -> Result<Self> {
        match (mold, expression) {
            (Some(_), Some(_)) => {
                bail!("Cannot use both -m/--mold and -e/--expression at the same time")
            }
            (None, None) => {
                bail!("Either -m/--mold or -e/--expression is required")
            }
            (Some(m), None) => Self::from_mold_str(m, false),
            (None, Some(e)) => Ok(Self::Inline(e.to_string())),
        }
    }

    /// Load the mold script source code.
    pub fn load(&self, no_cache: bool) -> Result<String> {
        match self {
            MoldSource::File(path) => {
                fs::read_to_string(path).with_context(|| format!("Mold not found: {path}"))
            }
            MoldSource::Url(url, token, catalog_hash) => {
                #[cfg(feature = "reqwest")]
                {
                    load_url_with_cache(url, token.as_deref(), catalog_hash.as_deref(), no_cache)
                }
                #[cfg(not(feature = "reqwest"))]
                {
                    let _ = (token, catalog_hash, no_cache);
                    bail!(
                        "HTTP mold loading is not available (compiled with the 'slim' feature): {}",
                        url
                    )
                }
            }
            MoldSource::Inline(expr) => {
                // If the user already wrote `def transform`, use as-is
                if expr.contains("def transform") {
                    Ok(expr.to_string())
                } else {
                    // Auto-wrap: the expression becomes the return value
                    Ok(format!(
                        "def transform(data, args, env, headers):\n    return {expr}"
                    ))
                }
            }
        }
    }

    /// Return the base directory for resolving relative paths from this mold.
    pub fn base_dir(&self) -> Option<String> {
        match self {
            MoldSource::File(path) => Path::new(path)
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map(|p| p.to_string_lossy().into_owned()),
            MoldSource::Url(url, _, catalog_hash) => {
                #[cfg(feature = "reqwest")]
                {
                    use sha2::{Digest, Sha256};
                    let cache_base = crate::registry::cache_base_dir();
                    let url_hash = hex::encode(Sha256::digest(url.as_bytes()));
                    if catalog_hash.is_some() {
                        Some(
                            cache_base
                                .join("molds")
                                .join(&url_hash[..16])
                                .to_string_lossy()
                                .into_owned(),
                        )
                    } else {
                        Some(cache_base.join("molds").to_string_lossy().into_owned())
                    }
                }
                #[cfg(not(feature = "reqwest"))]
                {
                    let _ = (url, catalog_hash);
                    None
                }
            }
            MoldSource::Inline(_) => None,
        }
    }
}

/// Fetch a mold from a URL, using a local disk cache.
///
/// Two cache strategies:
/// - **With `catalog_hash`** (registry-based molds): hash-based validation.
///   The catalog provides a content hash; cache hit if local `.cache-hash` matches.
/// - **Without `catalog_hash`** (direct-URL molds): TTL-based validation.
///   Controlled by `FIMOD_CACHE_TTL` (minutes; default 360 = 6h; 0 = infinite; <0 = disabled).
///
/// `FIMOD_CACHE_DIR` overrides the cache base directory (default: `~/.cache/fimod/`).
///
/// If `token` is provided it is sent as `Authorization: Bearer <token>`.
/// Otherwise, `GITHUB_TOKEN` / `GITLAB_TOKEN` are automatically tried based on
/// the URL domain.
#[cfg(feature = "reqwest")]
fn load_url_with_cache(
    url: &str,
    token: Option<&str>,
    catalog_hash: Option<&str>,
    no_cache: bool,
) -> Result<String> {
    use sha2::{Digest, Sha256};

    let cache_base = crate::registry::cache_base_dir();
    let url_hash = hex::encode(Sha256::digest(url.as_bytes()));

    // ── hash-based cache (registry molds) ─────────────────────────────────
    if let Some(expected_hash) = catalog_hash {
        let mold_cache_dir = cache_base.join("molds").join(&url_hash[..16]);
        let cache_hash_path = mold_cache_dir.join(".cache-hash");
        let cache_script_path = mold_cache_dir.join("mold.py");

        // Try cache hit: hash matches and script file exists.
        if !no_cache {
            if let Ok(cached_hash) = fs::read_to_string(&cache_hash_path) {
                if cached_hash.trim() == expected_hash && cache_script_path.is_file() {
                    return fs::read_to_string(&cache_script_path).with_context(|| {
                        format!(
                            "Failed to read cached mold: {}",
                            cache_script_path.display()
                        )
                    });
                }
            }
        }

        // Cache miss — fetch and store.
        let content = fetch_mold_content(url, token)?;
        let _ = fs::create_dir_all(&mold_cache_dir);
        let _ = fs::write(&cache_script_path, &content);
        let _ = fs::write(&cache_hash_path, expected_hash);
        return Ok(content);
    }

    // ── TTL-based cache (direct-URL molds) ────────────────────────────────
    use std::time::SystemTime;

    let legacy_cache_dir = cache_base.join("molds");
    let ttl_minutes: i64 = std::env::var("FIMOD_CACHE_TTL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(360);
    let ttl = ttl_minutes * 60;

    let cache_path = legacy_cache_dir.join(format!("{url_hash}.py"));

    if !no_cache && ttl >= 0 && cache_path.is_file() {
        let fresh = if ttl == 0 {
            true
        } else {
            let stale = (|| -> Option<bool> {
                let mtime = cache_path.metadata().ok()?.modified().ok()?;
                let age = SystemTime::now().duration_since(mtime).ok()?;
                Some(age.as_secs() >= ttl as u64)
            })()
            .unwrap_or(true);
            !stale
        };

        if fresh {
            return fs::read_to_string(&cache_path)
                .with_context(|| format!("Failed to read cached mold: {}", cache_path.display()));
        }
    }

    let content = fetch_mold_content(url, token)?;

    if ttl >= 0 {
        if let Err(e) =
            fs::create_dir_all(&legacy_cache_dir).and_then(|_| fs::write(&cache_path, &content))
        {
            eprintln!("[fimod] warning: could not write mold cache: {e}");
        }
    }

    Ok(content)
}

/// Fetch a mold script from a URL with optional Bearer token.
#[cfg(feature = "reqwest")]
fn fetch_mold_content(url: &str, token: Option<&str>) -> Result<String> {
    let client = reqwest::blocking::Client::new();
    let mut request = client.get(url);

    let resolved_token = token
        .map(|t| t.to_string())
        .or_else(|| crate::registry::token_for_url(url));

    if let Some(ref t) = resolved_token {
        request = request.header("Authorization", format!("Bearer {t}"));
    }

    let resp = request
        .send()
        .with_context(|| format!("Failed to fetch mold from URL: {url}"))?;

    if !resp.status().is_success() {
        bail!("Failed to fetch mold from {}: HTTP {}", url, resp.status());
    }

    resp.text()
        .with_context(|| format!("Failed to read response body from: {url}"))
}

/// Defaults extracted from `# fimod:` directives in a mold script header.
#[derive(Debug, Default)]
pub struct MoldDefaults {
    pub input_format: Option<String>,
    pub output_format: Option<String>,
    pub csv_delimiter: Option<String>,
    pub csv_output_delimiter: Option<String>,
    pub csv_no_input_header: bool,
    pub csv_no_output_header: bool,
    pub csv_header: Option<String>,
    pub no_follow: bool,
    /// Free-form documentation extracted from the module-level docstring (`"""..."""`).
    pub docs: Option<String>,
    /// Documented --arg parameters: (name, optional description)
    pub args: Vec<(String, Option<String>)>,
    /// Documented ENV variables: (var_name, optional description)
    pub envs: Vec<(String, Option<String>)>,
}

/// Parse `# fimod:` directives from the script preamble, and extract the
/// module-level docstring (`"""..."""` or `'''...'''`) if present.
///
/// Layout supported:
/// ```python
/// """Free-form docs (multi-line)."""
/// # fimod: input-format=csv
/// # fimod: arg=name  Description
/// def transform(data, args, env, headers): ...
/// ```
///
/// Directives are scanned in contiguous comment lines that follow the docstring
/// (or from the start of the script when there is no docstring).
/// Syntax: `# fimod: key=value, key2=value2` or `# fimod: key` for bools.
pub fn parse_mold_defaults(script: &str) -> MoldDefaults {
    let mut defaults = MoldDefaults::default();
    let lines: Vec<&str> = script.lines().collect();
    let n = lines.len();
    let mut i = 0;

    // ── Phase 1: extract leading module docstring ──────────────────────────
    // Skip blank lines before the potential docstring.
    while i < n && lines[i].trim().is_empty() {
        i += 1;
    }
    if i < n {
        let trimmed = lines[i].trim();
        let quote = if trimmed.starts_with("\"\"\"") {
            Some("\"\"\"")
        } else if trimmed.starts_with("'''") {
            Some("'''")
        } else {
            None
        };
        if let Some(q) = quote {
            let after_open = &trimmed[q.len()..];
            if let Some(inner) = after_open.strip_suffix(q) {
                // Single-line: """content"""
                let content = inner.trim();
                if !content.is_empty() {
                    defaults.docs = Some(content.to_string());
                }
                i += 1;
            } else {
                // Multi-line docstring
                let mut doc_lines: Vec<String> = Vec::new();
                let first_content = after_open.trim();
                if !first_content.is_empty() {
                    doc_lines.push(first_content.to_string());
                }
                i += 1;
                while i < n {
                    let raw = lines[i];
                    i += 1;
                    let rstripped = raw.trim_end();
                    if let Some(before_close) = rstripped.strip_suffix(q) {
                        let content = before_close.trim();
                        if !content.is_empty() {
                            doc_lines.push(content.to_string());
                        }
                        break;
                    }
                    doc_lines.push(rstripped.to_string());
                }
                // Strip leading/trailing blank lines from body
                while doc_lines.first().map(|s| s.is_empty()).unwrap_or(false) {
                    doc_lines.remove(0);
                }
                while doc_lines.last().map(|s| s.is_empty()).unwrap_or(false) {
                    doc_lines.pop();
                }
                if !doc_lines.is_empty() {
                    defaults.docs = Some(doc_lines.join("\n"));
                }
            }
        }
    }

    // ── Phase 2: scan for # fimod: directives ─────────────────────────────
    while i < n {
        let trimmed = lines[i].trim();
        i += 1;
        if trimmed.is_empty() {
            continue;
        }
        if !trimmed.starts_with('#') {
            break;
        }
        let Some(rest) = trimmed.strip_prefix("# fimod:") else {
            continue;
        };
        let rest = rest.trim();
        if rest.is_empty() {
            continue;
        }
        for item in rest.split(',') {
            let item = item.trim();
            if item.is_empty() {
                continue;
            }
            if let Some((key, value)) = item.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                match key {
                    "input-format" => defaults.input_format = Some(value.to_string()),
                    "output-format" => defaults.output_format = Some(value.to_string()),
                    "csv-delimiter" => defaults.csv_delimiter = Some(value.to_string()),
                    "csv-output-delimiter" => {
                        defaults.csv_output_delimiter = Some(value.to_string())
                    }
                    "csv-header" => defaults.csv_header = Some(value.to_string()),

                    "arg" | "env" => {
                        let (name, desc) = match value.split_once(|c: char| c.is_whitespace()) {
                            Some((n, d)) => {
                                let d = d.trim();
                                (
                                    n.trim().to_string(),
                                    if d.is_empty() {
                                        None
                                    } else {
                                        Some(d.to_string())
                                    },
                                )
                            }
                            None => (value.to_string(), None),
                        };
                        if !name.is_empty() {
                            if key == "arg" {
                                defaults.args.push((name, desc));
                            } else {
                                defaults.envs.push((name, desc));
                            }
                        }
                    }
                    _ => {} // unknown key, ignore
                }
            } else {
                match item {
                    "csv-no-input-header" => defaults.csv_no_input_header = true,
                    "csv-no-output-header" => defaults.csv_no_output_header = true,
                    "no-follow" => defaults.no_follow = true,
                    _ => {} // unknown bool key, ignore
                }
            }
        }
    }

    defaults
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_file() {
        let src = MoldSource::resolve(Some("script.py"), None).unwrap();
        assert!(matches!(src, MoldSource::File(p) if p == "script.py"));
    }

    #[test]
    fn test_resolve_url_http() {
        let src = MoldSource::resolve(Some("http://example.com/m.py"), None).unwrap();
        assert!(matches!(src, MoldSource::Url(u, _, _) if u == "http://example.com/m.py"));
    }

    #[test]
    fn test_resolve_url_https() {
        let src = MoldSource::resolve(Some("https://example.com/m.py"), None).unwrap();
        assert!(matches!(src, MoldSource::Url(u, _, _) if u == "https://example.com/m.py"));
    }

    #[test]
    fn test_resolve_inline() {
        let src = MoldSource::resolve(None, Some("data['x'] = 1")).unwrap();
        assert!(matches!(src, MoldSource::Inline(_)));
    }

    #[test]
    fn test_resolve_error_both() {
        let err = MoldSource::resolve(Some("f.py"), Some("expr")).unwrap_err();
        assert!(err.to_string().contains("Cannot use both"));
    }

    #[test]
    fn test_resolve_error_neither() {
        let err = MoldSource::resolve(None, None).unwrap_err();
        assert!(err.to_string().contains("required"));
    }

    #[test]
    fn test_inline_auto_wrap() {
        let src = MoldSource::Inline("data['x'] + 1".to_string());
        let script = src.load(false).unwrap();
        assert!(script.contains("def transform(data, args, env, headers):"));
        assert!(script.contains("return data['x'] + 1"));
    }

    #[test]
    fn test_inline_no_wrap_if_def_transform() {
        let code = "def transform(data):\n    return data";
        let src = MoldSource::Inline(code.to_string());
        let script = src.load(false).unwrap();
        assert_eq!(script, code);
    }

    // ─── parse_mold_defaults tests ──────────────────────────

    #[test]
    fn test_defaults_basic_key_value() {
        let script =
            "# fimod: input-format=csv, csv-delimiter=;\ndef transform(data):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.input_format.as_deref(), Some("csv"));
        assert_eq!(d.csv_delimiter.as_deref(), Some(";"));
    }

    #[test]
    fn test_defaults_multi_lines() {
        let script = "# fimod: input-format=csv\n# fimod: output-format=json\ndef transform(data):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.input_format.as_deref(), Some("csv"));
        assert_eq!(d.output_format.as_deref(), Some("json"));
    }

    #[test]
    fn test_defaults_bool_flags() {
        let script = "# fimod: csv-no-input-header\ndef transform(data):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert!(d.csv_no_input_header);
    }

    #[test]
    fn test_defaults_empty_when_no_directives() {
        let script = "def transform(data):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert!(d.input_format.is_none());
        assert!(d.output_format.is_none());
    }

    #[test]
    fn test_defaults_ignores_directives_after_code() {
        let script = "def transform(data):\n    return data\n# fimod: output-format=yaml\n";
        let d = parse_mold_defaults(script);
        assert!(d.output_format.is_none());
    }

    #[test]
    fn test_defaults_skips_non_fimod_comments() {
        let script = "# This is a regular comment\n# fimod: no-follow\ndef transform(data):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert!(d.no_follow);
    }

    #[test]
    fn test_defaults_mixed_bools_and_values() {
        let script =
            "# fimod: output-format=yaml, no-follow\ndef transform(data):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.output_format.as_deref(), Some("yaml"));
        assert!(d.no_follow);
    }

    #[test]
    fn test_defaults_no_follow() {
        let script =
            "# fimod: input-format=http, no-follow\ndef transform(data):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.input_format.as_deref(), Some("http"));
        assert!(d.no_follow);
    }

    #[test]
    fn test_defaults_output_format_raw() {
        let script = "# fimod: output-format=raw\ndef transform(data):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.output_format.as_deref(), Some("raw"));
    }

    // ─── docstring tests ───────────────────────────────────────

    #[test]
    fn test_docstring_single_line() {
        let script = "\"\"\"Short description.\"\"\"\n# fimod: output-format=json\ndef transform(data, args, env, headers):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.docs.as_deref(), Some("Short description."));
        assert_eq!(d.output_format.as_deref(), Some("json"));
    }

    #[test]
    fn test_docstring_multi_line() {
        let script = "\"\"\"\nLine one.\n\nLine two.\n\"\"\"\n# fimod: no-follow\ndef transform(data, args, env, headers):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.docs.as_deref(), Some("Line one.\n\nLine two."));
        assert!(d.no_follow);
    }

    #[test]
    fn test_docstring_single_quote() {
        let script = "'''Single quote doc.'''\n# fimod: no-follow\ndef transform(data, args, env, headers):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.docs.as_deref(), Some("Single quote doc."));
        assert!(d.no_follow);
    }

    #[test]
    fn test_docstring_not_present() {
        let script = "# fimod: output-format=json\ndef transform(data, args, env, headers):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert!(d.docs.is_none());
        assert_eq!(d.output_format.as_deref(), Some("json"));
    }

    #[test]
    fn test_docstring_leading_blank_lines() {
        let script = "\n\n\"\"\"Doc with blank lines before.\"\"\"\n# fimod: no-follow\ndef transform(data, args, env, headers):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.docs.as_deref(), Some("Doc with blank lines before."));
        assert!(d.no_follow);
    }

    // ─── resolve_directory_mold tests ─────────────────────────

    #[test]
    fn test_resolve_dir_named_script() {
        let tmp = tempfile::tempdir().unwrap();
        let mold_dir = tmp.path().join("my_mold");
        fs::create_dir(&mold_dir).unwrap();
        fs::write(
            mold_dir.join("my_mold.py"),
            "def transform(data): return data",
        )
        .unwrap();

        let result = resolve_directory_mold(&mold_dir).unwrap();
        assert!(result.ends_with("my_mold/my_mold.py"));
    }

    #[test]
    fn test_resolve_dir_dunder_main() {
        let tmp = tempfile::tempdir().unwrap();
        let mold_dir = tmp.path().join("my_mold");
        fs::create_dir(&mold_dir).unwrap();
        fs::write(
            mold_dir.join("__main__.py"),
            "def transform(data): return data",
        )
        .unwrap();

        let result = resolve_directory_mold(&mold_dir).unwrap();
        assert!(result.ends_with("my_mold/__main__.py"));
    }

    #[test]
    fn test_resolve_dir_named_takes_priority() {
        let tmp = tempfile::tempdir().unwrap();
        let mold_dir = tmp.path().join("my_mold");
        fs::create_dir(&mold_dir).unwrap();
        fs::write(mold_dir.join("my_mold.py"), "named").unwrap();
        fs::write(mold_dir.join("__main__.py"), "main").unwrap();

        let result = resolve_directory_mold(&mold_dir).unwrap();
        assert!(result.ends_with("my_mold/my_mold.py"));
    }

    #[test]
    fn test_resolve_dir_no_script_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let mold_dir = tmp.path().join("empty_mold");
        fs::create_dir(&mold_dir).unwrap();

        let err = resolve_directory_mold(&mold_dir).unwrap_err();
        assert!(err.to_string().contains("__main__.py"));
    }

    #[test]
    fn test_resolve_mold_source_from_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let mold_dir = tmp.path().join("converter");
        fs::create_dir(&mold_dir).unwrap();
        fs::write(
            mold_dir.join("converter.py"),
            "def transform(data): return data",
        )
        .unwrap();

        let dir_str = mold_dir.to_str().unwrap();
        let src = MoldSource::resolve(Some(dir_str), None).unwrap();
        match src {
            MoldSource::File(p) => assert!(p.ends_with("converter/converter.py")),
            _ => panic!("expected MoldSource::File"),
        }
    }
}
