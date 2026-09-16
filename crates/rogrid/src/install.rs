use std::path::Path;

use crate::binaries;
use crate::process;
use crate::template::{self, Vars};
use crate::tools::Tool;

/// An install step that did not complete, with what the user can do about it.
pub struct Failure {
    pub command: String,
    pub fix: String,
}

/// Runs every tool's install commands in `dir`, in the given order.
/// Keeps going after a failure so every problem is reported together.
pub fn run_all(tools: &[&dyn Tool], vars: &Vars, dir: &Path) -> Vec<Failure> {
    let mut failures = Vec::new();
    let folder = dir.file_name().map_or_else(
        || dir.display().to_string(),
        |f| f.to_string_lossy().into_owned(),
    );

    for tool in tools {
        let commands: Vec<String> = tool
            .install()
            .iter()
            .map(|command| template::fill(command, vars))
            .collect();

        if commands.is_empty() {
            continue;
        }

        if !tool.binary().is_some_and(binaries::is_installed) {
            failures.push(Failure {
                command: commands.join(" && "),
                fix: format!(
                    "{} is not installed. Install it from {} then run the command inside {folder}",
                    tool.name(),
                    tool.homepage()
                ),
            });
            continue;
        }

        for command in commands {
            println!("\n> {command}");
            if let Err(error) = process::run(&command, dir) {
                failures.push(Failure {
                    fix: format!(
                        "{error:#}. Fix the error above, then run `{command}` inside {folder}"
                    ),
                    command,
                });
            }
        }
    }

    failures
}
