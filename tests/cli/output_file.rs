use super::helpers::{setup_input, setup_mold};
use predicates::prelude::*;

// ── set_output_file() basic behaviour ──────────────────────────────────────────

/// set_output_file() writes output to the specified file instead of stdout.
#[test]
fn test_set_output_file_redirects_to_file() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"name":"alice"}"#);
    let out_path = dir.path().join("result.json");

    let mold = setup_mold(
        &dir,
        "m.py",
        &format!(
            r#"
def transform(data, args, env, headers):
    set_output_file("{}")
    return data
"#,
            out_path.display()
        ),
    );

    // stdout should be empty (output went to file)
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    // file must exist with expected content
    let content = std::fs::read_to_string(&out_path).unwrap();
    assert!(content.contains("alice"));
}

/// set_output_file() overrides the -o flag.
#[test]
fn test_set_output_file_overrides_dash_o() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"value":42}"#);
    let cli_out = dir.path().join("cli.json");
    let mold_out = dir.path().join("mold.json");

    let mold = setup_mold(
        &dir,
        "m.py",
        &format!(
            r#"
def transform(data, args, env, headers):
    set_output_file("{}")
    return data
"#,
            mold_out.display()
        ),
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold, "-o", &cli_out.to_string_lossy()])
        .assert()
        .success();

    // mold-specified file exists
    let content = std::fs::read_to_string(&mold_out).unwrap();
    assert!(content.contains("42"));

    // CLI -o file was NOT written (mold path won)
    assert!(!cli_out.exists());
}

/// set_output_file() accepts a path with a subdirectory (must exist).
#[test]
fn test_set_output_file_with_explicit_path() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"x":1}"#);
    let subdir = dir.path().join("out");
    std::fs::create_dir_all(&subdir).unwrap();
    let out_path = subdir.join("result.yaml");

    let mold = setup_mold(
        &dir,
        "m.py",
        &format!(
            r#"
def transform(data, args, env, headers):
    set_output_file("{}")
    set_input_format("yaml")
    return data
"#,
            out_path.display()
        ),
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    let content = std::fs::read_to_string(&out_path).unwrap();
    assert!(content.contains("x:"));
}

// ── set_output_file() + --arg-driven filename ─────────────────────────────────

/// Mold can compute the output filename from --arg.
#[test]
fn test_set_output_file_from_arg() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"count":7}"#);
    let out_path = dir.path().join("computed.json");

    let mold = setup_mold(
        &dir,
        "m.py",
        r#"
def transform(data, args, env, headers):
    filename = args.get("out", "default.json")
    set_output_file(filename)
    return data
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-m",
            &mold,
            "--arg",
            &format!("out={}", out_path.display()),
        ])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    let content = std::fs::read_to_string(&out_path).unwrap();
    assert!(content.contains("7"));
}

// ── set_output_file() error cases ─────────────────────────────────────────────

/// Empty path is rejected.
#[test]
fn test_set_output_file_empty_path_error() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{}"#);
    let mold = setup_mold(
        &dir,
        "m.py",
        r#"
def transform(data, args, env, headers):
    set_output_file("")
    return data
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .failure()
        .stderr(predicate::str::contains("must not be empty"));
}

// ── set_output_format("raw") — no HTTP input → error ─────────────────────────────────

/// set_output_format("raw") without --input-format http must fail with a clear message.
#[test]
fn test_set_output_format_raw_without_http_input_errors() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"x":1}"#);
    let mold = setup_mold(
        &dir,
        "m.py",
        r#"
def transform(data, args, env, headers):
    set_output_format("raw")
    set_output_file("out.bin")
    return data
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .failure()
        .stderr(predicate::str::contains("requires --input-format http"));
}

// ── set_output_format("raw") in intermediate chain step → error ──────────────────────

/// set_output_format("raw") in a non-final step must fail with a clear message.
#[test]
fn test_set_output_format_raw_in_intermediate_step_errors() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"x":1}"#);
    let mold1 = setup_mold(
        &dir,
        "step1.py",
        r#"
def transform(data, args, env, headers):
    set_output_format("raw")
    return data
"#,
    );
    let mold2 = setup_mold(
        &dir,
        "step2.py",
        r#"
def transform(data, args, env, headers):
    return data
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold1, "-m", &mold2])
        .assert()
        .failure()
        .stderr(predicate::str::contains("final step"));
}

// ── set_output_file() in multi-file slurp ─────────────────────────────────────

/// set_output_file() works in multi-file slurp mode too.
#[test]
fn test_set_output_file_in_multi_slurp() {
    let dir = assert_fs::TempDir::new().unwrap();
    let f1 = setup_input(&dir, "a.json", r#"{"x": 1}"#);
    let f2 = setup_input(&dir, "b.json", r#"{"x": 2}"#);
    let out_path = dir.path().join("combined.json");

    let mold = setup_mold(
        &dir,
        "m.py",
        &format!(
            r#"
def transform(data, args, env, headers):
    set_output_file("{}")
    return {{"total": sum(d["x"] for d in data)}}
"#,
            out_path.display()
        ),
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &f1, "-i", &f2, "-s", "-m", &mold])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    let content = std::fs::read_to_string(&out_path).unwrap();
    assert!(content.contains("3"));
}
