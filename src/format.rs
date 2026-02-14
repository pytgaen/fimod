use std::io::Cursor;
use std::path::Path;

use anyhow::{bail, Context, Result};
use monty::{DictPairs, MontyObject};
use serde_json::Value;

/// Supported data formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataFormat {
    Json,
    JsonCompact,
    Ndjson,
    Yaml,
    Toml,
    Csv,
    Txt,
    Lines,
    Raw,
    Http,
}

/// CSV-specific options.
#[derive(Debug, Clone)]
pub struct CsvOptions {
    /// Field delimiter for input (default: ',')
    pub delimiter: u8,
    /// Field delimiter for output (if None, uses `delimiter`)
    pub output_delimiter: Option<u8>,
    /// Don't read the first line as header (generate col0, col1, ...)
    pub no_input_header: bool,
    /// Don't write a header line in output
    pub no_output_header: bool,
    /// Explicit column names for input (implies no_input_header)
    pub header_names: Option<Vec<String>>,
}

impl CsvOptions {
    /// Returns the effective output delimiter (falls back to input delimiter).
    pub fn effective_output_delimiter(&self) -> u8 {
        self.output_delimiter.unwrap_or(self.delimiter)
    }
}

impl Default for CsvOptions {
    fn default() -> Self {
        Self {
            delimiter: b',',
            output_delimiter: None,
            no_input_header: false,
            no_output_header: false,
            header_names: None,
        }
    }
}

impl DataFormat {
    /// Detect format from file extension. Returns None if unknown.
    pub fn from_extension(path: &str) -> Option<DataFormat> {
        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        match ext.as_deref() {
            Some("json") => Some(DataFormat::Json),
            Some("ndjson" | "jsonl") => Some(DataFormat::Ndjson),
            Some("yaml" | "yml") => Some(DataFormat::Yaml),
            Some("toml") => Some(DataFormat::Toml),
            Some("csv" | "tsv") => Some(DataFormat::Csv),
            Some("txt" | "text") => Some(DataFormat::Txt),
            _ => None,
        }
    }

    /// Parse a string into a serde_json::Value according to this format.
    /// For CSV, use `parse_csv()` instead to pass options.
    pub fn parse(&self, content: &str) -> Result<Value> {
        match self {
            DataFormat::Json | DataFormat::JsonCompact => {
                serde_json::from_str(content).context("Failed to parse JSON")
            }
            DataFormat::Ndjson => {
                let values: Result<Vec<Value>> = content
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| {
                        serde_json::from_str(l).with_context(|| {
                            format!(
                                "Failed to parse NDJSON line: {}",
                                l.chars().take(50).collect::<String>()
                            )
                        })
                    })
                    .collect();
                Ok(Value::Array(values?))
            }
            DataFormat::Yaml => serde_norway::from_str(content).context("Failed to parse YAML"),
            DataFormat::Toml => {
                let toml_value: toml::Value =
                    toml::from_str(content).context("Failed to parse TOML")?;
                let json_str =
                    serde_json::to_string(&toml_value).context("Failed to convert TOML to JSON")?;
                serde_json::from_str(&json_str).context("Failed to parse converted TOML")
            }
            DataFormat::Csv => {
                bail!("Use parse_csv() with CsvOptions for CSV format")
            }
            DataFormat::Txt => Ok(Value::String(content.to_string())),
            DataFormat::Lines => {
                let lines: Vec<Value> = content
                    .lines()
                    .map(|l| Value::String(l.to_string()))
                    .collect();
                Ok(Value::Array(lines))
            }
            DataFormat::Raw => bail!("Raw format is output-only and cannot be used for input"),
            DataFormat::Http => {
                bail!("HTTP format is handled directly in the pipeline, not via parse()")
            }
        }
    }

    /// Serialize a serde_json::Value into a string according to this format.
    /// For CSV, use `serialize_csv()` instead to pass options.
    pub fn serialize(&self, value: &Value) -> Result<String> {
        match self {
            DataFormat::Json => serde_json::to_string_pretty(value)
                .context("Failed to serialize to JSON")
                .map(|mut s| {
                    s.push('\n');
                    s
                }),
            DataFormat::JsonCompact => serde_json::to_string(value)
                .context("Failed to serialize to compact JSON")
                .map(|mut s| {
                    s.push('\n');
                    s
                }),
            DataFormat::Ndjson => match value {
                Value::Array(arr) => {
                    let mut out = String::new();
                    for item in arr {
                        let line = serde_json::to_string(item)
                            .context("Failed to serialize NDJSON line")?;
                        out.push_str(&line);
                        out.push('\n');
                    }
                    Ok(out)
                }
                other => {
                    let line =
                        serde_json::to_string(other).context("Failed to serialize NDJSON")?;
                    Ok(format!("{line}\n"))
                }
            },
            DataFormat::Yaml => {
                serde_norway::to_string(value).context("Failed to serialize to YAML")
            }
            DataFormat::Toml => {
                toml::to_string_pretty(value).context("Failed to serialize to TOML")
            }
            DataFormat::Csv => {
                bail!("Use serialize_csv() with CsvOptions for CSV format")
            }
            DataFormat::Txt => match value {
                Value::String(s) => Ok(s.clone()),
                other => serde_json::to_string(other).context("Failed to stringify for TXT"),
            },
            DataFormat::Lines => match value {
                Value::Array(arr) => {
                    let mut out = String::new();
                    for item in arr {
                        match item {
                            Value::String(s) => out.push_str(s),
                            other => {
                                let s = serde_json::to_string(other)
                                    .context("Failed to stringify line")?;
                                out.push_str(&s);
                            }
                        }
                        out.push('\n');
                    }
                    Ok(out)
                }
                Value::String(s) => Ok(format!("{s}\n")),
                other => {
                    let s =
                        serde_json::to_string(other).context("Failed to stringify for lines")?;
                    Ok(format!("{s}\n"))
                }
            },
            DataFormat::Raw => {
                bail!("Raw format is handled as binary pass-through, not via serialize()")
            }
            DataFormat::Http => {
                bail!("HTTP format is input-only and cannot be used for output")
            }
        }
    }
}

