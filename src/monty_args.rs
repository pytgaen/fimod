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
