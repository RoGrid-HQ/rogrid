use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::{Command, exit};

/// Start the Rojo server for the current project
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_args: Args) -> Result<()> {
    if !Path::new("default.project.json").exists() {
        bail!("not a RoGrid project (no default.project.json here)");
    }

    ensure_packages()?;

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

/// Makes sure `Packages/` exists before Rojo starts. The runtime scripts
/// require `ReplicatedStorage.Packages.RoGrid`, so without it the game errors
/// the moment it runs in Studio. Missing usually means a fresh clone or a
/// `rogrid new` where `wally install` failed, so just run it now.
fn ensure_packages() -> Result<()> {
    if Path::new("Packages").is_dir() {
        return Ok(());
    }
    if !Path::new("wally.toml").exists() {
        // Project doesn't use Wally at all; nothing to install.
        return Ok(());
    }

    println!("Packages/ not found, running `wally install`...");
    let status = Command::new("wally")
        .arg("install")
        .status()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => anyhow::anyhow!("wally not found — run `rokit install` in this folder"),
            _ => anyhow::anyhow!("failed to start wally: {e}"),
        })?;

    if !status.success() {
        bail!("`wally install` failed (see output above)");
    }

    // Wally creates Packages/ only if wally.toml lists at least one dependency.
    Path::new("Packages")
        .is_dir()
        .then_some(())
        .context("`wally install` finished but Packages/ still doesn't exist; check the [dependencies] section of wally.toml")
}