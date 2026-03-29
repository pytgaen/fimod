use super::helpers::{setup_input, setup_mold};
use predicates::prelude::*;

#[test]
fn test_chain_two_expressions() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(
        &dir,
        "data.json",
        r#"{"users": [{"name": "Alice", "active": true}, {"name": "Bob", "active": false}]}"#,
    );

    // Step 1: extract the users list
    // Step 2: filter to active users only
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-e",
            r#"data["users"]"#,
            "-e",
            r#"[u for u in data if u["active"]]"#,
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Alice"))
        .stdout(predicate::str::contains("Bob").not());
}

#[test]
fn test_chain_three_expressions() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"[1, 2, 3, 4, 5]"#);

    // Step 1: keep even numbers
    // Step 2: multiply by 10
    // Step 3: count elements
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-e",
            "[x for x in data if x % 2 == 0]",
            "-e",
            "[x * 10 for x in data]",
            "-e",
            "len(data)",
        ])
        .assert()
        .success()
        .stdout(predicate::str::is_match("^2\n$").unwrap());
}

#[test]
fn test_chain_two_molds() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"name": "alice"}"#);

    // Step 1: uppercase the name field
    let mold1 = setup_mold(
        &dir,
        "upper.py",
        r#"
def transform(data, args, env, headers):
    data["name"] = data["name"].upper()
    return data
"#,
    );

    // Step 2: add a greeting field
    let mold2 = setup_mold(
        &dir,
        "greet.py",
        r#"
def transform(data, args, env, headers):
    data["greeting"] = f"Hello {data['name']}"
    return data
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold1, "-m", &mold2])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\": \"ALICE\""))
        .stdout(predicate::str::contains("\"greeting\": \"Hello ALICE\""));
}

#[test]
fn test_chain_expression_then_mold() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"name": "World"}"#);
    let mold = setup_mold(
        &dir,
        "greet.py",
        "def transform(data, args, env, headers):\n    data[\"greeting\"] = f\"Hello {data['name']}\"\n    return data\n",
    );

    // -e passes data through, then -m adds greeting
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", "data", "-m", &mold])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello World"));
}

#[test]
fn test_chain_args_available_all_steps() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"[]"#);

    // Both steps append the same arg value — args dict is available in every step.
    // Use json-compact output to get a single-line result for easy matching.
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-e",
            r#"data + [args["item"]]"#,
            "-e",
            r#"data + [args["item"]]"#,
            "--output-format",
            "json-compact",
            "--arg",
            "item=hello",
        ])
        .assert()
        .success()
        // "hello" appears twice in the compact array
        .stdout(predicate::str::contains(r#"["hello","hello"]"#));
}