impl std::fmt::Display for DataFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataFormat::Json => write!(f, "json"),
            DataFormat::JsonCompact => write!(f, "json-compact"),
            DataFormat::Ndjson => write!(f, "ndjson"),
            DataFormat::Yaml => write!(f, "yaml"),
            DataFormat::Toml => write!(f, "toml"),
            DataFormat::Csv => write!(f, "csv"),
            DataFormat::Txt => write!(f, "txt"),
            DataFormat::Lines => write!(f, "lines"),
            DataFormat::Raw => write!(f, "raw"),
            DataFormat::Http => write!(f, "http"),
        }
    }
}

/// Parse a format name string (from CLI args).
pub fn parse_format_name(name: &str) -> Result<DataFormat> {
    match name.to_lowercase().as_str() {
        "json" => Ok(DataFormat::Json),
        "json-compact" => Ok(DataFormat::JsonCompact),
        "ndjson" | "jsonl" => Ok(DataFormat::Ndjson),
        "yaml" | "yml" => Ok(DataFormat::Yaml),
        "toml" => Ok(DataFormat::Toml),
        "csv" | "tsv" => Ok(DataFormat::Csv),
        "txt" => Ok(DataFormat::Txt),
        "lines" => Ok(DataFormat::Lines),
        "raw" => Ok(DataFormat::Raw),
        "http" => Ok(DataFormat::Http),
        _ => bail!(
            "Unknown format: '{name}'. Supported: json, json-compact, ndjson, yaml, toml, csv, txt, lines, raw, http"
        ),
    }
}

/// Resolve the format to use: explicit CLI arg > file extension > fallback.
pub fn resolve_format(
    explicit: Option<&str>,
    file_path: Option<&str>,
    fallback: DataFormat,
) -> Result<DataFormat> {
    if let Some(name) = explicit {
        return parse_format_name(name);
    }
    if let Some(path) = file_path {
        if let Some(fmt) = DataFormat::from_extension(path) {
            return Ok(fmt);
        }
    }
    Ok(fallback)
}

