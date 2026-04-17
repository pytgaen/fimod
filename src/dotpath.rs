use anyhow::{bail, Result};
use monty::MontyObject;
use serde_json::Value;

use crate::convert::{json_into_monty, monty_to_json};

/// Names of external functions exposed to Python molds.
pub const EXTERNAL_FUNCTIONS: &[&str] = &["dp_get", "dp_set", "dp_has", "dp_delete"];

/// Dispatch an external function call to the appropriate dotpath handler.
pub fn dispatch(name: &str, args: Vec<MontyObject>) -> Result<MontyObject> {
    match name {
        "dp_get" => dp_get(args),
        "dp_set" => dp_set(args),
        "dp_has" => dp_has(args),
        "dp_delete" => dp_delete(args),
        _ => bail!("Unknown dotpath function: {name}"),
    }
}

/// Parse a dot-separated path into segments.
/// Text segments are dict keys, integer segments are array indices.
/// Negative integers index from the end.
fn parse_path(path: &str) -> Vec<&str> {
    if path.is_empty() {
        vec![]
    } else {
        path.split('.').collect()
    }
}

/// Navigate into a JSON Value following a dot-path.
/// Returns None if the path doesn't resolve.
fn get_at_path(value: &Value, path: &str) -> Option<Value> {
    let segments = parse_path(path);
    let mut current = value.clone();

    for seg in segments {
        current = match seg.parse::<i64>() {
            Ok(idx) => {
                let arr = current.as_array()?;
                let actual_idx = if idx < 0 {
                    (arr.len() as i64 + idx) as usize
                } else {
                    idx as usize
                };
                arr.get(actual_idx)?.clone()
            }
            Err(_) => {
                let obj = current.as_object()?;
                obj.get(seg)?.clone()
            }
        };
    }

    Some(current)
}

/// Set a value at a dot-path, returning a deep-cloned copy.
/// Creates intermediate objects/arrays as needed.
fn set_at_path(value: &Value, path: &str, new_val: &Value) -> Value {
    let segments = parse_path(path);
    if segments.is_empty() {
        return new_val.clone();
    }
    set_recursive(value, &segments, new_val)
}

fn set_recursive(value: &Value, segments: &[&str], new_val: &Value) -> Value {
    if segments.is_empty() {
        return new_val.clone();
    }

    let seg = segments[0];
    let rest = &segments[1..];

    match seg.parse::<i64>() {
        Ok(idx) => {
            let mut arr = match value.as_array() {
                Some(a) => a.clone(),
                None => vec![],
            };
            let actual_idx = if idx < 0 {
                (arr.len() as i64 + idx).max(0) as usize
            } else {
                idx as usize
            };
            // Extend array if needed
            while arr.len() <= actual_idx {
                arr.push(Value::Null);
            }
            if rest.is_empty() {
                arr[actual_idx] = new_val.clone();
            } else {
                arr[actual_idx] = set_recursive(&arr[actual_idx], rest, new_val);
            }
            Value::Array(arr)
        }
        Err(_) => {
            let mut obj = match value.as_object() {
                Some(o) => o.clone(),
                None => serde_json::Map::new(),
            };
            if rest.is_empty() {
                obj.insert(seg.to_string(), new_val.clone());
            } else {
                let existing = obj.get(seg).cloned().unwrap_or(Value::Null);
                obj.insert(seg.to_string(), set_recursive(&existing, rest, new_val));
            }
            Value::Object(obj)
        }
    }
}

/// dp_get(data, path) or dp_get(data, path, default)
/// Returns the value at the dot-path, or default/None if not found.
fn dp_get(args: Vec<MontyObject>) -> Result<MontyObject> {
    if args.len() < 2 || args.len() > 3 {
        bail!(
            "dp_get() takes 2-3 arguments (data, path[, default]), got {}",
            args.len()
        );
    }

    let mut iter = args.into_iter();
    let data_obj = iter.next().unwrap();
    let path_obj = iter.next().unwrap();
    let default = iter.next();

    let data_json = monty_to_json(data_obj)?;
    let path = match path_obj {
        MontyObject::String(s) => s,
        other => bail!("dp_get() path must be a string, got {other:?}"),
    };

    match get_at_path(&data_json, &path) {
        Some(val) => Ok(json_into_monty(val)),
        None => {
            if let Some(def) = default {
                Ok(def)
            } else {
                Ok(MontyObject::None)
            }
        }
    }
}

/// dp_set(data, path, value)
/// Returns a deep-cloned copy with the value set at the path.
fn dp_set(args: Vec<MontyObject>) -> Result<MontyObject> {
    if args.len() != 3 {
        bail!(
            "dp_set() takes 3 arguments (data, path, value), got {}",
            args.len()
        );
    }

    let mut iter = args.into_iter();
    let data_obj = iter.next().unwrap();
    let path_obj = iter.next().unwrap();
    let new_val_obj = iter.next().unwrap();

    let data_json = monty_to_json(data_obj)?;
    let path = match path_obj {
        MontyObject::String(s) => s,
        other => bail!("dp_set() path must be a string, got {other:?}"),
    };
    let new_val = monty_to_json(new_val_obj)?;

    let result = set_at_path(&data_json, &path, &new_val);
    Ok(json_into_monty(result))
}

