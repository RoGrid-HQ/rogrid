use crate::tools::{self, Step};
use anyhow::{bail, Context, Result};
use include_dir::{include_dir, Dir};
use std::path::Path;

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
    let git = tools::run("git", &["init", "-q"], dest);

    // Installs the tools pinned in rokit.toml (Rojo and Wally).
    // --no-trust-check skips Rokit's interactive "do you trust this author?"
    // prompt, which would otherwise block a scripted install.
    let rokit = tools::run("rokit", &["install", "--no-trust-check"], dest);

    // Installs the dependencies pinned in wally.toml.
    // Wally itself comes from rokit, so don't bother if that step failed.
    let wally = match rokit {
        Step::Ok => tools::run("wally", &["install"], dest),
        _ => Step::Skipped,
    };

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
"#
    );

    println!("Setup:");
    git.report("git init");
    rokit.report("rokit install");
    wally.report("wally install");

    if rokit.is_ok() && wally.is_ok() {
        println!("\nNext steps:\n  cd {}\n  rogrid dev\n\nEnjoy RoGrid!\n", args.name);
    } else {
        println!(
            "\nSome setup steps did not complete. Inside {}, run the failed commands above, then:\n  rogrid dev\n",
            args.name
        );
    }

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
            // {{rogrid_version}} becomes the CLI's own version (from Cargo.toml),
            // so a new project always depends on the matching rogrid-hq/core release.
            Some(text) => std::fs::write(
                &out,
                text.replace("{{project_name}}", name)
                    .replace("{{rogrid_version}}", env!("CARGO_PKG_VERSION")),
            ),
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
