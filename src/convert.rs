use anyhow::{bail, Result};
use monty_types::{
    unstable::{self, MontyNode},
    ObjectRef,
};
use monty_types::{
    MontyDate, MontyDateTime, MontyObject, MontyTime, MontyTimeDelta, MontyTimeZone,
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

fn fmt_time(time: &MontyTime) -> String {
    let mut result = format!("{:02}:{:02}:{:02}", time.hour, time.minute, time.second);
    if time.microsecond != 0 {
        result.push_str(&format!(".{:06}", time.microsecond));
    }
    if let Some(offset) = time.offset_seconds {
        let sign = if offset < 0 { '-' } else { '+' };
        let abs = offset.unsigned_abs();
        result.push_str(&format!("{sign}{:02}:{:02}", abs / 3600, (abs % 3600) / 60));
        if abs % 60 != 0 {
            result.push_str(&format!(":{:02}", abs % 60));
        }
    }
    result
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
        MontyObject::int(i)
    } else if let Some(u) = number.as_u64() {
        MontyObject::bigint(u.into())
    } else {
        MontyObject::float(number.as_f64().unwrap_or(0.0))
    }
}

/// Convert a serde_json::Value into a MontyObject for Monty consumption.
/// All serde stays in Rust — Monty only sees Python dicts/lists/primitives.
pub fn json_to_monty(value: &Value) -> MontyObject {
    match value {
        Value::Null => MontyObject::none(),
        Value::Bool(b) => MontyObject::bool(*b),
        Value::Number(n) => json_number_to_monty(n),
        Value::String(s) => MontyObject::string(s.clone()),
        Value::Array(arr) => MontyObject::list(arr.iter().map(json_to_monty)),
        Value::Object(map) => {
            let pairs: Vec<(MontyObject, MontyObject)> = map
                .iter()
                .map(|(k, v)| (MontyObject::string(k.clone()), json_to_monty(v)))
                .collect();
            MontyObject::dict(pairs)
        }
    }
}

/// Convert an owned serde_json::Value into a MontyObject, moving strings instead of cloning.
/// Use this on the hot path when the Value will not be needed after conversion.
pub fn json_into_monty(value: Value) -> MontyObject {
    match value {
        Value::Null => MontyObject::none(),
        Value::Bool(b) => MontyObject::bool(b),
        Value::Number(n) => json_number_to_monty(&n),
        Value::String(s) => MontyObject::string(s),
        Value::Array(arr) => MontyObject::list(arr.into_iter().map(json_into_monty)),
        Value::Object(map) => {
            let pairs: Vec<(MontyObject, MontyObject)> = map
                .into_iter()
                .map(|(k, v)| (MontyObject::string(k), json_into_monty(v)))
                .collect();
            MontyObject::dict(pairs)
        }
    }
}

/// Convert a MontyObject back into a serde_json::Value.
/// Borrows the graph while materializing the JSON tree; shared values expand as JSON.
/// This runs in Rust after Monty execution — all serialization stays Rust-side.
pub fn monty_to_json(obj: MontyObject) -> Result<Value> {
    object_to_json(obj.as_ref())
}

pub(crate) fn object_to_json(obj: ObjectRef<'_>) -> Result<Value> {
    match unstable::node(obj) {
        MontyNode::None => Ok(Value::Null),
        MontyNode::Bool(b) => Ok(Value::Bool(*b)),
        MontyNode::Int(i) => Ok(Value::Number((*i).into())),
        MontyNode::BigInt(bi) => {
            if let Ok(i) = i64::try_from(bi) {
                Ok(Value::Number(i.into()))
            } else if let Ok(u) = u64::try_from(bi) {
                Ok(Value::Number(u.into()))
            } else {
                Ok(Value::String(bi.to_string()))
            }
        }
        MontyNode::Float(f) => Number::from_f64(*f)
            .map(Value::Number)
            .ok_or_else(|| anyhow::anyhow!("Cannot represent float {f} as JSON number")),
        MontyNode::String(s) => Ok(Value::String(s.clone())),
        MontyNode::List(items) | MontyNode::Tuple(items) => items
            .iter()
            .map(|id| object_to_json(unstable::child(obj, *id)))
            .collect::<Result<Vec<_>>>()
            .map(Value::Array),
        MontyNode::Dict(pairs) => {
            let mut map = serde_json::Map::new();
            for (k, v) in pairs {
                let key = unstable::child(obj, *k).to_string();
                map.insert(key, object_to_json(unstable::child(obj, *v))?);
            }
            Ok(Value::Object(map))
        }
        MontyNode::Date(d) => Ok(Value::String(fmt_date(d))),
        MontyNode::DateTime(d) => Ok(Value::String(fmt_datetime(d))),
        MontyNode::Time(d) => Ok(Value::String(fmt_time(d))),
        MontyNode::TimeDelta(d) => Ok(Value::String(fmt_timedelta(d))),
        MontyNode::TimeZone(d) => Ok(Value::String(fmt_timezone(d))),
        other => bail!("Cannot convert MontyObject variant to JSON: {other:?}"),
    }
}