/// Parse CSV content into a serde_json::Value.
///
/// Returns `(Value, Option<Vec<String>>)`:
/// - When headers are present (from file or --csv-header): rows are objects, returns `Some(headers)`
/// - When headerless (--csv-no-input-header without --csv-header): rows are arrays, returns `None`
pub fn parse_csv(content: &str, opts: &CsvOptions) -> Result<(Value, Option<Vec<String>>)> {
    let cursor = Cursor::new(content.as_bytes());
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(opts.delimiter)
        .has_headers(!opts.no_input_header && opts.header_names.is_none())
        .from_reader(cursor);

    // Determine if we have named headers or are in headerless mode
    let header_mode: HeaderMode = if let Some(ref names) = opts.header_names {
        HeaderMode::Named(names.clone())
    } else if opts.no_input_header {
        HeaderMode::Headerless
    } else {
        let h = rdr
            .headers()
            .context("Failed to read CSV headers")?
            .iter()
            .map(|h| h.to_string())
            .collect();
        HeaderMode::Named(h)
    };

    match header_mode {
        HeaderMode::Named(headers) => {
            let record_to_obj = |record: &csv::StringRecord| -> Value {
                let mut obj = serde_json::Map::new();
                for (i, field) in record.iter().enumerate() {
                    let key = if i < headers.len() {
                        headers[i].clone()
                    } else {
                        format!("col{i}")
                    };
                    obj.insert(key, Value::String(field.to_string()));
                }
                Value::Object(obj)
            };

            let mut rows = Vec::new();
            for result in rdr.records() {
                let record = result.context("Failed to read CSV record")?;
                rows.push(record_to_obj(&record));
            }
            Ok((Value::Array(rows), Some(headers)))
        }
        HeaderMode::Headerless => {
            let record_to_array = |record: &csv::StringRecord| -> Value {
                Value::Array(
                    record
                        .iter()
                        .map(|f| Value::String(f.to_string()))
                        .collect(),
                )
            };

            let mut rows = Vec::new();
            for result in rdr.records() {
                let record = result.context("Failed to read CSV record")?;
                rows.push(record_to_array(&record));
            }
            Ok((Value::Array(rows), None))
        }
    }
}

enum HeaderMode {
    Named(Vec<String>),
    Headerless,
}

/// Parse CSV content directly into a MontyObject, bypassing the serde_json::Value intermediate.
///
/// Returns `(MontyObject, Option<Vec<String>>)` with the same semantics as `parse_csv`:
/// - Named headers → `MontyObject::List` of `MontyObject::Dict` entries, returns `Some(headers)`
/// - Headerless → `MontyObject::List` of `MontyObject::Tuple` entries, returns `None`
///
/// This avoids the `parse_csv → json_to_monty` double allocation for the hot CSV→CSV path.
pub fn csv_to_monty(
    content: &str,
    opts: &CsvOptions,
) -> Result<(MontyObject, Option<Vec<String>>)> {
    let cursor = Cursor::new(content.as_bytes());
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(opts.delimiter)
        .has_headers(!opts.no_input_header && opts.header_names.is_none())
        .from_reader(cursor);

    let header_mode: HeaderMode = if let Some(ref names) = opts.header_names {
        HeaderMode::Named(names.clone())
    } else if opts.no_input_header {
        HeaderMode::Headerless
    } else {
        let h = rdr
            .headers()
            .context("Failed to read CSV headers")?
            .iter()
            .map(|h| h.to_string())
            .collect();
        HeaderMode::Named(h)
    };

    // Pre-allocate: assume ~60 bytes per row as a rough heuristic
    let estimated_rows = content.len() / 60;

    match header_mode {
        HeaderMode::Named(headers) => {
            let mut rows: Vec<MontyObject> = Vec::with_capacity(estimated_rows);
            for result in rdr.records() {
                let record = result.context("Failed to read CSV record")?;
                let pairs: Vec<(MontyObject, MontyObject)> = record
                    .iter()
                    .enumerate()
                    .map(|(i, field)| {
                        let key = if i < headers.len() {
                            headers[i].clone()
                        } else {
                            format!("col{i}")
                        };
                        (
                            MontyObject::String(key),
                            MontyObject::String(field.to_string()),
                        )
                    })
                    .collect();
                rows.push(MontyObject::Dict(DictPairs::from(pairs)));
            }
            Ok((MontyObject::List(rows), Some(headers)))
        }
        HeaderMode::Headerless => {
            let mut rows: Vec<MontyObject> = Vec::with_capacity(estimated_rows);
            for result in rdr.records() {
                let record = result.context("Failed to read CSV record")?;
                let fields: Vec<MontyObject> = record
                    .iter()
                    .map(|f| MontyObject::String(f.to_string()))
                    .collect();
                rows.push(MontyObject::Tuple(fields));
            }
            Ok((MontyObject::List(rows), None))
        }
    }
}

