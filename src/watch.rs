//! File-watching mode for `fimod shape --watch`.
//!
//! Re-runs `run_shape_pipeline` whenever a watched file changes (the input
//! file from `-i` and any local mold files from `-m`). Activated with the
//! `watch` Cargo feature.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use fimod::mold::MoldSource;
use fimod::pipeline::CliResult;
use fimod::sandbox::SandboxPolicy;
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};

use crate::cli::ShapeArgs;

const DEBOUNCE_MS: u64 = 150;
/// Second-level debounce default: after we receive the first batch from
/// `notify-debouncer-mini`, wait this long and absorb any further batches
/// the debouncer flushes during that window. Coalesces the multiple `Any`
/// events that `notify` emits per `File::create` (truncate + write + close
/// can produce events arriving > DEBOUNCE_MS apart on Linux/inotify).
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

    let canonical_targets: HashSet<PathBuf> = watch_files
        .iter()
        .filter_map(|p| p.canonicalize().ok())
        .collect();

    // Pre-compute target filenames so noisy sibling events (.swp, .tmp, etc.)
    // can be rejected without a canonicalize() syscall on every event.
    let target_filenames: HashSet<OsString> = watch_files
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_os_string()))
        .collect();

    let (tx, rx) = mpsc::channel();
    let mut debouncer = new_debouncer(Duration::from_millis(DEBOUNCE_MS), tx)?;

    // Watch parent dirs (atomic-rename safe), filter events by filename.
    let mut watched_dirs: HashSet<PathBuf> = HashSet::new();
    for f in &watch_files {
        let parent = f
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        if watched_dirs.insert(parent.clone()) {
            debouncer
                .watcher()
                .watch(&parent, RecursiveMode::NonRecursive)?;
        }
    }

    let input_path: Option<PathBuf> = shape.input.first().map(PathBuf::from);
    let mut input_present = input_path.as_ref().is_some_and(|p| p.exists());
    let quiet = Duration::from_millis(quiet_ms());

    let mut run_n: u32 = 1;
    run_once(shape, policy, debug, msg_level, run_n);

    while let Ok(first) = rx.recv() {
        // Second-level debounce: notify can split a single logical write
        // into multiple events that the first-level debouncer flushes as
        // separate batches > DEBOUNCE_MS apart on Linux/inotify. We sleep
        // for the quiet window and drain everything pending so a single
        // trigger covers the full burst.
        std::thread::sleep(quiet);
        let mut batches = vec![first];
        while let Ok(more) = rx.try_recv() {
            batches.push(more);
        }
        let triggered = batches
            .into_iter()
            .filter_map(Result::ok)
            .flatten()
            .any(|e| {
                e.kind == DebouncedEventKind::Any
                    && e.path
                        .file_name()
                        .is_some_and(|n| target_filenames.contains(n))
                    && e.path
                        .canonicalize()
                        .map(|p| canonical_targets.contains(&p))
                        .unwrap_or(false)
            });

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

        if triggered {
            run_n += 1;
            run_once(shape, policy, debug, msg_level, run_n);
        }
    }

    Ok(CliResult::Done)
}

fn run_once(shape: &ShapeArgs, policy: &SandboxPolicy, debug: bool, msg_level: u8, run_n: u32) {
    let start = Instant::now();
    match crate::run_shape_pipeline(shape, policy, debug, msg_level) {
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
