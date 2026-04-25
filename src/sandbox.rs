//! Sandbox policy loader and resolver.
//!
//! Resolution order for `SandboxPolicy::resolve()`:
//! 1. `--sandbox-file <path>` CLI flag (explicit `""` forces zero authorization).
//! 2. `FIMOD_SANDBOX_FILE` env var (pointing at a missing file is a hard error).
//! 3. `~/.config/fimod/sandbox.toml` if present.
//! 4. Zero authorization: every capability denied, hard-coded safety limits applied.
//!
//! Hard-coded safety limits (`HARDCODED_MAX_DURATION`, `HARDCODED_MAX_MEMORY`) are
//! applied in all cases except when the policy file explicitly sets `"unlimited"`
//! or a higher value.

use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;

pub const HARDCODED_MAX_DURATION: Duration = Duration::from_secs(120);
pub const HARDCODED_MAX_MEMORY: usize = 1024 * 1024 * 1024;

pub const FIMOD_SANDBOX_FILE_ENV: &str = "FIMOD_SANDBOX_FILE";

/// A resolved sandbox policy used by the engine to gate OS calls and enforce limits.
#[derive(Debug, Clone)]
pub struct SandboxPolicy {
    pub allow_clock: bool,
    pub max_duration: Option<Duration>,
    pub max_memory: Option<usize>,
    pub allow_env: Vec<String>,
    pub source: PolicySource,
}

/// Where this policy was loaded from. Useful for error messages and debug output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicySource {
    /// No file loaded; zero-authorization defaults with hard-coded safety limits.
    ZeroAuth,
    /// Loaded from an explicit `--sandbox-file` path.
    CliFile(PathBuf),
    /// Loaded from the `FIMOD_SANDBOX_FILE` env var.
    EnvFile(PathBuf),
    /// Loaded from the canonical `~/.config/fimod/sandbox.toml` location.
    CanonicalFile(PathBuf),
}

#[derive(Debug, Deserialize)]
struct SandboxFile {
    sandbox: Option<SandboxTable>,
}

#[derive(Debug, Deserialize, Default)]
struct SandboxTable {
    allow_clock: Option<bool>,
    max_duration: Option<String>,
    max_memory: Option<String>,
    allow_env: Option<Vec<String>>,
}

impl SandboxPolicy {
    /// Zero-authorization policy: deny every capability, apply hard-coded safety limits.
    pub fn zero_authorization() -> Self {
        Self {
            allow_clock: false,
            max_duration: Some(HARDCODED_MAX_DURATION),
            max_memory: Some(HARDCODED_MAX_MEMORY),
            allow_env: Vec::new(),
            source: PolicySource::ZeroAuth,
        }
    }

    /// Resolve the effective policy following the 4-level precedence.
    ///
    /// `cli_file` is the raw string value of `--sandbox-file` when provided:
    /// - `None` → flag absent; fall back to env / canonical / zero-auth.
    /// - `Some("")` → explicit empty string; force zero authorization immediately.
    /// - `Some(path)` → load that file, error if missing.
    pub fn resolve(cli_file: Option<&str>) -> Result<Self> {
        if let Some(raw) = cli_file {
            if raw.is_empty() {
                return Ok(Self::zero_authorization());
            }
            let path = PathBuf::from(raw);
            return load_file(&path, PolicySource::CliFile(path.clone()))
                .with_context(|| format!("--sandbox-file {raw}"));
        }

        if let Ok(env_val) = env::var(FIMOD_SANDBOX_FILE_ENV) {
            if !env_val.is_empty() {
                let path = PathBuf::from(&env_val);
                if !path.exists() {
                    bail!("{FIMOD_SANDBOX_FILE_ENV} points to missing file: {env_val}");
                }
                return load_file(&path, PolicySource::EnvFile(path.clone()))
                    .with_context(|| format!("{FIMOD_SANDBOX_FILE_ENV}={env_val}"));
            }
        }

        if let Some(canonical) = canonical_path() {
            if canonical.exists() {
                return load_file(&canonical, PolicySource::CanonicalFile(canonical.clone()))
                    .with_context(|| format!("{}", canonical.display()));
            }
        }

        Ok(Self::zero_authorization())
    }

    /// Whether the env allowlist matches the given key (glob syntax: `*`, prefix, exact).
    pub fn env_allowed(&self, key: &str) -> bool {
        self.allow_env.iter().any(|pat| matches_glob(pat, key))
    }
}

fn canonical_path() -> Option<PathBuf> {
    crate::paths::config_dir()
        .ok()
        .map(|d| d.join("sandbox.toml"))
}

