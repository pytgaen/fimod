use super::helpers::setup_mold;
use assert_fs::prelude::*;
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

// ── New sandbox policy tests (0.5.0) ────────────────────────────────────────

/// Helper: write a TOML sandbox file and return its path.
fn setup_sandbox_file(dir: &assert_fs::TempDir, content: &str) -> String {
    let f = dir.child("sandbox.toml");
    f.write_str(content).unwrap();
    f.path().to_str().unwrap().to_string()
}

/// `datetime.now()` is denied by default (zero authorization) and raises PermissionError.
#[test]
fn test_sandbox_clock_denied_by_default() {
    let dir = assert_fs::TempDir::new().unwrap();
    let home = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "clock.py",
        r#"
from datetime import datetime

def transform(data, args, env, headers):
    return datetime.now().isoformat()
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-m", &mold, "--no-input"])
        .env("HOME", home.path())
        .env_remove("FIMOD_SANDBOX_FILE")
        .assert()
        .failure()
        .stderr(predicate::str::contains("datetime.now()"))
        .stderr(predicate::str::contains("allow_clock"));
}

/// `datetime.now()` returns a real value when `allow_clock = true` in sandbox.toml.
#[test]
fn test_sandbox_clock_allowed_via_file() {
    let dir = assert_fs::TempDir::new().unwrap();
    let home = assert_fs::TempDir::new().unwrap();
    let sandbox = setup_sandbox_file(&dir, "[sandbox]\nallow_clock = true\n");
    let mold = setup_mold(
        &dir,
        "clock_ok.py",
        r#"
from datetime import datetime

def transform(data, args, env, headers):
    d = datetime.now()
    return {"year": d.year}
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-m", &mold, "--no-input", "--sandbox-file", &sandbox])
        .env("HOME", home.path())
        .env_remove("FIMOD_SANDBOX_FILE")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""year":"#));
}

/// `date.today()` is denied by default and raises PermissionError.
#[test]
fn test_sandbox_date_today_denied_by_default() {
    let dir = assert_fs::TempDir::new().unwrap();
    let home = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "today.py",
        r#"
from datetime import date

def transform(data, args, env, headers):
    return date.today().isoformat()
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-m", &mold, "--no-input"])
        .env("HOME", home.path())
        .env_remove("FIMOD_SANDBOX_FILE")
        .assert()
        .failure()
        .stderr(predicate::str::contains("date.today()"))
        .stderr(predicate::str::contains("allow_clock"));
}

/// `os.getenv("LANG")` returns None unless `LANG` matches `allow_env` glob.
#[test]
fn test_sandbox_env_allowed_via_glob() {
    let dir = assert_fs::TempDir::new().unwrap();
    let home = assert_fs::TempDir::new().unwrap();
    let sandbox = setup_sandbox_file(&dir, "[sandbox]\nallow_env = [\"FIMOD_*\"]\n");
    let mold = setup_mold(
        &dir,
        "env.py",
        r#"
import os

def transform(data, args, env, headers):
    return {
        "allowed": os.getenv("FIMOD_TEST_VAR"),
        "denied": os.getenv("SECRET"),
    }
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-m", &mold, "--no-input", "--sandbox-file", &sandbox])
        .env("HOME", home.path())
        .env("FIMOD_TEST_VAR", "visible")
        .env("SECRET", "hidden")
        .env_remove("FIMOD_SANDBOX_FILE")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""allowed": "visible""#))
        .stdout(predicate::str::contains(r#""denied": null"#));
}

/// Infinite loop exits 137 when `max_duration = "1s"`.
#[test]
fn test_sandbox_timeout_exits_137() {
    let dir = assert_fs::TempDir::new().unwrap();
    let home = assert_fs::TempDir::new().unwrap();
    let sandbox = setup_sandbox_file(&dir, "[sandbox]\nmax_duration = \"1s\"\n");
    let mold = setup_mold(
        &dir,
        "loop.py",
        r#"
def transform(data, args, env, headers):
    i = 0
    while True:
        i += 1
    return i
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-m", &mold, "--no-input", "--sandbox-file", &sandbox])
        .env("HOME", home.path())
        .env_remove("FIMOD_SANDBOX_FILE")
        .assert()
        .code(137)
        .stderr(predicate::str::contains("sandbox exploded"))
        .stderr(predicate::str::contains("max_duration"));
}

/// `--sandbox-file=""` forces zero authorization even when canonical file is permissive.
#[test]
fn test_sandbox_empty_flag_forces_zero_auth() {
    let dir = assert_fs::TempDir::new().unwrap();
    let home = assert_fs::TempDir::new().unwrap();
    // Canonical file at ~/.config/fimod/sandbox.toml says allow_clock = true.
    let config_dir = home.child(".config/fimod");
    config_dir.create_dir_all().unwrap();
    config_dir
        .child("sandbox.toml")
        .write_str("[sandbox]\nallow_clock = true\n")
        .unwrap();

    let mold = setup_mold(
        &dir,
        "clock_zero.py",
        r#"
from datetime import datetime

def transform(data, args, env, headers):
    return datetime.now().isoformat()
"#,
    );

    // --sandbox-file="" should override the canonical permissive file.
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-m", &mold, "--no-input", "--sandbox-file="])
        .env("HOME", home.path())
        .env_remove("FIMOD_SANDBOX_FILE")
        .assert()
        .failure()
        .stderr(predicate::str::contains("allow_clock"));
}

/// `FIMOD_SANDBOX_FILE=/nonexistent` is a hard error with a clear message.
#[test]
fn test_sandbox_env_var_missing_file_errors() {
    let dir = assert_fs::TempDir::new().unwrap();
    let home = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "id.py",
        r#"
def transform(data, args, env, headers):
    return {"ok": True}
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-m", &mold, "--no-input"])
        .env("HOME", home.path())
        .env("FIMOD_SANDBOX_FILE", "/nonexistent/path/sandbox.toml")
        .assert()
        .failure()
        .stderr(predicate::str::contains("FIMOD_SANDBOX_FILE"))
        .stderr(predicate::str::contains("missing"));
}

/// Canonical `~/.config/fimod/sandbox.toml` is picked up automatically.
#[test]
fn test_sandbox_canonical_file_auto_loaded() {
    let dir = assert_fs::TempDir::new().unwrap();
    let home = assert_fs::TempDir::new().unwrap();
    let config_dir = home.child(".config/fimod");
    config_dir.create_dir_all().unwrap();
    config_dir
        .child("sandbox.toml")
        .write_str("[sandbox]\nallow_clock = true\n")
        .unwrap();

    let mold = setup_mold(
        &dir,
        "clock_canonical.py",
        r#"
from datetime import datetime

def transform(data, args, env, headers):
    return {"year": datetime.now().year}
"#,
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-m", &mold, "--no-input"])
        .env("HOME", home.path())
        .env_remove("FIMOD_SANDBOX_FILE")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""year":"#));
}
