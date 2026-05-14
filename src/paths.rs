//! Shared filesystem path helpers.
//!
//! Centralizes the `~/.config/fimod/` resolution so registry, sandbox, and setup
//! can't drift apart on how the config root is computed.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// `~/.config/fimod/` — errors if neither `HOME` nor `USERPROFILE` is set.
pub fn config_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .context("HOME environment variable not set")?;
    Ok(Path::new(&home).join(".config").join("fimod"))
}

/// Base directory for all fimod caches: `$FIMOD_CACHE_DIR` or `~/.cache/fimod/`.
/// Falls back to `./.cache/fimod/` if neither `HOME` nor `USERPROFILE` is set
/// (so test runs in detached environments still get a usable path).
pub fn cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("FIMOD_CACHE_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".cache").join("fimod")
}

/// Hex-encoded SHA-256 of arbitrary bytes. Used to derive stable cache keys
/// (catalog hash, mold cache directories) from URLs or content.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}
