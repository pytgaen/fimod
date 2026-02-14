use super::helpers::{setup_input, setup_mold};

#[test]
fn test_env_basic_access() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"x": 1}"#);

    let stdout = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .env("MY_TEST_VAR", "hello_fimod")
        .args([
            "-i",
            &input,
            "--env",
            "MY_TEST_VAR",
            "-e",
            r#"{"v": env["MY_TEST_VAR"]}"#,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert!(String::from_utf8(stdout).unwrap().contains("hello_fimod"));
}

#[test]
fn test_env_get_with_default() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"x": 1}"#);

    // env.get() with a default for a missing key
    let stdout = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "--env",
            "*",
            "-e",
            r#"{"v": env.get("FIMOD_NO_SUCH_VAR_XYZ", "default_value")}"#,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert!(String::from_utf8(stdout).unwrap().contains("default_value"));
}

#[test]
fn test_env_empty_without_flag() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"x": 1}"#);

    // Without --env, env should be an empty dict (not a NameError)
    let stdout = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"len(env)"#])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert!(String::from_utf8(stdout).unwrap().contains("0"));
}

#[test]
fn test_env_in_mold_file() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"name": "world"}"#);
    let mold = setup_mold(
        &dir,
        "greet.py",
        r#"def transform(data, args, env, headers):
    prefix = env.get("GREETING_PREFIX", "Hello")
    return {"msg": f"{prefix} {data['name']}"}
"#,
    );

    let stdout = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .env("GREETING_PREFIX", "Hi")
        .args(["-i", &input, "--env", "GREETING_*", "-m", &mold])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert!(String::from_utf8(stdout).unwrap().contains("Hi world"));
}

#[test]
fn test_env_available_in_all_chain_steps() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"x": 1}"#);

    // env must be accessible in step 2 of the chain, not just step 1
    let stdout = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .env("STEP_TAG", "tagged")
        .args([
            "-i",
            &input,
            "--env",
            "STEP_*",
            "-e",
            r#"{"x": data["x"] + 1}"#,
            "-e",
            r#"{"x": data["x"], "tag": env["STEP_TAG"]}"#,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(stdout).unwrap();
    assert!(stdout.contains("\"x\": 2"));
    assert!(stdout.contains("tagged"));
}

#[test]
fn test_env_pattern_glob() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"x": 1}"#);

    // --env 'FIMOD_TEST_*' should only include matching vars
    let stdout = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .env("FIMOD_TEST_A", "aaa")
        .env("FIMOD_TEST_B", "bbb")
        .env("OTHER_VAR", "should_not_appear")
        .args([
            "-i",
            &input,
            "--env",
            "FIMOD_TEST_*",
            "-e",
            r#"{"a": env.get("FIMOD_TEST_A", ""), "other": env.get("OTHER_VAR", "missing")}"#,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(stdout).unwrap();
    assert!(stdout.contains("\"a\": \"aaa\""));
    assert!(stdout.contains("\"other\": \"missing\""));
}

#[test]
fn test_env_multiple_patterns() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"x": 1}"#);

    // Multiple --env flags accumulate patterns
    let stdout = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .env("HOME_TEST", "h")
        .env("CI_TEST", "c")
        .args([
            "-i",
            &input,
            "--env",
            "HOME_TEST",
            "--env",
            "CI_TEST",
            "-e",
            r#"{"h": env["HOME_TEST"], "c": env["CI_TEST"]}"#,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(stdout).unwrap();
    assert!(stdout.contains("\"h\": \"h\""));
    assert!(stdout.contains("\"c\": \"c\""));
}

#[test]
fn test_env_comma_separated_patterns() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"x": 1}"#);

    // --env 'A,B' matches both A and B
    let stdout = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .env("ENV_A", "val_a")
        .env("ENV_B", "val_b")
        .args([
            "-i",
            &input,
            "--env",
            "ENV_A,ENV_B",
            "-e",
            r#"{"a": env["ENV_A"], "b": env["ENV_B"]}"#,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(stdout).unwrap();
    assert!(stdout.contains("\"a\": \"val_a\""));
    assert!(stdout.contains("\"b\": \"val_b\""));
}
