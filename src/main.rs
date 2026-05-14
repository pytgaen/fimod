#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::process;

use fimod::pipeline::CliResult;
use fimod::{registry, setup, test_runner};

use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::CompleteEnv;

mod cli;
mod cmd;
#[cfg(feature = "watch")]
mod watch;

use cli::{
    CacheAction, Cli, Commands, MoldAction, MontyAction, RegistryAction, SetupCategory,
    SetupDefaults,
};

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
        Commands::Registry { action } => match action {
            RegistryAction::List { output_format } => registry::list(&output_format),
            RegistryAction::Add {
                name,
                location,
                token_env,
            } => registry::add(&name, &location, token_env.as_deref()),
            RegistryAction::Show { name } => registry::show(&name),
            RegistryAction::Remove { name } => registry::remove(&name),
            RegistryAction::SetPriority {
                name,
                rank,
                clear,
                cascade,
            } => registry::set_priority(&name, rank, clear, cascade),
            RegistryAction::BuildCatalog { path, registry } => {
                registry::build_catalog(registry.as_deref(), path.as_deref())
            }
            RegistryAction::Setup { yes } => {
                eprintln!("warning: `fimod registry setup` is deprecated. Use `fimod setup registry defaults`. Will be removed in 0.10.0.");
                setup::registry_defaults(yes, false)
            }
            RegistryAction::Cache { action } => match action {
                CacheAction::Clear { name } => registry::cache_clear(name.as_deref()),
                CacheAction::Info => registry::cache_info(),
            },
        },
        Commands::Mold { action } => match action {
            MoldAction::List {
                registry,
                output_format,
            } => registry::list_molds(registry.as_deref(), output_format),
            MoldAction::Show {
                name,
                path,
                registry,
                output_format,
            } => match path {
                Some(p) => registry::show_mold_by_path(&p, name.as_deref(), output_format),
                None => registry::show_mold(
                    &name.expect("clap: --name is required when --path is absent"),
                    registry.as_deref(),
                    output_format,
                ),
            },
            MoldAction::Test { .. } => unreachable!("handled by dispatch()"),
        },
        Commands::Monty { action } => match action {
            MontyAction::Repl => cmd::monty::run_monty_repl(),
        },
        Commands::Setup { category } => match category {
            SetupCategory::Registry {
                action: SetupDefaults::Defaults { yes, force },
            } => setup::registry_defaults(yes, force),
            SetupCategory::Sandbox {
                action: SetupDefaults::Defaults { yes, force },
            } => setup::sandbox_defaults(yes, force),
            SetupCategory::All {
                action: SetupDefaults::Defaults { yes, force },
            } => setup::all_defaults(yes, force),
            SetupCategory::Completions { shell } => {
                cmd::completions::print_completion_script(shell)
            }
        },
    }
}
