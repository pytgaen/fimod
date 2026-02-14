use super::helpers::setup_mold;
use predicates::prelude::*;

/// Verify that pathlib.Path.exists() returns None (not the real filesystem value).
#[test]
fn test_sandbox_pathlib_exists_returns_null() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "sandbox_exists.py",
        r#"
from pathlib import Path

def transform(data, args, env, headers):
    data["exists"] = Path("/etc/passwd").exists()
    return data
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-m", &mold, "--input-format", "json"])
        .write_stdin(r#"{"test": 1}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""exists": null"#));
}

/// Verify that pathlib.Path.read_text() returns None (no file content leaked).
#[test]
fn test_sandbox_pathlib_read_text_returns_null() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "sandbox_read.py",
        r#"
from pathlib import Path

def transform(data, args, env, headers):
    content = Path("/etc/passwd").read_text()
    data["content"] = str(content)
    return data
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-m", &mold, "--input-format", "json"])
        .write_stdin(r#"{"test": 1}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""content": "None""#));
}

/// Verify that os.getenv() returns None (no env vars leaked).
#[test]
fn test_sandbox_os_getenv_returns_null() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "sandbox_env.py",
        r#"
import os

def transform(data, args, env, headers):
    data["home"] = os.getenv("HOME")
    data["path"] = os.getenv("PATH")
    return data
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-m", &mold, "--input-format", "json"])
        .write_stdin(r#"{"test": 1}"#)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""home": null"#))
        .stdout(predicate::str::contains(r#""path": null"#));
}

/// Verify that open() is not available (NameError).
#[test]
fn test_sandbox_open_not_defined() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "sandbox_open.py",
        r#"
def transform(data, args, env, headers):
    f = open("/etc/passwd", "r")
    data["content"] = f.read()
    return data
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-m", &mold, "--input-format", "json"])
        .write_stdin(r#"{"test": 1}"#)
        .assert()
        .failure()
        .stderr(predicate::str::contains("open"));
}

/// Verify that subprocess cannot be imported.
#[test]
fn test_sandbox_no_subprocess() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "sandbox_subprocess.py",
        r#"
import subprocess

def transform(data, args, env, headers):
    data["out"] = subprocess.check_output(["id"])
    return data
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-m", &mold, "--input-format", "json"])
        .write_stdin(r#"{"test": 1}"#)
        .assert()
        .failure();
}

/// Verify that socket cannot be imported.
#[test]
fn test_sandbox_no_socket() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "sandbox_socket.py",
        r#"
import socket

def transform(data, args, env, headers):
    data["out"] = str(socket.getaddrinfo("example.com", 80))
    return data
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-m", &mold, "--input-format", "json"])
        .write_stdin(r#"{"test": 1}"#)
        .assert()
        .failure();
}
