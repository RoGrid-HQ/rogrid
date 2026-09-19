use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::process;
use crate::tools::InstallStep;

/// Runs every tool's install commands in `dir`, in the given order.
/// Stops at the first failure and reports the commands still needed to finish.
/// Folders in `path_first` are searched before the rest of PATH.
pub fn run_all(steps: &[InstallStep], dir: &Path, path_first: &[PathBuf]) -> Result<()> {
    for (index, step) in steps.iter().enumerate() {
        let command = &step.command;
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
                    step.tool,
                    step.homepage
                )
            } else {
                "Fix the error below.".to_string()
            };
            let remaining = steps[index..]
                .iter()
                .map(|step| format!("  {}", step.command))
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
