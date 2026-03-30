use anyhow::{bail, Result};
use monty::{DictPairs, MontyObject};
use serde_json::Value;

/// Convert a serde_json::Value into a MontyObject for Monty consumption.
/// All serde stays in Rust — Monty only sees Python dicts/lists/primitives.
pub fn json_to_monty(value: &Value) -> MontyObject {
    match value {
        Value::Null => MontyObject::None,
        Value::Bool(b) => MontyObject::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                MontyObject::Int(i)
            } else if let Some(f) = n.as_f64() {
                MontyObject::Float(f)
            } else {
                // Fallback for u64 values that don't fit in i64
                MontyObject::Float(n.as_f64().unwrap_or(0.0))
            }
        }
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
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                MontyObject::Int(i)
            } else if let Some(f) = n.as_f64() {
                MontyObject::Float(f)
            } else {
                MontyObject::Float(n.as_f64().unwrap_or(0.0))
            }
        }
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
            // Try to fit into i64, otherwise use string representation
            if let Ok(i) = i64::try_from(bi.clone()) {
                Ok(Value::Number(i.into()))
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
        MontyObject::Date(d) => Ok(Value::String(format!(
            "{:04}-{:02}-{:02}",
            d.year, d.month, d.day
        ))),
        MontyObject::DateTime(dt) => {
            let base = format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
                dt.year, dt.month, dt.day, dt.hour, dt.minute, dt.second
            );
            let with_us = if dt.microsecond > 0 {
                format!("{base}.{:06}", dt.microsecond)
            } else {
                base
            };
            let with_tz = match dt.offset_seconds {
                Some(0) => format!("{with_us}+00:00"),
                Some(offset) => {
                    let sign = if offset < 0 { '-' } else { '+' };
                    let abs = offset.unsigned_abs();
                    format!("{with_us}{sign}{:02}:{:02}", abs / 3600, (abs % 3600) / 60)
                }
                None => with_us,
            };
            Ok(Value::String(with_tz))
        }
        MontyObject::TimeDelta(td) => {
            let total_seconds = td.days as i64 * 86400 + td.seconds as i64;
            let us = if td.microseconds != 0 {
                format!(".{:06}", td.microseconds.unsigned_abs())
            } else {
                String::new()
            };
            Ok(Value::String(format!("P{total_seconds}{us}S")))
        }
        MontyObject::TimeZone(tz) => {
            if let Some(ref name) = tz.name {
                Ok(Value::String(name.clone()))
            } else {
                let offset = tz.offset_seconds;
                let sign = if offset < 0 { '-' } else { '+' };
                let abs = offset.unsigned_abs();
                Ok(Value::String(format!(
                    "UTC{sign}{:02}:{:02}",
                    abs / 3600,
                    (abs % 3600) / 60
                )))
            }
        }
        other => bail!("Cannot convert MontyObject variant to JSON: {other:?}"),
    }
}
