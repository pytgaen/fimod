use super::helpers::setup_input;
use predicates::prelude::*;

#[test]
fn test_hs_md5() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#""hello""#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-e",
            r#"hs_md5(data)"#,
            "--output-format",
            "txt",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("5d41402abc4b2a76b9719d911017c592"));
}

#[test]
fn test_hs_sha256() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#""hello""#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-e",
            r#"hs_sha256(data)"#,
            "--output-format",
            "txt",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
        ));
}

#[test]
fn test_hs_sha1() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#""hello""#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-e",
            r#"hs_sha1(data)"#,
            "--output-format",
            "txt",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d",
        ));
}

#[test]
fn test_hs_md5_error_on_non_string() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"42"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"hs_md5(data)"#])
        .assert()
        .failure()
        .stderr(predicate::str::contains("expects a string"));
}