/// Whether JSON normalization preserves the value's types and contents.
pub(crate) fn is_json_native(obj: &MontyObject) -> bool {
    is_json_native_ref(obj.as_ref())
}

fn is_json_native_ref(obj: ObjectRef<'_>) -> bool {
    match unstable::node(obj) {
        MontyNode::None | MontyNode::Bool(_) | MontyNode::Int(_) | MontyNode::String(_) => true,
        MontyNode::Float(f) => f.is_finite(),
        MontyNode::BigInt(n) => i64::try_from(n).is_err() && u64::try_from(n).is_ok(),
        MontyNode::List(items) => items
            .iter()
            .all(|id| is_json_native_ref(unstable::child(obj, *id))),
        MontyNode::Dict(pairs) => {
            let mut keys = std::collections::HashSet::with_capacity(pairs.len());
            pairs.iter().all(|(k, v)| {
                unstable::child(obj, *k)
                    .as_str()
                    .is_some_and(|s| keys.insert(s))
                    && is_json_native_ref(unstable::child(obj, *v))
            })
        }
        _ => false,
    }
}

/// Direct serialization without materializing a JSON tree.
pub struct MontySerialize<'a>(pub &'a MontyObject);

impl serde::Serialize for MontySerialize<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        ObjectSerialize(self.0.as_ref()).serialize(serializer)
    }
}

pub(crate) struct ObjectSerialize<'a>(pub ObjectRef<'a>);
impl serde::Serialize for ObjectSerialize<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let obj = self.0;
        match unstable::node(obj) {
            MontyNode::None => serializer.serialize_none(),
            MontyNode::Bool(b) => serializer.serialize_bool(*b),
            MontyNode::Int(i) => serializer.serialize_i64(*i),
            MontyNode::BigInt(bi) => {
                if let Ok(i) = i64::try_from(bi) {
                    serializer.serialize_i64(i)
                } else if let Ok(u) = u64::try_from(bi) {
                    serializer.serialize_u64(u)
                } else {
                    serializer.serialize_str(&bi.to_string())
                }
            }
            MontyNode::Float(f) => serializer.serialize_f64(*f),
            MontyNode::String(s) => serializer.serialize_str(s),
            MontyNode::List(items) | MontyNode::Tuple(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for id in items {
                    seq.serialize_element(&ObjectSerialize(unstable::child(obj, *id)))?;
                }
                seq.end()
            }
            MontyNode::Dict(pairs) => {
                let mut map = serializer.serialize_map(Some(pairs.len()))?;
                for (k, v) in pairs {
                    map.serialize_entry(
                        &unstable::child(obj, *k).to_string(),
                        &ObjectSerialize(unstable::child(obj, *v)),
                    )?;
                }
                map.end()
            }
            MontyNode::Date(d) => serializer.serialize_str(&fmt_date(d)),
            MontyNode::DateTime(d) => serializer.serialize_str(&fmt_datetime(d)),
            MontyNode::Time(d) => serializer.serialize_str(&fmt_time(d)),
            MontyNode::TimeDelta(d) => serializer.serialize_str(&fmt_timedelta(d)),
            MontyNode::TimeZone(d) => serializer.serialize_str(&fmt_timezone(d)),
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
    fn time_serialization_preserves_fraction_and_second_offsets() {
        for (offset, expected) in [
            (None, "14:30:01.000042"),
            (Some(0), "14:30:01.000042+00:00"),
            (Some(-3661), "14:30:01.000042-01:01:01"),
        ] {
            let obj = MontyObject::time(MontyTime {
                hour: 14,
                minute: 30,
                second: 1,
                microsecond: 42,
                offset_seconds: offset,
                timezone_name: None,
                fold: 1,
            });
            assert_eq!(monty_to_json(obj.clone()).unwrap(), json!(expected));
            assert_eq!(
                serde_json::to_value(MontySerialize(&obj)).unwrap(),
                json!(expected)
            );
        }
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
        roundtrip_eq(&MontyObject::date(MontyDate {
            year: 2025,
            month: 5,
            day: 15,
        }));
    }

    #[test]
    fn monty_serialize_datetime_naive() {
        roundtrip_eq(&MontyObject::datetime(MontyDateTime {
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
        roundtrip_eq(&MontyObject::datetime(MontyDateTime {
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
        roundtrip_eq(&MontyObject::timedelta(MontyTimeDelta {
            days: 1,
            seconds: 3661,
            microseconds: 500_000,
        }));
    }

    #[test]
    fn monty_serialize_timezone_named() {
        roundtrip_eq(&MontyObject::timezone(MontyTimeZone {
            offset_seconds: 3600,
            name: Some("Europe/Paris".to_string()),
        }));
    }

    #[test]
    fn monty_serialize_timezone_unnamed() {
        roundtrip_eq(&MontyObject::timezone(MontyTimeZone {
            offset_seconds: -18000,
            name: None,
        }));
    }

    #[test]
    fn monty_serialize_tuple_is_array() {
        let obj = MontyObject::tuple(vec![
            MontyObject::int(1),
            MontyObject::string("two".to_string()),
            MontyObject::none(),
        ]);
        roundtrip_eq(&obj);
    }
}
