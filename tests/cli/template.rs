use super::helpers::setup_input;
use predicates::prelude::*;

#[test]
fn test_tpl_render_str_inline() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"name": "world"}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-e",
            r#"tpl_render_str("Hello {{ name }}!", data)"#,
            "--output-format",
            "txt",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello world!"));
}

#[test]
fn test_tpl_render_str_loop() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"items": ["a", "b", "c"]}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-e",
            r#"tpl_render_str("{% for x in items %}{{ x }},{% endfor %}", data)"#,
            "--output-format",
            "txt",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("a,b,c,"));
}

#[test]
fn test_tpl_render_str_auto_escape() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"html": "<b>bold</b>"}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-e",
            r#"tpl_render_str("{{ html }}", data, True)"#,
            "--output-format",
            "txt",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("&lt;b&gt;"));
}

#[test]
fn test_tpl_render_from_mold() {
    let dir = assert_fs::TempDir::new().unwrap();

    // Create a mold directory with a template
    let mold_dir = dir.path().join("my_mold");
    std::fs::create_dir_all(mold_dir.join("templates")).unwrap();

    std::fs::write(
        mold_dir.join("templates").join("hello.j2"),
        "Hello {{ name }}!",
    )
    .unwrap();

    std::fs::write(
        mold_dir.join("my_mold.py"),
        r#""""My test mold."""
def transform(data, args, env, headers):
    return tpl_render_from_mold("templates/hello.j2", data)
"#,
    )
    .unwrap();

    let input = setup_input(&dir, "data.json", r#"{"name": "fimod"}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-m",
            mold_dir.to_str().unwrap(),
            "--output-format",
            "txt",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello fimod!"));
}
