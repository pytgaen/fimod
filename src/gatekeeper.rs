use std::sync::{Arc, Mutex};

use anyhow::{bail, Result};
use monty::MontyObject;

/// Names of external functions exposed to Python molds.
pub const EXTERNAL_FUNCTIONS: &[&str] = &["gk_fail", "gk_assert", "gk_warn"];

/// Dispatch an external function call to the appropriate gatekeeper handler.
pub fn dispatch(
    name: &str,
    args: Vec<MontyObject>,
    exit_code: &Arc<Mutex<Option<i32>>>,
) -> Result<MontyObject> {
    match name {
        "gk_fail" => dispatch_fail(args, exit_code),
        "gk_assert" => dispatch_assert(args, exit_code),
        "gk_warn" => dispatch_warn(args),
        _ => bail!("Unknown gatekeeper function: {name}"),
    }
}

/// Python-style truthiness for MontyObject.
fn is_truthy(obj: &MontyObject) -> bool {
    match obj {
        MontyObject::None => false,
        MontyObject::Bool(b) => *b,
        MontyObject::Int(i) => *i != 0,
        MontyObject::Float(f) => *f != 0.0,
        MontyObject::String(s) => !s.is_empty(),
        MontyObject::List(l) => !l.is_empty(),
        // DictPairs doesn't expose len/is_empty — treat dicts as truthy.
        // In practice, gk_assert receives bools/None from comparisons and .get().
        _ => true,
    }
}

/// gk_fail(msg) — emit [ERROR] msg on stderr, set exit code to 1, return None.
fn dispatch_fail(
    args: Vec<MontyObject>,
    exit_code: &Arc<Mutex<Option<i32>>>,
) -> Result<MontyObject> {
    if args.len() != 1 {
        bail!("gk_fail() takes 1 argument (string), got {}", args.len());
    }
    let msg = match &args[0] {
        MontyObject::String(s) => s.as_str(),
        _ => bail!("gk_fail() expects a string argument"),
    };
    eprintln!("[ERROR] {msg}");
    let mut lock = exit_code.lock().unwrap();
    *lock = Some(1);
    Ok(MontyObject::None)
}

/// gk_assert(cond, msg) — if cond is falsy, behave like gk_fail(msg).
fn dispatch_assert(
    args: Vec<MontyObject>,
    exit_code: &Arc<Mutex<Option<i32>>>,
) -> Result<MontyObject> {
    if args.len() != 2 {
        bail!(
            "gk_assert() takes 2 arguments (condition, message), got {}",
            args.len()
        );
    }
    let msg = match &args[1] {
        MontyObject::String(s) => s.as_str(),
        _ => bail!("gk_assert() expects a string as second argument"),
    };
    if !is_truthy(&args[0]) {
        eprintln!("[ERROR] {msg}");
        let mut lock = exit_code.lock().unwrap();
        *lock = Some(1);
    }
    Ok(MontyObject::None)
}

/// gk_warn(cond, msg) — if cond is falsy, emit [WARN] msg on stderr. No exit.
fn dispatch_warn(args: Vec<MontyObject>) -> Result<MontyObject> {
    if args.len() != 2 {
        bail!(
            "gk_warn() takes 2 arguments (condition, message), got {}",
            args.len()
        );
    }
    let msg = match &args[1] {
        MontyObject::String(s) => s.as_str(),
        _ => bail!("gk_warn() expects a string as second argument"),
    };
    if !is_truthy(&args[0]) {
        eprintln!("[WARN] {msg}");
    }
    Ok(MontyObject::None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(val: &str) -> MontyObject {
        MontyObject::String(val.to_string())
    }

    fn exit() -> Arc<Mutex<Option<i32>>> {
        Arc::new(Mutex::new(None))
    }

    #[test]
    fn test_fail_sets_exit_code() {
        let ec = exit();
        let result = dispatch("gk_fail", vec![s("boom")], &ec).unwrap();
        assert_eq!(result, MontyObject::None);
        assert_eq!(*ec.lock().unwrap(), Some(1));
    }

    #[test]
    fn test_assert_truthy_no_exit() {
        let ec = exit();
        dispatch("gk_assert", vec![MontyObject::Bool(true), s("msg")], &ec).unwrap();
        assert_eq!(*ec.lock().unwrap(), None);
    }

    #[test]
    fn test_assert_falsy_sets_exit() {
        let ec = exit();
        dispatch("gk_assert", vec![MontyObject::Bool(false), s("bad")], &ec).unwrap();
        assert_eq!(*ec.lock().unwrap(), Some(1));
    }

    #[test]
    fn test_assert_none_is_falsy() {
        let ec = exit();
        dispatch("gk_assert", vec![MontyObject::None, s("missing")], &ec).unwrap();
        assert_eq!(*ec.lock().unwrap(), Some(1));
    }

    #[test]
    fn test_assert_nonempty_string_is_truthy() {
        let ec = exit();
        dispatch("gk_assert", vec![s("hello"), s("msg")], &ec).unwrap();
        assert_eq!(*ec.lock().unwrap(), None);
    }

    #[test]
    fn test_assert_empty_string_is_falsy() {
        let ec = exit();
        dispatch("gk_assert", vec![s(""), s("empty")], &ec).unwrap();
        assert_eq!(*ec.lock().unwrap(), Some(1));
    }

    #[test]
    fn test_assert_zero_is_falsy() {
        let ec = exit();
        dispatch("gk_assert", vec![MontyObject::Int(0), s("zero")], &ec).unwrap();
        assert_eq!(*ec.lock().unwrap(), Some(1));
    }

    #[test]
    fn test_warn_falsy_no_exit() {
        let ec = exit();
        dispatch("gk_warn", vec![MontyObject::Bool(false), s("warn")], &ec).unwrap();
        assert_eq!(*ec.lock().unwrap(), None);
    }

    #[test]
    fn test_warn_truthy_silent() {
        let ec = exit();
        dispatch("gk_warn", vec![MontyObject::Bool(true), s("warn")], &ec).unwrap();
        assert_eq!(*ec.lock().unwrap(), None);
    }

    #[test]
    fn test_fail_wrong_arg_count() {
        let ec = exit();
        assert!(dispatch("gk_fail", vec![], &ec).is_err());
    }

    #[test]
    fn test_assert_wrong_arg_count() {
        let ec = exit();
        assert!(dispatch("gk_assert", vec![s("only one")], &ec).is_err());
    }

    #[test]
    fn test_fail_wrong_type() {
        let ec = exit();
        assert!(dispatch("gk_fail", vec![MontyObject::Int(1)], &ec).is_err());
    }
}