fn load_file(path: &Path, source: PolicySource) -> Result<SandboxPolicy> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read sandbox file: {}", path.display()))?;
    let parsed: SandboxFile = toml::from_str(&content)
        .with_context(|| format!("failed to parse sandbox TOML: {}", path.display()))?;

    let table = parsed.sandbox.unwrap_or_default();

    let max_duration = match table.max_duration.as_deref() {
        Some(s) => parse_duration(s).with_context(|| format!("max_duration: {s:?}"))?,
        None => Some(HARDCODED_MAX_DURATION),
    };
    let max_memory = match table.max_memory.as_deref() {
        Some(s) => parse_size(s).with_context(|| format!("max_memory: {s:?}"))?,
        None => Some(HARDCODED_MAX_MEMORY),
    };

    Ok(SandboxPolicy {
        allow_clock: table.allow_clock.unwrap_or(false),
        max_duration,
        max_memory,
        allow_env: table.allow_env.unwrap_or_default(),
        source,
    })
}

/// Parse a duration string like `"30s"`, `"2m"`, `"500ms"`, `"1h"`, `"unlimited"`.
///
/// Returns `Ok(None)` for `"unlimited"`, `Ok(Some(d))` otherwise.
pub fn parse_duration(s: &str) -> Result<Option<Duration>> {
    let trimmed = s.trim();
    if trimmed.eq_ignore_ascii_case("unlimited") {
        return Ok(None);
    }

    let (num_part, unit) = split_numeric_suffix(trimmed);
    let num: u64 = num_part.parse().map_err(|_| {
        anyhow!("invalid duration: {s:?} (expected N{{ms|s|m|h}} or \"unlimited\")")
    })?;

    let d = match unit.to_ascii_lowercase().as_str() {
        "ms" => Duration::from_millis(num),
        "s" | "" => Duration::from_secs(num),
        "m" => Duration::from_secs(num * 60),
        "h" => Duration::from_secs(num * 3600),
        other => bail!("invalid duration unit: {other:?} (expected ms|s|m|h)"),
    };
    Ok(Some(d))
}

/// Parse a size string like `"500KB"`, `"1GB"`, `"unlimited"`. Binary units (KiB/MiB/GiB) accepted.
///
/// Returns `Ok(None)` for `"unlimited"`, `Ok(Some(bytes))` otherwise.
pub fn parse_size(s: &str) -> Result<Option<usize>> {
    let trimmed = s.trim();
    if trimmed.eq_ignore_ascii_case("unlimited") {
        return Ok(None);
    }

    let (num_part, unit) = split_numeric_suffix(trimmed);
    let num: u64 = num_part
        .parse()
        .map_err(|_| anyhow!("invalid size: {s:?} (expected N{{B|KB|MB|GB}} or \"unlimited\")"))?;

    let multiplier: u64 = match unit.to_ascii_uppercase().as_str() {
        "" | "B" => 1,
        "KB" => 1_000,
        "KIB" => 1_024,
        "MB" => 1_000_000,
        "MIB" => 1_024 * 1_024,
        "GB" => 1_000_000_000,
        "GIB" => 1_024 * 1_024 * 1_024,
        other => bail!("invalid size unit: {other:?} (expected B|KB|MB|GB)"),
    };
    let bytes = num
        .checked_mul(multiplier)
        .ok_or_else(|| anyhow!("size overflow: {s:?}"))?;
    Ok(Some(usize::try_from(bytes).unwrap_or(usize::MAX)))
}

fn split_numeric_suffix(s: &str) -> (&str, &str) {
    let boundary = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let (num, unit) = s.split_at(boundary);
    (num, unit.trim())
}

