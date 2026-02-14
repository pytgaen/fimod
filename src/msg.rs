use anyhow::{bail, Result};
use monty::MontyObject;

/// Names of external functions exposed to Python molds.
pub const EXTERNAL_FUNCTIONS: &[&str] = &[
    "msg_print",
    "msg_info",
    "msg_warn",
    "msg_error",
    "msg_verbose",
    "msg_trace",
];

/// Dispatch an external function call to the appropriate msg handler.
///
/// `msg_level` controls which functions produce output:
/// - 0 (`--quiet`): errors only
/// - 1 (default): print, info, warn, error
/// - 2 (`--msg-level=verbose`): + verbose
/// - 3 (`--msg-level=trace`): + trace
pub fn dispatch(name: &str, args: Vec<MontyObject>, msg_level: u8) -> Result<MontyObject> {
    match name {
        "msg_print" => msg_fn(args, "msg_print", "", msg_level, 1),
        "msg_info" => msg_fn(args, "msg_info", "[INFO] ", msg_level, 1),
        "msg_warn" => msg_fn(args, "msg_warn", "[WARN] ", msg_level, 1),
        "msg_error" => msg_fn(args, "msg_error", "[ERROR] ", msg_level, 0),
        "msg_verbose" => msg_fn(args, "msg_verbose", "[VERBOSE] ", msg_level, 2),
        "msg_trace" => msg_fn(args, "msg_trace", "[TRACE] ", msg_level, 3),
        _ => bail!("Unknown msg function: {name}"),
    }
}

/// Print `prefix + text` to stderr if `msg_level >= min_level`.
fn msg_fn(
    args: Vec<MontyObject>,
    name: &str,
    prefix: &str,
    msg_level: u8,
    min_level: u8,
) -> Result<MontyObject> {
    if args.len() != 1 {
        bail!("{}() takes 1 argument (string), got {}", name, args.len());
    }
    let text = match &args[0] {
        MontyObject::String(s) => s.as_str(),
        _ => bail!("{name}() expects a string argument"),
    };
    if msg_level >= min_level {
        eprintln!("{prefix}{text}");
    }
    Ok(MontyObject::None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(val: &str) -> MontyObject {
        MontyObject::String(val.to_string())
    }

    #[test]
    fn test_dispatch_returns_none() {
        let result = dispatch("msg_print", vec![s("hello")], 1).unwrap();
        assert_eq!(result, MontyObject::None);
    }

    #[test]
    fn test_suppressed_returns_none() {
        // msg_verbose at level 1 → suppressed but still returns None (not an error)
        let result = dispatch("msg_verbose", vec![s("hi")], 1).unwrap();
        assert_eq!(result, MontyObject::None);
    }

    #[test]
    fn test_error_always_visible() {
        // msg_error at level 0 (--quiet) → still runs, returns None
        let result = dispatch("msg_error", vec![s("oh no")], 0).unwrap();
        assert_eq!(result, MontyObject::None);
    }

    #[test]
    fn test_wrong_type() {
        let result = dispatch("msg_print", vec![MontyObject::Int(42)], 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_arg_count() {
        let result = dispatch("msg_info", vec![s("a"), s("b")], 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_function() {
        let result = dispatch("msg_unknown", vec![s("a")], 1);
        assert!(result.is_err());
    }
}
