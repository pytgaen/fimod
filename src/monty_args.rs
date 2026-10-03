//! Small helpers for extracting typed values out of `MontyObject` arguments
//! passed to external Python functions.

use anyhow::Result;
use monty_types::MontyObject;

/// Extract a `&str` from a `MontyObject::string`, with a label for the error
/// message. Used by external functions (`re_*`, `hs_*`, `gk_*`, …) to validate
/// their arguments uniformly.
pub(crate) fn expect_string<'a>(obj: &'a MontyObject, label: &str) -> Result<&'a str> {
    obj.as_ref()
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("{label} must be a string, got {obj:?}"))
}

pub(crate) fn expect_string_owned(obj: &MontyObject, label: &str) -> Result<String> {
    expect_string(obj, label).map(str::to_owned)
}

/// Borrow a dictionary or host-object attribute without copying its graph.
pub(crate) fn field<'a>(
    obj: monty_types::ObjectRef<'a>,
    key: &str,
) -> Option<monty_types::ObjectRef<'a>> {
    use monty_types::unstable::{self, MontyNode};
    let pairs = match unstable::node(obj) {
        MontyNode::Dict(pairs) | MontyNode::ClassInstance { attrs: pairs, .. } => pairs,
        MontyNode::ClassType(class) => &class.attrs,
        _ => return None,
    };
    pairs.iter().find_map(|(k, v)| {
        (unstable::child(obj, *k).as_str() == Some(key)).then(|| unstable::child(obj, *v))
    })
}
