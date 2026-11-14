//! A closed stdout (`fimod ... | head -1`) is a normal end, not an error.
//!
//! Every run here produces far more output than a pipe buffer (64 KiB on
//! Linux), so fimod still has bytes to write after the reader has gone.

#![cfg(unix)]

use super::helpers::{setup_input, setup_mold};
use std::io::Read;
use std::process::{Command, Stdio};

struct Outcome {
    code: Option<i32>,
    stderr: String,
}

/// Run fimod with piped stdout/stderr, read a few bytes, close stdout, wait.
fn run_and_close_stdout(args: &[&str]) -> Outcome {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fimod"))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn fimod");
    let mut stdout = child.stdout.take().unwrap();
    let mut first = [0u8; 8];
    stdout
        .read_exact(&mut first)
        .expect("fimod produced output");
    drop(stdout);
    let output = child.wait_with_output().expect("wait for fimod");
    Outcome {
        code: output.status.code(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn assert_silent_success(outcome: &Outcome, label: &str) {
    assert_eq!(outcome.code, Some(0), "{label}: exit status");
    assert!(
        outcome.stderr.is_empty(),
        "{label}: unexpected stderr: {}",
        outcome.stderr
    );
}

const RECORDS_MOLD: &str = r#"
def transform(data, **_):
    return [{"id": i, "name": f"user number {i}", "email": f"user{i}@example.com", "active": i % 2 == 0} for i in range(40000)]
"#;

const STRINGS_MOLD: &str = r#"
def transform(data, **_):
    return [f"line number {i} with some padding text" for i in range(80000)]
"#;

#[test]
fn test_closed_stdout_is_silent_success_for_every_output_format() {
    let dir = assert_fs::TempDir::new().unwrap();
    let records = setup_mold(&dir, "records.py", RECORDS_MOLD);
    let strings = setup_mold(&dir, "strings.py", STRINGS_MOLD);
    for format in ["json", "json-compact", "ndjson", "yaml", "csv"] {
        let outcome = run_and_close_stdout(&[
            "shape",
            "--no-input",
            "-m",
            &records,
            "--output-format",
            format,
        ]);
        assert_silent_success(&outcome, format);
    }
    let outcome = run_and_close_stdout(&[
        "shape",
        "--no-input",
        "-m",
        &strings,
        "--output-format",
        "lines",
    ]);
    assert_silent_success(&outcome, "lines");
}

fn large_json_array() -> String {
    let items: Vec<String> = (0..80000)
        .map(|i| {
            format!(r#"{{"id":{i},"name":"user number {i} with padding text","active":true}}"#)
        })
        .collect();
    format!("[{}]", items.join(","))
}

fn large_ndjson() -> String {
    (0..40000)
        .map(|i| format!(r#"{{"id":{i},"name":"user number {i}","active":true}}"#))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn test_closed_stdout_is_silent_success_for_identity_conversions() {
    let dir = assert_fs::TempDir::new().unwrap();
    let array = setup_input(&dir, "big.json", &large_json_array());
    let ndjson = setup_input(&dir, "big.ndjson", &large_ndjson());

    let outcome = run_and_close_stdout(&["shape", "-i", &array, "-e", "data"]);
    assert_silent_success(&outcome, "json array, default format");
    for format in ["ndjson", "csv"] {
        let outcome = run_and_close_stdout(&[
            "shape",
            "-i",
            &array,
            "-e",
            "data",
            "--output-format",
            format,
        ]);
        assert_silent_success(&outcome, format);
    }
    let outcome = run_and_close_stdout(&[
        "shape",
        "-i",
        &ndjson,
        "-e",
        "data",
        "--output-format",
        "json",
    ]);
    assert_silent_success(&outcome, "ndjson to json");
}

#[test]
fn test_closed_stdout_is_silent_success_for_mold_print() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "print.py",
        r#"
def transform(data, **_):
    for i in range(200000):
        print(f"printed line {i}")
    set_exit(4)
    return {"done": True}
"#,
    );
    let outcome = run_and_close_stdout(&["shape", "--no-input", "-m", &mold]);
    assert_eq!(outcome.code, Some(4), "mold print: set_exit code");
    assert!(outcome.stderr.is_empty(), "stderr: {}", outcome.stderr);
}

#[test]
fn test_closed_stdout_keeps_set_exit_code() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "exit3.py",
        r#"
def transform(data, **_):
    set_exit(3)
    return [{"id": i, "name": f"user number {i}", "email": f"user{i}@example.com"} for i in range(40000)]
"#,
    );
    let outcome = run_and_close_stdout(&["shape", "--no-input", "-m", &mold]);
    assert_eq!(outcome.code, Some(3));
    assert!(outcome.stderr.is_empty(), "stderr: {}", outcome.stderr);
}

#[test]
fn test_mold_print_output_is_unchanged_when_stdout_is_open() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "prints.py",
        r#"
import sys

def transform(data, **_):
    print("a", "b", sep="-", end="!")
    print()
    print("x", file=sys.stderr)
    return {"k": 1}
"#,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_fimod"))
        .args(["shape", "--no-input", "-m", &mold])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "a-b!\n{\n  \"k\": 1\n}\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "x\n");
}

