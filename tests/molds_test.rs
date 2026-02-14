use predicates::prelude::*;
use std::{collections::HashMap, fs, path::Path};

// ── Fixture runner ────────────────────────────────────────────────────────────

/// Metadata from an optional `{case}.toml` alongside input/expected files.
///
/// ```toml
/// # File: basic.run-test.toml
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
/// # --env patterns exposed to the mold
/// env_patterns = ["MY_PREFIX_*"]
///
/// # Skip this case entirely
/// skip = true
///
/// [args]       # --arg key=value pairs
/// field = "dept"
///
/// [env_vars]   # process env vars injected for this run
/// MY_PREFIX_FOO = "bar"
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
    env_patterns: Vec<String>,
    #[serde(default)]
    args: HashMap<String, String>,
    #[serde(default)]
    env_vars: HashMap<String, String>,
    #[serde(default)]
    skip: bool,
}

fn find_file_with_middle(dir: &Path, stem: &str, middle: &str) -> Option<std::path::PathBuf> {
    let prefix = format!("{stem}{middle}");
    fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .find(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.starts_with(&prefix))
                .unwrap_or(false)
        })
        .map(|e| e.path())
}

/// JSON-aware comparison: parses both sides and compares as `serde_json::Value`
/// so whitespace / indentation differences are ignored.
/// Falls back to trimmed string equality for non-JSON formats.
fn outputs_equal(expected: &str, actual: &str, ext: &str) -> bool {
    match ext {
        "json" => {
            let e = serde_json::from_str::<serde_json::Value>(expected);
            let a = serde_json::from_str::<serde_json::Value>(actual);
            match (e, a) {
                (Ok(ev), Ok(av)) => ev == av,
                _ => expected.trim() == actual.trim(),
            }
        }
        _ => expected.trim() == actual.trim(),
    }
}

