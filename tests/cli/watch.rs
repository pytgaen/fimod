#![cfg(feature = "watch")]

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use assert_fs::prelude::*;

const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const RERUN_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

// Must exceed DEBOUNCE_MS (150) in src/watch.rs so the watcher is armed
// in rx.recv() before the second write — otherwise the event can be lost.
const DEBOUNCE_GAP: Duration = Duration::from_millis(300);

struct ChildGuard(Option<Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut c) = self.0.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

#[test]
fn test_watch_reruns_pipeline_on_input_change() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = dir.child("in.json");
    let output = dir.child("out.json");
    input.write_str(r#"{"x": 1}"#).unwrap();

    let bin = assert_cmd::cargo::cargo_bin("fimod");
    let child = Command::new(bin)
        .arg("shape")
        .args([
            "-i",
            input.path().to_str().unwrap(),
            "-o",
            output.path().to_str().unwrap(),
            "-e",
            "data",
            "--watch",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn fimod --watch");
    let _guard = ChildGuard(Some(child));

    poll_until(
        || read_x(output.path()) == Some(1),
        STARTUP_TIMEOUT,
        "run #1 to write x=1",
    );

    thread::sleep(DEBOUNCE_GAP);
    input.write_str(r#"{"x": 2}"#).unwrap();

    poll_until(
        || read_x(output.path()) == Some(2),
        RERUN_TIMEOUT,
        "run #2 to write x=2",
    );
}

fn read_x(path: &Path) -> Option<i64> {
    let content = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&content).ok()?;
    v.get("x")?.as_i64()
}

fn poll_until<F: FnMut() -> bool>(mut cond: F, timeout: Duration, description: &str) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if cond() {
            return;
        }
        thread::sleep(POLL_INTERVAL);
    }
    panic!("timed out after {timeout:?} waiting for {description}");
}

#[test]
fn test_watch_missing_input_at_startup_fails_cleanly() {
    let dir = assert_fs::TempDir::new().unwrap();
    let nonexistent = dir.path().join("does_not_exist.json");

    let bin = assert_cmd::cargo::cargo_bin("fimod");
    let mut child = Command::new(bin)
        .arg("shape")
        .args(["-i", nonexistent.to_str().unwrap(), "-e", "data", "--watch"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn fimod --watch");

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Ok(Some(_)) = child.try_wait() {
            let output = child.wait_with_output().unwrap();
            assert!(
                !output.status.success(),
                "expected failure when input is missing"
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("Failed to read")
                    || stderr.contains("not found")
                    || stderr.contains("No such")
                    || stderr.contains("does_not_exist"),
                "expected clear error in stderr, got: {stderr:?}"
            );
            return;
        }
        thread::sleep(POLL_INTERVAL);
    }

    let _ = child.kill();
    let _ = child.wait();
    panic!("fimod --watch hung when input was missing — should bail at startup");
}

#[test]
fn test_watch_debounces_rapid_writes_into_single_rerun() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = dir.child("in.json");
    let output = dir.child("out.json");
    input.write_str(r#"{"n":0}"#).unwrap();

    let bin = assert_cmd::cargo::cargo_bin("fimod");
    let mut child = Command::new(bin)
        .arg("shape")
        .args([
            "-i",
            input.path().to_str().unwrap(),
            "-o",
            output.path().to_str().unwrap(),
            "-e",
            "data",
            "--watch",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn fimod --watch");

    let stderr_lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let stderr_pipe = child.stderr.take().expect("stderr piped");
    let stderr_lines_thread = Arc::clone(&stderr_lines);
    thread::spawn(move || {
        for line in BufReader::new(stderr_pipe).lines().map_while(Result::ok) {
            stderr_lines_thread.lock().unwrap().push(line);
        }
    });
    let _guard = ChildGuard(Some(child));

    poll_until(
        || read_n(output.path()) == Some(0),
        STARTUP_TIMEOUT,
        "run #1 to write n=0",
    );

    thread::sleep(DEBOUNCE_GAP);

    for i in 1..=5 {
        input.write_str(&format!(r#"{{"n":{i}}}"#)).unwrap();
    }

    poll_until(
        || read_n(output.path()) == Some(5),
        RERUN_TIMEOUT,
        "debounced rerun to write n=5",
    );

    thread::sleep(Duration::from_millis(500));

    let lines = stderr_lines.lock().unwrap();
    let run_count = lines.iter().filter(|l| l.contains("[watch] run #")).count();
    assert_eq!(
        run_count, 2,
        "expected 1 initial run + 1 debounced rerun, got {run_count} — debounce broken: {lines:?}"
    );
}

fn read_n(path: &Path) -> Option<i64> {
    let content = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&content).ok()?;
    v.get("n")?.as_i64()
}

#[test]
fn test_watch_detects_atomic_save_via_rename() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = dir.child("in.json");
    let output = dir.child("out.json");
    input.write_str(r#"{"n":0}"#).unwrap();

    let bin = assert_cmd::cargo::cargo_bin("fimod");
    let child = Command::new(bin)
        .arg("shape")
        .args([
            "-i",
            input.path().to_str().unwrap(),
            "-o",
            output.path().to_str().unwrap(),
            "-e",
            "data",
            "--watch",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn fimod --watch");
    let _guard = ChildGuard(Some(child));

    poll_until(
        || read_n(output.path()) == Some(0),
        STARTUP_TIMEOUT,
        "initial run #1 to write n=0",
    );

    thread::sleep(DEBOUNCE_GAP);

    let tmp = tempfile::NamedTempFile::new_in(dir.path()).expect("tmp in dir");
    std::fs::write(tmp.path(), r#"{"n":42}"#).expect("write tmp");
    tmp.persist(input.path()).expect("atomic rename to in.json");

    poll_until(
        || read_n(output.path()) == Some(42),
        RERUN_TIMEOUT,
        "rerun after atomic save (rename) to write n=42",
    );
}
