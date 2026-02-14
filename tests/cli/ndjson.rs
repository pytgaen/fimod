use super::helpers::setup_input;
use predicates::prelude::*;

#[test]
fn test_ndjson_input_to_json() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "--input-format",
            "ndjson",
            "--output-format",
            "json",
            "-e",
            "data",
        ])
        .write_stdin("{\"name\":\"Alice\"}\n{\"name\":\"Bob\"}\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\": \"Alice\""))
        .stdout(predicate::str::contains("\"name\": \"Bob\""));
}

#[test]
fn test_ndjson_output() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"[{"name": "Alice"}, {"name": "Bob"}]"#);

    let output = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "--output-format", "ndjson", "-e", "data"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    let lines: Vec<&str> = stdout.trim_end().split('\n').collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("\"name\":\"Alice\""));
    assert!(lines[1].contains("\"name\":\"Bob\""));
}

#[test]
fn test_ndjson_file_extension_detection() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.ndjson", "{\"a\":1}\n{\"b\":2}\n");

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", "len(data)", "--output-format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2"));
}

#[test]
fn test_ndjson_len_expression() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--input-format", "ndjson", "-e", "len(data)"])
        .write_stdin("{\"name\":\"Alice\"}\n{\"name\":\"Bob\"}\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("2"));
}

#[test]
fn test_ndjson_roundtrip() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "--input-format",
            "ndjson",
            "--output-format",
            "ndjson",
            "-e",
            "data",
        ])
        .write_stdin("{\"a\":1}\n{\"b\":2}\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("{\"a\":1}"))
        .stdout(predicate::str::contains("{\"b\":2}"));
}

#[test]
fn test_slurp_multiple_json() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--input-format", "json", "-s", "-e", "len(data)"])
        .write_stdin("{\"a\":1}\n{\"b\":2}")
        .assert()
        .success()
        .stdout(predicate::str::contains("2"));
}

#[test]
fn test_slurp_single_json() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--input-format", "json", "-s", "-e", "len(data)"])
        .write_stdin("{\"a\":1}")
        .assert()
        .success()
        .stdout(predicate::str::contains("1"));
}

#[test]
fn test_slurp_ndjson() {
    // Slurp + NDJSON: NDJSON already produces an array, slurp is a no-op
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--input-format", "ndjson", "-s", "-e", "len(data)"])
        .write_stdin("{\"a\":1}\n{\"b\":2}\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("2"));
}

#[test]
fn test_slurp_count() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--input-format", "json", "-s", "-e", "len(data)"])
        .write_stdin("{\"a\":1}{\"b\":2}{\"c\":3}")
        .assert()
        .success()
        .stdout(predicate::str::contains("3"));
}

#[test]
fn test_slurp_file() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "multi.json", "{\"a\":1}\n{\"b\":2}\n{\"c\":3}\n");

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-s", "-e", "len(data)"])
        .assert()
        .success()
        .stdout(predicate::str::contains("3"));
}

#[test]
fn test_slurp_with_expression() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "--input-format",
            "json",
            "-s",
            "-e",
            "[d for d in data if \"a\" in d]",
        ])
        .write_stdin("{\"a\":1}\n{\"b\":2}\n{\"a\":3}")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"a\": 1"))
        .stdout(predicate::str::contains("\"a\": 3"));
}
