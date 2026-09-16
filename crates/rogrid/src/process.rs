use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

/// Runs a whitespace-separated command in `dir` with the terminal attached,
/// so prompts and progress bars work. Fails if it cannot start or exits non-zero.
pub fn run(command: &str, dir: &Path) -> Result<()> {
    let mut parts = command.split_whitespace();
    let program = parts.next().context("empty command")?;

    let status = Command::new(program)
        .args(parts)
        .current_dir(dir)
        .status()
        .with_context(|| format!("could not start {program}"))?;

    if !status.success() {
        bail!("{command} {status}");
    }

    Ok(())
}
