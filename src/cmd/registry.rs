use anyhow::Result;

use fimod::registry;

use crate::cli::{CacheAction, RegistryAction};

pub fn dispatch(action: RegistryAction) -> Result<()> {
    match action {
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
            crate::cmd::setup::registry_defaults(crate::cmd::setup::SetupOptions::new(
                yes, false, false,
            ))
        }
        RegistryAction::Cache { action } => match action {
            CacheAction::Clear { name } => registry::cache_clear(name.as_deref()),
            CacheAction::Info => registry::cache_info(),
        },
    }
}
