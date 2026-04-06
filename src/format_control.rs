use std::sync::{Arc, Mutex};

use anyhow::{bail, Result};
use monty::MontyObject;

/// Names of external functions exposed to Python molds.
pub const EXTERNAL_FUNCTIONS: &[&str] = &[
    "set_input_format",
    "set_output_format",
    "set_output_file",
    "cast_input_format",
];

/// Dispatch an external function call for format/output file control.
pub fn dispatch(
    name: &str,
    args: Vec<MontyObject>,
    format_override: &Arc<Mutex<Option<String>>>,
    output_file: &Arc<Mutex<Option<String>>>,
) -> Result<MontyObject> {
    match name {
        "set_input_format" => dispatch_set_input_format(args, format_override),
        "set_output_format" => dispatch_set_output_format(args, format_override),
        "set_output_file" => dispatch_set_output_file(args, output_file),
        "cast_input_format" => dispatch_cast_input_format(args, format_override),
        _ => bail!("Unknown format_control function: {name}"),
    }
}

/// set_input_format(name) — re-parses the result with the given format between chain steps.
///
/// Only accepts parseable DataFormat names (json, yaml, toml, csv, txt, lines, ndjson).
/// Raw and Http are output-only / input-only respectively and cannot be used here.
fn dispatch_set_input_format(
    args: Vec<MontyObject>,
    format_override: &Arc<Mutex<Option<String>>>,
) -> Result<MontyObject> {
    if args.len() != 1 {
        bail!(
            "set_input_format() takes 1 argument (format name), got {}",
            args.len()
        );
    }
    let name = match &args[0] {
        MontyObject::String(s) => s.clone(),
        _ => bail!("set_input_format() expects a string argument"),
    };

    // raw is output-only — use set_output_format("raw") for binary output.
    if name == "raw" {
        bail!("set_input_format(\"raw\") is invalid: raw is output-only; use set_output_format(\"raw\") for binary output");
    }

    crate::format::parse_format_name(&name)?;

    let mut lock = format_override.lock().unwrap();
    *lock = Some(name);
    Ok(MontyObject::None)
}

/// cast_input_format(name, value) — like set_input_format() but returns `value`.
///
/// Allows combining the format hint and the return value in a single expression:
///   `cast_input_format("json", data["body"])`
/// is equivalent to:
///   `set_input_format("json"); return data["body"]`
fn dispatch_cast_input_format(
    args: Vec<MontyObject>,
    format_override: &Arc<Mutex<Option<String>>>,
) -> Result<MontyObject> {
    if args.len() != 2 {
        bail!(
            "cast_input_format() takes 2 arguments (format name, value), got {}",
            args.len()
        );
    }
    let name = match &args[0] {
        MontyObject::String(s) => s.clone(),
        _ => bail!("cast_input_format() expects a string as first argument"),
    };

    if name == "raw" {
        bail!("cast_input_format(\"raw\") is invalid: raw is output-only; use set_output_format(\"raw\") for binary output");
    }

    crate::format::parse_format_name(&name)?;

    let mut lock = format_override.lock().unwrap();
    *lock = Some(name);
    Ok(args.into_iter().nth(1).unwrap())
}

/// set_output_format(name) — sets the output format for the final step.
///
/// Accepts all DataFormat names including "raw" (binary pass-through).
/// Unlike set_input_format(), this is purely an output directive and cannot re-parse
/// data between intermediate chain steps.
fn dispatch_set_output_format(
    args: Vec<MontyObject>,
    format_override: &Arc<Mutex<Option<String>>>,
) -> Result<MontyObject> {
    if args.len() != 1 {
        bail!(
            "set_output_format() takes 1 argument (format name), got {}",
            args.len()
        );
    }
    let name = match &args[0] {
        MontyObject::String(s) => s.clone(),
        _ => bail!("set_output_format() expects a string argument"),
    };

    crate::format::parse_format_name(&name)?;

    let mut lock = format_override.lock().unwrap();
    *lock = Some(name);
    Ok(MontyObject::None)
}

