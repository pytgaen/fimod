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

#[test]
fn test_txt_non_string_output_falls_back_to_compact_json() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "test.txt", "ignored\n");
    let mold = setup_mold(
        &dir,
        "obj.py",
        r#"def transform(data, **_):
    return {"key": "value", "n": 42}
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold, "--output-format", "txt"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""key":"value""#))
        .stdout(predicate::str::contains(r#""n":42"#));
}

#[test]
fn test_txt_chain_preserves_string_through_pipe() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "in.txt", "Hello World\n");
    let intermediate = dir.path().join("mid.txt");
    let mold = setup_mold(&dir, "upper.py", UPPER_MOLD);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-m",
            &mold,
            "--output-format",
            "txt",
            "-o",
            intermediate.to_str().unwrap(),
        ])
        .assert()
        .success();

    let mid = std::fs::read_to_string(&intermediate).unwrap();
    assert_eq!(mid.trim(), "HELLO WORLD");

    let identity = setup_mold(
        &dir,
        "id.py",
        r#"def transform(data, **_):
    return data
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            intermediate.to_str().unwrap(),
            "--input-format",
            "txt",
            "-m",
            &identity,
            "--output-format",
            "txt",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("HELLO WORLD"));
}
