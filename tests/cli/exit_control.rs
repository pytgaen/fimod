use super::helpers::{setup_input, setup_mold};
use predicates::prelude::*;

#[test]
fn test_check_truthy_array() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"[1, 2, 3]"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"data"#, "--check"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn test_check_falsy_empty_array() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"[]"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"data"#, "--check"])
        .assert()
        .code(1)
        .stdout(predicate::str::is_empty());
}

#[test]
fn test_check_falsy_null() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"null"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"data"#, "--check"])
        .assert()
        .code(1)
        .stdout(predicate::str::is_empty());
}

#[test]
fn test_check_falsy_false() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"true"#);

    // true is truthy
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"False"#, "--check"])
        .assert()
        .code(1)
        .stdout(predicate::str::is_empty());
}

#[test]
fn test_check_no_stdout() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"key": "value"}"#);

    // --check should produce no stdout even for truthy result
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"data"#, "--check"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn test_set_exit_custom_code() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"status": "error"}"#);

    let script = r#"
def transform(data, args, env, headers):
    set_exit(42)
    return data
"#;
    let mold = setup_mold(&dir, "exit42.py", script);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .code(42);
}

#[test]
fn test_set_exit_priority_over_check() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"[1, 2, 3]"#);

    // set_exit(5) should override --check (which would give 0 for truthy)
    let script = r#"
def transform(data, args, env, headers):
    set_exit(5)
    return data
"#;
    let mold = setup_mold(&dir, "exit5.py", script);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold, "--check"])
        .assert()
        .code(5)
        .stdout(predicate::str::is_empty());
}
