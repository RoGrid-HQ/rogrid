//! Runs the external tools RoGrid drives (git, rokit, wally, rojo) with one
//! consistent way of handling "not installed" and "exited with an error".

use anyhow::{anyhow, Result};
use std::path::Path;
use std::process::{Command, ExitStatus};

/// Outcome of one external tool invocation, for callers that want to keep
/// going and report everything at the end (like `rogrid new`).
pub enum Step {
    Ok,
    Failed,   // ran, but exited non-zero
    NotFound(String), // the tool isn't installed / not on PATH; carries its name for the hint
    Skipped,  // not attempted because an earlier step it depends on failed
}

impl Step {
    pub fn is_ok(&self) -> bool {
        matches!(self, Step::Ok)
    }

    /// Prints one line of a setup report, e.g. `  [ok] rokit install`.
    /// A missing tool gets the same install hint `status` puts in its error.
    pub fn report(&self, label: &str) {
        let (mark, note) = match self {
            Step::Ok => ("[ok]", String::new()),
            Step::Failed => ("[!!]", " (exited with an error, see output above)".to_string()),
            Step::NotFound(tool) => ("[!!]", format!(" ({tool} not found — {})", install_hint(tool))),
            Step::Skipped => ("[--]", " (skipped, depends on a failed step)".to_string()),
        };
        println!("  {mark} {label}{note}");
    }
}

/// Runs `tool` with `args` inside `cwd`, streaming its output to the terminal.
/// Never fails: a missing or failing tool becomes a `Step` for the caller to report.
pub fn run(tool: &str, args: &[&str], cwd: &Path) -> Step {
    match Command::new(tool).args(args).current_dir(cwd).status() {
        Ok(s) if s.success() => Step::Ok,
        Ok(_) => Step::Failed,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Step::NotFound(tool.to_string()),
        Err(_) => Step::Failed,
    }
}

/// Runs `tool` with `args` in the current directory and returns its exit status.
/// A tool that can't be started is an error with a hint on how to install it.
/// The caller decides what a non-zero exit means.
pub fn status(tool: &str, args: &[&str]) -> Result<ExitStatus> {
    Command::new(tool).args(args).status().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => anyhow!("{tool} not found — {}", install_hint(tool)),
        _ => anyhow!("failed to start {tool}: {e}"),
    })
}

/// How to get a tool that isn't on PATH.
fn install_hint(tool: &str) -> &'static str {
    match tool {
        "rojo" | "wally" => "run `rokit install` in this folder",
        "rokit" => "install Rokit from https://github.com/rojo-rbx/rokit",
        _ => "install it and make sure it's on your PATH",
    }
}
