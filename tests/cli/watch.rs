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
        let tmp = tempfile::NamedTempFile::new_in(dir.path()).expect("tmp in dir");
        std::fs::write(tmp.path(), format!(r#"{{"n":{i}}}"#)).expect("write tmp");
        tmp.persist(input.path()).expect("atomic rename to in.json");
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

#[test]
fn test_watch_warns_on_delete_then_reruns_on_recreate() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = dir.child("in.json");
    let output = dir.child("out.json");
    input.write_str(r#"{"n":1}"#).unwrap();

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
        || read_n(output.path()) == Some(1),
        STARTUP_TIMEOUT,
        "run #1 to write n=1",
    );

    thread::sleep(DEBOUNCE_GAP);
    std::fs::remove_file(input.path()).expect("remove input");

    // Wait > COALESCE_MS (500ms) so the unlink lands in its own batch and
    // triggers the (true → false) transition warning, before the recreate.
    thread::sleep(Duration::from_millis(800));

    poll_until(
        || {
            stderr_lines
                .lock()
                .unwrap()
                .iter()
                .any(|l| l.contains("[watch] warn: input removed"))
        },
        Duration::from_secs(3),
        "warning about removed input",
    );

    input.write_str(r#"{"n":99}"#).unwrap();

    poll_until(
        || read_n(output.path()) == Some(99),
        RERUN_TIMEOUT,
        "rerun after recreate to write n=99",
    );
}

#[test]
fn test_watch_reruns_on_mold_file_change() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = dir.child("in.json");
    let mold = dir.child("mold.py");
    let output = dir.child("out.json");
    input.write_str(r#"{}"#).unwrap();
    mold.write_str("def transform(data, **_):\n    data['n'] = 1\n    return data\n")
        .unwrap();

    let bin = assert_cmd::cargo::cargo_bin("fimod");
    let child = Command::new(bin)
        .arg("shape")
        .args([
            "-i",
            input.path().to_str().unwrap(),
            "-m",
            mold.path().to_str().unwrap(),
            "-o",
            output.path().to_str().unwrap(),
            "--watch",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn fimod --watch");
    let _guard = ChildGuard(Some(child));

    poll_until(
        || read_n(output.path()) == Some(1),
        STARTUP_TIMEOUT,
        "run #1 to write n=1 from initial mold",
    );

    thread::sleep(DEBOUNCE_GAP);
    mold.write_str("def transform(data, **_):\n    data['n'] = 2\n    return data\n")
        .unwrap();

    poll_until(
        || read_n(output.path()) == Some(2),
        RERUN_TIMEOUT,
        "rerun after editing mold.py to write n=2",
    );
}

#[test]
fn test_watch_survives_mold_panic_and_reruns_after_fix() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = dir.child("in.json");
    let mold = dir.child("mold.py");
    let output = dir.child("out.json");
    input.write_str(r#"{"n":0}"#).unwrap();
    mold.write_str("def transform(data, **_):\n    data['n'] = 1\n    return data\n")
        .unwrap();

    let bin = assert_cmd::cargo::cargo_bin("fimod");
    let mut child = Command::new(bin)
        .arg("shape")
        .args([
            "-i",
            input.path().to_str().unwrap(),
            "-m",
            mold.path().to_str().unwrap(),
            "-o",
            output.path().to_str().unwrap(),
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
        || read_n(output.path()) == Some(1),
        STARTUP_TIMEOUT,
        "run #1 to write n=1",
    );

    thread::sleep(DEBOUNCE_GAP);
    mold.write_str("def transform(data, **_):\n    raise ValueError('boom')\n")
        .unwrap();

    poll_until(
        || {
            stderr_lines
                .lock()
                .unwrap()
                .iter()
                .any(|l| l.contains("[watch] run #2 failed"))
        },
        RERUN_TIMEOUT,
        "run #2 to fail with [watch] run #2 failed marker",
    );

    thread::sleep(DEBOUNCE_GAP);
    mold.write_str("def transform(data, **_):\n    data['n'] = 99\n    return data\n")
        .unwrap();

    poll_until(
        || read_n(output.path()) == Some(99),
        RERUN_TIMEOUT,
        "run #3 to write n=99 after fixing mold (watcher must still be alive)",
    );
}

#[test]
fn test_watch_survives_malformed_input_and_reruns_after_fix() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = dir.child("in.json");
    let output = dir.child("out.json");
    input.write_str(r#"{"n":1}"#).unwrap();

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
        || read_n(output.path()) == Some(1),
        STARTUP_TIMEOUT,
        "run #1 to write n=1",
    );

    thread::sleep(DEBOUNCE_GAP);
    input.write_str("not json {{{").unwrap();

    poll_until(
        || {
            stderr_lines
                .lock()
                .unwrap()
                .iter()
                .any(|l| l.contains("[watch] run #2 failed"))
        },
        RERUN_TIMEOUT,
        "run #2 to fail on malformed input",
    );

    thread::sleep(DEBOUNCE_GAP);
    input.write_str(r#"{"n":99}"#).unwrap();

    poll_until(
        || read_n(output.path()) == Some(99),
        RERUN_TIMEOUT,
        "run #3 to write n=99 after fixing input (watcher must still be alive)",
    );
}

#[cfg(unix)]
fn assert_watch_exits_on_signal(kill_arg: &str, expected_signo: i32) {
    use std::os::unix::process::ExitStatusExt;

    let dir = assert_fs::TempDir::new().unwrap();
    let input = dir.child("in.json");
    let output = dir.child("out.json");
    input.write_str(r#"{"n":1}"#).unwrap();

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
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn fimod --watch");

    poll_until(
        || read_n(output.path()) == Some(1),
        STARTUP_TIMEOUT,
        "run #1 before signal",
    );

    let pid = child.id().to_string();
    Command::new("kill")
        .args([kill_arg, &pid])
        .status()
        .expect("kill");

    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        match child.try_wait().expect("try_wait") {
            Some(s) => break s,
            None => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("watcher did not exit within 3s of {kill_arg}");
                }
                thread::sleep(POLL_INTERVAL);
            }
        }
    };

    assert_eq!(
        status.signal(),
        Some(expected_signo),
        "expected exit by signal {expected_signo} ({kill_arg}), got {status:?}"
    );
}

#[test]
fn test_watch_quiet_ms_env_overrides_default() {
    // Pin que FIMOD_WATCH_QUIET_MS est bien lu en baissant la fenêtre de
    // coalescing : default 500ms coalescerait 2 writes espacés de 800ms
    // (run_count == 2), override 50ms ne coalescera PAS (run_count == 3).
    // L'inverse (override grand vs default petit) est moins discriminant
    // car sensible au timing inotify cross-process.
    let dir = assert_fs::TempDir::new().unwrap();
    let input = dir.child("in.json");
    let output = dir.child("out.json");
    input.write_str(r#"{"n":0}"#).unwrap();

    let bin = assert_cmd::cargo::cargo_bin("fimod");
    let mut child = Command::new(bin)
        .env("FIMOD_WATCH_QUIET_MS", "50")
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
    input.write_str(r#"{"n":1}"#).unwrap();
    // 800ms gap >> default 500ms (would coalesce) >> override 50ms (won't).
    thread::sleep(Duration::from_millis(800));
    input.write_str(r#"{"n":2}"#).unwrap();

    poll_until(
        || read_n(output.path()) == Some(2),
        RERUN_TIMEOUT,
        "rerun to write n=2",
    );

    // Quiet window of 50ms is over long before this; any pending rerun
    // had ample time to fire.
    thread::sleep(Duration::from_millis(500));

    let lines = stderr_lines.lock().unwrap();
    let run_count = lines.iter().filter(|l| l.contains("[watch] run #")).count();
    assert!(
        run_count >= 3,
        "expected >= 3 runs (init + 2 non-coalesced) under FIMOD_WATCH_QUIET_MS=50, got {run_count}: {lines:?}"
    );
}

#[cfg(unix)]
#[test]
fn test_watch_exits_on_sigint() {
    assert_watch_exits_on_signal("-INT", 2);
}

#[cfg(unix)]
#[test]
fn test_watch_exits_on_sigterm() {
    assert_watch_exits_on_signal("-TERM", 15);
}
