use super::helpers::{setup_input, setup_mold};
use assert_fs::prelude::*;
use predicates::prelude::*;

#[test]
fn test_batch_two_files_output_dir() {
    let dir = assert_fs::TempDir::new().unwrap();
    let file1 = setup_input(&dir, "alice.json", r#"{"name": "alice"}"#);
    let file2 = setup_input(&dir, "bob.json", r#"{"name": "bob"}"#);
    let out_dir = dir.child("out");
    let out_dir_str = out_dir.path().to_str().unwrap().to_string();

    let mold = setup_mold(
        &dir,
        "upper.py",
        r#"
def transform(data, args, env, headers):
    data["name"] = data["name"].upper()
    return data
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &file1, &file2, "-m", &mold, "-o", &out_dir_str])
        .assert()
        .success();

    // Both output files should exist with transformed content
    let alice_out = out_dir.child("alice.json");
    alice_out.assert(predicate::path::exists());
    let content = std::fs::read_to_string(alice_out.path()).unwrap();
    assert!(
        content.contains("ALICE"),
        "alice.json should contain ALICE, got: {content}"
    );

    let bob_out = out_dir.child("bob.json");
    bob_out.assert(predicate::path::exists());
    let content = std::fs::read_to_string(bob_out.path()).unwrap();
    assert!(
        content.contains("BOB"),
        "bob.json should contain BOB, got: {content}"
    );
}

#[test]
fn test_batch_in_place() {
    let dir = assert_fs::TempDir::new().unwrap();
    let file1 = setup_input(&dir, "f1.json", r#"{"val": "hello"}"#);
    let file2 = setup_input(&dir, "f2.json", r#"{"val": "world"}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &file1,
            &file2,
            "-e",
            r#"{"val": data["val"].upper()}"#,
            "--in-place",
        ])
        .assert()
        .success();

    let content1 = std::fs::read_to_string(&file1).unwrap();
    assert!(
        content1.contains("HELLO"),
        "f1.json should contain HELLO, got: {content1}"
    );

    let content2 = std::fs::read_to_string(&file2).unwrap();
    assert!(
        content2.contains("WORLD"),
        "f2.json should contain WORLD, got: {content2}"
    );
}

#[test]
fn test_batch_requires_output_dir_or_in_place() {
    let dir = assert_fs::TempDir::new().unwrap();
    let file1 = setup_input(&dir, "a.json", r#"{"x": 1}"#);
    let file2 = setup_input(&dir, "b.json", r#"{"x": 2}"#);

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &file1, &file2, "-e", "data"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "requires -o/--output directory or --in-place",
        ));
}

#[test]
fn test_batch_with_chain() {
    let dir = assert_fs::TempDir::new().unwrap();
    let file1 = setup_input(&dir, "c1.json", r#"{"items": [1, 2, 3]}"#);
    let file2 = setup_input(&dir, "c2.json", r#"{"items": [4, 5, 6]}"#);
    let out_dir = dir.child("chain_out");
    let out_dir_str = out_dir.path().to_str().unwrap().to_string();

    // Chain: extract items list, then sum them
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &file1,
            &file2,
            "-e",
            r#"data["items"]"#,
            "-e",
            "sum(data)",
            "-o",
            &out_dir_str,
        ])
        .assert()
        .success();

    let out1 = out_dir.child("c1.json");
    out1.assert(predicate::path::exists());
    let content1 = std::fs::read_to_string(out1.path()).unwrap();
    assert!(
        content1.trim() == "6",
        "c1 sum should be 6, got: {content1}"
    );

    let out2 = out_dir.child("c2.json");
    out2.assert(predicate::path::exists());
    let content2 = std::fs::read_to_string(out2.path()).unwrap();
    assert!(
        content2.trim() == "15",
        "c2 sum should be 15, got: {content2}"
    );
}

#[test]
fn test_batch_output_must_be_directory() {
    let dir = assert_fs::TempDir::new().unwrap();
    let file1 = setup_input(&dir, "x1.json", r#"{"x": 1}"#);
    let file2 = setup_input(&dir, "x2.json", r#"{"x": 2}"#);

    // Create an existing regular file (not a directory) as the output target
    let existing = dir.child("output.txt");
    existing.write_str("not a directory").unwrap();
    let out_path = existing.path().to_str().unwrap().to_string();

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &file1, &file2, "-e", "data", "-o", &out_path])
        .assert()
        .failure()
        .stderr(predicate::str::contains("must be a directory"));
}
