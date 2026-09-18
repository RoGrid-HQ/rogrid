use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// Runs a whitespace-separated command in `dir` with the terminal attached,
/// so prompts and progress bars work. Fails if it cannot start or exits non-zero.
///
/// Folders in `path_first` are searched before the rest of PATH, for the
/// command itself and for anything it starts.
pub fn run(command: &str, dir: &Path, path_first: &[PathBuf]) -> Result<()> {
    let mut parts = command.split_whitespace();
    let program = parts.next().context("empty command")?;

    let mut process = Command::new(program);
    process.args(parts).current_dir(dir);
    if let Some(path) = search_path(path_first) {
        process.env("PATH", path);
    }

    let status = process
        .status()
        .with_context(|| format!("could not start {program}"))?;

    if !status.success() {
        bail!("{command} {status}");
    }

    Ok(())
}

/// Whether a command runs and exits successfully in `dir`, with its output hidden.
pub fn works(command: &str, dir: &Path) -> bool {
    let mut parts = command.split_whitespace();
    let Some(program) = parts.next() else {
        return false;
    };

    Command::new(program)
        .args(parts)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// PATH with `first` ahead of everything already on it. `None` leaves PATH alone.
pub fn search_path(first: &[PathBuf]) -> Option<OsString> {
    if first.is_empty() {
        return None;
    }
    let current = env::var_os("PATH").unwrap_or_default();
    let dirs = first.iter().cloned().chain(env::split_paths(&current));
    env::join_paths(dirs).ok()
}

/// Whether a likely executable can be found on PATH, for display only.
/// Actually running the command is the authoritative availability check.
pub fn is_installed(name: &str) -> bool {
    let Some(path) = env::var_os("PATH") else {
        return false;
    };

    env::split_paths(&path).any(|dir| {
        let file = dir.join(name);
        #[cfg(windows)]
        let file = file.with_extension("exe");

        let Ok(metadata) = file.metadata() else {
            return false;
        };
        if !metadata.is_file() {
            return false;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            metadata.permissions().mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            true
        }
    })
}
