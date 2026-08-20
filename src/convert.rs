use anyhow::{bail, Result};
use monty_types::{
    DictPairs, MontyDate, MontyDateTime, MontyObject, MontyTimeDelta, MontyTimeZone,
};
use serde::ser::{SerializeMap, SerializeSeq};
use serde_json::{Number, Value};

fn fmt_date(d: &MontyDate) -> String {
    format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)
}

fn fmt_datetime(dt: &MontyDateTime) -> String {
    let base = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        dt.year, dt.month, dt.day, dt.hour, dt.minute, dt.second
    );
    let with_us = if dt.microsecond > 0 {
        format!("{base}.{:06}", dt.microsecond)
    } else {
        base
    };
    match dt.offset_seconds {
        Some(0) => format!("{with_us}+00:00"),
        Some(offset) => {
            let sign = if offset < 0 { '-' } else { '+' };
            let abs = offset.unsigned_abs();
            format!("{with_us}{sign}{:02}:{:02}", abs / 3600, (abs % 3600) / 60)
        }
        None => with_us,
    }
}

fn fmt_timedelta(td: &MontyTimeDelta) -> String {
    let total_seconds = td.days as i64 * 86400 + td.seconds as i64;
    let us = if td.microseconds != 0 {
        format!(".{:06}", td.microseconds.unsigned_abs())
    } else {
        String::new()
    };
    format!("P{total_seconds}{us}S")
}

fn fmt_timezone(tz: &MontyTimeZone) -> String {
    if let Some(name) = &tz.name {
        name.clone()
    } else {
        let offset = tz.offset_seconds;
        let sign = if offset < 0 { '-' } else { '+' };
        let abs = offset.unsigned_abs();
        format!("UTC{sign}{:02}:{:02}", abs / 3600, (abs % 3600) / 60)
    }
}

fn json_number_to_monty(number: &Number) -> MontyObject {
    if let Some(i) = number.as_i64() {
        MontyObject::Int(i)
    } else if let Some(u) = number.as_u64() {
        MontyObject::BigInt(u.into())
    } else {
        MontyObject::Float(number.as_f64().unwrap_or(0.0))
    }
}

/// Convert a serde_json::Value into a MontyObject for Monty consumption.
/// All serde stays in Rust — Monty only sees Python dicts/lists/primitives.
pub fn json_to_monty(value: &Value) -> MontyObject {
    match value {
        Value::Null => MontyObject::None,
        Value::Bool(b) => MontyObject::Bool(*b),
        Value::Number(n) => json_number_to_monty(n),
        Value::String(s) => MontyObject::String(s.clone()),
        Value::Array(arr) => MontyObject::List(arr.iter().map(json_to_monty).collect()),
        Value::Object(map) => {
            let pairs: Vec<(MontyObject, MontyObject)> = map
                .iter()
                .map(|(k, v)| (MontyObject::String(k.clone()), json_to_monty(v)))
                .collect();
            MontyObject::Dict(DictPairs::from(pairs))
        }
    }
}

/// Convert an owned serde_json::Value into a MontyObject, moving strings instead of cloning.
/// Use this on the hot path when the Value will not be needed after conversion.
pub fn json_into_monty(value: Value) -> MontyObject {
    match value {
        Value::Null => MontyObject::None,
        Value::Bool(b) => MontyObject::Bool(b),
        Value::Number(n) => json_number_to_monty(&n),
        Value::String(s) => MontyObject::String(s),
        Value::Array(arr) => MontyObject::List(arr.into_iter().map(json_into_monty).collect()),
        Value::Object(map) => {
            let pairs: Vec<(MontyObject, MontyObject)> = map
                .into_iter()
                .map(|(k, v)| (MontyObject::String(k), json_into_monty(v)))
                .collect();
            MontyObject::Dict(DictPairs::from(pairs))
        }
    }
}

/// Convert a MontyObject back into a serde_json::Value.
/// Takes ownership to avoid cloning strings on the return path.
/// This runs in Rust after Monty execution — all serialization stays Rust-side.
pub fn monty_to_json(obj: MontyObject) -> Result<Value> {
    match obj {
        MontyObject::None => Ok(Value::Null),
        MontyObject::Bool(b) => Ok(Value::Bool(b)),
        MontyObject::Int(i) => Ok(Value::Number(i.into())),
        MontyObject::BigInt(bi) => {
            if let Ok(i) = i64::try_from(&bi) {
                Ok(Value::Number(i.into()))
            } else if let Ok(u) = u64::try_from(&bi) {
                Ok(Value::Number(u.into()))
            } else {
                Ok(Value::String(bi.to_string()))
            }
        }
        MontyObject::Float(f) => serde_json::Number::from_f64(f)
            .map(Value::Number)
            .ok_or_else(|| anyhow::anyhow!("Cannot represent float {f} as JSON number")),
        MontyObject::String(s) => Ok(Value::String(s)),
        MontyObject::List(items) | MontyObject::Tuple(items) => {
            let arr: Result<Vec<Value>> = items.into_iter().map(monty_to_json).collect();
            Ok(Value::Array(arr?))
        }
        MontyObject::Dict(pairs) => {
            let mut map = serde_json::Map::new();
            for (k, v) in pairs {
                let key = match k {
                    MontyObject::String(s) => s,
                    other => format!("{other}"),
                };
                map.insert(key, monty_to_json(v)?);
            }
            Ok(Value::Object(map))
        }
        MontyObject::Date(d) => Ok(Value::String(fmt_date(&d))),
        MontyObject::DateTime(dt) => Ok(Value::String(fmt_datetime(&dt))),
        MontyObject::TimeDelta(td) => Ok(Value::String(fmt_timedelta(&td))),
        MontyObject::TimeZone(tz) => Ok(Value::String(fmt_timezone(&tz))),
        other => bail!("Cannot convert MontyObject variant to JSON: {other:?}"),
    }
}

