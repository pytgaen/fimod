use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::convert;
use crate::engine;
use crate::format::{self, DataFormat};
use crate::mold::MoldSource;
use anyhow::{bail, Context, Result};

/// Metadata from an optional `{case}.run-test.toml` alongside input/expected files.
///
/// ```toml
/// # Redirect to a different input file (relative to test dir)
/// input_file = "basic.input.json"
///
/// # Expected exit code (default 0)
/// exit_code = 1
///
/// # Override --output-format or --input-format
/// output_format = "json-compact"
/// input_format = "json"
///
/// # Skip this case entirely
/// skip = true
///
/// [args]       # --arg key=value pairs
/// field = "dept"
///
/// [env_vars]   # process env vars injected for this run
/// MY_VAR = "value"
/// ```
#[derive(serde::Deserialize, Default)]
struct CaseMeta {
    input_file: Option<String>,
    expected_file: Option<String>,
    #[serde(default)]
    exit_code: i32,
    output_format: Option<String>,
    input_format: Option<String>,
    #[serde(default)]
    args: HashMap<String, String>,
    #[serde(default)]
    env_vars: HashMap<String, String>,
    #[serde(default)]
    skip: bool,
}

struct TestCase {
    /// Display name (e.g. `basic`)
    name: String,
    input_path: PathBuf,
    expected_path: PathBuf,
    meta: CaseMeta,
}

/// Scan `tests_dir` for `*.input.*` / `*.expected.*` pairs, enriched by
/// optional `*.run-test.toml` files.
fn discover_test_cases(tests_dir: &str) -> Result<Vec<TestCase>> {
    let dir = Path::new(tests_dir);
    if !dir.is_dir() {
        bail!("Not a directory: {tests_dir}");
    }

    let mut all_files: Vec<PathBuf> = fs::read_dir(dir)
        .with_context(|| format!("Cannot read directory: {tests_dir}"))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    all_files.sort();

    // Pass 1: collect *.run-test.toml metadata keyed by case name
    let mut metas: HashMap<String, CaseMeta> = HashMap::new();
    for path in &all_files {
        let fname = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if let Some(case_name) = fname.strip_suffix(".run-test.toml") {
            let content = fs::read_to_string(path)
                .with_context(|| format!("Cannot read: {}", path.display()))?;
            let meta: CaseMeta = toml::from_str(&content)
                .with_context(|| format!("Invalid TOML in: {}", path.display()))?;
            metas.insert(case_name.to_string(), meta);
        }
    }

    // Pass 2: discover *.input.* files and build cases
    let mut cases = Vec::new();
    for path in &all_files {
        let fname = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };

        let Some(dot_input_pos) = fname.find(".input.") else {
            continue;
        };
        let base = &fname[..dot_input_pos];
        let ext = &fname[dot_input_pos + ".input.".len()..];

        let meta = metas.remove(base).unwrap_or_default();

        if meta.skip {
            continue;
        }

        let input_path = if let Some(ref f) = meta.input_file {
            dir.join(f)
        } else {
            path.clone()
        };

        let expected_path = if let Some(ref f) = meta.expected_file {
            dir.join(f)
        } else {
            let expected_same = dir.join(format!("{base}.expected.{ext}"));
            if expected_same.is_file() {
                expected_same
            } else {
                let prefix = format!("{base}.expected.");
                match all_files.iter().find(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with(&prefix))
                        .unwrap_or(false)
                }) {
                    Some(p) => p.clone(),
                    None => {
                        eprintln!("[warn] no expected file for: {fname}");
                        continue;
                    }
                }
            }
        };

        cases.push(TestCase {
            name: base.to_string(),
            input_path,
            expected_path,
            meta,
        });
    }

    Ok(cases)
}