/// set_output_file(path) — stores the output file path in the mutex, returns None.
///
/// When set, the final output is written to this file instead of stdout or the -o path.
/// Useful in molds that determine the output filename dynamically.
fn dispatch_set_output_file(
    args: Vec<MontyObject>,
    output_file: &Arc<Mutex<Option<String>>>,
) -> Result<MontyObject> {
    if args.len() != 1 {
        bail!(
            "set_output_file() takes 1 argument (file path), got {}",
            args.len()
        );
    }
    let path = match &args[0] {
        MontyObject::String(s) => s.clone(),
        _ => bail!("set_output_file() expects a string argument"),
    };
    if path.is_empty() {
        bail!("set_output_file() path must not be empty");
    }
    let mut lock = output_file.lock().unwrap();
    *lock = Some(path);
    Ok(MontyObject::None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::type_complexity)]
    fn mk() -> (Arc<Mutex<Option<String>>>, Arc<Mutex<Option<String>>>) {
        (Arc::new(Mutex::new(None)), Arc::new(Mutex::new(None)))
    }

    // ── set_input_format ──────────────────────────────────────────────────────

    #[test]
    fn test_set_input_format_stores_name() {
        let (fmt, out) = mk();
        dispatch(
            "set_input_format",
            vec![MontyObject::String("json".to_string())],
            &fmt,
            &out,
        )
        .unwrap();
        assert_eq!(*fmt.lock().unwrap(), Some("json".to_string()));
    }

    #[test]
    fn test_set_input_format_rejects_raw() {
        let (fmt, out) = mk();
        let err = dispatch(
            "set_input_format",
            vec![MontyObject::String("raw".to_string())],
            &fmt,
            &out,
        )
        .unwrap_err();
        // "raw" is output-only — set_output_format("raw") must be used instead
        assert!(err.to_string().contains("output-only"));
    }

    #[test]
    fn test_set_input_format_validates_name() {
        let (fmt, out) = mk();
        let err = dispatch(
            "set_input_format",
            vec![MontyObject::String("invalid_format".to_string())],
            &fmt,
            &out,
        )
        .unwrap_err();
        assert!(err.to_string().contains("Unknown format"));
    }

    #[test]
    fn test_set_input_format_wrong_type() {
        let (fmt, out) = mk();
        assert!(dispatch("set_input_format", vec![MontyObject::Int(42)], &fmt, &out).is_err());
    }

    #[test]
    fn test_set_input_format_wrong_arg_count() {
        let (fmt, out) = mk();
        assert!(dispatch("set_input_format", vec![], &fmt, &out).is_err());
    }

    // ── cast_input_format ─────────────────────────────────────────────────────

    #[test]
    fn test_cast_input_format_stores_name_and_returns_value() {
        let (fmt, out) = mk();
        let result = dispatch(
            "cast_input_format",
            vec![
                MontyObject::String("json".to_string()),
                MontyObject::String("the_body".to_string()),
            ],
            &fmt,
            &out,
        )
        .unwrap();
        assert_eq!(*fmt.lock().unwrap(), Some("json".to_string()));
        assert_eq!(result, MontyObject::String("the_body".to_string()));
    }

    #[test]
    fn test_cast_input_format_rejects_raw() {
        let (fmt, out) = mk();
        let err = dispatch(
            "cast_input_format",
            vec![MontyObject::String("raw".to_string()), MontyObject::None],
            &fmt,
            &out,
        )
        .unwrap_err();
        assert!(err.to_string().contains("output-only"));
    }

    #[test]
    fn test_cast_input_format_wrong_arg_count() {
        let (fmt, out) = mk();
        assert!(dispatch(
            "cast_input_format",
            vec![MontyObject::String("json".to_string())],
            &fmt,
            &out,
        )
        .is_err());
    }

    // ── set_output_format ─────────────────────────────────────────────────────

    #[test]
    fn test_set_output_format_raw_is_valid() {
        let (fmt, out) = mk();
        dispatch(
            "set_output_format",
            vec![MontyObject::String("raw".to_string())],
            &fmt,
            &out,
        )
        .unwrap();
        assert_eq!(*fmt.lock().unwrap(), Some("raw".to_string()));
    }

    #[test]
    fn test_set_output_format_accepts_standard_formats() {
        for name in &[
            "json",
            "json-compact",
            "yaml",
            "toml",
            "csv",
            "txt",
            "lines",
            "raw",
        ] {
            let (fmt, out) = mk();
            dispatch(
                "set_output_format",
                vec![MontyObject::String(name.to_string())],
                &fmt,
                &out,
            )
            .unwrap();
            assert_eq!(*fmt.lock().unwrap(), Some(name.to_string()));
        }
    }

    #[test]
    fn test_set_output_format_rejects_unknown() {
        let (fmt, out) = mk();
        let err = dispatch(
            "set_output_format",
            vec![MontyObject::String("xml".to_string())],
            &fmt,
            &out,
        )
        .unwrap_err();
        assert!(err.to_string().contains("Unknown format"));
    }

    // ── set_output_file ───────────────────────────────────────────────────────

    #[test]
    fn test_set_output_file_stores_path() {
        let (fmt, out) = mk();
        let result = dispatch(
            "set_output_file",
            vec![MontyObject::String("output.json".to_string())],
            &fmt,
            &out,
        )
        .unwrap();
        assert_eq!(result, MontyObject::None);
        assert_eq!(*out.lock().unwrap(), Some("output.json".to_string()));
    }

    #[test]
    fn test_set_output_file_empty_path_rejected() {
        let (fmt, out) = mk();
        let err = dispatch(
            "set_output_file",
            vec![MontyObject::String(String::new())],
            &fmt,
            &out,
        )
        .unwrap_err();
        assert!(err.to_string().contains("must not be empty"));
    }

    #[test]
    fn test_set_output_file_wrong_type() {
        let (fmt, out) = mk();
        assert!(dispatch("set_output_file", vec![MontyObject::Int(1)], &fmt, &out).is_err());
    }
}
