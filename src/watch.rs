//! File-watching mode for `fimod shape --watch`.
//!
//! Re-runs `run_shape_pipeline` whenever a watched file changes (the input
//! file from `-i` and any local mold files from `-m`). Activated with the
//! `watch` Cargo feature.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use anyhow::Result;
use fimod::mold::MoldSource;
use fimod::pipeline::CliResult;
use fimod::sandbox::SandboxPolicy;
use notify::event::{AccessKind, AccessMode, ModifyKind};
use notify::{Event, EventKind, RecursiveMode, Watcher};

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

    // Watch parent dirs (atomic-rename safe), filter events by filename.
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
    let mut input_present = input_path.as_ref().is_some_and(|p| p.exists());
    let quiet = Duration::from_millis(quiet_ms());

    let mut run_n: u32 = 1;
    run_once(shape, policy, debug, msg_level, run_n);

    while let Ok(first) = rx.recv() {
        // Debounce: notify can split a single logical write into multiple
        // events. Wait until the stream is quiet so one trigger covers the
        // full burst.
        let Some(event) = first.ok() else {
            continue;
        };
        if !event_triggers_rerun(&event, &targets) {
            continue;
        }

        drain_quiet_period(&rx, &targets, quiet);

        // Track input file existence transitions across batches. An atomic
        // save (rename) resolves within the quiet window — fichier existe
        // début et fin de batch — donc transition (true, true), pas de
        // warning. Un vrai unlink prolongé déclenche (true, false) et logue.
        if let Some(ref ip) = input_path {
            let now_present = ip.exists();
            if input_present && !now_present {
                eprintln!("[watch] warn: input removed, waiting for it to reappear...");
            }
            input_present = now_present;
        }

        run_n += 1;
        run_once(shape, policy, debug, msg_level, run_n);
    }

    Ok(CliResult::Done)
}

fn run_once(shape: &ShapeArgs, policy: &SandboxPolicy, debug: bool, msg_level: u8, run_n: u32) {
    let start = Instant::now();
    match crate::cmd::shape::run_shape_pipeline(shape, policy, debug, msg_level) {
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

fn drain_quiet_period(
    rx: &mpsc::Receiver<notify::Result<Event>>,
    targets: &WatchTargets,
    quiet: Duration,
) {
    let mut deadline = Instant::now() + quiet;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining) {
            Ok(Ok(event)) if event_triggers_rerun(&event, targets) => {
                deadline = Instant::now() + quiet;
            }
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
        }
    }
}

struct WatchTargets {
    filenames: HashSet<OsString>,
    canonical_paths: HashSet<PathBuf>,
    absolute_paths: HashSet<PathBuf>,
}

impl WatchTargets {
    fn new(files: &[PathBuf]) -> Result<Self> {
        let filenames = files
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_os_string()))
            .collect();
        let canonical_paths = files.iter().filter_map(|p| p.canonicalize().ok()).collect();
        let absolute_paths = files
            .iter()
            .map(|p| absolute_normalized_path(p))
            .collect::<Result<_>>()?;

        Ok(Self {
            filenames,
            canonical_paths,
            absolute_paths,
        })
    }

    fn contains_event_path(&self, path: &Path) -> bool {
        path.file_name().is_some_and(|n| self.filenames.contains(n))
            && (path
                .canonicalize()
                .is_ok_and(|p| self.canonical_paths.contains(&p))
                || absolute_normalized_path(path).is_ok_and(|p| self.absolute_paths.contains(&p)))
    }
}

fn event_triggers_rerun(event: &Event, targets: &WatchTargets) -> bool {
    is_write_like_event(&event.kind) && event.paths.iter().any(|p| targets.contains_event_path(p))
}

fn is_write_like_event(kind: &EventKind) -> bool {
    match kind {
        EventKind::Create(_)
        | EventKind::Remove(_)
        | EventKind::Access(AccessKind::Close(AccessMode::Write))
        | EventKind::Modify(ModifyKind::Data(_))
        | EventKind::Modify(ModifyKind::Name(_))
        | EventKind::Modify(ModifyKind::Any)
        | EventKind::Modify(ModifyKind::Other)
        | EventKind::Any
        | EventKind::Other => true,
        EventKind::Access(_) | EventKind::Modify(ModifyKind::Metadata(_)) => false,
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
    use notify::event::{DataChange, MetadataKind, RenameMode};

    #[test]
    fn write_like_filter_ignores_read_access() {
        assert!(!is_write_like_event(&EventKind::Access(AccessKind::Open(
            AccessMode::Read
        ))));
        assert!(!is_write_like_event(&EventKind::Access(AccessKind::Close(
            AccessMode::Read
        ))));
    }

    #[test]
    fn write_like_filter_accepts_real_changes() {
        assert!(is_write_like_event(&EventKind::Modify(ModifyKind::Data(
            DataChange::Content
        ))));
        assert!(is_write_like_event(&EventKind::Modify(ModifyKind::Name(
            RenameMode::Both
        ))));
        assert!(is_write_like_event(&EventKind::Access(AccessKind::Close(
            AccessMode::Write
        ))));
    }

    #[test]
    fn write_like_filter_ignores_metadata_only_changes() {
        assert!(!is_write_like_event(&EventKind::Modify(
            ModifyKind::Metadata(MetadataKind::Any)
        )));
    }

    #[test]
    fn target_matching_survives_deleted_paths() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("in.json");
        std::fs::write(&input, "{}").unwrap();
        let targets = WatchTargets::new(std::slice::from_ref(&input)).unwrap();
        std::fs::remove_file(&input).unwrap();

        assert!(targets.contains_event_path(&input));
    }
}
