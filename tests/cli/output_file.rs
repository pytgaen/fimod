use super::helpers::{setup_input, setup_mold};
use predicates::prelude::*;

#[test]
fn test_final_ndjson_format_from_cli_extension_or_mold_has_identical_output() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"[{"b":2,"a":1},{"b":4,"a":3}]"#);
    for mode in ["explicit", "extension", "format_override", "file_override"] {
        let target = dir.path().join(format!("{mode}.ndjson"));
        let unused = dir.path().join(format!("{mode}.yaml"));
        let script = match mode {
            "format_override" => "def transform(data, **_):\n    set_output_format('ndjson')\n    return data\n",
            "file_override" => "def transform(data, args, **_):\n    set_output_file(args['out'])\n    return data\n",
            _ => "def transform(data, **_):\n    return data\n",
        };
        let mold = setup_mold(&dir, &format!("{mode}.py"), script);
        let mut cmd = assert_cmd::cargo_bin_cmd!("fimod");
        cmd.args(["s", "-i", &input, "-m", &mold]);
        if mode == "file_override" {
            cmd.arg("-o")
                .arg(&unused)
                .arg("--arg")
                .arg(format!("out={}", target.display()));
        } else {
            cmd.arg("-o").arg(&target);
        }
        if mode == "explicit" {
            cmd.args(["--output-format", "ndjson"]);
        } else if mode == "format_override" {
            cmd.args(["--output-format", "yaml"]);
        }
        cmd.assert().success().stdout("");
        assert_eq!(
            std::fs::read_to_string(target).unwrap(),
            "{\"b\":2,\"a\":1}\n{\"b\":4,\"a\":3}\n",
            "{mode}"
        );
        assert!(!unused.exists());
    }
}

#[test]
fn test_final_output_format_keeps_normalization_and_newlines() {
    let dir = assert_fs::TempDir::new().unwrap();
    for (format, expression, expected) in [
        ("json-compact", "{'b': 2, 'a': 1}", "{\"b\":2,\"a\":1}\n"),
        ("ndjson", "[{'a': 1}, {'a': 2}]", "{\"a\":1}\n{\"a\":2}\n"),
        ("lines", "['hello', 7]", "hello\n7\n"),
        ("txt", "'hello'", "hello"),
        ("txt", "date(2026, 9, 9)", "2026-09-09"),
        ("lines", "(date(2026, 9, 9), 7)", "2026-09-09\n7\n"),
        ("txt", "18446744073709551616", "18446744073709551616"),
        (
            "json-compact",
            "{1: 'first', '1': 'last'}",
            "{\"1\":\"last\"}\n",
        ),
    ] {
        let mold = setup_mold(&dir, "format.py", &format!("from datetime import date\ndef transform(data, **_):\n    set_output_format('{format}')\n    return {expression}\n"));
        for debug in [false, true] {
            let mut cmd = assert_cmd::cargo_bin_cmd!("fimod");
            cmd.args(["s", "--no-input", "-m", &mold]);
            if debug {
                cmd.arg("--debug");
            }
            cmd.assert().success().stdout(expected);
        }
    }
}

#[test]
fn test_inferred_output_does_not_hide_nonfinite_float_errors() {
    let dir = assert_fs::TempDir::new().unwrap();
    let target = dir.path().join("out.ndjson");
    std::fs::write(&target, "keep me").unwrap();
    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["s", "--no-input", "-e", "float('nan')", "-o"])
        .arg(&target)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cannot represent float"));
    assert_eq!(std::fs::read_to_string(target).unwrap(), "keep me");
}

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
def transform(data, args, env, headers, **_):
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
def transform(data, args, env, headers, **_):
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
def transform(data, args, env, headers, **_):
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
def transform(data, args, env, headers, **_):
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
def transform(data, args, env, headers, **_):
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
def transform(data, args, env, headers, **_):
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
def transform(data, args, env, headers, **_):
    set_output_format("raw")
    return data
"#,
    );
    let mold2 = setup_mold(
        &dir,
        "step2.py",
        r#"
def transform(data, args, env, headers, **_):
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
def transform(data, args, env, headers, **_):
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
