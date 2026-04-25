//! Shared filesystem path helpers.
//!
//! Centralizes the `~/.config/fimod/` resolution so registry, sandbox, and setup
//! can't drift apart on how the config root is computed.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// `~/.config/fimod/` — errors if neither `HOME` nor `USERPROFILE` is set.
pub(crate) fn config_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .context("HOME environment variable not set")?;
    Ok(Path::new(&home).join(".config").join("fimod"))
}
