#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::process;

use fimod::pipeline::CliResult;
use fimod::{registry, test_runner};

use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::CompleteEnv;

mod cli;
mod cmd;
#[cfg(feature = "watch")]
mod watch;

use cli::{Cli, Commands, MoldAction, MontyAction, SetupCategory, SetupDefaults};

fn main() -> Result<()> {
    CompleteEnv::with_factory(Cli::command).complete();

    let cli = Cli::parse();
    let result = dispatch(cli);

    match result {
        Ok(CliResult::Done) => Ok(()),
        Ok(CliResult::Exit(code)) => process::exit(code),
        Err(e) => {
            if let Some(sandbox_err) = e.downcast_ref::<fimod::engine::SandboxLimitExceeded>() {
                eprintln!("{sandbox_err}");
                process::exit(fimod::engine::SANDBOX_EXPLODED_EXIT_CODE);
            }
            Err(e)
        }
    }
}

fn dispatch(cli: Cli) -> Result<CliResult> {
    match cli.command {
        Some(Commands::Shape(shape)) => cmd::shape::run_shape(*shape),
        // `mold test` can request exit code 1 on failure; handle separately.
        Some(Commands::Mold {
            action: MoldAction::Test { mold, tests_dir },
        }) => test_runner::run(&mold, &tests_dir),
        Some(other) => dispatch_other(other).map(|()| CliResult::Done),
        None => {
            Cli::command().print_help()?;
            Ok(CliResult::Exit(2))
        }
    }
}

/// Dispatch all non-`Shape` subcommands (which never request a process exit
/// with a custom code, only success or anyhow error).
fn dispatch_other(cmd: Commands) -> Result<()> {
    match cmd {
        Commands::Shape(_) => unreachable!("handled by dispatch()"),
        Commands::Registry { action } => cmd::registry::dispatch(action),
        Commands::Mold { action } => cmd::mold::dispatch(action),
        Commands::Monty { action } => match action {
            MontyAction::Repl => cmd::monty::run_monty_repl(),
        },
        Commands::Setup { category } => match category {
            SetupCategory::Registry {
                action: SetupDefaults::Defaults { yes, force },
            } => cmd::setup::registry_defaults(yes, force),
            SetupCategory::Sandbox {
                action: SetupDefaults::Defaults { yes, force },
            } => cmd::setup::sandbox_defaults(yes, force),
            SetupCategory::All {
                action: SetupDefaults::Defaults { yes, force },
            } => cmd::setup::all_defaults(yes, force),
            SetupCategory::Completions { shell } => {
                cmd::completions::print_completion_script(shell)
            }
        },
    }
}
