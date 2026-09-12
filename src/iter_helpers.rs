use anyhow::{bail, Result};
use indexmap::IndexMap;
#[cfg(test)]
use monty_types::DictPairs;
use monty_types::MontyObject;
use serde_json::Value;

use crate::convert::{is_json_native, json_into_monty, monty_to_json};

/// Retain the helpers' historical JSON normalization for Python-only types,
/// while keeping ordinary JSON records in their owned Monty representation.
fn json_compatible_array(data: MontyObject, name: &str) -> Result<Vec<MontyObject>> {
    let data = if is_json_native(&data) {
        data
    } else {
        json_into_monty(monty_to_json(data)?)
    };
    match data {
        MontyObject::List(items) => Ok(items),
        _ => bail!("{name}() expects an array"),
    }
}

fn field_value<'a>(item: &'a MontyObject, key: &str) -> &'a MontyObject {
    if let MontyObject::Dict(pairs) = item {
        for (k, value) in pairs {
            if matches!(k, MontyObject::String(s) if s == key) {
                return value;
            }
        }
    }
    &MontyObject::None
}

/// Cached ordering projection, preserving Null < Bool < Number < String <
/// Array < Object and the historical f64 comparison of JSON numbers.
#[derive(PartialEq)]
enum OrderingKey {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array,
    Object,
}

impl OrderingKey {
    fn rank(&self) -> u8 {
        match self {
            Self::Null => 0,
            Self::Bool(_) => 1,
            Self::Number(_) => 2,
            Self::String(_) => 3,
            Self::Array => 4,
            Self::Object => 5,
        }
    }
}

// json_compatible_array rejects NaN/infinities before constructing keys.
impl Eq for OrderingKey {}
impl PartialOrd for OrderingKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for OrderingKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rank()
            .cmp(&other.rank())
            .then_with(|| match (self, other) {
                (Self::Bool(a), Self::Bool(b)) => a.cmp(b),
                (Self::Number(a), Self::Number(b)) => {
                    a.partial_cmp(b).expect("finite ordering keys")
                }
                (Self::String(a), Self::String(b)) => a.cmp(b),
                _ => std::cmp::Ordering::Equal,
            })
    }
}

fn ordering_key(item: &MontyObject, key: &str) -> OrderingKey {
    match field_value(item, key) {
        MontyObject::None => OrderingKey::Null,
        MontyObject::Bool(b) => OrderingKey::Bool(*b),
        MontyObject::Int(n) => OrderingKey::Number(*n as f64),
        MontyObject::BigInt(n) => {
            OrderingKey::Number(u64::try_from(n).expect("normalized JSON integer") as f64)
        }
        MontyObject::Float(f) => OrderingKey::Number(*f),
        MontyObject::String(s) => OrderingKey::String(s.clone()),
        MontyObject::List(_) => OrderingKey::Array,
        MontyObject::Dict(_) => OrderingKey::Object,
        _ => unreachable!("ordering keys come from JSON-normalized records"),
    }
}

/// Names of external functions exposed to Python molds.
pub const EXTERNAL_FUNCTIONS: &[&str] = &[
    "it_keys",
    "it_values",
    "it_flatten",
    "it_group_by",
    "it_sort_by",
    "it_unique",
    "it_unique_by",
    "it_count_by",
    "it_min_by",
    "it_max_by",
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
        "it_count_by" => it_count_by(args),
        "it_min_by" => it_min_by(args),
        "it_max_by" => it_max_by(args),
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

/// Stringify the value of `key` in `item` for use as a group/count bucket key.
/// String values are used verbatim; other scalars go through Display; missing
/// fields become the literal `"null"`.
fn stringify_group_key(item: &Value, key: &str) -> String {
    match item.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => "null".to_string(),
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
        let group_key = stringify_group_key(&item, &key);
        groups.entry(group_key).or_default().push(item);
    }

    let mut map = serde_json::Map::new();
    for (k, v) in groups {
        map.insert(k, Value::Array(v));
    }
    Ok(json_into_monty(Value::Object(map)))
}

