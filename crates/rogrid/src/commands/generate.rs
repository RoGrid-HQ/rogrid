use anyhow::{bail, Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Generate a service, controller, or shared module
#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    kind: Kind,
}

#[derive(clap::Subcommand)]
enum Kind {
    /// Create a server service with Init and Start methods
    Service(Name),
    /// Create a client controller with Init and Start methods
    Controller(Name),
    /// Create a shared module
    Module(Name),
}

#[derive(clap::Args)]
struct Name {
    /// Luau identifier, e.g. Inventory (nested paths are not supported)
    name: String,
}

pub fn run(args: Args) -> Result<()> {
    // Templates are compiled into the executable; no template files need to be installed.
    let (input, folder, suffix, template) = match args.kind {
        Kind::Service(args) => (
            args.name,
            "src/server/services",
            "Service",
            include_str!("../../../../templates/generators/service.luau"),
        ),
        Kind::Controller(args) => (
            args.name,
            "src/client/controllers",
            "Controller",
            include_str!("../../../../templates/generators/controller.luau"),
        ),
        Kind::Module(args) => (
            args.name,
            "src/shared",
            "",
            include_str!("../../../../templates/generators/module.luau"),
        ),
    };

    validate_name(&input)?;
    let name = if input.ends_with(suffix) {
        input
    } else {
        format!("{input}{suffix}")
    };
    let root = project_root(&std::env::current_dir().context("reading current directory")?)?;
    let relative = Path::new(folder).join(format!("{name}.luau"));
    let dest = root.join(&relative);

    fs::create_dir_all(root.join(folder)).with_context(|| format!("creating {folder}"))?;
    // create_new refuses existing files, even if another process creates one concurrently.
    let mut file = match OpenOptions::new().write(true).create_new(true).open(&dest) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            bail!(
                "'{}' already exists; choose a different name",
                dest.display()
            );
        }
        Err(error) => return Err(error).with_context(|| format!("creating {}", dest.display())),
    };
    file.write_all(template.replace("{{name}}", &name).as_bytes())
        .with_context(|| format!("writing {}", dest.display()))?;

    println!("Created {}", relative.display());
    Ok(())
}

fn project_root(start: &Path) -> Result<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join("default.project.json").is_file())
        .map(Path::to_path_buf)
        .context(
            "not inside a RoGrid project (no default.project.json in this folder or its parents)",
        )
}

fn validate_name(name: &str) -> Result<()> {
    let mut chars = name.chars();
    let valid_start = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    if !valid_start || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        bail!("name must start with a letter or '_' and contain only ASCII letters, numbers, or '_' (no paths or file extensions)");
    }
    if matches!(
        name,
        "and"
            | "break"
            | "do"
            | "else"
            | "elseif"
            | "end"
            | "false"
            | "for"
            | "function"
            | "if"
            | "in"
            | "local"
            | "nil"
            | "not"
            | "or"
            | "repeat"
            | "return"
            | "then"
            | "true"
            | "until"
            | "while"
            | "continue"
    ) {
        bail!("'{name}' is a Luau keyword; choose a different name");
    }
    // Windows reserves these filenames even when they have an extension.
    let upper = name.to_ascii_uppercase();
    if matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            upper
                .strip_prefix(prefix)
                .is_some_and(|tail| tail.len() == 1 && matches!(tail.as_bytes()[0], b'1'..=b'9'))
        })
    {
        bail!("'{name}' is a reserved filename; choose a different name");
    }
    Ok(())
}