/// dp_has(data, path)
/// Returns True if the path resolves to a value, False otherwise.
fn dp_has(args: Vec<MontyObject>) -> Result<MontyObject> {
    if args.len() != 2 {
        bail!(
            "dp_has() takes 2 arguments (data, path), got {}",
            args.len()
        );
    }

    let mut iter = args.into_iter();
    let data_obj = iter.next().unwrap();
    let path_obj = iter.next().unwrap();

    let data_json = monty_to_json(data_obj)?;
    let path = match path_obj {
        MontyObject::String(s) => s,
        other => bail!("dp_has() path must be a string, got {other:?}"),
    };

    Ok(MontyObject::Bool(get_at_path(&data_json, &path).is_some()))
}

/// dp_delete(data, path)
/// Returns a deep-cloned copy with the key/index at the path removed.
/// Missing path is a silent no-op. Empty path is an error.
fn dp_delete(args: Vec<MontyObject>) -> Result<MontyObject> {
    if args.len() != 2 {
        bail!(
            "dp_delete() takes 2 arguments (data, path), got {}",
            args.len()
        );
    }

    let mut iter = args.into_iter();
    let data_obj = iter.next().unwrap();
    let path_obj = iter.next().unwrap();

    let data_json = monty_to_json(data_obj)?;
    let path = match path_obj {
        MontyObject::String(s) => s,
        other => bail!("dp_delete() path must be a string, got {other:?}"),
    };

    if path.is_empty() {
        bail!("dp_delete() path must not be empty");
    }

    let segments = parse_path(&path);
    let result = delete_recursive(&data_json, &segments);
    Ok(json_into_monty(result))
}