/// Serialize a serde_json::Value (expected: array of objects) to CSV string.
pub fn serialize_csv(value: &Value, opts: &CsvOptions) -> Result<String> {
    let rows = value.as_array().ok_or_else(|| {
        anyhow::anyhow!(
            "CSV output expects an array of objects, got {}. \
             Hint: your transform must return a list of dicts.",
            match value {
                Value::Object(_) => "an object",
                Value::String(_) => "a string",
                Value::Number(_) => "a number",
                Value::Bool(_) => "a boolean",
                Value::Null => "null",
                Value::Array(_) => unreachable!(),
            }
        )
    })?;

    if rows.is_empty() {
        return Ok(String::new());
    }

    let mut output = Vec::new();
    {
        let mut wtr = csv::WriterBuilder::new()
            .delimiter(opts.effective_output_delimiter())
            .from_writer(&mut output);

        // Determine column order from the first object's keys
        let first_obj = rows[0]
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("CSV output: each row must be an object"))?;
        let columns: Vec<String> = first_obj.keys().cloned().collect();

        // Write header if needed
        if !opts.no_output_header {
            wtr.write_record(&columns)
                .context("Failed to write CSV header")?;
        }

        // Write data rows
        for row in rows {
            let obj = row
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("CSV output: each row must be an object"))?;
            let fields: Vec<String> = columns
                .iter()
                .map(|col| {
                    obj.get(col)
                        .map(|v| match v {
                            Value::String(s) => s.clone(),
                            Value::Null => String::new(),
                            other => other.to_string(),
                        })
                        .unwrap_or_default()
                })
                .collect();
            wtr.write_record(&fields)
                .context("Failed to write CSV record")?;
        }

        wtr.flush().context("Failed to flush CSV writer")?;
    }

    String::from_utf8(output).context("CSV output is not valid UTF-8")
}

