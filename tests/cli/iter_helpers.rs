use super::helpers::setup_input;
use predicates::prelude::*;

#[test]
fn test_it_keys() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"name": "Alice", "age": 30}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"it_keys(data)"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\""))
        .stdout(predicate::str::contains("\"age\""));
}

#[test]
fn test_it_values() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"name": "Alice", "age": 30}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"it_values(data)"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"Alice\""))
        .stdout(predicate::str::contains("30"));
}

#[test]
fn test_it_flatten() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"[1, [2, [3, 4]], 5]"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-e",
            r#"it_flatten(data)"#,
            "--output-format",
            "json-compact",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("[1,2,3,4,5]"));
}

#[test]
fn test_it_group_by() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "data.json",
        r#"[{"name":"Alice","dept":"eng"},{"name":"Bob","dept":"sales"},{"name":"Carol","dept":"eng"}]"#,
    );

    let output = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"it_group_by(data, "dept")"#])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("eng"));
    assert!(stdout.contains("sales"));
    assert!(stdout.contains("Alice"));
    assert!(stdout.contains("Carol"));
}

#[test]
fn test_it_sort_by() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "data.json",
        r#"[{"name":"Charlie","age":30},{"name":"Alice","age":25},{"name":"Bob","age":35}]"#,
    );

    let output = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-e",
            r#"it_sort_by(data, "name")"#,
            "--output-format",
            "json-compact",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    // Sorted alphabetically by name: Alice < Bob < Charlie
    let alice_pos = stdout.find("Alice").unwrap();
    let bob_pos = stdout.find("Bob").unwrap();
    let charlie_pos = stdout.find("Charlie").unwrap();
    assert!(alice_pos < bob_pos);
    assert!(bob_pos < charlie_pos);
}

#[test]
fn test_it_unique() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"[1, 2, 3, 2, 1, 4]"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-e",
            r#"it_unique(data)"#,
            "--output-format",
            "json-compact",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("[1,2,3,4]"));
}

#[test]
fn test_it_unique_by() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "data.json",
        r#"[{"id":1,"name":"Alice"},{"id":2,"name":"Bob"},{"id":1,"name":"Alice2"}]"#,
    );

    let output = assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", r#"it_unique_by(data, "id")"#])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("Alice"));
    assert!(stdout.contains("Bob"));
    assert!(!stdout.contains("Alice2"));
}
