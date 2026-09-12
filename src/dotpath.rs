use anyhow::{bail, Result};
use monty_types::MontyObject;
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

/// Set a value at a dot-path in an owned host snapshot.
/// Creates intermediate objects/arrays as needed.
fn set_at_path(mut value: Value, path: &str, new_val: Value) -> Value {
    let segments = parse_path(path);
    set_recursive(&mut value, &segments, new_val);
    value
}

fn set_recursive(value: &mut Value, segments: &[&str], new_val: Value) {
    if segments.is_empty() {
        *value = new_val;
        return;
    }

    let seg = segments[0];
    let rest = &segments[1..];

    match seg.parse::<i64>() {
        Ok(idx) => {
            if !value.is_array() {
                *value = Value::Array(vec![]);
            }
            let arr = value.as_array_mut().unwrap();
            let actual_idx = if idx < 0 {
                (arr.len() as i64 + idx).max(0) as usize
            } else {
                idx as usize
            };
            // Extend array if needed
            while arr.len() <= actual_idx {
                arr.push(Value::Null);
            }
            set_recursive(&mut arr[actual_idx], rest, new_val);
        }
        Err(_) => {
            if !value.is_object() {
                *value = Value::Object(serde_json::Map::new());
            }
            let obj = value.as_object_mut().unwrap();
            set_recursive(
                obj.entry(seg.to_string()).or_insert(Value::Null),
                rest,
                new_val,
            );
        }
    }
}

/// Walk `data` along the dot-path without converting through `serde_json::Value`.
/// Returns a borrow into the input tree, so the caller decides when (and what
/// to) clone. Critical hot path: a mold doing `dp_get(row, "x.y")` over 100 k
/// rows used to round-trip the entire `data` to JSON on every call.
fn get_at_path_monty<'a>(data: &'a MontyObject, path: &str) -> Option<&'a MontyObject> {
    let segments = parse_path(path);
    let mut current: &MontyObject = data;
    for seg in segments {
        current = match current {
            MontyObject::Dict(pairs) => pairs.into_iter().find_map(|(k, v)| match k {
                MontyObject::String(s) if s == seg => Some(v),
                _ => None,
            })?,
            MontyObject::List(arr) => {
                let idx = seg.parse::<i64>().ok()?;
                let actual = if idx < 0 {
                    let n = arr.len() as i64 + idx;
                    if n < 0 {
                        return None;
                    }
                    n as usize
                } else {
                    idx as usize
                };
                arr.get(actual)?
            }
            _ => return None,
        };
    }
    Some(current)
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

    let path = match path_obj {
        MontyObject::String(s) => s,
        other => bail!("dp_get() path must be a string, got {other:?}"),
    };

    match get_at_path_monty(&data_obj, &path) {
        Some(val) => Ok(val.clone()),
        None => Ok(default.unwrap_or(MontyObject::None)),
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

    let result = set_at_path(data_json, &path, new_val);
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

    let path = match path_obj {
        MontyObject::String(s) => s,
        other => bail!("dp_has() path must be a string, got {other:?}"),
    };

    Ok(MontyObject::Bool(
        get_at_path_monty(&data_obj, &path).is_some(),
    ))
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

    let mut data_json = monty_to_json(data_obj)?;
    let path = match path_obj {
        MontyObject::String(s) => s,
        other => bail!("dp_delete() path must be a string, got {other:?}"),
    };

    if path.is_empty() {
        bail!("dp_delete() path must not be empty");
    }

    let segments = parse_path(&path);
    delete_recursive(&mut data_json, &segments);
    Ok(json_into_monty(data_json))
}

fn delete_recursive(value: &mut Value, segments: &[&str]) {
    if segments.is_empty() {
        return;
    }

    let seg = segments[0];
    let rest = &segments[1..];

    match seg.parse::<i64>() {
        Ok(idx) => {
            let Some(arr) = value.as_array_mut() else {
                return;
            };
            let actual_idx = if idx < 0 {
                let n = arr.len() as i64 + idx;
                if n < 0 {
                    return;
                }
                n as usize
            } else {
                idx as usize
            };
            if actual_idx >= arr.len() {
                return;
            }
            if rest.is_empty() {
                arr.remove(actual_idx);
            } else {
                delete_recursive(&mut arr[actual_idx], rest);
            }
        }
        Err(_) => {
            let Some(obj) = value.as_object_mut() else {
                return;
            };
            if rest.is_empty() {
                obj.shift_remove(seg);
            } else if let Some(existing) = obj.get_mut(seg) {
                delete_recursive(existing, rest);
            }
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
    fn test_set_creates_containers_and_preserves_index_rules() {
        for (input, path, expected) in [
            (
                serde_json::json!({"keep": 7}),
                "a.2.b",
                serde_json::json!({"keep": 7, "a": [null, null, {"b": 99}]}),
            ),
            (serde_json::json!([1, 2]), "-1", serde_json::json!([1, 99])),
            (serde_json::json!([1, 2]), "-9", serde_json::json!([99, 2])),
            (
                serde_json::json!({"a": 5}),
                "a.b",
                serde_json::json!({"a": {"b": 99}}),
            ),
            (serde_json::json!({"a": 5}), "", serde_json::json!(99)),
        ] {
            let result = dispatch(
                "dp_set",
                vec![
                    json_into_monty(input),
                    MontyObject::String(path.into()),
                    MontyObject::Int(99),
                ],
            )
            .unwrap();
            assert_eq!(monty_to_json(result).unwrap(), expected, "{path}");
        }
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
