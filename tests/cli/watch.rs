#![cfg(feature = "watch")]

use std::path::Path;
use std::process::{Child, Command, Stdio};
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
