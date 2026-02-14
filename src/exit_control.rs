use std::sync::{Arc, Mutex};

use anyhow::{bail, Result};
use monty::MontyObject;

/// Names of external functions exposed to Python molds.
pub const EXTERNAL_FUNCTIONS: &[&str] = &["set_exit"];

/// Dispatch an external function call for exit code control.
pub fn dispatch(
    name: &str,
    args: Vec<MontyObject>,
    exit_code: &Arc<Mutex<Option<i32>>>,
) -> Result<MontyObject> {
    match name {
        "set_exit" => dispatch_set_exit(args, exit_code),
        _ => bail!("Unknown exit_control function: {name}"),
    }
}

/// set_exit(code) — stores the exit code (0-255) in the mutex, returns None.
fn dispatch_set_exit(
    args: Vec<MontyObject>,
    exit_code: &Arc<Mutex<Option<i32>>>,
) -> Result<MontyObject> {
    if args.len() != 1 {
        bail!(
            "set_exit() takes 1 argument (exit code), got {}",
            args.len()
        );
    }
    let code = match &args[0] {
        MontyObject::Int(i) => {
            if *i < 0 || *i > 255 {
                bail!("set_exit() code must be 0-255, got {i}");
            }
            *i as i32
        }
        _ => bail!("set_exit() expects an integer argument"),
    };

    let mut lock = exit_code.lock().unwrap();
    *lock = Some(code);
    Ok(MontyObject::None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_exit_stores_code() {
        let exit_code = Arc::new(Mutex::new(None));
        let result = dispatch("set_exit", vec![MontyObject::Int(42)], &exit_code).unwrap();
        assert_eq!(result, MontyObject::None);
        assert_eq!(*exit_code.lock().unwrap(), Some(42));
    }

    #[test]
    fn test_set_exit_out_of_range() {
        let exit_code = Arc::new(Mutex::new(None));
        assert!(dispatch("set_exit", vec![MontyObject::Int(256)], &exit_code).is_err());
        assert!(dispatch("set_exit", vec![MontyObject::Int(-1)], &exit_code).is_err());
    }

    #[test]
    fn test_set_exit_wrong_type() {
        let exit_code = Arc::new(Mutex::new(None));
        assert!(dispatch(
            "set_exit",
            vec![MontyObject::String("bad".to_string())],
            &exit_code
        )
        .is_err());
    }
}