/// Match a simple glob pattern against a string.
/// Supports `*` suffix (`FIMOD_*`), prefix (`*_TOKEN`), contains (`*INNER*`), or exact match.
fn matches_glob(pattern: &str, input: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let starts = pattern.starts_with('*');
    let ends = pattern.ends_with('*');
    let body = pattern.trim_matches('*');
    if body.is_empty() {
        return starts || ends;
    }
    match (starts, ends) {
        (true, true) => input.contains(body),
        (true, false) => input.ends_with(body),
        (false, true) => input.starts_with(body),
        (false, false) => input == pattern,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_units() {
        assert_eq!(
            parse_duration("500ms").unwrap(),
            Some(Duration::from_millis(500))
        );
        assert_eq!(
            parse_duration("30s").unwrap(),
            Some(Duration::from_secs(30))
        );
        assert_eq!(
            parse_duration("2m").unwrap(),
            Some(Duration::from_secs(120))
        );
        assert_eq!(
            parse_duration("1h").unwrap(),
            Some(Duration::from_secs(3600))
        );
        assert_eq!(parse_duration("unlimited").unwrap(), None);
        assert_eq!(parse_duration("UNLIMITED").unwrap(), None);
        assert_eq!(parse_duration("45").unwrap(), Some(Duration::from_secs(45)));
    }

    #[test]
    fn parse_duration_errors() {
        assert!(parse_duration("").is_err());
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("1d").is_err());
        assert!(parse_duration("-5s").is_err());
    }

    #[test]
    fn parse_size_units() {
        assert_eq!(parse_size("500").unwrap(), Some(500));
        assert_eq!(parse_size("500B").unwrap(), Some(500));
        assert_eq!(parse_size("2KB").unwrap(), Some(2_000));
        assert_eq!(parse_size("2KiB").unwrap(), Some(2_048));
        assert_eq!(parse_size("1MB").unwrap(), Some(1_000_000));
        assert_eq!(parse_size("1GB").unwrap(), Some(1_000_000_000));
        assert_eq!(parse_size("1GiB").unwrap(), Some(1_073_741_824));
        assert_eq!(parse_size("unlimited").unwrap(), None);
    }

    #[test]
    fn parse_size_errors() {
        assert!(parse_size("").is_err());
        assert!(parse_size("abc").is_err());
        assert!(parse_size("1TB").is_err());
    }

    #[test]
    fn glob_matching() {
        assert!(matches_glob("*", "anything"));
        assert!(matches_glob("FIMOD_*", "FIMOD_REGISTRY"));
        assert!(!matches_glob("FIMOD_*", "OTHER_VAR"));
        assert!(matches_glob("*_TOKEN", "GITHUB_TOKEN"));
        assert!(!matches_glob("*_TOKEN", "TOKEN_GITHUB"));
        assert!(matches_glob("*INNER*", "X_INNER_Y"));
        assert!(matches_glob("LANG", "LANG"));
        assert!(!matches_glob("LANG", "LANGUAGE"));
    }

    #[test]
    fn env_allowed_checks_all_patterns() {
        let policy = SandboxPolicy {
            allow_clock: false,
            max_duration: None,
            max_memory: None,
            allow_env: vec!["FIMOD_*".into(), "LANG".into()],
            source: PolicySource::ZeroAuth,
        };
        assert!(policy.env_allowed("FIMOD_REGISTRY"));
        assert!(policy.env_allowed("LANG"));
        assert!(!policy.env_allowed("HOME"));
    }

    #[test]
    fn zero_authorization_defaults() {
        let p = SandboxPolicy::zero_authorization();
        assert!(!p.allow_clock);
        assert_eq!(p.max_duration, Some(HARDCODED_MAX_DURATION));
        assert_eq!(p.max_memory, Some(HARDCODED_MAX_MEMORY));
        assert!(p.allow_env.is_empty());
        assert_eq!(p.source, PolicySource::ZeroAuth);
    }

    #[test]
    fn load_file_full_policy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.toml");
        std::fs::write(
            &path,
            r#"
[sandbox]
allow_clock = true
max_duration = "30s"
max_memory = "500MB"
allow_env = ["FIMOD_*", "LANG"]
"#,
        )
        .unwrap();
        let p = load_file(&path, PolicySource::CliFile(path.clone())).unwrap();
        assert!(p.allow_clock);
        assert_eq!(p.max_duration, Some(Duration::from_secs(30)));
        assert_eq!(p.max_memory, Some(500_000_000));
        assert_eq!(p.allow_env, vec!["FIMOD_*".to_string(), "LANG".to_string()]);
    }

    #[test]
    fn load_file_defaults_missing_fields_to_hardcoded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.toml");
        std::fs::write(&path, "[sandbox]\nallow_clock = true\n").unwrap();
        let p = load_file(&path, PolicySource::CliFile(path.clone())).unwrap();
        assert!(p.allow_clock);
        assert_eq!(p.max_duration, Some(HARDCODED_MAX_DURATION));
        assert_eq!(p.max_memory, Some(HARDCODED_MAX_MEMORY));
        assert!(p.allow_env.is_empty());
    }

    #[test]
    fn load_file_unlimited() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.toml");
        std::fs::write(
            &path,
            "[sandbox]\nmax_duration = \"unlimited\"\nmax_memory = \"unlimited\"\n",
        )
        .unwrap();
        let p = load_file(&path, PolicySource::CliFile(path.clone())).unwrap();
        assert_eq!(p.max_duration, None);
        assert_eq!(p.max_memory, None);
    }

    #[test]
    fn resolve_cli_empty_forces_zero_auth() {
        let p = SandboxPolicy::resolve(Some("")).unwrap();
        assert_eq!(p.source, PolicySource::ZeroAuth);
        assert!(!p.allow_clock);
    }

    #[test]
    fn resolve_cli_missing_file_errors() {
        let err = SandboxPolicy::resolve(Some("/nonexistent/path/sandbox.toml")).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("sandbox") || msg.contains("read"));
    }
}
