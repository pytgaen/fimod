use super::helpers::{setup_input, setup_mold};
use predicates::prelude::*;

#[test]
fn test_lines_input() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.txt", "hello\nworld\n");
    let mold = setup_mold(
        &dir,
        "identity.py",
        "def transform(data, args, env, headers):\n    return data\n",
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-m",
            &mold,
            "--input-format",
            "lines",
            "--output-format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"hello\""))
        .stdout(predicate::str::contains("\"world\""));
}

#[test]
fn test_lines_to_json() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.txt", "hello\nworld\n");

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "--input-format",
            "lines",
            "--output-format",
            "json",
            "-e",
            "[l.upper() for l in data]",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"HELLO\""))
        .stdout(predicate::str::contains("\"WORLD\""));
}

#[test]
fn test_lines_roundtrip() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.txt", "aaa\nbbb\nccc\n");

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "--input-format",
            "lines",
            "--output-format",
            "lines",
            "-e",
            "[l.upper() for l in data if l]",
        ])
        .assert()
        .success()
        .stdout(predicate::str::is_match("AAA\nBBB\nCCC\n").unwrap());
}

#[test]
fn test_lines_ndjson_output() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.json", r#"[{"name": "Alice"}, {"name": "Bob"}]"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "--output-format", "lines", "-e", "data"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"{"name":"Alice"}"#))
        .stdout(predicate::str::contains(r#"{"name":"Bob"}"#));
}

#[test]
fn test_csv_output_error_on_non_array() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.json", r#"{"name": "Alice"}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "--output-format", "csv", "-e", "data"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "CSV output expects an array of objects",
        ))
        .stderr(predicate::str::contains("Hint"));
}