/// Serde serializer wrapper for `MontyObject` that produces the same JSON as `monty_to_json`.
///
/// Avoids allocating an intermediate `serde_json::Value` when writing directly to a serializer
/// (e.g. `serde_json::to_writer`). Semantics are kept in sync with `monty_to_json`.
pub struct MontySerialize<'a>(pub &'a MontyObject);

impl serde::Serialize for MontySerialize<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        match self.0 {
            MontyObject::None => serializer.serialize_none(),
            MontyObject::Bool(b) => serializer.serialize_bool(*b),
            MontyObject::Int(i) => serializer.serialize_i64(*i),
            MontyObject::BigInt(bi) => {
                if let Ok(i) = i64::try_from(bi) {
                    serializer.serialize_i64(i)
                } else if let Ok(u) = u64::try_from(bi) {
                    serializer.serialize_u64(u)
                } else {
                    serializer.serialize_str(&bi.to_string())
                }
            }
            MontyObject::Float(f) => serializer.serialize_f64(*f),
            MontyObject::String(s) => serializer.serialize_str(s),
            MontyObject::List(items) | MontyObject::Tuple(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(&MontySerialize(item))?;
                }
                seq.end()
            }
            MontyObject::Dict(pairs) => {
                let mut map = serializer.serialize_map(Some(pairs.len()))?;
                for (k, v) in pairs {
                    let key = match k {
                        MontyObject::String(s) => s.clone(),
                        other => format!("{other}"),
                    };
                    map.serialize_entry(&key, &MontySerialize(v))?;
                }
                map.end()
            }
            MontyObject::Date(d) => serializer.serialize_str(&fmt_date(d)),
            MontyObject::DateTime(dt) => serializer.serialize_str(&fmt_datetime(dt)),
            MontyObject::TimeDelta(td) => serializer.serialize_str(&fmt_timedelta(td)),
            MontyObject::TimeZone(tz) => serializer.serialize_str(&fmt_timezone(tz)),
            other => Err(serde::ser::Error::custom(format!(
                "Cannot convert MontyObject variant to JSON: {other:?}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use monty_types::{MontyDate, MontyDateTime, MontyTimeDelta, MontyTimeZone};
    use serde_json::json;

    use super::*;

    fn roundtrip_eq(obj: &MontyObject) {
        let via_serialize: Value = serde_json::to_value(MontySerialize(obj)).unwrap();
        let via_convert = monty_to_json(obj.clone()).unwrap();
        assert_eq!(via_serialize, via_convert);
    }

    #[test]
    fn monty_serialize_matches_monty_to_json_common_types() {
        let input = json!([
            {"id": 0, "name": "alice", "active": true,  "score": 42,  "tag": null},
            {"id": 1, "name": "bob",   "active": false, "score": 0.5, "tags": [1, 2, 3]},
            {"id": 2, "nested": {"x": -100, "y": [[1, "two"], null]}},
        ]);
        let monty = json_into_monty(input);
        roundtrip_eq(&monty);
    }

    #[test]
    fn json_integer_roundtrip_preserves_u64_range() {
        for input in [
            "9007199254740993",
            "9223372036854775808",
            "18446744073709551615",
        ] {
            let value: Value = serde_json::from_str(input).unwrap();

            for monty in [json_to_monty(&value), json_into_monty(value.clone())] {
                assert_eq!(
                    serde_json::to_string(&MontySerialize(&monty)).unwrap(),
                    input
                );
                assert_eq!(monty_to_json(monty).unwrap(), value);
            }
        }
    }

    #[test]
    fn monty_serialize_date() {
        roundtrip_eq(&MontyObject::Date(MontyDate {
            year: 2025,
            month: 5,
            day: 15,
        }));
    }

    #[test]
    fn monty_serialize_datetime_naive() {
        roundtrip_eq(&MontyObject::DateTime(MontyDateTime {
            year: 2025,
            month: 5,
            day: 15,
            hour: 14,
            minute: 30,
            second: 0,
            microsecond: 0,
            offset_seconds: None,
            timezone_name: None,
        }));
    }

    #[test]
    fn monty_serialize_datetime_aware_with_microseconds() {
        roundtrip_eq(&MontyObject::DateTime(MontyDateTime {
            year: 2025,
            month: 5,
            day: 15,
            hour: 14,
            minute: 30,
            second: 0,
            microsecond: 123_456,
            offset_seconds: Some(3600),
            timezone_name: None,
        }));
    }

    #[test]
    fn monty_serialize_timedelta() {
        roundtrip_eq(&MontyObject::TimeDelta(MontyTimeDelta {
            days: 1,
            seconds: 3661,
            microseconds: 500_000,
        }));
    }

    #[test]
    fn monty_serialize_timezone_named() {
        roundtrip_eq(&MontyObject::TimeZone(MontyTimeZone {
            offset_seconds: 3600,
            name: Some("Europe/Paris".to_string()),
        }));
    }

    #[test]
    fn monty_serialize_timezone_unnamed() {
        roundtrip_eq(&MontyObject::TimeZone(MontyTimeZone {
            offset_seconds: -18000,
            name: None,
        }));
    }

    #[test]
    fn monty_serialize_tuple_is_array() {
        let obj = MontyObject::Tuple(vec![
            MontyObject::Int(1),
            MontyObject::String("two".to_string()),
            MontyObject::None,
        ]);
        roundtrip_eq(&obj);
    }
}
