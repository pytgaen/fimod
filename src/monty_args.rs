//! Small helpers for extracting typed values out of `MontyObject` arguments
//! passed to external Python functions.

use anyhow::{bail, Result};
use monty::MontyObject;

/// Extract a `&str` from a `MontyObject::String`, with a label for the error
/// message. Used by external functions (`re_*`, `hs_*`, `gk_*`, …) to validate
/// their arguments uniformly.
pub(crate) fn expect_string<'a>(obj: &'a MontyObject, label: &str) -> Result<&'a str> {
    match obj {
        MontyObject::String(s) => Ok(s.as_str()),
        _ => bail!("{label} must be a string, got {obj:?}"),
    }
}

/// Like [`expect_string`] but returns an owned `String` (clones from the
/// underlying `MontyObject::String` storage). For callers that need to store
/// the value past the lifetime of the `args` slice.
pub(crate) fn expect_string_owned(obj: &MontyObject, label: &str) -> Result<String> {
    match obj {
        MontyObject::String(s) => Ok(s.clone()),
        _ => bail!("{label} must be a string, got {obj:?}"),
    }
}
