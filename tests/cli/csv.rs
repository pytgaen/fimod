use super::helpers::{setup_input, setup_mold, CSV_GREET_MOLD};
use predicates::prelude::*;

#[test]
fn test_csv_with_header() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.csv", "name,age\nAlice,30\nBob,25\n");
    let mold = setup_mold(&dir, "greet.py", CSV_GREET_MOLD);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold, "--output-format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"greeting\": \"Hello Alice\""))
        .stdout(predicate::str::contains("\"greeting\": \"Hello Bob\""));
}

#[test]
fn test_csv_to_csv() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.csv", "name,age\nAlice,30\n");
    let mold = setup_mold(&dir, "greet.py", CSV_GREET_MOLD);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold, "--output-format", "csv"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello Alice"));
}

#[test]
fn test_csv_no_input_header() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "raw.csv", "Alice,30\nBob,25\n");
    let mold = setup_mold(
        &dir,
        "identity.py",
        "def transform(data, args, env, headers):\n    return data\n",
    );

    // Headerless CSV rows are now arrays, not col0/col1 dicts
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-m",
            &mold,
            "--csv-no-input-header",
            "--output-format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"Alice\""))
        .stdout(predicate::str::contains("\"30\""))
        .stdout(predicate::str::contains("[").count(3)); // outer array + 2 inner arrays
}

#[test]
fn test_csv_headers_global_injected() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.csv", "name,age\nAlice,30\n");
    let mold = setup_mold(
        &dir,
        "use_headers.py",
        "def transform(data, args, env, headers):\n    return headers\n",
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold, "--output-format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\""))
        .stdout(predicate::str::contains("\"age\""));
}

#[test]
fn test_csv_no_headers_global_when_headerless() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "raw.csv", "Alice,30\n");
    // headers should be None when using --csv-no-input-header without --csv-header
    let mold = setup_mold(
        &dir,
        "try_headers.py",
        "def transform(data, args, env, headers):\n    return data\n",
    );

    // Should succeed (headers is None when no header line)
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-m",
            &mold,
            "--csv-no-input-header",
            "--output-format",
            "json",
        ])
        .assert()
        .success();
}

#[test]
fn test_csv_custom_header() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "raw.csv", "Alice,30\nBob,25\n");
    let mold = setup_mold(&dir, "greet.py", CSV_GREET_MOLD);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-m",
            &mold,
            "--csv-header",
            "name,age",
            "--output-format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"greeting\": \"Hello Alice\""));
}

#[test]
fn test_csv_no_output_header() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.csv", "name,age\nAlice,30\n");
    let mold = setup_mold(
        &dir,
        "identity.py",
        "def transform(data, args, env, headers):\n    return data\n",
    );

    let output = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-m",
            &mold,
            "--output-format",
            "csv",
            "--csv-no-output-header",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    // Should NOT contain header line, just data
    assert!(!stdout.starts_with("name") && !stdout.starts_with("age"));
    assert!(stdout.contains("Alice"));
}

#[test]
fn test_csv_delimiter_tab() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.tsv", "name\tage\nAlice\t30\n");
    let mold = setup_mold(&dir, "greet.py", CSV_GREET_MOLD);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-m",
            &mold,
            "--csv-delimiter",
            "\t",
            "--output-format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"greeting\": \"Hello Alice\""));
}
