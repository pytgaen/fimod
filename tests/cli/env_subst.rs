use super::helpers::setup_input;
use predicates::prelude::*;

#[test]
fn test_env_subst_basic() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"url": "https://${HOST}/api"}"#);

    let mold = r#"def transform(data, args, env, headers):
    data["url"] = env_subst(data["url"], env)
    return data
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file, "--env", "HOST"])
        .env("HOST", "example.com")
        .assert()
        .success()
        .stdout(predicate::str::contains("https://example.com/api"));
}

#[test]
fn test_env_subst_multiple_vars() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "data.json",
        r#"{"tpl": "${PROTO}://${HOST}:${PORT}"}"#,
    );

    let mold = r#"def transform(data, args, env, headers):
    return {"url": env_subst(data["tpl"], env)}
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file, "--env", "PROTO,HOST,PORT"])
        .env("PROTO", "https")
        .env("HOST", "prod.local")
        .env("PORT", "8443")
        .assert()
        .success()
        .stdout(predicate::str::contains("https://prod.local:8443"));
}

#[test]
fn test_env_subst_unknown_var_left_as_is() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"tpl": "${HOST}/${MISSING}"}"#);

    let mold = r#"def transform(data, args, env, headers):
    return {"result": env_subst(data["tpl"], env)}
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file, "--env", "HOST"])
        .env("HOST", "example.com")
        .assert()
        .success()
        .stdout(predicate::str::contains("example.com/${MISSING}"));
}

#[test]
fn test_env_subst_respects_env_filter() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"tpl": "${SECRET}"}"#);

    let mold = r#"def transform(data, args, env, headers):
    return {"result": env_subst(data["tpl"], env)}
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    // SECRET exists in environment but --env only allows HOST
    // So env dict won't contain SECRET, and ${SECRET} stays as-is
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file, "--env", "HOST"])
        .env("HOST", "example.com")
        .env("SECRET", "s3cr3t")
        .assert()
        .success()
        .stdout(predicate::str::contains("${SECRET}"));
}

#[test]
fn test_env_subst_no_vars_passthrough() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"tpl": "plain text"}"#);

    let mold = r#"def transform(data, args, env, headers):
    return {"result": env_subst(data["tpl"], env)}
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file])
        .assert()
        .success()
        .stdout(predicate::str::contains("plain text"));
}

#[test]
fn test_env_subst_stdout_not_polluted() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"x": 1}"#);

    let mold = r#"def transform(data, args, env, headers):
    env_subst("${A}", env)
    return data
"#;
    let mold_file = setup_input(&dir, "m.py", mold);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold_file])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""x": 1"#));
}
