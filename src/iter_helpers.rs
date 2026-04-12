use anyhow::{bail, Result};
use indexmap::IndexMap;
#[cfg(test)]
use monty::DictPairs;
use monty::MontyObject;
use serde_json::Value;

use crate::convert::{json_into_monty, monty_to_json};

/// Names of external functions exposed to Python molds.
pub const EXTERNAL_FUNCTIONS: &[&str] = &[
    "it_keys",
    "it_values",
    "it_flatten",
    "it_group_by",
    "it_sort_by",
    "it_unique",
    "it_unique_by",
];

/// Dispatch an external function call to the appropriate iter_helpers handler.
pub fn dispatch(name: &str, args: Vec<MontyObject>) -> Result<MontyObject> {
    match name {
        "it_keys" => it_keys(args),
        "it_values" => it_values(args),
        "it_flatten" => it_flatten(args),
        "it_group_by" => it_group_by(args),
        "it_sort_by" => it_sort_by(args),
        "it_unique" => it_unique(args),
        "it_unique_by" => it_unique_by(args),
        _ => bail!("Unknown iter_helpers function: {name}"),
    }
}

/// it_keys(dict) → list of keys
fn it_keys(args: Vec<MontyObject>) -> Result<MontyObject> {
    if args.len() != 1 {
        bail!("it_keys() takes 1 argument (dict), got {}", args.len());
    }
    match args.into_iter().next().unwrap() {
        MontyObject::Dict(pairs) => {
            let keys: Vec<MontyObject> = pairs.into_iter().map(|(k, _)| k).collect();
            Ok(MontyObject::List(keys))
        }
        other => bail!("it_keys() expects a dict, got {other:?}"),
    }
}

/// it_values(dict) → list of values
fn it_values(args: Vec<MontyObject>) -> Result<MontyObject> {
    if args.len() != 1 {
        bail!("it_values() takes 1 argument (dict), got {}", args.len());
    }
    match args.into_iter().next().unwrap() {
        MontyObject::Dict(pairs) => {
            let values: Vec<MontyObject> = pairs.into_iter().map(|(_, v)| v).collect();
            Ok(MontyObject::List(values))
        }
        other => bail!("it_values() expects a dict, got {other:?}"),
    }
}

/// it_flatten(array) → recursively flattened array
fn it_flatten(args: Vec<MontyObject>) -> Result<MontyObject> {
    if args.len() != 1 {
        bail!("it_flatten() takes 1 argument (array), got {}", args.len());
    }
    let json = monty_to_json(args.into_iter().next().unwrap())?;
    let result = flatten_recursive(json)?;
    Ok(json_into_monty(result))
}

fn flatten_recursive(value: Value) -> Result<Value> {
    match value {
        Value::Array(arr) => {
            let mut flat = Vec::new();
            for item in arr {
                match item {
                    v @ Value::Array(_) => {
                        if let Value::Array(inner) = flatten_recursive(v)? {
                            flat.extend(inner);
                        }
                    }
                    other => flat.push(other),
                }
            }
            Ok(Value::Array(flat))
        }
        _ => bail!("it_flatten() expects an array"),
    }
}

/// it_group_by(array, key) → dict of lists, grouped by value of field
fn it_group_by(args: Vec<MontyObject>) -> Result<MontyObject> {
    if args.len() != 2 {
        bail!(
            "it_group_by() takes 2 arguments (array, key), got {}",
            args.len()
        );
    }
    let mut iter = args.into_iter();
    let data_obj = iter.next().unwrap();
    let key = match iter.next().unwrap() {
        MontyObject::String(s) => s,
        other => bail!("it_group_by() key must be a string, got {other:?}"),
    };

    let arr = match monty_to_json(data_obj)? {
        Value::Array(arr) => arr,
        _ => bail!("it_group_by() expects an array"),
    };

    let mut groups: IndexMap<String, Vec<Value>> = IndexMap::new();
    for item in arr {
        let group_key = match item.get(&key) {
            Some(Value::String(s)) => s.clone(),
            Some(v) => v.to_string(),
            None => "null".to_string(),
        };
        groups.entry(group_key).or_default().push(item);
    }

    let mut map = serde_json::Map::new();
    for (k, v) in groups {
        map.insert(k, Value::Array(v));
    }
    Ok(json_into_monty(Value::Object(map)))
}

