//! Serde interop helpers for bridging serde_json with other serializers.
//!
//! Monty v0.0.14+ pulls in `serde_json` with the `arbitrary_precision` feature,
//! which changes the on-wire representation of `serde_json::Number` when
//! serialized through a *non-serde_json* serializer: numbers become a map
//! `{"$serde_json::private::Number": "42"}` instead of bare JSON numbers.
//!
//! That breaks any code path that hands a `serde_json::Value` to TOML, YAML,
//! minijinja, or any other serializer in the tree. Cargo features are additive
//! and global, so fimod can't opt out once Monty turns it on.
//!
//! [`NativeNumbers`] is a newtype wrapper whose `Serialize` impl walks the
//! `Value` tree and emits each `Number` as a proper `i64`/`u64`/`f64` via the
//! serde visitor protocol — bypassing the private map representation.

use serde::ser::{SerializeMap, SerializeSeq};
use serde_json::Value;

/// Serialize-only wrapper that emits `serde_json::Number` values as native
/// integers/floats instead of the `arbitrary_precision` private map.
///
/// Use this whenever a `serde_json::Value` is handed to a non-serde_json
/// serializer (TOML, YAML, minijinja, ...). Zero-cost: no cloning or
/// normalization, just a different `Serialize` impl.
///
/// Values larger than `u64::MAX` (extremely rare, only from Monty's BigInt
/// path) fall back to serializing as a string, since no target format has a
/// generic "arbitrary-precision integer" type. Callers that need BigInt
/// fidelity should serialize through serde_json directly.
pub struct NativeNumbers<'a>(pub &'a Value);

impl serde::Serialize for NativeNumbers<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Value::Null => serializer.serialize_unit(),
            Value::Bool(b) => serializer.serialize_bool(*b),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    serializer.serialize_i64(i)
                } else if let Some(u) = n.as_u64() {
                    serializer.serialize_u64(u)
                } else if let Some(f) = n.as_f64() {
                    serializer.serialize_f64(f)
                } else {
                    serializer.serialize_str(&n.to_string())
                }
            }
            Value::String(s) => serializer.serialize_str(s),
            Value::Array(arr) => {
                let mut seq = serializer.serialize_seq(Some(arr.len()))?;
                for item in arr {
                    seq.serialize_element(&NativeNumbers(item))?;
                }
                seq.end()
            }
            Value::Object(obj) => {
                let mut map = serializer.serialize_map(Some(obj.len()))?;
                for (k, v) in obj {
                    map.serialize_entry(k, &NativeNumbers(v))?;
                }
                map.end()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The regression this whole module exists to prevent: under
    /// `arbitrary_precision`, a bare `Value::Number` serialized through a
    /// non-serde_json serializer (here: toml) would emit the private map.
    #[test]
    fn integer_serializes_as_bare_number_in_toml() {
        let value = json!({ "port": 8080 });
        let toml_str = toml::to_string(&NativeNumbers(&value)).unwrap();
        assert_eq!(toml_str.trim(), "port = 8080");
    }

    #[test]
    fn float_serializes_as_bare_number_in_toml() {
        let value = json!({ "ratio": 1.5 });
        let toml_str = toml::to_string(&NativeNumbers(&value)).unwrap();
        assert_eq!(toml_str.trim(), "ratio = 1.5");
    }

    #[test]
    fn nested_numbers_recurse_into_arrays_and_objects() {
        let value = json!({
            "outer": {
                "list": [1, 2, 3],
                "nested": { "inner": 42 }
            }
        });
        let toml_str = toml::to_string(&NativeNumbers(&value)).unwrap();
        assert!(toml_str.contains("list = [1, 2, 3]"));
        assert!(toml_str.contains("inner = 42"));
        assert!(!toml_str.contains("$serde_json::private::Number"));
    }

    #[test]
    fn non_numeric_variants_pass_through_unchanged() {
        let value = json!({
            "name": "alice",
            "active": true,
            "tags": ["a", "b"]
        });
        let toml_str = toml::to_string(&NativeNumbers(&value)).unwrap();
        assert!(toml_str.contains(r#"name = "alice""#));
        assert!(toml_str.contains("active = true"));
        assert!(toml_str.contains(r#"tags = ["a", "b"]"#));
    }

    /// `null` maps to serde `unit`; TOML rejects it, but YAML/minijinja accept it.
    /// Verified via serde_json round-trip: the bare value re-serializes as `null`.
    #[test]
    fn null_serializes_as_unit() {
        let value = Value::Null;
        let json_str = serde_json::to_string(&NativeNumbers(&value)).unwrap();
        assert_eq!(json_str, "null");
    }
}
