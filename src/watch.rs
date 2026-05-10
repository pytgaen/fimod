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
use notify_debouncer_mini::new_debouncer;

use crate::ShapeArgs;

const DEBOUNCE_MS: u64 = 150;

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

    let mut run_n: u32 = 1;
    run_once(shape, policy, debug, msg_level, run_n);

    loop {
        match rx.recv() {
            Ok(Ok(events)) => {
                let triggered = events.iter().any(|e| {
                    e.path
                        .file_name()
                        .is_some_and(|n| target_filenames.contains(n))
                        && e.path
                            .canonicalize()
                            .map(|p| canonical_targets.contains(&p))
                            .unwrap_or(false)
                });
                if triggered {
                    run_n += 1;
                    run_once(shape, policy, debug, msg_level, run_n);
                }
            }
            Ok(Err(_)) => continue,
            Err(_) => break,
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