fn run_fixture(
    mold_name: &str,
    case_name: &str,
    test_dir: &Path,
    meta: &CaseMeta,
) -> Result<(), String> {
    let mold_path = format!("molds/{mold_name}");

    let input_path = match &meta.input_file {
        Some(f) => test_dir.join(f),
        None => find_file_with_middle(test_dir, case_name, ".input.")
            .ok_or_else(|| format!("[{mold_name}/{case_name}] no input file found"))?,
    };

    let expected_path = match &meta.expected_file {
        Some(f) => test_dir.join(f),
        None => find_file_with_middle(test_dir, case_name, ".expected.")
            .ok_or_else(|| format!("[{mold_name}/{case_name}] no expected file found"))?,
    };

    let expected = fs::read_to_string(&expected_path)
        .map_err(|e| format!("[{mold_name}/{case_name}] cannot read expected: {e}"))?;
    let expected_ext = expected_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("txt");

    let mut cmd = assert_cmd::cargo_bin_cmd!("fimod");
    cmd.arg("shape")
        .args(["-i", input_path.to_str().unwrap(), "-m", &mold_path]);

    let mut sorted_args: Vec<_> = meta.args.iter().collect();
    sorted_args.sort_by_key(|(k, _)| k.as_str());
    for (k, v) in sorted_args {
        cmd.args(["--arg", &format!("{k}={v}")]);
    }

    for (k, v) in &meta.env_vars {
        cmd.env(k, v);
    }
    for pattern in &meta.env_patterns {
        cmd.args(["--env", pattern]);
    }

    if let Some(ref fmt) = meta.output_format {
        cmd.args(["--output-format", fmt]);
    }
    if let Some(ref fmt) = meta.input_format {
        cmd.args(["--input-format", fmt]);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("[{mold_name}/{case_name}] failed to run fimod: {e}"))?;

    let actual_code = output.status.code().unwrap_or(-1);
    if actual_code != meta.exit_code {
        return Err(format!(
            "[{}/{}] exit code: expected {}, got {}\nstderr:\n{}",
            mold_name,
            case_name,
            meta.exit_code,
            actual_code,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let actual = String::from_utf8_lossy(&output.stdout);
    if !outputs_equal(&expected, &actual, expected_ext) {
        return Err(format!(
            "[{}/{}] output mismatch\n--- expected ---\n{}\n--- actual ---\n{}",
            mold_name,
            case_name,
            expected.trim(),
            actual.trim()
        ));
    }

    Ok(())
}

fn discover_cases(test_dir: &Path) -> Vec<(String, CaseMeta)> {
    let mut cases: HashMap<String, CaseMeta> = HashMap::new();
    let Ok(entries) = fs::read_dir(test_dir) else {
        return vec![];
    };
    let mut files: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    files.sort_by_key(|e| e.file_name());

    // Pass 1: *.run-test.toml files → explicit case definitions (with args / env / etc.)
    for entry in &files {
        let path = entry.path();
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !filename.ends_with(".run-test.toml") {
            continue;
        }
        let case_name = filename
            .strip_suffix(".run-test.toml")
            .unwrap_or("")
            .to_string();
        let content = fs::read_to_string(&path).unwrap_or_default();
        let meta: CaseMeta = toml::from_str(&content).unwrap_or_default();
        cases.insert(case_name, meta);
    }

    // Pass 2: *.input.* files → implicit cases with no args (unless TOML already defines them)
    for entry in &files {
        let name = entry.file_name();
        let name = name.to_str().unwrap_or("");
        if let Some(idx) = name.find(".input.") {
            let case_name = &name[..idx];
            if !cases.contains_key(case_name) {
                cases.insert(case_name.to_string(), CaseMeta::default());
            }
        }
    }

    let mut result: Vec<_> = cases.into_iter().collect();
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

/// Auto-discover and run all fixture cases under `tests-molds/`.
///
/// Each subdirectory maps to a mold (`molds/{name}/`). Cases come from
/// `*.input.*` + `*.expected.*` pairs. An optional `{case}.run-test.toml` enriches
/// a case with `--arg`, `--env`, exit code, or format overrides.
#[test]
fn test_mold_fixtures() {
    let fixtures_root = Path::new("tests-molds");
    let mut failures: Vec<String> = Vec::new();
    let mut ran = 0usize;

    let mut mold_dirs: Vec<_> = fs::read_dir(fixtures_root)
        .expect("tests-molds/ directory not found")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    mold_dirs.sort_by_key(|e| e.file_name());

    for mold_entry in mold_dirs {
        let test_dir = mold_entry.path();
        let mold_name = test_dir.file_name().unwrap().to_str().unwrap().to_string();

        if !Path::new(&format!("molds/{mold_name}")).exists() {
            continue;
        }

        for (case_name, meta) in discover_cases(&test_dir) {
            if meta.skip {
                continue;
            }
            ran += 1;
            if let Err(e) = run_fixture(&mold_name, &case_name, &test_dir, &meta) {
                failures.push(e);
            }
        }
    }

    assert!(ran > 0, "No fixture cases discovered — check tests-molds/");
    assert!(
        failures.is_empty(),
        "{} fixture(s) failed:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

// ── Monty behaviour probes ────────────────────────────────────────────────────

/// Verify that dict.update() works in Monty (in-place merge).
/// Equivalent Python: data.update({"b": 2}); return data
#[test]
fn test_monty_dict_update() {
    let expr =
        "def transform(data, args, env, headers):\n    data.update({\"b\": 2})\n    return data";
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", "tests/data/monty/dict_a.json", "-e", expr])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"a\""))
        .stdout(predicate::str::contains("\"b\""));
}

// ── Hand-written mold tests ───────────────────────────────────────────────────

#[test]
fn test_poetry_to_uv_conversion_target_poetry() {
    let input = "tests/data/molds/poetry_to_uv/input.toml";
    let expected = "tests/data/molds/poetry_to_uv/expected_poetry.toml";
    let mold = "molds/poetry_migrate";

    let expected_content =
        std::fs::read_to_string(expected).expect("Failed to read expected output file");

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", input, "-m", mold, "--arg", "target=poetry"])
        .assert()
        .success()
        .stdout(predicate::str::contains(expected_content.trim()));
}

#[test]
fn test_poetry_to_uv_conversion_target_uv() {
    let input = "tests/data/molds/poetry_to_uv/input.toml";
    let expected = "tests/data/molds/poetry_to_uv/expected_uv.toml";
    let mold = "molds/poetry_migrate";

    let expected_content =
        std::fs::read_to_string(expected).expect("Failed to read expected output file");

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", input, "-m", mold, "--arg", "target=uv"])
        .assert()
        .success()
        .stdout(predicate::str::contains(expected_content.trim()));
}

#[test]
fn test_skylos_to_gitlab_conversion() {
    let input = "tests/data/molds/skylos_to_gitlab/input.json";
    let expected = "tests/data/molds/skylos_to_gitlab/expected.json";
    let mold = "molds/skylos_to_gitlab";

    let expected_content =
        std::fs::read_to_string(expected).expect("Failed to read expected output file");

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", input, "-m", mold])
        .assert()
        .success()
        .stdout(predicate::str::contains(expected_content.trim()));
}
