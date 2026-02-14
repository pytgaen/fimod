use super::helpers::{setup_input, setup_mold};
use predicates::prelude::*;

#[test]
fn test_dp_get_simple() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"a": 1, "b": 2}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"dp_get(data, "a")"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("1"));
}

#[test]
fn test_dp_get_nested() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"a": {"b": {"c": 42}}}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"dp_get(data, "a.b.c")"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("42"));
}

#[test]
fn test_dp_get_array_index() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"items": [10, 20, 30]}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"dp_get(data, "items.1")"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("20"));
}

#[test]
fn test_dp_get_negative_index() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"items": [10, 20, 30]}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"dp_get(data, "items.-1")"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("30"));
}

#[test]
fn test_dp_get_absent_returns_none() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"a": 1}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"dp_get(data, "b.c")"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("null"));
}

#[test]
fn test_dp_get_with_default() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"a": 1}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-e",
            r#"dp_get(data, "missing", "fallback")"#,
            "--output-format",
            "txt",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("fallback"));
}

#[test]
fn test_dp_set_flat() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"a": 1}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"dp_set(data, "b", 2)"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"b\": 2"))
        .stdout(predicate::str::contains("\"a\": 1"));
}

#[test]
fn test_dp_set_nested() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"a": {"b": 1}}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"dp_set(data, "a.c", 99)"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"c\": 99"))
        .stdout(predicate::str::contains("\"b\": 1"));
}

#[test]
fn test_dp_set_no_mutation() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"a": {"b": 1}}"#);

    // dp_set returns a copy; verify original is unchanged in mold
    let script = r#"
def transform(data, args, env, headers):
    modified = dp_set(data, "a.b", 999)
    return {"original_b": dp_get(data, "a.b"), "modified_b": dp_get(modified, "a.b")}
"#;
    let mold = setup_mold(&dir, "check.py", script);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"original_b\": 1"))
        .stdout(predicate::str::contains("\"modified_b\": 999"));
}
