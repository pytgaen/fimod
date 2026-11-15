//! Command output on stdout, with a closed stdout treated as a normal end.
//!
//! When the reader of stdout goes away (`fimod mold list | head -1`), nothing
//! went wrong: it got what it wanted. `println!` and `print!` panic in that
//! case, so every command that is not `fimod shape` writes through
//! [`stdout_print!`] and [`stdout_println!`] instead:
//!
//! - `BrokenPipe` is remembered and later writes are skipped. The command
//!   carries on and exits with the status it would have had.
//! - Any other write error is remembered (the first one) and later writes are
//!   skipped. `main` turns it into a failure through [`take_write_error`] once
//!   the command is otherwise successful.
//!
//! The state is process-wide, behind a mutex, because the macros are called
//! from deep inside listing code that has no handle to thread a writer
//! through, and the rule has to hold across all of those call sites at once.
//! The lock is not re-entrant: a `Display` implementation must not print through
//! this module while its output is being formatted by it.
//!
//! `fimod shape` has its own handling in `pipeline.rs` and `engine.rs`.

use std::fmt;
use std::io::{self, Write};
use std::sync::Mutex;

struct State {
    closed: bool,
    error: Option<io::Error>,
}

static STATE: Mutex<State> = Mutex::new(State {
    closed: false,
    error: None,
});

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn record(state: &mut State, result: io::Result<()>) {
    match result {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => state.closed = true,
        Err(error) => {
            state.error.get_or_insert(error);
        }
    }
}

fn skip(state: &State) -> bool {
    state.closed || state.error.is_some()
}

/// Write formatted text to stdout. Never fails and never panics: see the
/// module documentation.
pub fn write_fmt(args: fmt::Arguments<'_>) {
    let mut state = state();
    if skip(&state) {
        return;
    }
    let result = io::stdout().lock().write_fmt(args);
    record(&mut state, result);
}

/// Write raw bytes to stdout, with the same rule as [`write_fmt`].
pub fn write_bytes(bytes: &[u8]) {
    let mut state = state();
    if skip(&state) {
        return;
    }
    let result = io::stdout().lock().write_all(bytes);
    record(&mut state, result);
}

/// Flush stdout, with the same rule as [`write_fmt`].
pub fn flush() {
    let mut state = state();
    if skip(&state) {
        return;
    }
    let result = io::stdout().lock().flush();
    record(&mut state, result);
}

/// Flush stdout, then hand over the first write error other than
/// `BrokenPipe`, if any. Meant for `main`, once a command has finished.
pub fn take_write_error() -> Option<io::Error> {
    flush();
    state().error.take()
}

/// An [`io::Write`] adapter over [`write_bytes`] and [`flush`], for code that
/// takes a writer (an interactive prompt). It always reports success.
pub struct Stdout;

impl Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        write_bytes(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        flush();
        Ok(())
    }
}

/// Like `print!`, but a closed stdout is a normal end.
#[macro_export]
macro_rules! stdout_print {
    ($($arg:tt)*) => {
        $crate::stdout::write_fmt(::std::format_args!($($arg)*))
    };
}

/// Like `println!`, but a closed stdout is a normal end.
#[macro_export]
macro_rules! stdout_println {
    () => {
        $crate::stdout::write_fmt(::std::format_args!("\n"))
    };
    ($($arg:tt)*) => {
        $crate::stdout::write_fmt(::std::format_args!("{}\n", ::std::format_args!($($arg)*)))
    };
}
