use std::io;
use std::path::Path;

use anyhow::{bail, Context, Result};
use clap_complete::env::Shells;

use crate::cli::CompletionShell;

pub fn print_completion_script(shell: Option<CompletionShell>) -> Result<()> {
    let shell = shell.map_or_else(detect_shell, Ok)?;
    let shell_name = match shell {
        CompletionShell::Bash => "bash",
        CompletionShell::Zsh => "zsh",
        CompletionShell::Fish => "fish",
        CompletionShell::Elvish => "elvish",
        CompletionShell::Powershell => "powershell",
    };
    let shells = Shells::builtins();
    let completer = shells
        .completer(shell_name)
        .with_context(|| format!("unsupported shell: {shell_name}"))?;
    completer
        .write_registration("COMPLETE", "fimod", "fimod", "fimod", &mut io::stdout())
        .context("failed to write completion script")?;
    Ok(())
}

fn detect_shell() -> Result<CompletionShell> {
    let shell_path = std::env::var("SHELL").unwrap_or_default();
    let basename = Path::new(&shell_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    match basename {
        "bash" => Ok(CompletionShell::Bash),
        "zsh" => Ok(CompletionShell::Zsh),
        "fish" => Ok(CompletionShell::Fish),
        "elvish" => Ok(CompletionShell::Elvish),
        "pwsh" | "powershell" => Ok(CompletionShell::Powershell),
        "" => bail!(
            "could not detect shell: $SHELL is empty — pass --shell <SHELL> (bash|zsh|fish|elvish|powershell)"
        ),
        other => bail!(
            "unrecognized shell `{other}` from $SHELL — pass --shell <SHELL> (bash|zsh|fish|elvish|powershell)"
        ),
    }
}
