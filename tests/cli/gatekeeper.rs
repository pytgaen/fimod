use super::helpers::setup_input;
use predicates::prelude::*;

#[test]
fn test_gk_fail_exits_1() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"x": 1}"#);

    let mold = r#"def transform(data, args, env, headers):
    gk_fail("blocked by gatekeeper")
    return data
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("[ERROR] blocked by gatekeeper"));
}

#[test]
fn test_gk_fail_still_returns_data() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"x": 1}"#);

    let mold = r#"def transform(data, args, env, headers):
    gk_fail("problem")
    return data
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(r#""x": 1"#));
}

#[test]
fn test_gk_assert_passing() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"version": "v2"}"#);

    let mold = r#"def transform(data, args, env, headers):
    gk_assert(data.get("version"), "missing version")
    return data
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file])
        .assert()
        .success()
        .stdout(predicate::str::contains("v2"));
}

#[test]
fn test_gk_assert_failing_none() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"name": "alice"}"#);

    let mold = r#"def transform(data, args, env, headers):
    gk_assert(data.get("version"), "missing version field")
    return data
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("[ERROR] missing version field"));
}

#[test]
fn test_gk_assert_failing_false() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"coverage": 50}"#);

    let mold = r#"def transform(data, args, env, headers):
    gk_assert(data["coverage"] >= 80, "coverage below 80%")
    return data
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("[ERROR] coverage below 80%"));
}

#[test]
fn test_gk_assert_multiple_first_fails() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"a": 1}"#);

    let mold = r#"def transform(data, args, env, headers):
    gk_assert(data.get("x"), "missing x")
    gk_assert(data.get("a"), "missing a")
    return data
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    // Both assertions run, exit code is 1 from the first failure
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("[ERROR] missing x"));
}

#[test]
fn test_gk_warn_falsy_emits_warning() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"items": []}"#);

    let mold = r#"def transform(data, args, env, headers):
    gk_warn(len(data.get("items", [])) > 0, "items list is empty")
    return data
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file])
        .assert()
        .success()
        .stderr(predicate::str::contains("[WARN] items list is empty"));
}

#[test]
fn test_gk_warn_truthy_silent() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"items": [1, 2]}"#);

    let mold = r#"def transform(data, args, env, headers):
    gk_warn(len(data["items"]) > 0, "items list is empty")
    return data
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    let output = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("[WARN]"));
}

#[test]
fn test_gk_combined_with_msg() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"coverage": 50, "version": "v2"}"#);

    let mold = r#"def transform(data, args, env, headers):
    msg_info("validating record")
    gk_assert(data.get("version"), "missing version")
    gk_assert(data["coverage"] >= 80, "coverage too low")
    return data
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file])
        .assert()
        .code(1)
        .stderr(
            predicate::str::contains("[INFO] validating record")
                .and(predicate::str::contains("[ERROR] coverage too low")),
        );
}
