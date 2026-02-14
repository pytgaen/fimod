use super::helpers::{setup_input, setup_mold, GREET_MOLD};
use assert_fs::prelude::*;
use predicates::prelude::*;

#[test]
fn test_json_to_json() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.json", r#"{"name": "Alice", "age": 30}"#);
    let mold = setup_mold(&dir, "greet.py", GREET_MOLD);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"greeting\": \"Hello Alice\""));
}

#[test]
fn test_json_to_yaml() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.json", r#"{"name": "Alice", "age": 30}"#);
    let mold = setup_mold(&dir, "greet.py", GREET_MOLD);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold, "--output-format", "yaml"])
        .assert()
        .success()
        .stdout(predicate::str::contains("greeting: Hello Alice"));
}

#[test]
fn test_yaml_to_json() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.yaml", "name: Bob\nage: 25\n");
    let mold = setup_mold(&dir, "greet.py", GREET_MOLD);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold, "--output-format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"greeting\": \"Hello Bob\""));
}

#[test]
fn test_json_to_toml() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.json", r#"{"name": "Alice", "age": 30}"#);
    let mold = setup_mold(&dir, "greet.py", GREET_MOLD);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold, "--output-format", "toml"])
        .assert()
        .success()
        .stdout(predicate::str::contains("greeting = \"Hello Alice\""));
}

#[test]
fn test_auto_detect_by_extension() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.yaml", "name: Charlie\nage: 40\n");
    let mold = setup_mold(&dir, "greet.py", GREET_MOLD);
    let output_path = dir.child("out.toml");

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-m",
            &mold,
            "-o",
            output_path.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    output_path.assert(predicate::str::contains("greeting = \"Hello Charlie\""));
}