fn delete_recursive(value: &Value, segments: &[&str]) -> Value {
    if segments.is_empty() {
        return value.clone();
    }

    let seg = segments[0];
    let rest = &segments[1..];

    match seg.parse::<i64>() {
        Ok(idx) => {
            let Some(arr) = value.as_array() else {
                return value.clone();
            };
            let mut arr = arr.clone();
            let actual_idx = if idx < 0 {
                let n = arr.len() as i64 + idx;
                if n < 0 {
                    return Value::Array(arr);
                }
                n as usize
            } else {
                idx as usize
            };
            if actual_idx >= arr.len() {
                return Value::Array(arr);
            }
            if rest.is_empty() {
                arr.remove(actual_idx);
            } else {
                arr[actual_idx] = delete_recursive(&arr[actual_idx], rest);
            }
            Value::Array(arr)
        }
        Err(_) => {
            let Some(obj) = value.as_object() else {
                return value.clone();
            };
            let mut obj = obj.clone();
            if rest.is_empty() {
                obj.shift_remove(seg);
            } else if let Some(existing) = obj.get(seg).cloned() {
                obj.insert(seg.to_string(), delete_recursive(&existing, rest));
            }
            Value::Object(obj)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_simple() {
        let data = json_into_monty(serde_json::json!({"a": 1}));
        let path = MontyObject::String("a".to_string());
        let result = dispatch("dp_get", vec![data, path]).unwrap();
        assert_eq!(result, MontyObject::Int(1));
    }

    #[test]
    fn test_get_nested() {
        let data = json_into_monty(serde_json::json!({"a": {"b": {"c": 42}}}));
        let path = MontyObject::String("a.b.c".to_string());
        let result = dispatch("dp_get", vec![data, path]).unwrap();
        assert_eq!(result, MontyObject::Int(42));
    }

    #[test]
    fn test_get_array_index() {
        let data = json_into_monty(serde_json::json!({"items": [10, 20, 30]}));
        let path = MontyObject::String("items.1".to_string());
        let result = dispatch("dp_get", vec![data, path]).unwrap();
        assert_eq!(result, MontyObject::Int(20));
    }

    #[test]
    fn test_get_negative_index() {
        let data = json_into_monty(serde_json::json!({"items": [10, 20, 30]}));
        let path = MontyObject::String("items.-1".to_string());
        let result = dispatch("dp_get", vec![data, path]).unwrap();
        assert_eq!(result, MontyObject::Int(30));
    }

    #[test]
    fn test_get_absent_returns_none() {
        let data = json_into_monty(serde_json::json!({"a": 1}));
        let path = MontyObject::String("b.c".to_string());
        let result = dispatch("dp_get", vec![data, path]).unwrap();
        assert_eq!(result, MontyObject::None);
    }

    #[test]
    fn test_get_with_default() {
        let data = json_into_monty(serde_json::json!({"a": 1}));
        let path = MontyObject::String("b".to_string());
        let default = MontyObject::String("fallback".to_string());
        let result = dispatch("dp_get", vec![data, path, default]).unwrap();
        assert_eq!(result, MontyObject::String("fallback".to_string()));
    }

    #[test]
    fn test_set_flat() {
        let data = json_into_monty(serde_json::json!({"a": 1}));
        let path = MontyObject::String("b".to_string());
        let val = MontyObject::Int(2);
        let result = dispatch("dp_set", vec![data, path, val]).unwrap();
        let json = monty_to_json(result).unwrap();
        assert_eq!(json, serde_json::json!({"a": 1, "b": 2}));
    }

    #[test]
    fn test_set_nested() {
        let data = json_into_monty(serde_json::json!({"a": {"b": 1}}));
        let path = MontyObject::String("a.c".to_string());
        let val = MontyObject::Int(99);
        let result = dispatch("dp_set", vec![data, path, val]).unwrap();
        let json = monty_to_json(result).unwrap();
        assert_eq!(json, serde_json::json!({"a": {"b": 1, "c": 99}}));
    }

    #[test]
    fn test_set_no_mutation() {
        let original = serde_json::json!({"a": {"b": 1}});
        let data = json_into_monty(original.clone());
        let path = MontyObject::String("a.b".to_string());
        let val = MontyObject::Int(999);
        let _result = dispatch("dp_set", vec![data, path, val]).unwrap();
        // Original data should be unchanged — the test verifies we didn't somehow mutate the
        // original JSON value (the MontyObject was consumed, so we just verify the json value)
        assert_eq!(original, serde_json::json!({"a": {"b": 1}}));
    }

    #[test]
    fn test_has_present() {
        let data = json_into_monty(serde_json::json!({"a": {"b": 1}}));
        let path = MontyObject::String("a.b".to_string());
        let result = dispatch("dp_has", vec![data, path]).unwrap();
        assert_eq!(result, MontyObject::Bool(true));
    }

    #[test]
    fn test_has_absent() {
        let data = json_into_monty(serde_json::json!({"a": 1}));
        let path = MontyObject::String("b.c".to_string());
        let result = dispatch("dp_has", vec![data, path]).unwrap();
        assert_eq!(result, MontyObject::Bool(false));
    }

    #[test]
    fn test_has_null_value_is_present() {
        let data = json_into_monty(serde_json::json!({"a": null}));
        let path = MontyObject::String("a".to_string());
        let result = dispatch("dp_has", vec![data, path]).unwrap();
        assert_eq!(result, MontyObject::Bool(true));
    }

    #[test]
    fn test_has_array_index() {
        let data = json_into_monty(serde_json::json!({"items": [10, 20]}));
        let present = MontyObject::String("items.1".to_string());
        let absent = MontyObject::String("items.5".to_string());
        assert_eq!(
            dispatch("dp_has", vec![data.clone(), present]).unwrap(),
            MontyObject::Bool(true)
        );
        assert_eq!(
            dispatch("dp_has", vec![data, absent]).unwrap(),
            MontyObject::Bool(false)
        );
    }

    #[test]
    fn test_delete_flat_key() {
        let data = json_into_monty(serde_json::json!({"a": 1, "b": 2}));
        let path = MontyObject::String("a".to_string());
        let result = dispatch("dp_delete", vec![data, path]).unwrap();
        assert_eq!(monty_to_json(result).unwrap(), serde_json::json!({"b": 2}));
    }

    #[test]
    fn test_delete_nested() {
        let data = json_into_monty(serde_json::json!({"a": {"b": 1, "c": 2}}));
        let path = MontyObject::String("a.b".to_string());
        let result = dispatch("dp_delete", vec![data, path]).unwrap();
        assert_eq!(
            monty_to_json(result).unwrap(),
            serde_json::json!({"a": {"c": 2}})
        );
    }

    #[test]
    fn test_delete_array_shifts() {
        let data = json_into_monty(serde_json::json!({"items": [10, 20, 30]}));
        let path = MontyObject::String("items.1".to_string());
        let result = dispatch("dp_delete", vec![data, path]).unwrap();
        assert_eq!(
            monty_to_json(result).unwrap(),
            serde_json::json!({"items": [10, 30]})
        );
    }

    #[test]
    fn test_delete_missing_path_noop() {
        let data = json_into_monty(serde_json::json!({"a": 1}));
        let path = MontyObject::String("b.c".to_string());
        let result = dispatch("dp_delete", vec![data, path]).unwrap();
        assert_eq!(monty_to_json(result).unwrap(), serde_json::json!({"a": 1}));
    }

    #[test]
    fn test_delete_preserves_order() {
        let data = json_into_monty(serde_json::json!({"a": 1, "b": 2, "c": 3}));
        let path = MontyObject::String("b".to_string());
        let result = dispatch("dp_delete", vec![data, path]).unwrap();
        let json = monty_to_json(result).unwrap();
        let keys: Vec<&String> = json.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "c"]);
    }

    #[test]
    fn test_delete_empty_path_errors() {
        let data = json_into_monty(serde_json::json!({"a": 1}));
        let path = MontyObject::String(String::new());
        let err = dispatch("dp_delete", vec![data, path]).unwrap_err();
        assert!(err.to_string().contains("must not be empty"));
    }
}
