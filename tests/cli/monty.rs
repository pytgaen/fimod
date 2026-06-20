use assert_fs::prelude::*;
use predicates::prelude::*;

#[test]
fn test_monty_repl_help_mentions_sandbox_file() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["monty", "repl", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--sandbox-file <PATH>"));
}

#[test]
fn test_monty_repl_empty_sandbox_file_denies_clock() {
    assert_cmd::cargo_bin_cmd!("fimod")
        .args(["monty", "repl", "--sandbox-file="])
        .write_stdin("from datetime import datetime\ndatetime.now()\n")
        .assert()
        .success()
        .stderr(predicate::str::contains("PermissionError"))
        .stderr(predicate::str::contains(
            "datetime.now() denied by sandbox policy",
        ));
}

#[test]
fn test_monty_repl_sandbox_file_allows_clock() {
    let dir = assert_fs::TempDir::new().unwrap();
    let policy = dir.child("sandbox.toml");
    policy
        .write_str("[sandbox]\nallow_clock = true\nmax_duration = \"10s\"\n")
        .unwrap();

    assert_cmd::cargo_bin_cmd!("fimod")
        .args([
            "monty",
            "repl",
            "--sandbox-file",
            policy.path().to_str().unwrap(),
        ])
        .write_stdin("from datetime import datetime\ndatetime.now() is not None\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("True"));
}