/// Run a single test case. Returns `Ok(None)` on pass, `Ok(Some(msg))` on failure.
fn run_case(script: &str, case: &TestCase, mold_base_dir: Option<&str>) -> Result<Option<String>> {
    let input_str = case.input_path.to_str().unwrap_or("");
    let input_content = fs::read_to_string(&case.input_path)
        .with_context(|| format!("Cannot read: {}", case.input_path.display()))?;

    let input_fmt = format::resolve_format(
        case.meta.input_format.as_deref(),
        Some(input_str),
        DataFormat::Json,
    )?;
    let input_data = input_fmt.parse(&input_content)?;

    let extra_args: Vec<(String, String)> = {
        let mut v: Vec<_> = case
            .meta
            .args
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    };

    let env_value: serde_json::Value = serde_json::to_value(&case.meta.env_vars).unwrap();

    // Temporarily inject env_vars into the process environment
    for (k, v) in &case.meta.env_vars {
        std::env::set_var(k, v);
    }

    let opts = engine::MoldOptions {
        extra_args: &extra_args,
        env_value: &env_value,
        headers_value: &serde_json::Value::Null,
        debug: false,
        msg_level: 1,
        mold_base_dir,
    };
    let execute_result = engine::execute_mold(script, convert::json_to_monty(&input_data), &opts);

    // Always clean up env vars, even if execute_mold returned an error
    for k in case.meta.env_vars.keys() {
        std::env::remove_var(k);
    }

    let (result, exit_code, fmt_override, _out_file) = execute_result?;

    let actual_exit = exit_code.unwrap_or(0);
    if actual_exit != case.meta.exit_code {
        return Ok(Some(format!(
            "exit code: expected {}, got {actual_exit}",
            case.meta.exit_code
        )));
    }

    let expected_str = case.expected_path.to_str().unwrap_or("");
    let expected_content = fs::read_to_string(&case.expected_path)
        .with_context(|| format!("Cannot read: {}", case.expected_path.display()))?;

    let output_fmt = format::resolve_format(
        fmt_override
            .as_deref()
            .or(case.meta.output_format.as_deref()),
        Some(expected_str),
        input_fmt,
    )?;

    let serialized = output_fmt.serialize(&result)?;

    // JSON-aware comparison: ignore whitespace/indentation differences
    let ext = case
        .expected_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let equal = if ext == "json" {
        let e = serde_json::from_str::<serde_json::Value>(&expected_content);
        let a = serde_json::from_str::<serde_json::Value>(&serialized);
        match (e, a) {
            (Ok(ev), Ok(av)) => ev == av,
            _ => expected_content.trim() == serialized.trim(),
        }
    } else {
        expected_content.trim() == serialized.trim()
    };

    if equal {
        Ok(None)
    } else {
        Ok(Some(format!(
            "output mismatch\n    expected: {}\n    got:      {}",
            expected_content.trim(),
            serialized.trim()
        )))
    }
}

/// Entry point for `fimod mold test <mold> <tests_dir>`.
pub fn run(mold_path: &str, tests_dir: &str) -> Result<()> {
    let cases = discover_test_cases(tests_dir)?;

    if cases.is_empty() {
        println!("No test cases found in {tests_dir}");
        return Ok(());
    }

    let source = MoldSource::from_mold_str(mold_path, false)?;
    let base_dir = source.base_dir();
    let script = source.load(false)?;

    let mut passed = 0usize;
    let mut failed = 0usize;

    for case in &cases {
        match run_case(&script, case, base_dir.as_deref()) {
            Ok(None) => {
                println!("  ✓ {}", case.name);
                passed += 1;
            }
            Ok(Some(msg)) => {
                println!("  ✗ {} — {msg}", case.name);
                failed += 1;
            }
            Err(e) => {
                println!("  ✗ {} — error: {:#}", case.name, e);
                failed += 1;
            }
        }
    }

    println!();
    if failed == 0 {
        println!(
            "{} test{} passed",
            passed,
            if passed == 1 { "" } else { "s" }
        );
    } else {
        println!(
            "{} test{}, {} passed, {} failed",
            cases.len(),
            if cases.len() == 1 { "" } else { "s" },
            passed,
            failed
        );
        std::process::exit(1);
    }

    Ok(())
}
