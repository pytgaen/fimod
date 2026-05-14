use anyhow::Result;

use fimod::registry;

use crate::cli::MoldAction;

pub fn dispatch(action: MoldAction) -> Result<()> {
    match action {
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
        MoldAction::Test { .. } => unreachable!("handled by main dispatch"),
    }
}