#[test]
#[cfg(target_os = "linux")]
fn test_full_stdout_still_fails() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "small.py",
        "def transform(data, **_):\n    return {\"k\": 1}\n",
    );
    for format in ["json", "json-compact"] {
        let full = std::fs::OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_fimod"))
            .args([
                "shape",
                "--no-input",
                "-m",
                &mold,
                "--output-format",
                format,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::from(full))
            .stderr(Stdio::piped())
            .output()
            .unwrap();
        assert_ne!(output.status.code(), Some(0), "{format}: must fail");
        assert!(!output.stderr.is_empty(), "{format}: stderr must explain");
    }
}

/// Run fimod with a stdout pipe whose read end is already closed.
fn run_with_stdout_already_closed(args: &[&str]) -> Outcome {
    let (reader, writer) = std::io::pipe().expect("pipe");
    drop(reader);
    let output = Command::new(env!("CARGO_BIN_EXE_fimod"))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .output()
        .expect("run fimod");
    Outcome {
        code: output.status.code(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn assert_reports_error(outcome: &Outcome, label: &str) {
    assert!(
        matches!(outcome.code, Some(code) if code != 0),
        "{label}: must fail, got {:?}",
        outcome.code
    );
    assert!(!outcome.stderr.is_empty(), "{label}: stderr must explain");
}

fn records(count: usize) -> Vec<String> {
    (0..count)
        .map(|i| format!(r#"{{"id":{i},"name":"user number {i}"}}"#))
        .collect()
}

#[test]
fn test_data_errors_survive_a_closed_stdout_in_streaming_conversions() {
    let dir = assert_fs::TempDir::new().unwrap();
    // 20 records, or about 5 KiB of them, before the malformed element. The
    // error must sit inside the 8 KiB output buffer: past it, the failed
    // flush would end the stream first, and that is a normal end.
    for (label, count) in [("short", 20), ("5KiB", 100)] {
        let broken_array = setup_input(
            &dir,
            &format!("broken-{label}.json"),
            &format!("[{},{{\"id\": oops}}]", records(count).join(",")),
        );
        let broken_ndjson = setup_input(
            &dir,
            &format!("broken-{label}.ndjson"),
            &format!("{}\n{{\"id\": oops}}\n", records(count).join("\n")),
        );
        for format in ["ndjson", "csv"] {
            let outcome = run_with_stdout_already_closed(&[
                "shape",
                "-i",
                &broken_array,
                "-e",
                "data",
                "--output-format",
                format,
            ]);
            assert_reports_error(&outcome, &format!("{label} array to {format}"));
        }
        let outcome = run_with_stdout_already_closed(&[
            "shape",
            "-i",
            &broken_ndjson,
            "-e",
            "data",
            "--output-format",
            "json",
        ]);
        assert_reports_error(&outcome, &format!("{label} ndjson to json"));
    }

    // A parse error whose quoted text equals the BrokenPipe message must not
    // be mistaken for the write failure.
    let pipe_text = std::io::Error::from_raw_os_error(32).to_string();
    let lookalike = setup_input(
        &dir,
        "lookalike.ndjson",
        &format!("{}\n{pipe_text}\n", records(20).join("\n")),
    );
    for format in ["json", "json-compact"] {
        let outcome = run_with_stdout_already_closed(&[
            "shape",
            "-i",
            &lookalike,
            "-e",
            "data",
            "--output-format",
            format,
        ]);
        assert_reports_error(&outcome, &format!("lookalike ndjson to {format}"));
    }

    let mixed = setup_input(&dir, "mixed.json", r#"[{"a":1},[1,2]]"#);
    let outcome = run_with_stdout_already_closed(&[
        "shape",
        "-i",
        &mixed,
        "-e",
        "data",
        "--output-format",
        "csv",
    ]);
    assert_reports_error(&outcome, "mixed rows to csv");
}

#[test]
#[cfg(target_os = "linux")]
fn test_full_stdout_error_survives_a_mold_that_swallows_print_failure() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "swallow.py",
        r#"
def transform(data, **_):
    try:
        print("hello")
    except Exception:
        pass
    return {"k": 1}
"#,
    );
    let out = dir.path().join("out.json");
    let full = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_fimod"))
        .args(["shape", "--no-input", "-m", &mold, "-o"])
        .arg(&out)
        .stdin(Stdio::null())
        .stdout(Stdio::from(full))
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_ne!(output.status.code(), Some(0), "must fail");
    assert!(!output.stderr.is_empty(), "stderr must explain");
}

// ── commands other than `shape` ──────────────────────────────────────────────

/// Run fimod with `HOME` isolated, an optional stdin text, and a stdout pipe
/// whose read end is already closed.
fn run_closed(home: &std::path::Path, args: &[&str], stdin: Option<&str>) -> Outcome {
    use std::io::Write;
    let (reader, writer) = std::io::pipe().expect("pipe");
    drop(reader);
    let mut child = Command::new(env!("CARGO_BIN_EXE_fimod"))
        .args(args)
        .env("HOME", home)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn fimod");
    if let Some(text) = stdin {
        let mut pipe = child.stdin.take().unwrap();
        pipe.write_all(text.as_bytes()).unwrap();
    }
    let output = child.wait_with_output().expect("wait for fimod");
    Outcome {
        code: output.status.code(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// An isolated `HOME` with one local registry (`my`) holding one mold.
fn home_with_registry() -> (assert_fs::TempDir, assert_fs::TempDir) {
    let home = assert_fs::TempDir::new().unwrap();
    let molds = assert_fs::TempDir::new().unwrap();
    std::fs::write(
        molds.path().join("cleanup.py"),
        "def transform(data, **_):\n    return data\n",
    )
    .unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_fimod"))
        .args(["registry", "add", "my", molds.path().to_str().unwrap()])
        .env("HOME", home.path())
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());
    (home, molds)
}

#[test]
fn test_closed_stdout_is_silent_success_for_listing_commands() {
    let (home, _molds) = home_with_registry();
    let commands: [&[&str]; 5] = [
        &["mold", "list"],
        &["registry", "list"],
        &["registry", "cache", "info"],
        &["setup", "sandbox", "show"],
        &["setup", "completions", "--shell", "bash"],
    ];
    for args in commands {
        let outcome = run_closed(home.path(), args, None);
        assert_silent_success(&outcome, &args.join(" "));
    }
}

#[test]
fn test_closed_stdout_keeps_mold_test_exit_status() {
    let home = assert_fs::TempDir::new().unwrap();
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "id.py",
        "def transform(data, **_):\n    return data\n",
    );
    let tests = dir.path().join("tests");
    std::fs::create_dir(&tests).unwrap();
    std::fs::write(tests.join("ok.input.json"), r#"{"a": 1}"#).unwrap();
    std::fs::write(tests.join("ok.expected.json"), "{\n  \"a\": 1\n}\n").unwrap();
    let tests_dir = tests.to_str().unwrap();

    let outcome = run_closed(home.path(), &["mold", "test", &mold, tests_dir], None);
    assert_silent_success(&outcome, "passing case");

    std::fs::write(tests.join("bad.input.json"), r#"{"a": 1}"#).unwrap();
    std::fs::write(tests.join("bad.expected.json"), r#"{"a": 2}"#).unwrap();
    let outcome = run_closed(home.path(), &["mold", "test", &mold, tests_dir], None);
    assert_eq!(outcome.code, Some(1), "failing case: exit status");
    assert!(
        !outcome.stderr.contains("panicked"),
        "failing case: {}",
        outcome.stderr
    );
}

#[test]
fn test_closed_stdout_is_silent_success_for_repl() {
    let home = assert_fs::TempDir::new().unwrap();
    let input = "import sys\nprint(\"a\")\nprint(\"e\", file=sys.stderr)\n1+1\n";
    let args = ["monty", "repl", "--sandbox-file="];

    let open = Command::new(env!("CARGO_BIN_EXE_fimod"))
        .args(args)
        .env("HOME", home.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.take().unwrap().write_all(input.as_bytes())?;
            child.wait_with_output()
        })
        .unwrap();
    assert!(open.status.success());
    assert_eq!(String::from_utf8_lossy(&open.stdout), "a\n2\n");
    assert_eq!(String::from_utf8_lossy(&open.stderr), "e\n");

    let outcome = run_closed(home.path(), &args, Some("print(\"a\")\n1+1\n"));
    // An error raised to `print()` would show up here as `error: ...`.
    assert_silent_success(&outcome, "repl");
}

#[test]
#[cfg(target_os = "linux")]
fn test_full_stdout_fails_listing_commands() {
    let (home, _molds) = home_with_registry();
    for args in [["mold", "list"], ["registry", "list"]] {
        let full = std::fs::OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_fimod"))
            .args(args)
            .env("HOME", home.path())
            .stdin(Stdio::null())
            .stdout(Stdio::from(full))
            .stderr(Stdio::piped())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        let label = args.join(" ");
        assert_ne!(output.status.code(), Some(0), "{label}: must fail");
        assert!(
            stderr.contains("Failed to write output to stdout"),
            "{label}: stderr: {stderr}"
        );
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_full_stdout_is_reported_when_mold_test_fails() {
    let home = assert_fs::TempDir::new().unwrap();
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "id.py",
        "def transform(data, **_):\n    return data\n",
    );
    let tests = dir.path().join("tests");
    std::fs::create_dir(&tests).unwrap();
    std::fs::write(tests.join("bad.input.json"), r#"{"a": 1}"#).unwrap();
    std::fs::write(tests.join("bad.expected.json"), r#"{"a": 2}"#).unwrap();
    let full = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_fimod"))
        .args(["mold", "test", &mold, tests.to_str().unwrap()])
        .env("HOME", home.path())
        .stdin(Stdio::null())
        .stdout(Stdio::from(full))
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("Failed to write output to stdout"),
        "stderr: {stderr}"
    );
}
