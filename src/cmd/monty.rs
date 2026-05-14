use anyhow::Result;
use monty::MontyObject;

use fimod::MONTY_VERSION;

pub fn run_monty_repl() -> Result<()> {
    use monty::{detect_repl_continuation_mode, MontyRepl, NoLimitTracker, ReplContinuationMode};
    use rustyline::error::ReadlineError;
    use rustyline::DefaultEditor;

    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stdin());

    if is_tty {
        eprintln!(
            "Monty REPL v{MONTY_VERSION} — fimod v{} (exit or Ctrl+D to quit)",
            env!("CARGO_PKG_VERSION")
        );
    }

    let mut rl = DefaultEditor::new()?;
    let mut repl = MontyRepl::new("repl.py", NoLimitTracker);
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
            repl_feed(&mut repl, &pending_snippet);
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
                repl_feed(&mut repl, &pending_snippet);
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

fn repl_feed(repl: &mut monty::MontyRepl<monty::NoLimitTracker>, snippet: &str) {
    match repl.feed_run(snippet, vec![], monty::PrintWriter::Stdout) {
        Ok(output) => {
            if output != MontyObject::None {
                println!("{output}");
            }
        }
        Err(err) => eprintln!("error:\n{err}"),
    }
}
