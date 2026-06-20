use super::helpers::setup_input;
use predicates::prelude::*;

fn normalized_stdout(output: &[u8]) -> String {
    String::from_utf8(output.to_vec())
        .unwrap()
        .replace("\r\n", "\n")
}

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

#[test]
fn test_ndjson_to_csv_with_explicit_header_projects_object_rows() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "test.ndjson",
        "{\"a\":1,\"b\":2}\n{\"a\":3,\"c\":99}\n",
    );

    let assert = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-e",
            "data",
            "--output-format",
            "csv",
            "--csv-header",
            "a,b,c",
        ])
        .assert()
        .success();

    let stdout = normalized_stdout(&assert.get_output().stdout);
    assert_eq!(stdout, "a,b,c\n1,2,\n3,,99\n");
}

#[test]
fn test_ndjson_to_csv_scan_window_unions_first_n_rows() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.ndjson", "{\"a\":1}\n{\"b\":2}\n{\"c\":3}\n");

    let assert = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-e",
            "data",
            "--output-format",
            "csv",
            "--csv-scan",
            "2",
        ])
        .assert()
        .success();

    let stdout = normalized_stdout(&assert.get_output().stdout);
    assert_eq!(stdout, "a,b\n1,\n,2\n,\n");
}

#[test]
fn test_ndjson_to_csv_scan_zero_unions_all_rows() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.ndjson", "{\"a\":1}\n{\"b\":2}\n{\"c\":3}\n");

    let assert = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-e",
            "data",
            "--output-format",
            "csv",
            "--csv-scan",
            "0",
        ])
        .assert()
        .success();

    let stdout = normalized_stdout(&assert.get_output().stdout);
    assert_eq!(stdout, "a,b,c\n1,,\n,2,\n,,3\n");
}

#[test]
fn test_json_array_to_csv_identity_streams_with_default_first_row_keys() {
    let assert = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "--input-format",
            "json",
            "--output-format",
            "csv",
            "-e",
            "data",
        ])
        .write_stdin("[{\"a\":1,\"b\":2},{\"a\":3,\"c\":99}]\n")
        .assert()
        .success();

    let stdout = normalized_stdout(&assert.get_output().stdout);
    assert_eq!(stdout, "a,b\n1,2\n3,\n");
}

#[test]
fn test_json_array_to_csv_identity_streams_with_explicit_header() {
    let assert = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "--input-format",
            "json",
            "--output-format",
            "csv",
            "--csv-header",
            "a,b,c",
            "-e",
            "data",
        ])
        .write_stdin("[{\"a\":1,\"b\":2},{\"a\":3,\"c\":99}]\n")
        .assert()
        .success();

    let stdout = normalized_stdout(&assert.get_output().stdout);
    assert_eq!(stdout, "a,b,c\n1,2,\n3,,99\n");
}

#[test]
fn test_json_array_to_csv_identity_streams_with_scan_window() {
    let assert = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "--input-format",
            "json",
            "--output-format",
            "csv",
            "--csv-scan",
            "2",
            "-e",
            "data",
        ])
        .write_stdin("[{\"a\":1},{\"b\":2},{\"c\":3}]\n")
        .assert()
        .success();

    let stdout = normalized_stdout(&assert.get_output().stdout);
    assert_eq!(stdout, "a,b\n1,\n,2\n,\n");
}

#[test]
fn test_json_array_to_csv_identity_scan_zero_unions_all_rows() {
    let assert = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "--input-format",
            "json",
            "--output-format",
            "csv",
            "--csv-scan",
            "0",
            "-e",
            "data",
        ])
        .write_stdin("[{\"a\":1},{\"b\":2},{\"c\":3}]\n")
        .assert()
        .success();

    let stdout = normalized_stdout(&assert.get_output().stdout);
    assert_eq!(stdout, "a,b,c\n1,,\n,2,\n,,3\n");
}
