use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

/// A compiled mold step ready for execution.
#[derive(Clone)]
pub struct MoldStep {
    /// Where this step's script came from. Single source of truth for
    /// label, base directory, and reload semantics.
    pub source: MoldSource,
    pub script: String,
    pub defaults: MoldDefaults,
    /// Per-step args provided via `Step.create(args={...})` from a prior step's
    /// `pipeline.insert_next` / `pipeline.append`. Merged with CLI `--arg`
    /// (step values win on key conflict). `None` for steps not injected with
    /// explicit args (they receive only CLI args, current behaviour).
    pub runtime_args: Option<serde_json::Value>,
    /// Where this step came from in the pipeline: CLI args or runtime injection.
    pub origin: StepOrigin,
}

/// Origin of a step in the pipeline — distinguishes CLI-supplied steps from
/// those injected at runtime by a parent mold via `pipeline.insert_next` /
/// `pipeline.append`.
#[derive(Clone, Debug)]
pub enum StepOrigin {
    /// Provided via CLI args (`-m` / `-e`).
    Cli,
    /// Injected at runtime by a parent mold. `parent_step` is 0-based.
    Injected { parent_step: usize },
}

impl MoldStep {
    pub fn label(&self) -> String {
        self.source.label()
    }

    pub fn base_dir(&self) -> Option<String> {
        self.source.base_dir()
    }

    /// Error context line: `step N/M (label)` or
    /// `step N/M (label, injected by step P)`.
    /// `idx` is 0-based; `total` is the chain length.
    pub fn error_context(&self, idx: usize, total: usize) -> String {
        let label = self.label();
        match &self.origin {
            StepOrigin::Cli => format!("step {}/{} ({})", idx + 1, total, label),
            StepOrigin::Injected { parent_step } => format!(
                "step {}/{} ({}, injected by step {})",
                idx + 1,
                total,
                label,
                parent_step + 1
            ),
        }
    }
}

/// Truncate a string to `head + '…' + tail` if it exceeds `head + tail` chars.
fn truncate_middle(s: &str, head: usize, tail: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= head + tail {
        return s.to_string();
    }
    let h: String = chars.iter().take(head).collect();
    let t: String = chars
        .iter()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{h}…{t}")
}

/// Where the mold script comes from.
///
/// Each non-inline variant carries a `display_ref`: the form the user originally
/// typed (e.g. `@common/flatten`, `./local.py`, full URL). It is set by
/// [`MoldSource::from_mold_str`] and surfaces in error messages and `--debug`
/// output via [`MoldSource::label`]. For sources constructed elsewhere (e.g.
/// directly from `registry.rs`) `display_ref` defaults to the resolved path/URL.
#[derive(Debug, Clone)]
pub enum MoldSource {
    /// Local file path.
    File {
        /// Resolved absolute or relative path to the script on disk.
        path: String,
        /// User-facing reference (what was typed): `./local.py`, `@local/x`,
        /// or absolute path. Defaults to `path` when not provided.
        display_ref: String,
    },
    /// HTTP/HTTPS URL.
    ///
    /// `token` is populated from a named registry's `token_env` override
    /// or default `GITHUB_TOKEN`/`GITLAB_TOKEN` for known domains.
    /// `catalog_hash` is present only for registry-based molds.
    /// `companion_files` are listed in catalog.toml and fetched into the
    /// same cache directory.
    Url {
        url: String,
        token: Option<String>,
        catalog_hash: Option<String>,
        companion_files: Vec<String>,
        /// User-facing reference: `@registry/name` for catalog refs, or the
        /// URL itself when passed directly via `-m https://...`.
        display_ref: String,
    },
    /// Inline expression passed via `-e`.
    Inline(String),
}

impl std::fmt::Display for MoldSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MoldSource::File { path, .. } => write!(f, "file({path})"),
            MoldSource::Url { url, .. } => write!(f, "url({url})"),
            MoldSource::Inline(_) => write!(f, "inline(-e)"),
        }
    }
}

impl MoldSource {
    /// Construct a `File` source. `display_ref` defaults to `path`; use
    /// [`MoldSource::with_display_ref`] to override after construction.
    pub fn file(path: String) -> Self {
        Self::File {
            display_ref: path.clone(),
            path,
        }
    }

    /// True if `s` refers to a local mold script (not `@registry/...`, not an
    /// HTTP URL). Used to decide what to watch and to skip non-file references.
    pub fn is_local_path(s: &str) -> bool {
        !s.starts_with('@') && !crate::http::is_url(s)
    }

