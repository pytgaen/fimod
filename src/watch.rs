//! File-watching mode for `fimod shape --watch`.
//!
//! Re-runs `run_shape_pipeline` whenever a watched file changes (the input
//! file from `-i` and any local mold files from `-m`). Activated with the
//! `watch` Cargo feature.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
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
) -> Result<()> {
    let watch_files = collect_watch_files(shape);
    if watch_files.is_empty() {
        bail!("--watch: no watchable files (this is a bug)");
    }

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

    Ok(())
}

fn run_once(
    shape: &ShapeArgs,
    policy: &SandboxPolicy,
    debug: bool,
    msg_level: u8,
    run_n: u32,
) {
    let start = Instant::now();
    eprint!("[watch] run #{run_n} ... ");
    // is_multi_slurp + is_batch are always false in watch (rejected by validation)
    match crate::run_shape_pipeline(shape, policy, debug, msg_level, false, false) {
        Ok(()) => eprintln!("ok ({}ms)", start.elapsed().as_millis()),
        Err(e) => eprintln!("failed ({}ms)\n  {:#}", start.elapsed().as_millis(), e),
    }
}

fn collect_watch_files(shape: &ShapeArgs) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Some(input) = shape.input.first() {
        files.push(PathBuf::from(input));
    }
    for m in &shape.mold {
        if !m.starts_with('@') {
            let p = PathBuf::from(m);
            if p.exists() {
                files.push(p);
            }
        }
    }
    files
}
