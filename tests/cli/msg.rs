use super::helpers::setup_input;
use predicates::prelude::*;

#[test]
fn test_msg_print() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"name": "alice"}"#);

    let mold = r#"def transform(data, args, env, headers):
    msg_print("hello stderr")
    return data
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file])
        .assert()
        .success()
        .stderr(predicate::str::contains("hello stderr"))
        .stdout(predicate::str::contains("alice"));
}

#[test]
fn test_msg_info() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"x": 1}"#);

    let mold = r#"def transform(data, args, env, headers):
    msg_info("starting")
    return data
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file])
        .assert()
        .success()
        .stderr(predicate::str::contains("[INFO] starting"))
        .stdout(predicate::str::contains(r#""x": 1"#));
}

#[test]
fn test_msg_warn() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"x": 1}"#);

    let mold = r#"def transform(data, args, env, headers):
    msg_warn("caution")
    return data
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file])
        .assert()
        .success()
        .stderr(predicate::str::contains("[WARN] caution"))
        .stdout(predicate::str::contains(r#""x": 1"#));
}

#[test]
fn test_msg_error() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"x": 1}"#);

    let mold = r#"def transform(data, args, env, headers):
    msg_error("something broke")
    return data
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file])
        .assert()
        .success()
        .stderr(predicate::str::contains("[ERROR] something broke"))
        .stdout(predicate::str::contains(r#""x": 1"#));
}

#[test]
fn test_msg_multiple_in_mold_file() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"val": 42}"#);

    let mold = r#"def transform(data, args, env, headers):
    msg_info("processing record")
    msg_warn("value is high")
    return data
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file])
        .assert()
        .success()
        .stderr(
            predicate::str::contains("[INFO] processing record")
                .and(predicate::str::contains("[WARN] value is high")),
        )
        .stdout(predicate::str::contains(r#""val": 42"#));
}

#[test]
fn test_msg_stdout_not_polluted() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"ok": true}"#);

    let mold = r#"def transform(data, args, env, headers):
    msg_print("noise")
    msg_info("info")
    msg_warn("warn")
    msg_error("err")
    return data
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    let output = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("[INFO]"));
    assert!(!stdout.contains("[WARN]"));
    assert!(!stdout.contains("[ERROR]"));
    assert!(!stdout.contains("noise"));
    assert!(stdout.contains(r#""ok": true"#));
}
