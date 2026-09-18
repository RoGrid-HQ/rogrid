use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::process;
use crate::template::{self, Vars};
use crate::tools::Tool;

/// Runs every tool's install commands in `dir`, in the given order.
/// Stops at the first failure and reports the commands still needed to finish.
/// Folders in `path_first` are searched before the rest of PATH.
pub fn run_all(tools: &[&dyn Tool], vars: &Vars, dir: &Path, path_first: &[PathBuf]) -> Result<()> {
    let steps: Vec<_> = tools
        .iter()
        .flat_map(|tool| {
            tool.install()
                .iter()
                .map(move |command| (*tool, template::fill(command, vars)))
        })
        .collect();

    for (index, (tool, command)) in steps.iter().enumerate() {
        println!("\n> {command}");
        let result = process::run(command, dir, path_first);
        let missing = result.as_ref().err().is_some_and(|error| {
            error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound)
        });
        result.with_context(|| {
            let advice = if missing {
                format!(
                    "{} could not be found. Install it from {} and make it available on PATH.",
                    tool.name(),
                    tool.homepage()
                )
            } else {
                "Fix the error below.".to_string()
            };
            let remaining = steps[index..]
                .iter()
                .map(|(_, command)| format!("  {command}"))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "installation stopped at `{command}`. {advice}\nRun these commands in {} to finish:\n{remaining}",
                dir.display()
            )
        })?;
    }

    Ok(())
}
