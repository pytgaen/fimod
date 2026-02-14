use super::helpers::{setup_input, setup_mold, GREET_MOLD};
use predicates::prelude::*;

#[test]
fn test_missing_input_file() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(&dir, "greet.py", GREET_MOLD);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", "/tmp/nonexistent_fm_test.json", "-m", &mold])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to read input file"));
}

#[test]
fn test_missing_mold() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.json", r#"{"name": "Alice"}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", "/tmp/nonexistent_fm_mold.py"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Mold not found"));
}

#[test]
fn test_stdin_input() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(&dir, "greet.py", GREET_MOLD);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-m", &mold, "--input-format", "json"])
        .write_stdin(r#"{"name": "Stdin"}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"greeting\": \"Hello Stdin\""));
}
