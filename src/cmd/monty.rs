use anyhow::Result;
use monty::{ExcType, LimitedTracker, MontyObject, MontyRepl, NameLookupResult, ReplProgress};

use fimod::{sandbox::SandboxPolicy, MONTY_VERSION};

type FimodRepl = MontyRepl<LimitedTracker>;
type ReplRunResult = Result<(FimodRepl, MontyObject), Box<(FimodRepl, String)>>;

pub fn run_monty_repl(sandbox_file: Option<String>) -> Result<()> {
    use monty::{detect_repl_continuation_mode, ReplContinuationMode};
    use rustyline::error::ReadlineError;
    use rustyline::DefaultEditor;

    let policy = SandboxPolicy::resolve(sandbox_file.as_deref())?;
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stdin());

    if is_tty {
        eprintln!(
            "Monty REPL v{MONTY_VERSION} — fimod v{} (exit or Ctrl+D to quit)",
            env!("CARGO_PKG_VERSION")
        );
    }

    let mut rl = DefaultEditor::new()?;
    let mut repl = Some(MontyRepl::new(
        "repl.py",
        LimitedTracker::new(fimod::engine::sandbox_resource_limits(&policy)),
    ));
    let mut pending_snippet = String::new();
    let mut continuation_mode = ReplContinuationMode::Complete;

    loop {
        let prompt = if continuation_mode == ReplContinuationMode::Complete {
            ">>> "
        } else {
            "... "
        };

        let line = match rl.readline(prompt) {
            Ok(l) => l,
            Err(ReadlineError::Interrupted) => continue,
            Err(ReadlineError::Eof) => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        let _ = rl.add_history_entry(&line);

        let snippet = line.trim_end();
        if continuation_mode == ReplContinuationMode::Complete && snippet.is_empty() {
            continue;
        }
        if continuation_mode == ReplContinuationMode::Complete && snippet == "exit" {
            return Ok(());
        }

        pending_snippet.push_str(snippet);
        pending_snippet.push('\n');

        if continuation_mode == ReplContinuationMode::IncompleteBlock && snippet.is_empty() {
            repl_feed(&mut repl, &pending_snippet, &policy);
            pending_snippet.clear();
            continuation_mode = ReplContinuationMode::Complete;
            continue;
        }

        let detected = detect_repl_continuation_mode(&pending_snippet);
        match detected {
            ReplContinuationMode::Complete => {
                if continuation_mode == ReplContinuationMode::IncompleteBlock {
                    continue;
                }
                repl_feed(&mut repl, &pending_snippet, &policy);
                pending_snippet.clear();
                continuation_mode = ReplContinuationMode::Complete;
            }
            ReplContinuationMode::IncompleteBlock => {
                continuation_mode = ReplContinuationMode::IncompleteBlock;
            }
            ReplContinuationMode::IncompleteImplicit => {
                if continuation_mode != ReplContinuationMode::IncompleteBlock {
                    continuation_mode = ReplContinuationMode::IncompleteImplicit;
                }
            }
        }
    }
}

fn repl_feed(repl: &mut Option<FimodRepl>, snippet: &str, policy: &SandboxPolicy) {
    if let (Some(limit), Some(active_repl)) = (policy.max_duration, repl.as_mut()) {
        active_repl.tracker_mut().set_max_duration(limit);
    }

    let active_repl = repl.take().expect("REPL session must be available");
    match execute_repl_snippet(active_repl, snippet, policy) {
        Ok((returned_repl, output)) => {
            if output != MontyObject::None {
                println!("{output}");
            }
            *repl = Some(returned_repl);
        }
        Err(err) => {
            let (returned_repl, err) = *err;
            eprintln!("error:\n{err}");
            *repl = Some(returned_repl);
        }
    }
}

fn execute_repl_snippet(repl: FimodRepl, snippet: &str, policy: &SandboxPolicy) -> ReplRunResult {
    let mut progress = match repl.feed_start(snippet, vec![], monty::PrintWriter::Stdout) {
        Ok(progress) => progress,
        Err(err) => return Err(Box::new((err.repl, format_repl_error(err.error, policy)))),
    };

    loop {
        match progress {
            ReplProgress::Complete { repl, value } => return Ok((repl, value)),
            ReplProgress::OsCall(mut call) => {
                let function_call = call.take_function_call();
                let result = fimod::engine::sandbox_os_call_result(function_call, policy);
                progress = call
                    .resume(result, monty::PrintWriter::Stdout)
                    .map_err(|err| Box::new((err.repl, format_repl_error(err.error, policy))))?;
            }
            ReplProgress::NameLookup(lookup) => {
                progress = lookup
                    .resume(NameLookupResult::Undefined, monty::PrintWriter::Stdout)
                    .map_err(|err| Box::new((err.repl, format_repl_error(err.error, policy))))?;
            }
            ReplProgress::FunctionCall(call) => {
                return Err(Box::new((
                    call.into_repl(),
                    "external function calls are not supported in the REPL".to_string(),
                )));
            }
            ReplProgress::ResolveFutures(state) => {
                return Err(Box::new((
                    state.into_repl(),
                    "async futures are not supported in the REPL".to_string(),
                )));
            }
        }
    }
}

fn format_repl_error(err: monty::MontyException, policy: &SandboxPolicy) -> String {
    match err.exc_type() {
        ExcType::TimeoutError | ExcType::MemoryError => {
            fimod::engine::translate_monty_error(err, policy).to_string()
        }
        _ => err.to_string(),
    }
}
