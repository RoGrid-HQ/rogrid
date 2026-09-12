use anyhow::{bail, Context, Result};
use include_dir::{include_dir, Dir};
use std::path::Path;
use std::process::Command;

// Bakes the whole templates/default folder into the binary at compile time.
// $CARGO_MANIFEST_DIR = crates/rogrid, so ../../ is the repo root.
static TEMPLATE: Dir = include_dir!("$CARGO_MANIFEST_DIR/../../templates/default");

/// Create a new RoGrid project
#[derive(clap::Args)]
pub struct Args {
    /// Project name (also used as the folder name)
    pub name: String,
}

pub fn run(args: Args) -> Result<()> {
    let dest = Path::new(&args.name);
    if dest.exists() {
        bail!("'{}' already exists", args.name);
    }

    // Extract the project name from the last path component and validate it.
    let name = dest
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .ok_or_else(|| anyhow::anyhow!("project name must be only letters, numbers, '-' or '_'"))?
        .to_string();

    // Copy every embedded file to disk, swapping {{project_name}} for the real name.
    write_dir(&TEMPLATE, dest, &name)?;

    // -q = quiet. Suppresses git's "Initialized empty repository in ..." line
    // so our own output stays clean.
    run_tool(dest, "git", &["init", "-q"]);

    // Installs the tools pinned in rokit.toml (just Rojo for now).
    // --no-trust-check skips Rokit's interactive "do you trust this author?"
    // prompt, which would otherwise block a scripted install.
    run_tool(dest, "rokit", &["install", "--no-trust-check"]);

    println!(
        r#"
╔═════════════════════════════════════════════════════╗
║    ____     ___     ____   ____     ___    ____     ║
║   |  _ \   / _ \   / ___| |  _ \   |_ _|  |  _ \    ║
║   | |_) | | | | | | |  _  | |_) |   | |   | | | |   ║
║   |  _ <  | |_| | | |_| | |  _ <    | |   | |_| |   ║
║   |_| \_\  \___/   \____| |_| \_\  |___|  |____/    ║
║                                                     ║
║              RoGrid project created!                ║
╚═════════════════════════════════════════════════════╝

Next steps:
  cd {folder}
  rogrid dev

Enjoy RoGrid!
"#,
        folder = args.name
    );

    Ok(())
}

/// Recursively writes an embedded directory to `dest`.
fn write_dir(dir: &Dir, dest: &Path, name: &str) -> Result<()> {
    for file in dir.files() {
        // file.path() is already relative to the template root, e.g. "src/server/runtime.server.luau"
        let out = dest.join(file.path());
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        match file.contents_utf8() {
            // Text file: do the placeholder replacement.
            Some(text) => std::fs::write(&out, text.replace("{{project_name}}", name)),
            // Binary file (e.g. a future .rbxm map): write bytes untouched.
            None => std::fs::write(&out, file.contents()),
        }
        .with_context(|| format!("writing {}", out.display()))?;
    }
    for sub in dir.dirs() {
        write_dir(sub, dest, name)?;
    }
    Ok(())
}

/// Runs an external command inside `cwd`. Never fails the whole `new`:
/// a missing git or rokit is a warning, the project files are already on disk.
fn run_tool(cwd: &Path, tool: &str, args: &[&str]) {
    match Command::new(tool).args(args).current_dir(cwd).status() {
        Ok(s) if s.success() => {}
        Ok(_) => eprintln!("warning: `{tool}` exited with an error"),
        Err(_) => eprintln!("warning: `{tool}` not found, skipping"),
    }
}