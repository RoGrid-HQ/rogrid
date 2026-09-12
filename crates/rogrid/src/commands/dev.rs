use anyhow::{bail, Result};
use std::path::Path;
use std::process::{Command, exit};

/// Start the Rojo server for the current project
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_args: Args) -> Result<()> {
    if !Path::new("default.project.json").exists() {
        bail!("not a RoGrid project (no default.project.json here)");
    }

    // Inherit stdio so Rojo's output streams straight to the terminal.
    // TODO: Pipe stdout (Rojo only writes its start banner there), keep stderr
    // inherited (all errors/warnings go through env_logger to stderr), pass a
    // fixed --port, and poll GET /api/rojo until it responds. Then print our
    // own "ready" message with the project name. If the process exits before
    // the API answers, report "Rojo failed to start". Don't parse Rojo's text
    // output. It has no stability guarantee; exit code + HTTP API do.
    let status = Command::new("rojo")
        .arg("serve")
        .status()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => anyhow::anyhow!("rojo not found — run `rokit install` in this folder"),
            _ => anyhow::anyhow!("failed to start rojo: {e}"),
        })?;

    // Exit with the same exit code as Rojo, or 1 if unavailable
    exit(status.code().unwrap_or(1));
}