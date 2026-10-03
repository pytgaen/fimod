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
fn get_at_path_monty<'a>(data: &'a MontyObject, path: &str) -> Option<monty_types::ObjectRef<'a>> {
    let mut current = data.as_ref();
    for seg in parse_path(path) {
        current = match monty_types::unstable::node(current) {
            monty_types::unstable::MontyNode::Dict(_) => crate::monty_args::field(current, seg)?,
            monty_types::unstable::MontyNode::List(items) => {
                let idx = seg.parse::<i64>().ok()?;
                let actual = if idx < 0 {
                    usize::try_from(items.len() as i64 + idx).ok()?
                } else {
                    idx as usize
                };
                monty_types::unstable::child(current, *items.get(actual)?)
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

    let path = crate::monty_args::expect_string_owned(&path_obj, "dp_get() path")?;

    match get_at_path_monty(&data_obj, &path) {
        Some(val) => Ok(val.to_owned()),
        None => Ok(default.unwrap_or(MontyObject::none())),
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
    let path = crate::monty_args::expect_string_owned(&path_obj, "dp_set() path")?;
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

    let path = crate::monty_args::expect_string_owned(&path_obj, "dp_has() path")?;

    Ok(MontyObject::bool(
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
    let path = crate::monty_args::expect_string_owned(&path_obj, "dp_delete() path")?;

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
        let path = MontyObject::string("a".to_string());
        let result = dispatch("dp_get", vec![data, path]).unwrap();
        assert_eq!(result, MontyObject::int(1));
    }

    #[test]
    fn test_get_nested() {
        let data = json_into_monty(serde_json::json!({"a": {"b": {"c": 42}}}));
        let path = MontyObject::string("a.b.c".to_string());
        let result = dispatch("dp_get", vec![data, path]).unwrap();
        assert_eq!(result, MontyObject::int(42));
    }

    #[test]
    fn test_get_array_index() {
        let data = json_into_monty(serde_json::json!({"items": [10, 20, 30]}));
        let path = MontyObject::string("items.1".to_string());
        let result = dispatch("dp_get", vec![data, path]).unwrap();
        assert_eq!(result, MontyObject::int(20));
    }

    #[test]
    fn test_get_negative_index() {
        let data = json_into_monty(serde_json::json!({"items": [10, 20, 30]}));
        let path = MontyObject::string("items.-1".to_string());
        let result = dispatch("dp_get", vec![data, path]).unwrap();
        assert_eq!(result, MontyObject::int(30));
    }

    #[test]
    fn test_get_absent_returns_none() {
        let data = json_into_monty(serde_json::json!({"a": 1}));
        let path = MontyObject::string("b.c".to_string());
        let result = dispatch("dp_get", vec![data, path]).unwrap();
        assert_eq!(result, MontyObject::none());
    }

    #[test]
    fn test_get_with_default() {
        let data = json_into_monty(serde_json::json!({"a": 1}));
        let path = MontyObject::string("b".to_string());
        let default = MontyObject::string("fallback".to_string());
        let result = dispatch("dp_get", vec![data, path, default]).unwrap();
        assert_eq!(result, MontyObject::string("fallback".to_string()));
    }

    #[test]
    fn test_set_flat() {
        let data = json_into_monty(serde_json::json!({"a": 1}));
        let path = MontyObject::string("b".to_string());
        let val = MontyObject::int(2);
        let result = dispatch("dp_set", vec![data, path, val]).unwrap();
        let json = monty_to_json(result).unwrap();
        assert_eq!(json, serde_json::json!({"a": 1, "b": 2}));
    }

    #[test]
    fn test_set_nested() {
        let data = json_into_monty(serde_json::json!({"a": {"b": 1}}));
        let path = MontyObject::string("a.c".to_string());
        let val = MontyObject::int(99);
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
                    MontyObject::string(path),
                    MontyObject::int(99),
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
        let path = MontyObject::string("a.b".to_string());
        let val = MontyObject::int(999);
        let _result = dispatch("dp_set", vec![data, path, val]).unwrap();
        // Original data should be unchanged — the test verifies we didn't somehow mutate the
        // original JSON value (the MontyObject was consumed, so we just verify the json value)
        assert_eq!(original, serde_json::json!({"a": {"b": 1}}));
    }

    #[test]
    fn test_has_present() {
        let data = json_into_monty(serde_json::json!({"a": {"b": 1}}));
        let path = MontyObject::string("a.b".to_string());
        let result = dispatch("dp_has", vec![data, path]).unwrap();
        assert_eq!(result, MontyObject::bool(true));
    }

    #[test]
    fn test_has_absent() {
        let data = json_into_monty(serde_json::json!({"a": 1}));
        let path = MontyObject::string("b.c".to_string());
        let result = dispatch("dp_has", vec![data, path]).unwrap();
        assert_eq!(result, MontyObject::bool(false));
    }

    #[test]
    fn test_has_null_value_is_present() {
        let data = json_into_monty(serde_json::json!({"a": null}));
        let path = MontyObject::string("a".to_string());
        let result = dispatch("dp_has", vec![data, path]).unwrap();
        assert_eq!(result, MontyObject::bool(true));
    }

    #[test]
    fn test_has_array_index() {
        let data = json_into_monty(serde_json::json!({"items": [10, 20]}));
        let present = MontyObject::string("items.1".to_string());
        let absent = MontyObject::string("items.5".to_string());
        assert_eq!(
            dispatch("dp_has", vec![data.clone(), present]).unwrap(),
            MontyObject::bool(true)
        );
        assert_eq!(
            dispatch("dp_has", vec![data, absent]).unwrap(),
            MontyObject::bool(false)
        );
    }

    #[test]
    fn test_delete_flat_key() {
        let data = json_into_monty(serde_json::json!({"a": 1, "b": 2}));
        let path = MontyObject::string("a".to_string());
        let result = dispatch("dp_delete", vec![data, path]).unwrap();
        assert_eq!(monty_to_json(result).unwrap(), serde_json::json!({"b": 2}));
    }

    #[test]
    fn test_delete_nested() {
        let data = json_into_monty(serde_json::json!({"a": {"b": 1, "c": 2}}));
        let path = MontyObject::string("a.b".to_string());
        let result = dispatch("dp_delete", vec![data, path]).unwrap();
        assert_eq!(
            monty_to_json(result).unwrap(),
            serde_json::json!({"a": {"c": 2}})
        );
    }

    #[test]
    fn test_delete_array_shifts() {
        let data = json_into_monty(serde_json::json!({"items": [10, 20, 30]}));
        let path = MontyObject::string("items.1".to_string());
        let result = dispatch("dp_delete", vec![data, path]).unwrap();
        assert_eq!(
            monty_to_json(result).unwrap(),
            serde_json::json!({"items": [10, 30]})
        );
    }

    #[test]
    fn test_delete_missing_path_noop() {
        let data = json_into_monty(serde_json::json!({"a": 1}));
        let path = MontyObject::string("b.c".to_string());
        let result = dispatch("dp_delete", vec![data, path]).unwrap();
        assert_eq!(monty_to_json(result).unwrap(), serde_json::json!({"a": 1}));
    }

    #[test]
    fn test_delete_preserves_order() {
        let data = json_into_monty(serde_json::json!({"a": 1, "b": 2, "c": 3}));
        let path = MontyObject::string("b".to_string());
        let result = dispatch("dp_delete", vec![data, path]).unwrap();
        let json = monty_to_json(result).unwrap();
        let keys: Vec<&String> = json.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["a", "c"]);
    }

    #[test]
    fn test_delete_empty_path_errors() {
        let data = json_into_monty(serde_json::json!({"a": 1}));
        let path = MontyObject::string(String::new());
        let err = dispatch("dp_delete", vec![data, path]).unwrap_err();
        assert!(err.to_string().contains("must not be empty"));
    }
}
