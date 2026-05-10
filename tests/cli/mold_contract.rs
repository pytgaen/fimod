use super::helpers::{setup_input, setup_mold};
use predicates::prelude::*;

#[test]
fn test_mold_signature_data_only_with_kwargs_catchall() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"name":"alice"}"#);
    let mold = setup_mold(
        &dir,
        "m.py",
        r#"def transform(data, **_):
    data["greeting"] = "hello " + data["name"]
    return data
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"greeting\": \"hello alice\""));
}

#[test]
fn test_mold_signature_data_args_with_kwargs_catchall() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"name":"alice"}"#);
    let mold = setup_mold(
        &dir,
        "m.py",
        r#"def transform(data, args, **_):
    data["lang"] = args["lang"]
    return data
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold, "--arg", "lang=fr"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"lang\": \"fr\""));
}

#[test]
fn test_mold_set_output_format_overrides_default_format() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"key":"value"}"#);
    let mold = setup_mold(
        &dir,
        "fmt.py",
        r#"def transform(data, **_):
    set_output_format("yaml")
    return data
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .stdout(predicate::str::contains("key: value"))
        .stdout(predicate::str::contains("\"key\"").not());
}

#[test]
fn test_arg_values_passed_as_strings_no_type_coercion() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"x":1}"#);
    let mold = setup_mold(
        &dir,
        "m.py",
        r#"def transform(data, args, **_):
    return {
        "n_is_str": args["n"] == "42",
        "f_is_str": args["f"] == "3.14",
        "b_is_str": args["b"] == "true",
        "j_is_str": args["j"] == "[1,2,3]",
    }
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-m",
            &mold,
            "--arg",
            "n=42",
            "--arg",
            "f=3.14",
            "--arg",
            "b=true",
            "--arg",
            "j=[1,2,3]",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"n_is_str\": true"))
        .stdout(predicate::str::contains("\"f_is_str\": true"))
        .stdout(predicate::str::contains("\"b_is_str\": true"))
        .stdout(predicate::str::contains("\"j_is_str\": true"));
}

#[test]
fn test_mold_signature_full_kwargs_data_args_env_headers() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"x":1}"#);
    let mold = setup_mold(
        &dir,
        "m.py",
        r#"def transform(data, args, env, headers, **_):
    return {
        "data": data,
        "got_arg": args["a"],
        "got_env": env["FIMOD_TEST_VAR"],
        "headers_is_none": headers is None,
    }
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-m",
            &mold,
            "--arg",
            "a=42",
            "--env",
            "FIMOD_TEST_VAR",
        ])
        .env("FIMOD_TEST_VAR", "abc")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"got_arg\": \"42\""))
        .stdout(predicate::str::contains("\"got_env\": \"abc\""))
        .stdout(predicate::str::contains("\"headers_is_none\": true"));
}