/// Parse the delimiter CLI arg. Supports '\t' for tab.
pub fn parse_delimiter(s: &str) -> Result<u8> {
    match s {
        "\\t" | "\t" => Ok(b'\t'),
        s if s.len() == 1 => Ok(s.as_bytes()[0]),
        _ => bail!("Invalid CSV delimiter: '{s}'. Must be a single character or '\\t' for tab."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_extension() {
        assert_eq!(
            DataFormat::from_extension("data.json"),
            Some(DataFormat::Json)
        );
        assert_eq!(
            DataFormat::from_extension("data.yaml"),
            Some(DataFormat::Yaml)
        );
        assert_eq!(
            DataFormat::from_extension("data.yml"),
            Some(DataFormat::Yaml)
        );
        assert_eq!(
            DataFormat::from_extension("data.toml"),
            Some(DataFormat::Toml)
        );
        assert_eq!(
            DataFormat::from_extension("data.csv"),
            Some(DataFormat::Csv)
        );
        assert_eq!(
            DataFormat::from_extension("data.tsv"),
            Some(DataFormat::Csv)
        );
        assert_eq!(
            DataFormat::from_extension("data.txt"),
            Some(DataFormat::Txt)
        );
        assert_eq!(DataFormat::from_extension("data.unknown"), None);
        assert_eq!(DataFormat::from_extension("noext"), None);
    }

    #[test]
    fn test_parse_serialize_json() {
        let input = r#"{"name": "Alice", "age": 30}"#;
        let value = DataFormat::Json.parse(input).unwrap();
        assert_eq!(value["name"], "Alice");
        assert_eq!(value["age"], 30);

        let output = DataFormat::Json.serialize(&value).unwrap();
        let reparsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(reparsed, value);
    }

    #[test]
    fn test_parse_serialize_yaml() {
        let input = "name: Alice\nage: 30\n";
        let value = DataFormat::Yaml.parse(input).unwrap();
        assert_eq!(value["name"], "Alice");
        assert_eq!(value["age"], 30);

        let output = DataFormat::Yaml.serialize(&value).unwrap();
        assert!(output.contains("name: Alice"));
    }

    #[test]
    fn test_parse_serialize_toml() {
        let input = "name = \"Alice\"\nage = 30\n";
        let value = DataFormat::Toml.parse(input).unwrap();
        assert_eq!(value["name"], "Alice");
        assert_eq!(value["age"], 30);

        let output = DataFormat::Toml.serialize(&value).unwrap();
        assert!(output.contains("name = \"Alice\""));
    }

    #[test]
    fn test_parse_csv_with_header() {
        let input = "name,age\nAlice,30\nBob,25\n";
        let opts = CsvOptions::default();
        let (value, headers) = parse_csv(input, &opts).unwrap();
        let arr = value.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["name"], "Alice");
        assert_eq!(arr[0]["age"], "30");
        assert_eq!(arr[1]["name"], "Bob");
        assert_eq!(headers, Some(vec!["name".to_string(), "age".to_string()]));
    }

    #[test]
    fn test_parse_csv_no_input_header() {
        let input = "Alice,30\nBob,25\n";
        let opts = CsvOptions {
            no_input_header: true,
            ..Default::default()
        };
        let (value, headers) = parse_csv(input, &opts).unwrap();
        assert!(headers.is_none());
        let arr = value.as_array().unwrap();
        // Headerless rows are arrays, not objects
        assert_eq!(arr[0], serde_json::json!(["Alice", "30"]));
        assert_eq!(arr[1], serde_json::json!(["Bob", "25"]));
    }

    #[test]
    fn test_parse_csv_custom_header() {
        let input = "Alice,30\nBob,25\n";
        let opts = CsvOptions {
            header_names: Some(vec!["prenom".to_string(), "age".to_string()]),
            ..Default::default()
        };
        let (value, headers) = parse_csv(input, &opts).unwrap();
        let arr = value.as_array().unwrap();
        assert_eq!(arr[0]["prenom"], "Alice");
        assert_eq!(arr[0]["age"], "30");
        assert_eq!(headers, Some(vec!["prenom".to_string(), "age".to_string()]));
    }

    #[test]
    fn test_serialize_csv_with_header() {
        let value = serde_json::json!([
            {"name": "Alice", "age": "30"},
            {"name": "Bob", "age": "25"}
        ]);
        let opts = CsvOptions::default();
        let output = serialize_csv(&value, &opts).unwrap();
        let lines: Vec<&str> = output.trim().split('\n').collect();
        assert!(lines[0].contains("name"));
        assert!(lines[0].contains("age"));
        assert_eq!(lines.len(), 3); // header + 2 rows
    }

    #[test]
    fn test_serialize_csv_no_output_header() {
        let value = serde_json::json!([
            {"name": "Alice", "age": "30"}
        ]);
        let opts = CsvOptions {
            no_output_header: true,
            ..Default::default()
        };
        let output = serialize_csv(&value, &opts).unwrap();
        let lines: Vec<&str> = output.trim().split('\n').collect();
        assert_eq!(lines.len(), 1); // no header, just 1 data row
        assert!(!output.contains("name")); // no header names
    }

    #[test]
    fn test_csv_delimiter_tab() {
        let input = "name\tage\nAlice\t30\n";
        let opts = CsvOptions {
            delimiter: b'\t',
            ..Default::default()
        };
        let (value, _) = parse_csv(input, &opts).unwrap();
        let arr = value.as_array().unwrap();
        assert_eq!(arr[0]["name"], "Alice");
        assert_eq!(arr[0]["age"], "30");
    }

    #[test]
    fn test_parse_txt() {
        let input = "Hello World\n";
        let value = DataFormat::Txt.parse(input).unwrap();
        assert_eq!(value, Value::String("Hello World\n".to_string()));
    }

    #[test]
    fn test_serialize_txt() {
        // String → raw output
        let value = Value::String("HELLO WORLD".to_string());
        let output = DataFormat::Txt.serialize(&value).unwrap();
        assert_eq!(output, "HELLO WORLD");

        // Non-string → JSON compact
        let value = serde_json::json!({"key": "value"});
        let output = DataFormat::Txt.serialize(&value).unwrap();
        assert_eq!(output, r#"{"key":"value"}"#);
    }

    #[test]
    fn test_parse_lines() {
        let input = "hello\nworld\n";
        let value = DataFormat::Lines.parse(input).unwrap();
        let arr = value.as_array().unwrap();
        assert_eq!(arr.len(), 2); // str::lines() strips trailing newline
        assert_eq!(arr[0], Value::String("hello".to_string()));
        assert_eq!(arr[1], Value::String("world".to_string()));
    }

    #[test]
    fn test_serialize_lines() {
        let value = serde_json::json!(["hello", "world"]);
        let output = DataFormat::Lines.serialize(&value).unwrap();
        assert_eq!(output, "hello\nworld\n");
    }

    #[test]
    fn test_serialize_lines_ndjson() {
        let value = serde_json::json!([{"name": "Alice"}, {"name": "Bob"}]);
        let output = DataFormat::Lines.serialize(&value).unwrap();
        assert!(output.contains(r#"{"name":"Alice"}"#));
        assert!(output.contains(r#"{"name":"Bob"}"#));
    }

    #[test]
    fn test_serialize_lines_single_value() {
        let value = Value::String("hello".to_string());
        let output = DataFormat::Lines.serialize(&value).unwrap();
        assert_eq!(output, "hello\n");

        let value = serde_json::json!(42);
        let output = DataFormat::Lines.serialize(&value).unwrap();
        assert_eq!(output, "42\n");
    }

    #[test]
    fn test_parse_format_name_lines() {
        let fmt = parse_format_name("lines").unwrap();
        assert_eq!(fmt, DataFormat::Lines);
    }

    #[test]
    fn test_detect_extension_ndjson() {
        assert_eq!(
            DataFormat::from_extension("data.ndjson"),
            Some(DataFormat::Ndjson)
        );
        assert_eq!(
            DataFormat::from_extension("data.jsonl"),
            Some(DataFormat::Ndjson)
        );
    }

    #[test]
    fn test_parse_ndjson() {
        let input = "{\"a\":1}\n{\"b\":2}\n{\"c\":3}\n";
        let value = DataFormat::Ndjson.parse(input).unwrap();
        let arr = value.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0]["a"], 1);
        assert_eq!(arr[1]["b"], 2);
        assert_eq!(arr[2]["c"], 3);
    }

    #[test]
    fn test_parse_ndjson_skip_empty() {
        let input = "{\"a\":1}\n\n{\"b\":2}\n\n";
        let value = DataFormat::Ndjson.parse(input).unwrap();
        let arr = value.as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn test_serialize_ndjson() {
        let value = serde_json::json!([{"name": "Alice"}, {"name": "Bob"}]);
        let output = DataFormat::Ndjson.serialize(&value).unwrap();
        let lines: Vec<&str> = output.trim_end().split('\n').collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"name\":\"Alice\""));
        assert!(lines[1].contains("\"name\":\"Bob\""));
    }

    #[test]
    fn test_serialize_ndjson_single() {
        let value = serde_json::json!({"x": 42});
        let output = DataFormat::Ndjson.serialize(&value).unwrap();
        assert_eq!(output, "{\"x\":42}\n");
    }

    #[test]
    fn test_parse_format_name_ndjson() {
        assert_eq!(parse_format_name("ndjson").unwrap(), DataFormat::Ndjson);
        assert_eq!(parse_format_name("jsonl").unwrap(), DataFormat::Ndjson);
    }

    #[test]
    fn test_resolve_format() {
        // Explicit takes priority
        let fmt = resolve_format(Some("yaml"), Some("data.json"), DataFormat::Json).unwrap();
        assert_eq!(fmt, DataFormat::Yaml);

        // Fall back to extension
        let fmt = resolve_format(None, Some("data.toml"), DataFormat::Json).unwrap();
        assert_eq!(fmt, DataFormat::Toml);

        // Fall back to default
        let fmt = resolve_format(None, None, DataFormat::Json).unwrap();
        assert_eq!(fmt, DataFormat::Json);

        // Unknown extension → fallback
        let fmt = resolve_format(None, Some("data.xyz"), DataFormat::Yaml).unwrap();
        assert_eq!(fmt, DataFormat::Yaml);
    }
}