/// it_sort_by(array, key) → sorted array (stable sort by field value)
fn it_sort_by(args: Vec<MontyObject>) -> Result<MontyObject> {
    if args.len() != 2 {
        bail!(
            "it_sort_by() takes 2 arguments (array, key), got {}",
            args.len()
        );
    }
    let mut iter = args.into_iter();
    let data_obj = iter.next().unwrap();
    let key = match iter.next().unwrap() {
        MontyObject::String(s) => s,
        other => bail!("it_sort_by() key must be a string, got {other:?}"),
    };

    let mut arr = match monty_to_json(data_obj)? {
        Value::Array(arr) => arr,
        _ => bail!("it_sort_by() expects an array"),
    };

    arr.sort_by(|a, b| {
        let va = a.get(&key).unwrap_or(&Value::Null);
        let vb = b.get(&key).unwrap_or(&Value::Null);
        cmp_json_values(va, vb)
    });

    Ok(json_into_monty(Value::Array(arr)))
}

/// Compare two JSON values for sorting.
/// Order: Null < Bool < Number < String < Array < Object
fn cmp_json_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    fn type_rank(v: &Value) -> u8 {
        match v {
            Value::Null => 0,
            Value::Bool(_) => 1,
            Value::Number(_) => 2,
            Value::String(_) => 3,
            Value::Array(_) => 4,
            Value::Object(_) => 5,
        }
    }

    let ra = type_rank(a);
    let rb = type_rank(b);
    if ra != rb {
        return ra.cmp(&rb);
    }

    match (a, b) {
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        (Value::Number(a), Value::Number(b)) => {
            let fa = a.as_f64().unwrap_or(0.0);
            let fb = b.as_f64().unwrap_or(0.0);
            fa.partial_cmp(&fb).unwrap_or(Ordering::Equal)
        }
        (Value::String(a), Value::String(b)) => a.cmp(b),
        _ => Ordering::Equal,
    }
}

/// it_unique(array) → deduplicated array (preserves first occurrence)
fn it_unique(args: Vec<MontyObject>) -> Result<MontyObject> {
    if args.len() != 1 {
        bail!("it_unique() takes 1 argument (array), got {}", args.len());
    }

    let arr = match monty_to_json(args.into_iter().next().unwrap())? {
        Value::Array(arr) => arr,
        _ => bail!("it_unique() expects an array"),
    };

    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for item in arr {
        let key = serde_json::to_string(&item).unwrap_or_default();
        if seen.insert(key) {
            result.push(item);
        }
    }

    Ok(json_into_monty(Value::Array(result)))
}

