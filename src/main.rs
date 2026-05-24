#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::process;

use fimod::pipeline::CliResult;
use fimod::{registry, test_runner};

use anyhow::Result;
use clap::{ArgMatches, CommandFactory, FromArgMatches};
use clap_complete::CompleteEnv;

mod cli;
mod cmd;
#[cfg(feature = "watch")]
mod watch;

use cli::{Cli, Commands, MoldAction, MontyAction, SetupCategory, SetupDefaults};
use cmd::setup::SetupOptions;

fn main() -> Result<()> {
    CompleteEnv::with_factory(Cli::command).complete();

    let matches = Cli::command().get_matches();
    let cli = Cli::from_arg_matches(&matches)?;
    let result = dispatch(cli, &matches);

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

fn dispatch(cli: Cli, matches: &ArgMatches) -> Result<CliResult> {
    match cli.command {
        Some(Commands::Shape(shape)) => {
            let script_refs = cmd::shape::script_refs_from_matches(matches, &shape);
            cmd::shape::run_shape(*shape, script_refs)
        }
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
                action:
                    SetupDefaults::Defaults {
                        yes,
                        force,
                        if_needed,
                    },
            } => cmd::setup::registry_defaults(SetupOptions::new(yes, force, if_needed)),
            SetupCategory::Sandbox {
                action:
                    SetupDefaults::Defaults {
                        yes,
                        force,
                        if_needed,
                    },
            } => cmd::setup::sandbox_defaults(SetupOptions::new(yes, force, if_needed)),
            SetupCategory::All {
                action:
                    SetupDefaults::Defaults {
                        yes,
                        force,
                        if_needed,
                    },
            } => cmd::setup::all_defaults(SetupOptions::new(yes, force, if_needed)),
            SetupCategory::Completions { shell } => {
                cmd::completions::print_completion_script(shell)
            }
        },
    }
}
