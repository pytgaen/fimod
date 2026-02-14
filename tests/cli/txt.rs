use super::helpers::{setup_input, setup_mold, UPPER_MOLD};
use predicates::prelude::*;

#[test]
fn test_txt_to_json() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.txt", "Hello World\n");
    let mold = setup_mold(&dir, "upper.py", UPPER_MOLD);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold, "--output-format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"HELLO WORLD\""));
}

#[test]
fn test_txt_to_txt() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.txt", "Hello World\n");
    let mold = setup_mold(&dir, "upper.py", UPPER_MOLD);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .stdout(predicate::str::is_match("HELLO WORLD").unwrap());
}