/// it_unique_by(array, key) → deduplicated by field value
fn it_unique_by(args: Vec<MontyObject>) -> Result<MontyObject> {
    if args.len() != 2 {
        bail!(
            "it_unique_by() takes 2 arguments (array, key), got {}",
            args.len()
        );
    }
    let mut iter = args.into_iter();
    let data_obj = iter.next().unwrap();
    let key = match iter.next().unwrap() {
        MontyObject::String(s) => s,
        other => bail!("it_unique_by() key must be a string, got {other:?}"),
    };

    let arr = match monty_to_json(data_obj)? {
        Value::Array(arr) => arr,
        _ => bail!("it_unique_by() expects an array"),
    };

    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for item in arr {
        let field_val = item.get(&key).unwrap_or(&Value::Null);
        let hash_key = serde_json::to_string(field_val).unwrap_or_default();
        if seen.insert(hash_key) {
            result.push(item);
        }
    }

    Ok(json_into_monty(Value::Array(result)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(val: &str) -> MontyObject {
        MontyObject::String(val.to_string())
    }

    #[test]
    fn test_it_keys() {
        let dict = MontyObject::Dict(DictPairs::from(vec![
            (s("a"), MontyObject::Int(1)),
            (s("b"), MontyObject::Int(2)),
        ]));
        let result = dispatch("it_keys", vec![dict]).unwrap();
        match result {
            MontyObject::List(keys) => {
                assert_eq!(keys, vec![s("a"), s("b")]);
            }
            _ => panic!("Expected list"),
        }
    }

    #[test]
    fn test_it_values() {
        let dict = MontyObject::Dict(DictPairs::from(vec![
            (s("a"), MontyObject::Int(1)),
            (s("b"), MontyObject::Int(2)),
        ]));
        let result = dispatch("it_values", vec![dict]).unwrap();
        match result {
            MontyObject::List(vals) => {
                assert_eq!(vals, vec![MontyObject::Int(1), MontyObject::Int(2)]);
            }
            _ => panic!("Expected list"),
        }
    }

    #[test]
    fn test_it_flatten() {
        let data = json_into_monty(serde_json::json!([1, [2, [3, 4]], 5]));
        let result = dispatch("it_flatten", vec![data]).unwrap();
        let json = monty_to_json(result).unwrap();
        assert_eq!(json, serde_json::json!([1, 2, 3, 4, 5]));
    }

    #[test]
    fn test_it_group_by() {
        let data = json_into_monty(serde_json::json!([
            {"name": "Alice", "dept": "eng"},
            {"name": "Bob", "dept": "sales"},
            {"name": "Carol", "dept": "eng"},
        ]));
        let result = dispatch("it_group_by", vec![data, s("dept")]).unwrap();
        let json = monty_to_json(result).unwrap();
        assert_eq!(json["eng"].as_array().unwrap().len(), 2);
        assert_eq!(json["sales"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_it_group_by_preserves_insertion_order() {
        let data = json_into_monty(serde_json::json!([
            {"name": "Alice", "dept": "eng"},
            {"name": "Bob", "dept": "sales"},
            {"name": "Carol", "dept": "hr"},
            {"name": "Dave", "dept": "eng"},
        ]));
        let result = dispatch("it_group_by", vec![data, s("dept")]).unwrap();
        let json = monty_to_json(result).unwrap();
        let keys: Vec<&String> = json.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["eng", "sales", "hr"]);
    }

    #[test]
    fn test_it_sort_by() {
        let data = json_into_monty(serde_json::json!([
            {"name": "Charlie", "age": 30},
            {"name": "Alice", "age": 25},
            {"name": "Bob", "age": 35},
        ]));
        let result = dispatch("it_sort_by", vec![data, s("age")]).unwrap();
        let json = monty_to_json(result).unwrap();
        let arr = json.as_array().unwrap();
        assert_eq!(arr[0]["name"], "Alice");
        assert_eq!(arr[1]["name"], "Charlie");
        assert_eq!(arr[2]["name"], "Bob");
    }

    #[test]
    fn test_it_unique() {
        let data = json_into_monty(serde_json::json!([1, 2, 3, 2, 1, 4]));
        let result = dispatch("it_unique", vec![data]).unwrap();
        let json = monty_to_json(result).unwrap();
        assert_eq!(json, serde_json::json!([1, 2, 3, 4]));
    }

    #[test]
    fn test_it_unique_by() {
        let data = json_into_monty(serde_json::json!([
            {"id": 1, "name": "Alice"},
            {"id": 2, "name": "Bob"},
            {"id": 1, "name": "Alice2"},
        ]));
        let result = dispatch("it_unique_by", vec![data, s("id")]).unwrap();
        let json = monty_to_json(result).unwrap();
        let arr = json.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["name"], "Alice");
        assert_eq!(arr[1]["name"], "Bob");
    }
}