/// it_sort_by(array, key[, reverse]) → sorted array (stable sort by field value)
fn it_sort_by(args: Vec<MontyObject>) -> Result<MontyObject> {
    if args.len() < 2 || args.len() > 3 {
        bail!(
            "it_sort_by() takes 2-3 arguments (array, key[, reverse]), got {}",
            args.len()
        );
    }
    let mut iter = args.into_iter();
    let data_obj = iter.next().unwrap();
    let key = match iter.next().unwrap() {
        MontyObject::String(s) => s,
        other => bail!("it_sort_by() key must be a string, got {other:?}"),
    };
    let reverse = match iter.next() {
        None => false,
        Some(MontyObject::Bool(b)) => b,
        Some(other) => bail!("it_sort_by() reverse must be a bool, got {other:?}"),
    };

    let mut arr = json_compatible_array(data_obj, "it_sort_by")?;
    // Cache compact keys and permute the existing payload vector in place.
    if reverse {
        arr.sort_by_cached_key(|item| std::cmp::Reverse(ordering_key(item, &key)));
    } else {
        arr.sort_by_cached_key(|item| ordering_key(item, &key));
    }
    Ok(MontyObject::List(arr))
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

/// it_count_by(array, key) → dict of counts, grouped by field value (insertion order)
fn it_count_by(args: Vec<MontyObject>) -> Result<MontyObject> {
    if args.len() != 2 {
        bail!(
            "it_count_by() takes 2 arguments (array, key), got {}",
            args.len()
        );
    }
    let mut iter = args.into_iter();
    let data_obj = iter.next().unwrap();
    let key = match iter.next().unwrap() {
        MontyObject::String(s) => s,
        other => bail!("it_count_by() key must be a string, got {other:?}"),
    };

    let arr = json_compatible_array(data_obj, "it_count_by")?;

    // serde_json::Map preserves insertion order (see `preserve_order` feature).
    let mut counts: serde_json::Map<String, Value> = serde_json::Map::new();
    for item in arr {
        let group_key = match field_value(&item, &key) {
            MontyObject::String(s) => s.clone(),
            value => monty_to_json(value.clone())?.to_string(),
        };
        let current = counts.get(&group_key).and_then(Value::as_u64).unwrap_or(0);
        counts.insert(group_key, Value::Number((current + 1).into()));
    }
    Ok(json_into_monty(Value::Object(counts)))
}

/// it_min_by(array, key) → element with smallest field value, or None if array is empty.
/// On ties, returns the first element.
fn it_min_by(args: Vec<MontyObject>) -> Result<MontyObject> {
    extremum_by(args, "it_min_by", false)
}

/// it_max_by(array, key) → element with largest field value, or None if array is empty.
/// On ties, returns the first element.
fn it_max_by(args: Vec<MontyObject>) -> Result<MontyObject> {
    extremum_by(args, "it_max_by", true)
}

fn extremum_by(args: Vec<MontyObject>, name: &str, take_max: bool) -> Result<MontyObject> {
    if args.len() != 2 {
        bail!(
            "{name}() takes 2 arguments (array, key), got {}",
            args.len()
        );
    }
    let mut iter = args.into_iter();
    let data_obj = iter.next().unwrap();
    let key = match iter.next().unwrap() {
        MontyObject::String(s) => s,
        other => bail!("{name}() key must be a string, got {other:?}"),
    };

    let arr = json_compatible_array(data_obj, name)?;
    let mut best: Option<(OrderingKey, MontyObject)> = None;
    for item in arr {
        let value = ordering_key(&item, &key);
        let replace = best.as_ref().is_none_or(|(best_key, _)| {
            let order = value.cmp(best_key);
            if take_max {
                order.is_gt()
            } else {
                order.is_lt()
            }
        });
        if replace {
            best = Some((value, item));
        }
    }
    Ok(best.map_or(MontyObject::None, |(_, item)| item))
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

    #[test]
    fn test_it_sort_by_reverse() {
        let data = json_into_monty(serde_json::json!([
            {"name": "Charlie", "age": 30},
            {"name": "Alice", "age": 25},
            {"name": "Bob", "age": 35},
        ]));
        let result = dispatch("it_sort_by", vec![data, s("age"), MontyObject::Bool(true)]).unwrap();
        let json = monty_to_json(result).unwrap();
        let arr = json.as_array().unwrap();
        assert_eq!(arr[0]["name"], "Bob");
        assert_eq!(arr[1]["name"], "Charlie");
        assert_eq!(arr[2]["name"], "Alice");
    }

    #[test]
    fn test_ordering_keeps_mixed_types_missing_fields_and_stable_ties() {
        let data = serde_json::json!([
            {"id": 0, "k": {}}, {"id": 1, "k": [2]},
            {"id": 2, "k": "a"}, {"id": 3, "k": 1.0},
            {"id": 4, "k": 1}, {"id": 5, "k": true},
            {"id": 6, "k": false}, {"id": 7}, {"id": 8, "k": null},
            {"id": 9, "k": [1]}
        ]);
        for (reverse, expected) in [
            (false, vec![7, 8, 6, 5, 3, 4, 2, 1, 9, 0]),
            (true, vec![0, 1, 9, 2, 3, 4, 5, 6, 7, 8]),
        ] {
            let result = dispatch(
                "it_sort_by",
                vec![
                    json_into_monty(data.clone()),
                    s("k"),
                    MontyObject::Bool(reverse),
                ],
            )
            .unwrap();
            let value = monty_to_json(result).unwrap();
            let ids: Vec<i64> = value
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v["id"].as_i64().unwrap())
                .collect();
            assert_eq!(ids, expected);
        }
        for (name, id) in [("it_min_by", 7), ("it_max_by", 0)] {
            let result = dispatch(name, vec![json_into_monty(data.clone()), s("k")]).unwrap();
            assert_eq!(monty_to_json(result).unwrap()["id"], id);
        }
    }

    #[test]
    fn test_ordering_preserves_f64_integer_ties() {
        for (first, second) in [
            (9_007_199_254_740_993u64, 9_007_199_254_740_992u64),
            (u64::MAX, u64::MAX - 1),
        ] {
            let data = serde_json::json!([{"id":0,"k":first},{"id":1,"k":second}]);
            for reverse in [false, true] {
                let result = dispatch(
                    "it_sort_by",
                    vec![
                        json_into_monty(data.clone()),
                        s("k"),
                        MontyObject::Bool(reverse),
                    ],
                )
                .unwrap();
                assert_eq!(monty_to_json(result).unwrap(), data);
            }
            for name in ["it_min_by", "it_max_by"] {
                let result = dispatch(name, vec![json_into_monty(data.clone()), s("k")]).unwrap();
                assert_eq!(monty_to_json(result).unwrap()["id"], 0);
            }
        }
    }

    #[test]
    fn test_native_helpers_preserve_json_normalization() {
        let row = MontyObject::Dict(DictPairs::from(vec![
            (MontyObject::Int(1), s("replaced")),
            (s("1"), s("kept")),
            (s("k"), MontyObject::Int(1)),
            (s("payload"), MontyObject::Tuple(vec![MontyObject::Int(7)])),
            (
                s("date"),
                MontyObject::Date(monty_types::MontyDate {
                    year: 2026,
                    month: 9,
                    day: 9,
                }),
            ),
            (
                s("big"),
                MontyObject::BigInt("18446744073709551616".parse().unwrap()),
            ),
        ]));
        let expected = json_into_monty(
            serde_json::json!({"1":"kept","k":1,"payload":[7],"date":"2026-09-09","big":"18446744073709551616"}),
        );
        for name in ["it_sort_by", "it_min_by", "it_max_by"] {
            let result =
                dispatch(name, vec![MontyObject::Tuple(vec![row.clone()]), s("k")]).unwrap();
            let expected = if name == "it_sort_by" {
                MontyObject::List(vec![expected.clone()])
            } else {
                expected.clone()
            };
            assert_eq!(result, expected, "{name}");
        }
        let result = dispatch(
            "it_count_by",
            vec![MontyObject::Tuple(vec![row]), s("date")],
        )
        .unwrap();
        assert_eq!(
            monty_to_json(result).unwrap(),
            serde_json::json!({"2026-09-09":1})
        );
    }

    #[test]
    fn test_native_helpers_still_reject_invalid_unselected_fields() {
        for name in ["it_sort_by", "it_count_by", "it_min_by", "it_max_by"] {
            for invalid in [MontyObject::Float(f64::NAN), MontyObject::Bytes(vec![1])] {
                let data = MontyObject::List(vec![MontyObject::Dict(DictPairs::from(vec![
                    (s("k"), MontyObject::Int(1)),
                    (s("unused"), invalid),
                ]))]);
                assert!(dispatch(name, vec![data, s("k")]).is_err(), "{name}");
            }
        }
    }

    #[test]
    fn test_it_sort_by_reverse_false_matches_default() {
        let data = serde_json::json!([
            {"name": "Charlie", "age": 30},
            {"name": "Alice", "age": 25},
            {"name": "Bob", "age": 35},
        ]);
        let r1 = dispatch("it_sort_by", vec![json_into_monty(data.clone()), s("age")]).unwrap();
        let r2 = dispatch(
            "it_sort_by",
            vec![json_into_monty(data), s("age"), MontyObject::Bool(false)],
        )
        .unwrap();
        assert_eq!(monty_to_json(r1).unwrap(), monty_to_json(r2).unwrap());
    }

    #[test]
    fn test_it_count_by() {
        let data = json_into_monty(serde_json::json!([
            {"name": "Alice", "dept": "eng"},
            {"name": "Bob", "dept": "sales"},
            {"name": "Carol", "dept": "eng"},
            {"name": "Dave", "dept": "eng"},
        ]));
        let result = dispatch("it_count_by", vec![data, s("dept")]).unwrap();
        let json = monty_to_json(result).unwrap();
        assert_eq!(json["eng"], 3);
        assert_eq!(json["sales"], 1);
    }

    #[test]
    fn test_it_count_by_preserves_insertion_order() {
        let data = json_into_monty(serde_json::json!([
            {"dept": "eng"},
            {"dept": "sales"},
            {"dept": "hr"},
            {"dept": "eng"},
        ]));
        let result = dispatch("it_count_by", vec![data, s("dept")]).unwrap();
        let json = monty_to_json(result).unwrap();
        let keys: Vec<&String> = json.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["eng", "sales", "hr"]);
    }

    #[test]
    fn test_it_count_by_empty() {
        let data = json_into_monty(serde_json::json!([]));
        let result = dispatch("it_count_by", vec![data, s("dept")]).unwrap();
        let json = monty_to_json(result).unwrap();
        assert_eq!(json, serde_json::json!({}));
    }

    #[test]
    fn test_it_min_by() {
        let data = json_into_monty(serde_json::json!([
            {"name": "Charlie", "age": 30},
            {"name": "Alice", "age": 25},
            {"name": "Bob", "age": 35},
        ]));
        let result = dispatch("it_min_by", vec![data, s("age")]).unwrap();
        let json = monty_to_json(result).unwrap();
        assert_eq!(json["name"], "Alice");
    }

    #[test]
    fn test_it_max_by() {
        let data = json_into_monty(serde_json::json!([
            {"name": "Charlie", "age": 30},
            {"name": "Alice", "age": 25},
            {"name": "Bob", "age": 35},
        ]));
        let result = dispatch("it_max_by", vec![data, s("age")]).unwrap();
        let json = monty_to_json(result).unwrap();
        assert_eq!(json["name"], "Bob");
    }

    #[test]
    fn test_it_min_by_empty_returns_none() {
        let data = json_into_monty(serde_json::json!([]));
        let result = dispatch("it_min_by", vec![data, s("age")]).unwrap();
        assert_eq!(result, MontyObject::None);
    }

    #[test]
    fn test_it_max_by_empty_returns_none() {
        let data = json_into_monty(serde_json::json!([]));
        let result = dispatch("it_max_by", vec![data, s("age")]).unwrap();
        assert_eq!(result, MontyObject::None);
    }

    #[test]
    fn test_it_min_by_ties_return_first() {
        let data = json_into_monty(serde_json::json!([
            {"name": "Alice", "age": 25},
            {"name": "Bob", "age": 25},
            {"name": "Carol", "age": 30},
        ]));
        let result = dispatch("it_min_by", vec![data, s("age")]).unwrap();
        let json = monty_to_json(result).unwrap();
        assert_eq!(json["name"], "Alice");
    }

    #[test]
    fn test_it_max_by_ties_return_first() {
        let data = json_into_monty(serde_json::json!([
            {"name": "Alice", "age": 25},
            {"name": "Bob", "age": 35},
            {"name": "Carol", "age": 35},
        ]));
        let result = dispatch("it_max_by", vec![data, s("age")]).unwrap();
        let json = monty_to_json(result).unwrap();
        assert_eq!(json["name"], "Bob");
    }
}
