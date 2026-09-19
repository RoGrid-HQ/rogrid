#[path = "../../config.rs"]
mod config;
mod packages;
mod versions;

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Task,
}

#[derive(Subcommand)]
enum Task {
    /// Maintainer commands. Only `publish` uploads packages.
    Release {
        #[command(subcommand)]
        command: Release,
    },
}

#[derive(Subcommand)]
enum Release {
    /// Update versions and write editable release notes. Does not commit or publish.
    Prepare {
        #[arg(long, value_enum)]
        cli: versions::Bump,
        #[arg(long, value_enum, default_value = "keep")]
        runtime: versions::Bump,
        /// Previous published CLI tag; defaults to the current CLI version's tag.
        #[arg(long)]
        since: Option<String>,
    },
    /// Check version consistency, optionally against the previous release.
    Check {
        #[arg(long)]
        base: Option<String>,
    },
    /// Print the CLI/runtime versions as JSON for workflows.
    Info,
    /// Build both registry archives without uploading them.
    Package {
        #[arg(long, default_value = "target/release-packages")]
        output: PathBuf,
    },
    /// Publish missing packages, verifying existing and uploaded contents.
    Publish {
        #[arg(long)]
        artifacts: PathBuf,
    },
    /// Verify registry contents without publishing anything.
    Verify {
        #[arg(long)]
        artifacts: PathBuf,
    },
    /// Install both registry packages with the candidate CLI and build starter places.
    Smoke {
        #[arg(long)]
        cli: PathBuf,
    },
}

fn main() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("xtask must live in the repository")?;
    std::env::set_current_dir(root)?;
    let Task::Release { command } = Args::parse().command;
    match command {
        Release::Prepare {
            cli,
            runtime,
            since,
        } => versions::prepare(root, cli, runtime, since),
        Release::Check { base } => versions::check(root, base.as_deref()).map(|_| ()),
        Release::Info => {
            let versions = versions::check(root, None)?;
            println!(
                "{}",
                serde_json::json!({
                    "cli": versions.cli.to_string(),
                    "runtime": versions.runtime.to_string(),
                    "tag": format!("v{}", versions.cli),
                })
            );
            Ok(())
        }
        Release::Package { output } => packages::bundle(root, &output),
        Release::Publish { artifacts } => packages::publish(root, &artifacts),
        Release::Verify { artifacts } => packages::verify(root, &artifacts),
        Release::Smoke { cli } => packages::smoke(&cli),
    }
}

/// Never format command arguments into errors: auth commands contain secrets.
fn run(command: &mut Command) -> Result<()> {
    let program = command.get_program().to_string_lossy().into_owned();
    let status = command
        .status()
        .with_context(|| format!("could not run {program}"))?;
    ensure!(status.success(), "{program} failed ({status})");
    Ok(())
}

fn output(command: &mut Command) -> Result<String> {
    let program = command.get_program().to_string_lossy().into_owned();
    let result = command
        .output()
        .with_context(|| format!("could not run {program}"))?;
    ensure!(
        result.status.success(),
        "{program} failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).context("command output was not UTF-8")
}
