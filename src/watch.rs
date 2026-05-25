//! File-watching mode for `fimod shape --watch`.
//!
//! Re-runs `run_shape_pipeline` whenever a watched file changes (the input
//! file from `-i` and any local mold files from `-m`). Activated with the
//! `watch` Cargo feature.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant, SystemTime};

use anyhow::Result;
use fimod::mold::MoldSource;
use fimod::pipeline::{CliResult, ScriptRef};
use fimod::sandbox::SandboxPolicy;
use notify::{Event, RecursiveMode, Watcher};

use crate::cli::ShapeArgs;

/// Debounce default: after we receive the first relevant filesystem event,
/// wait this long and absorb any further events in that window. Coalesces
/// the multiple events that `notify` emits per `File::create` (truncate +
/// write + close) and rapid editor atomic-save bursts.
/// Override via `FIMOD_WATCH_QUIET_MS=<ms>`.
const DEFAULT_QUIET_MS: u64 = 500;

fn quiet_ms() -> u64 {
    std::env::var("FIMOD_WATCH_QUIET_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_QUIET_MS)
}

pub fn run_watch(
    shape: &ShapeArgs,
    script_refs: &[ScriptRef],
    policy: &SandboxPolicy,
    debug: bool,
    msg_level: u8,
) -> Result<CliResult> {
    if let Some(input) = shape.input.first() {
        if !std::path::Path::new(input).exists() {
            anyhow::bail!("Failed to read input file '{input}': file does not exist");
        }
    }

    let watch_files = collect_watch_files(shape);

    eprintln!(
        "[watch] watching {}",
        watch_files
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let targets = WatchTargets::new(&watch_files)?;

    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(tx)?;

    // Watch parent dirs (atomic-rename safe), then snapshot the explicit
    // target files after each event burst.
    let mut watched_dirs: HashSet<PathBuf> = HashSet::new();
    for f in &watch_files {
        let parent = f
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        if watched_dirs.insert(parent.clone()) {
            watcher.watch(&parent, RecursiveMode::NonRecursive)?;
        }
    }

    let input_path: Option<PathBuf> = shape.input.first().map(PathBuf::from);
    let input_index = input_path
        .as_ref()
        .and_then(|p| targets.index_of(p).ok().flatten());
    let quiet = Duration::from_millis(quiet_ms());
    let mut snapshots = targets.snapshots();

    let mut run_n: u32 = 1;
    run_once(shape, script_refs, policy, debug, msg_level, run_n);

    while let Ok(first) = rx.recv() {
        let Ok(_) = first else {
            continue;
        };

        // Debounce: notify can split a single logical write into multiple
        // events. Wait briefly, then inspect the actual watched files. That
        // snapshot gate ignores read/access noise and stale delayed events
        // that platforms such as macOS may emit after the content is already
        // handled.
        drain_quiet_period(&rx, quiet);
        let new_snapshots = targets.snapshots();
        if new_snapshots == snapshots {
            continue;
        }

        // Track input file existence transitions across batches. An atomic
        // save (rename) resolves within the quiet window — fichier existe
        // début et fin de batch — donc transition (true, true), pas de
        // warning. Un vrai unlink prolongé déclenche (true, false) et logue.
        if let Some(index) = input_index {
            if snapshots[index].exists && !new_snapshots[index].exists {
                eprintln!("[watch] warn: input removed, waiting for it to reappear...");
            }
        }

        snapshots = new_snapshots;
        run_n += 1;
        run_once(shape, script_refs, policy, debug, msg_level, run_n);
    }

    Ok(CliResult::Done)
}

fn run_once(
    shape: &ShapeArgs,
    script_refs: &[ScriptRef],
    policy: &SandboxPolicy,
    debug: bool,
    msg_level: u8,
    run_n: u32,
) {
    let start = Instant::now();
    match crate::cmd::shape::run_shape_pipeline(shape, script_refs, policy, debug, msg_level) {
        Ok(_) => eprintln!(
            "[watch] run #{run_n} ok ({}ms)",
            start.elapsed().as_millis()
        ),
        Err(e) => eprintln!(
            "[watch] run #{run_n} failed ({}ms)\n  {:#}",
            start.elapsed().as_millis(),
            e
        ),
    }
}

fn drain_quiet_period(rx: &mpsc::Receiver<notify::Result<Event>>, quiet: Duration) {
    let deadline = Instant::now() + quiet;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining) {
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
        }
    }
}

struct WatchTargets {
    files: Vec<PathBuf>,
}

impl WatchTargets {
    fn new(files: &[PathBuf]) -> Result<Self> {
        let files = files
            .iter()
            .map(|p| absolute_normalized_path(p))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { files })
    }

    fn index_of(&self, path: &Path) -> Result<Option<usize>> {
        let path = absolute_normalized_path(path)?;
        Ok(self.files.iter().position(|p| p == &path))
    }

    fn snapshots(&self) -> Vec<FileSnapshot> {
        self.files
            .iter()
            .map(|path| FileSnapshot::from_path(path))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FileSnapshot {
    exists: bool,
    len: Option<u64>,
    modified: Option<SystemTime>,
}

impl FileSnapshot {
    fn from_path(path: &Path) -> Self {
        match std::fs::metadata(path) {
            Ok(metadata) => Self {
                exists: true,
                len: Some(metadata.len()),
                modified: metadata.modified().ok(),
            },
            Err(_) => Self {
                exists: false,
                len: None,
                modified: None,
            },
        }
    }
}

fn absolute_normalized_path(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(normalize_path(&absolute))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn collect_watch_files(shape: &ShapeArgs) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Some(input) = shape.input.first() {
        files.push(PathBuf::from(input));
    }
    for m in &shape.mold {
        if MoldSource::is_local_path(m) {
            let p = PathBuf::from(m);
            if p.exists() {
                files.push(p);
            }
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_ignore_read_access() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("in.json");
        std::fs::write(&input, "{}").unwrap();
        let targets = WatchTargets::new(std::slice::from_ref(&input)).unwrap();

        let before = targets.snapshots();
        let _ = std::fs::read_to_string(&input).unwrap();

        assert_eq!(targets.snapshots(), before);
    }

    #[test]
    fn snapshots_detect_content_changes() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("in.json");
        std::fs::write(&input, "{}").unwrap();
        let targets = WatchTargets::new(std::slice::from_ref(&input)).unwrap();

        let before = targets.snapshots();
        std::fs::write(&input, r#"{"changed": true}"#).unwrap();

        assert_ne!(targets.snapshots(), before);
    }

    #[test]
    fn snapshots_track_deleted_paths() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("in.json");
        std::fs::write(&input, "{}").unwrap();
        let targets = WatchTargets::new(std::slice::from_ref(&input)).unwrap();

        let before = targets.snapshots();
        std::fs::remove_file(&input).unwrap();
        let after = targets.snapshots();

        assert!(before[0].exists);
        assert!(!after[0].exists);
        assert_ne!(after, before);
    }
}
