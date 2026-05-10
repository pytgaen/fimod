use super::helpers::setup_input;
use predicates::prelude::*;

#[test]
fn test_csv_to_ndjson() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.csv", "name,age\nAlice,30\nBob,25\n");

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", "data", "--output-format", "ndjson"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"{"name":"Alice","age":"30"}"#))
        .stdout(predicate::str::contains(r#"{"name":"Bob","age":"25"}"#));
}

#[test]
fn test_ndjson_to_csv() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "test.ndjson",
        "{\"a\":1,\"b\":2}\n{\"a\":3,\"b\":4}\n",
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", "data", "--output-format", "csv"])
        .assert()
        .success()
        .stdout(predicate::str::contains("a,b"))
        .stdout(predicate::str::contains("1,2"))
        .stdout(predicate::str::contains("3,4"));
}

#[test]
fn test_ndjson_to_csv_with_mixed_shapes_uses_first_row_keys() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "test.ndjson",
        "{\"a\":1,\"b\":2}\n{\"a\":3,\"c\":99}\n",
    );

    let assert = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", "data", "--output-format", "csv"])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("a,b"), "header from first row: {stdout:?}");
    assert!(stdout.contains("1,2"), "first row intact: {stdout:?}");
    assert!(
        stdout.contains("3,\n") || stdout.contains("3,\r\n"),
        "second row: missing 'b' renders as empty, extra 'c' is dropped: {stdout:?}"
    );
}