    /// Short, human-readable label for error messages and debug output.
    ///
    /// Examples:
    /// - `-m @flatten`     → `@flatten`
    /// - `-m ./local.py`   → `./local.py`
    /// - `-m https://x`    → `https://x`
    /// - `-e 'data + 1'`   → `-e 'data + 1'`
    /// - `-e '<long…>'`    → `-e '<25 chars>…<25 chars>'` (when expr > 50 chars)
    pub fn label(&self) -> String {
        match self {
            MoldSource::File { display_ref, .. } | MoldSource::Url { display_ref, .. } => {
                truncate_middle(display_ref, 30, 30)
            }
            MoldSource::Inline(expr) => format!("-e '{}'", truncate_middle(expr, 25, 25)),
        }
    }

    /// Override `display_ref` (no-op for `Inline`). Used by `from_mold_str`
    /// to preserve the user-typed form (`@registry/name`) after delegating
    /// resolution to the registry.
    fn with_display_ref(mut self, new_ref: String) -> Self {
        match &mut self {
            MoldSource::File { display_ref, .. } | MoldSource::Url { display_ref, .. } => {
                *display_ref = new_ref;
            }
            MoldSource::Inline(_) => {}
        }
        self
    }
}

/// Resolve a directory path to a mold script.
///
/// Standard 3-rule local-mold script lookup under `base/`.
///
/// Tries in order:
/// 1. `base/<name>.py`              (flat; nested names → `base/foo/bar.py`)
/// 2. `base/<name>/<last(name)>.py` (directory, named script)
/// 3. `base/<name>/__main__.py`     (directory, __main__)
///
/// Returns the first existing path or `None`. The caller decides whether
/// the absence is an error and supplies any error context.
pub fn find_script(base: &Path, name: &str) -> Option<PathBuf> {
    let last = name.split('/').next_back().unwrap_or(name);
    [
        base.join(format!("{name}.py")),
        base.join(name).join(format!("{last}.py")),
        base.join(name).join("__main__.py"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// Lookup order:
/// 1. `<dir>/<dirname>.py` (convention: script named after the mold directory)
/// 2. `<dir>/__main__.py`  (Python package convention)
/// 3. The single `.py` file present in `<dir>` (unambiguous fallback)
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

    // 3. Single .py in directory
    let mut py_files: Vec<_> = fs::read_dir(dir)
        .with_context(|| format!("Cannot read directory: {}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "py"))
        .collect();
    py_files.sort_by_key(|e| e.file_name());

    match py_files.len() {
        1 => Ok(py_files[0].path().to_string_lossy().into_owned()),
        0 => bail!("no .py script found in {dir_name}/"),
        _ => {
            let names: Vec<_> = py_files
                .iter()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            bail!(
                "ambiguous mold directory: {dir_name}/ contains multiple .py files ({})\npass the script path directly with -m",
                names.join(", ")
            )
        }
    }
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
        // Registry reference (@name or @source/name) — preserve the typed form.
        if let Some(spec) = s.strip_prefix('@') {
            let inner = crate::registry::resolve(spec, no_cache)?;
            return Ok(inner.with_display_ref(s.to_string()));
        }

        // Direct URL — auto-detect auth token from domain
        if crate::http::is_url(s) {
            let token = crate::registry::token_for_url(s);
            return Ok(Self::Url {
                url: s.to_string(),
                token,
                catalog_hash: None,
                companion_files: vec![],
                display_ref: s.to_string(),
            });
        }

        // Local path or directory
        let path = Path::new(s);
        if path.is_dir() {
            let resolved = resolve_directory_mold(path)
                .with_context(|| format!("No mold script found in directory: {s}"))?;
            Ok(Self::File {
                path: resolved,
                display_ref: s.to_string(),
            })
        } else {
            Ok(Self::File {
                path: s.to_string(),
                display_ref: s.to_string(),
            })
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
            MoldSource::File { path, .. } => {
                fs::read_to_string(path).with_context(|| format!("Mold not found: {path}"))
            }
            MoldSource::Url {
                url,
                token,
                catalog_hash,
                companion_files,
                ..
            } => {
                #[cfg(feature = "reqwest")]
                {
                    load_url_with_cache(
                        url,
                        token.as_deref(),
                        catalog_hash.as_deref(),
                        companion_files,
                        no_cache,
                    )
                }
                #[cfg(not(feature = "reqwest"))]
                {
                    let _ = (token, catalog_hash, companion_files, no_cache);
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
                        "def transform(data, args, env, headers, pipeline, **_):\n    return {expr}"
                    ))
                }
            }
        }
    }

    /// Return the base directory for resolving relative paths from this mold.
    pub fn base_dir(&self) -> Option<String> {
        match self {
            MoldSource::File { path, .. } => Path::new(path)
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map(|p| p.to_string_lossy().into_owned()),
            MoldSource::Url {
                url, catalog_hash, ..
            } => {
                #[cfg(feature = "reqwest")]
                {
                    let cache_base = crate::paths::cache_dir();
                    let url_hash = crate::paths::sha256_hex(url.as_bytes());
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
    companion_urls: &[String],
    no_cache: bool,
) -> Result<String> {
    let cache_base = crate::paths::cache_dir();
    let url_hash = crate::paths::sha256_hex(url.as_bytes());

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
        if let Err(e) = write_hashed_mold_cache(
            &cache_script_path,
            &cache_hash_path,
            &content,
            expected_hash,
        ) {
            eprintln!("[fimod] warning: could not write mold cache: {e:#}");
        }

        // Fetch companion files (templates, data, etc.) into the cache dir.
        fetch_companion_files(&mold_cache_dir, url, companion_urls, token);

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
        if let Err(e) = write_cache_file_atomic(&cache_path, content.as_bytes()) {
            eprintln!("[fimod] warning: could not write mold cache: {e}");
        }
    }

    Ok(content)
}

#[cfg(feature = "reqwest")]
fn write_hashed_mold_cache(
    cache_script_path: &Path,
    cache_hash_path: &Path,
    content: &str,
    expected_hash: &str,
) -> Result<()> {
    write_cache_file_atomic(cache_script_path, content.as_bytes())?;
    write_cache_file_atomic(cache_hash_path, expected_hash.as_bytes())
}

#[cfg(feature = "reqwest")]
fn write_cache_file_atomic(path: &Path, content: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create cache directory: {}", parent.display()))?;
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("cache");
    let pid = std::process::id();
    let tmp_path = path.with_file_name(format!(".{file_name}.tmp.{pid}"));

    fs::write(&tmp_path, content)
        .with_context(|| format!("Failed to write cache temp file: {}", tmp_path.display()))?;

    if let Err(rename_err) = fs::rename(&tmp_path, path) {
        if path.exists() {
            if let Err(remove_err) = fs::remove_file(path) {
                let _ = fs::remove_file(&tmp_path);
                return Err(rename_err).with_context(|| {
                    format!(
                        "Failed to replace cache file: {} (also failed to remove existing file: {remove_err})",
                        path.display()
                    )
                });
            }

            if let Err(second_err) = fs::rename(&tmp_path, path) {
                let _ = fs::remove_file(&tmp_path);
                return Err(second_err)
                    .with_context(|| format!("Failed to replace cache file: {}", path.display()));
            }

            return Ok(());
        }

        let _ = fs::remove_file(&tmp_path);
        return Err(rename_err)
            .with_context(|| format!("Failed to install cache file: {}", path.display()));
    }

    Ok(())
}

/// Download companion files (templates, data) into the mold cache directory.
/// Each companion URL is resolved relative to the script URL's parent directory.
#[cfg(feature = "reqwest")]
fn fetch_companion_files(
    cache_dir: &std::path::Path,
    script_url: &str,
    companion_urls: &[String],
    token: Option<&str>,
) {
    // Compute the base URL (parent of the script URL)
    let script_base = script_url
        .rfind('/')
        .map(|i| &script_url[..i])
        .unwrap_or(script_url);

    for companion_url in companion_urls {
        // Derive relative path from companion URL vs script base
        let rel_path = companion_url
            .strip_prefix(&format!("{script_base}/"))
            .unwrap_or(companion_url);

        let target = match companion_cache_path(cache_dir, rel_path) {
            Ok(target) => target,
            Err(e) => {
                eprintln!("[fimod] warning: skipping unsafe companion file '{rel_path}': {e}");
                continue;
            }
        };

        match fetch_mold_content(companion_url, token) {
            Ok(content) => {
                if let Err(e) = write_cache_file_atomic(&target, content.as_bytes()) {
                    eprintln!(
                        "[fimod] warning: could not write companion file '{rel_path}': {e:#}"
                    );
                }
            }
            Err(e) => {
                eprintln!("[fimod] warning: could not fetch companion file '{rel_path}': {e:#}");
            }
        }
    }
}

#[cfg(feature = "reqwest")]
fn companion_cache_path(cache_dir: &Path, rel_path: &str) -> Result<PathBuf> {
    let bytes = rel_path.as_bytes();
    let has_windows_drive_prefix =
        bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';

    if rel_path.starts_with('/') || rel_path.starts_with('\\') || has_windows_drive_prefix {
        bail!("unsafe companion file path: {rel_path:?}");
    }

    let mut safe_path = PathBuf::new();
    for segment in rel_path.split(['/', '\\']) {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            bail!("unsafe companion file path: {rel_path:?}");
        }
        safe_path.push(segment);
    }

    if safe_path.as_os_str().is_empty() {
        bail!("unsafe companion file path: {rel_path:?}");
    }

    Ok(cache_dir.join(safe_path))
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
        request = request.bearer_auth(t);
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
#[derive(Debug, Default, Clone)]
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
    /// Typed `--arg` declarations extracted from `arg=name:type...` directives.
    pub arg_specs: Vec<MoldArgSpec>,
    /// Documented ENV variables: (var_name, optional description)
    pub envs: Vec<(String, Option<String>)>,
    /// Names of directives declared with `!=` (forced — not overridable by the CLI).
    pub forced: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoldArgSpec {
    pub name: String,
    pub arg_type: MoldArgType,
    pub optional: bool,
    pub default: Option<String>,
}

impl MoldArgSpec {
    pub fn declaration(&self) -> String {
        let mut out = format!("{}:{}", self.name, self.arg_type.label());
        if self.optional {
            out.push('?');
        }
        if let Some(default) = &self.default {
            out.push('=');
            out.push_str(default);
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoldArgType {
    Str,
    Int,
    Float,
    Bool,
    Json,
    Unknown(String),
}

impl MoldArgType {
    pub fn parse(raw: &str) -> Self {
        match raw {
            "str" => Self::Str,
            "int" => Self::Int,
            "float" => Self::Float,
            "bool" => Self::Bool,
            "json" => Self::Json,
            other => Self::Unknown(other.to_string()),
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Str => "str",
            Self::Int => "int",
            Self::Float => "float",
            Self::Bool => "bool",
            Self::Json => "json",
            Self::Unknown(raw) => raw,
        }
    }
}

/// Parse `# fimod:` directives from the script preamble, and extract the
/// module-level docstring (`"""..."""` or `'''...'''`) if present.
///
/// Layout supported:
/// ```python
/// """Free-form docs (multi-line)."""
/// # fimod: input-format=csv
/// # fimod: arg=name  Description
/// def transform(data, args, **_): ...
/// ```
///
/// Directives are scanned in contiguous comment lines that follow the docstring
/// (or from the start of the script when there is no docstring).
/// Syntax: `# fimod: key=value, key2=value2` or `# fimod: key` for bools.
/// Split a `# fimod:` directive line on commas, respecting quoted strings and
/// bracketed values such as JSON defaults.
/// Supports `"..."` and `'...'` with backslash escapes (`\"`, `\'`, `\\`).
fn split_directives(input: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut start = 0;
    let bytes = input.as_bytes();
    let mut i = 0;
    let mut in_quote: Option<u8> = None;
    let mut depth = 0usize;

    while i < bytes.len() {
        match (bytes[i], in_quote) {
            (b'\\', Some(_)) => {
                i += 2; // skip escaped char
                continue;
            }
            (b'"' | b'\'', None) => in_quote = Some(bytes[i]),
            (q, Some(open)) if q == open => in_quote = None,
            (b'{' | b'[' | b'(', None) => depth += 1,
            (b'}' | b']' | b')', None) => depth = depth.saturating_sub(1),
            (b',', None) if depth == 0 => {
                items.push(&input[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    items.push(&input[start..]);
    items
}

fn split_arg_spec_desc(input: &str) -> (&str, Option<&str>) {
    let mut in_quote: Option<char> = None;
    let mut escaped = false;
    let mut depth = 0usize;

    for (idx, c) in input.char_indices() {
        if let Some(open) = in_quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == open {
                in_quote = None;
            }
            continue;
        }

        match c {
            '"' | '\'' => in_quote = Some(c),
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth = depth.saturating_sub(1),
            c if c.is_whitespace() && depth == 0 => {
                let desc = input[idx..].trim();
                return (
                    input[..idx].trim(),
                    if desc.is_empty() { None } else { Some(desc) },
                );
            }
            _ => {}
        }
    }

    (input.trim(), None)
}

/// Strip surrounding quotes and unescape `\"`, `\'`, `\\` in a description.
/// Returns the string as-is if not quoted.
fn unquote_desc(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let q = s.as_bytes()[0];
    if q != b'"' && q != b'\'' {
        return Some(s.to_string());
    }
    let inner = if s.len() >= 2 && s.as_bytes()[s.len() - 1] == q {
        &s[1..s.len() - 1]
    } else {
        &s[1..]
    };
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(next) if next == q as char || next == '\\' => out.push(next),
                Some(next) => {
                    out.push(c);
                    out.push(next);
                }
                None => out.push(c),
            }
        } else {
            out.push(c);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn parse_arg_directive(value: &str) -> Option<(String, Option<String>, Option<MoldArgSpec>)> {
    let (spec_raw, desc_raw) = split_arg_spec_desc(value);
    if spec_raw.is_empty() {
        return None;
    }
    let desc = desc_raw.and_then(unquote_desc);

    let Some((name_raw, type_raw)) = spec_raw.split_once(':') else {
        return Some((spec_raw.to_string(), desc, None));
    };
    let name = name_raw.trim();
    if name.is_empty() {
        return None;
    }

    let (type_part, default) = match type_raw.split_once('=') {
        Some((ty, default)) => (ty.trim(), Some(default.trim().to_string())),
        None => (type_raw.trim(), None),
    };
    let (type_name, optional) = match type_part.strip_suffix('?') {
        Some(ty) => (ty.trim(), true),
        None => (type_part, false),
    };
    if type_name.is_empty() {
        return Some((name.to_string(), desc, None));
    }

    let spec = MoldArgSpec {
        name: name.to_string(),
        arg_type: MoldArgType::parse(type_name),
        optional,
        default,
    };
    Some((name.to_string(), desc, Some(spec)))
}

/// Read the mold script at `path` and parse its `MoldDefaults`.
///
/// Errors propagate the IO failure with no extra context; callers add
/// their own (`.with_context()`, `.unwrap_or_default()`, `.ok()…`).
pub fn load_defaults(path: &Path) -> Result<MoldDefaults> {
    let script = fs::read_to_string(path)?;
    Ok(parse_mold_defaults(&script))
}

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
        for item in split_directives(rest) {
            let item = item.trim();
            if item.is_empty() {
                continue;
            }
            if let Some((key_raw, value)) = item.split_once('=') {
                let key_raw = key_raw.trim();
                let (key, forced) = if let Some(stripped) = key_raw.strip_suffix('!') {
                    (stripped, true)
                } else {
                    (key_raw, false)
                };
                let value = value.trim();
                if forced {
                    defaults.forced.insert(key.to_string());
                }
                match key {
                    "input-format" => defaults.input_format = Some(value.to_string()),
                    "output-format" => defaults.output_format = Some(value.to_string()),
                    "csv-delimiter" => defaults.csv_delimiter = Some(value.to_string()),
                    "csv-output-delimiter" => {
                        defaults.csv_output_delimiter = Some(value.to_string())
                    }
                    "csv-header" => defaults.csv_header = Some(value.to_string()),

                    "arg" => {
                        let Some((name, desc, spec)) = parse_arg_directive(value) else {
                            continue;
                        };
                        defaults.args.push((name, desc));
                        if let Some(spec) = spec {
                            if let Some(existing) =
                                defaults.arg_specs.iter_mut().find(|s| s.name == spec.name)
                            {
                                *existing = spec;
                            } else {
                                defaults.arg_specs.push(spec);
                            }
                        }
                    }
                    "env" => {
                        let (name, desc) = match value.split_once(|c: char| c.is_whitespace()) {
                            Some((n, d)) => {
                                let d = d.trim();
                                (
                                    n.trim().to_string(),
                                    if d.is_empty() { None } else { unquote_desc(d) },
                                )
                            }
                            None => (value.to_string(), None),
                        };
                        if !name.is_empty() {
                            defaults.envs.push((name, desc));
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
        assert!(matches!(src, MoldSource::File { ref path, .. } if path == "script.py"));
    }

    #[test]
    fn test_resolve_url_http() {
        let src = MoldSource::resolve(Some("http://example.com/m.py"), None).unwrap();
        assert!(matches!(src, MoldSource::Url { ref url, .. } if url == "http://example.com/m.py"));
    }

    #[test]
    fn test_resolve_url_https() {
        let src = MoldSource::resolve(Some("https://example.com/m.py"), None).unwrap();
        assert!(
            matches!(src, MoldSource::Url { ref url, .. } if url == "https://example.com/m.py")
        );
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
        assert!(script.contains("def transform(data, args, env, headers, pipeline, **_):"));
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

    // ─── forced directive (!=) tests ──────────────────────────────

    #[test]
    fn test_forced_output_format() {
        let script = "# fimod: output-format!=yaml\ndef transform(data):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.output_format.as_deref(), Some("yaml"));
        assert!(d.forced.contains("output-format"));
    }

    #[test]
    fn test_forced_input_format() {
        let script = "# fimod: input-format!=csv\ndef transform(data):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.input_format.as_deref(), Some("csv"));
        assert!(d.forced.contains("input-format"));
    }

    #[test]
    fn test_default_format_not_in_forced() {
        let script = "# fimod: output-format=json\ndef transform(data):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.output_format.as_deref(), Some("json"));
        assert!(!d.forced.contains("output-format"));
    }

    #[test]
    fn test_forced_mixed_with_default() {
        let script =
            "# fimod: output-format!=yaml, input-format=json\ndef transform(data):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.output_format.as_deref(), Some("yaml"));
        assert_eq!(d.input_format.as_deref(), Some("json"));
        assert!(d.forced.contains("output-format"));
        assert!(!d.forced.contains("input-format"));
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

    // ─── quoted description tests ───────────────────────────────

    #[test]
    fn test_arg_quoted_desc_with_commas() {
        let script = "# fimod: arg=build \"Build backend: hatchling, setuptools, flit\"\ndef transform(data, args, **_):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.args.len(), 1);
        assert_eq!(d.args[0].0, "build");
        assert_eq!(
            d.args[0].1.as_deref(),
            Some("Build backend: hatchling, setuptools, flit")
        );
    }

    #[test]
    fn test_arg_quoted_desc_single_quotes() {
        let script =
            "# fimod: arg=name 'It\\'s a name'\ndef transform(data, args, **_):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.args[0].1.as_deref(), Some("It's a name"));
    }

    #[test]
    fn test_arg_quoted_desc_with_escaped_quotes() {
        let script = "# fimod: arg=style \"Use \\\"compact\\\" or \\\"pretty\\\"\"\ndef transform(data, args, **_):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(
            d.args[0].1.as_deref(),
            Some("Use \"compact\" or \"pretty\"")
        );
    }

    #[test]
    fn test_arg_quoted_desc_mixed_quotes_no_escape() {
        let script = "# fimod: arg=style 'Use \"compact\" or \"pretty\"'\ndef transform(data, args, **_):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(
            d.args[0].1.as_deref(),
            Some("Use \"compact\" or \"pretty\"")
        );
    }

    #[test]
    fn test_arg_quoted_with_other_directives() {
        let script = "# fimod: arg=build \"hatchling, setuptools, flit\", output-format=json\ndef transform(data, args, **_):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.args.len(), 1);
        assert_eq!(d.args[0].1.as_deref(), Some("hatchling, setuptools, flit"));
        assert_eq!(d.output_format.as_deref(), Some("json"));
    }

    #[test]
    fn test_arg_unquoted_desc_unchanged() {
        let script =
            "# fimod: arg=build Build backend\ndef transform(data, args, **_):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.args[0].1.as_deref(), Some("Build backend"));
    }

    #[test]
    fn test_arg_typed_required() {
        let script =
            "# fimod: arg=threshold:int Minimum score\ndef transform(data, args, **_):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.args[0].0, "threshold");
        assert_eq!(d.args[0].1.as_deref(), Some("Minimum score"));
        assert_eq!(d.arg_specs.len(), 1);
        assert_eq!(d.arg_specs[0].name, "threshold");
        assert_eq!(d.arg_specs[0].arg_type, MoldArgType::Int);
        assert!(!d.arg_specs[0].optional);
        assert_eq!(d.arg_specs[0].default, None);
    }

    #[test]
    fn test_arg_typed_optional_with_default() {
        let script = "# fimod: arg=threshold:int?=10 \"Minimum score\"\ndef transform(data, args, **_):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.args[0].0, "threshold");
        assert_eq!(d.args[0].1.as_deref(), Some("Minimum score"));
        assert_eq!(d.arg_specs[0].arg_type, MoldArgType::Int);
        assert!(d.arg_specs[0].optional);
        assert_eq!(d.arg_specs[0].default.as_deref(), Some("10"));
        assert_eq!(d.arg_specs[0].declaration(), "threshold:int?=10");
    }

    #[test]
    fn test_arg_json_default_with_comma_does_not_split_directive() {
        let script = "# fimod: arg=filter:json?={\"a\":1,\"b\":2}, output-format=json\ndef transform(data, args, **_):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.output_format.as_deref(), Some("json"));
        assert_eq!(d.arg_specs[0].name, "filter");
        assert_eq!(d.arg_specs[0].arg_type, MoldArgType::Json);
        assert!(d.arg_specs[0].optional);
        assert_eq!(d.arg_specs[0].default.as_deref(), Some("{\"a\":1,\"b\":2}"));
    }

    #[test]
    fn test_env_quoted_desc() {
        let script = "# fimod: env=TOKEN \"API token, required\"\ndef transform(data, env, **_):\n    return data\n";
        let d = parse_mold_defaults(script);
        assert_eq!(d.envs[0].0, "TOKEN");
        assert_eq!(d.envs[0].1.as_deref(), Some("API token, required"));
    }

    #[cfg(feature = "reqwest")]
    #[test]
    fn test_hashed_cache_keeps_old_hash_when_script_write_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path().join("cache");
        fs::create_dir(&cache_dir).unwrap();

        let cache_script_path = cache_dir.join("mold.py");
        let cache_hash_path = cache_dir.join(".cache-hash");
        fs::create_dir(&cache_script_path).unwrap();
        fs::write(&cache_hash_path, "old-hash").unwrap();

        let err = write_hashed_mold_cache(
            &cache_script_path,
            &cache_hash_path,
            "def transform(data, **_): return data",
            "new-hash",
        )
        .unwrap_err();

        assert!(err.to_string().contains("Failed to replace cache file"));
        assert_eq!(fs::read_to_string(cache_hash_path).unwrap(), "old-hash");
        assert!(cache_script_path.is_dir());
    }

    #[cfg(feature = "reqwest")]
    #[test]
    fn test_companion_cache_path_rejects_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path().join("cache");

        let safe = companion_cache_path(&cache_dir, "templates/hello.j2").unwrap();
        assert_eq!(safe, cache_dir.join("templates").join("hello.j2"));

        let safe_windows_sep = companion_cache_path(&cache_dir, r"templates\hello.j2").unwrap();
        assert_eq!(
            safe_windows_sep,
            cache_dir.join("templates").join("hello.j2")
        );

        for path in [
            "../foo",
            "templates/../../foo",
            "/tmp/foo",
            r"\tmp\foo",
            r"C:\tmp\foo",
            "C:/tmp/foo",
        ] {
            let err = companion_cache_path(&cache_dir, path).unwrap_err();
            assert!(err.to_string().contains("unsafe companion file path"));
        }
    }

    #[cfg(feature = "reqwest")]
    #[test]
    fn test_fetch_mold_content_strips_auth_on_cross_origin_redirect() {
        let server_a = httpmock::MockServer::start();
        let server_b = httpmock::MockServer::start();

        let with_auth = server_b.mock(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/mold.py")
                .header_exists("authorization");
            then.status(200).body("with auth");
        });
        let without_auth = server_b.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/mold.py");
            then.status(200)
                .body("def transform(data, **_): return data");
        });

        let target_url = format!("{}/mold.py", server_b.base_url());
        let _redirect = server_a.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/redirect.py");
            then.status(302).header("location", &target_url);
        });

        let redirect_url = format!("{}/redirect.py", server_a.base_url());
        let content = fetch_mold_content(&redirect_url, Some("secret")).unwrap();

        assert_eq!(content, "def transform(data, **_): return data");
        with_auth.assert_calls(0);
        without_auth.assert_calls(1);
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
        assert!(err.to_string().contains("no .py script found in"));
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
            MoldSource::File { path, .. } => assert!(path.ends_with("converter/converter.py")),
            _ => panic!("expected MoldSource::File"),
        }
    }
}
