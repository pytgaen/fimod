use super::helpers::{setup_input, setup_mold, GREET_MOLD};
use predicates::prelude::*;

#[test]
fn test_inline_expression_simple() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.json", r#"{"name": "Alice", "age": 30}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"{"upper": data["name"].upper()}"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"upper\": \"ALICE\""));
}

#[test]
fn test_inline_expression_with_def_transform() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.json", r#"{"name": "Bob"}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i", &input,
            "-e", "def transform(data, args, env, headers):\n    data[\"greeting\"] = f\"Hi {data['name']}\"\n    return data",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"greeting\": \"Hi Bob\""));
}

#[test]
fn test_inline_expression_stdin() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--input-format", "json", "-e", r#"{"count": len(data)}"#])
        .write_stdin(r#"{"a": 1, "b": 2, "c": 3}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\": 3"));
}

#[test]
fn test_mold_then_expression() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.json", r#"{"name": "World"}"#);
    let mold = setup_mold(&dir, "greet.py", GREET_MOLD);

    // -m adds greeting, then -e extracts it
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold, "-e", r#"data["greeting"]"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello World"));
}

#[test]
fn test_error_neither_mold_nor_expression() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.json", r#"{"x": 1}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}
