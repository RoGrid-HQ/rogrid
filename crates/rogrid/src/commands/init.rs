pub mod help;
mod install;
mod prompt;

use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::builder::PossibleValuesParser;

use crate::process;
use crate::template;
use crate::tools::{self, Tool};

/// The flags and arguments of `rogrid init`.
#[derive(clap::Args)]
pub struct Args {
    /// Folder to create, or `.` for the current folder. Prompted for when omitted.
    #[arg(value_name = "FOLDER")]
    pub target: Option<String>,

    /// Project name. Defaults to the folder name; prompted for with `.`.
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,

    /// Set up the project in a folder that is not empty, overwriting files it writes.
    #[arg(long)]
    pub force: bool,

    /// Package manager to use. Prompted for when omitted.
    #[arg(long, value_name = "NAME", value_parser = PossibleValuesParser::new(tools::names(tools::PACKAGE_MANAGERS)))]
    pub package_manager: Option<String>,

    /// Tool manager to use. Prompted for when omitted.
    #[arg(long, value_name = "NAME", value_parser = PossibleValuesParser::new(tools::names(tools::TOOL_MANAGERS)))]
    pub tool_manager: Option<String>,
}

/// Each choice comes from its flag when given, otherwise from a prompt.
pub fn run(args: Args) -> Result<()> {
    let Args {
        target,
        name,
        force,
        package_manager,
        tool_manager,
    } = args;
    let cwd = env::current_dir().context("could not read the current directory")?;
    let (path, name) = resolve_target(&cwd, target, name, force)?;

    let package_manager = match package_manager {
        Some(name) => tools::find(tools::PACKAGE_MANAGERS, &name)?,
        None => prompt::select("Package manager:", tools::PACKAGE_MANAGERS)?,
    };
    let tool_manager = match tool_manager {
        Some(name) => tools::find(tools::TOOL_MANAGERS, &name)?,
        None => prompt::select("Tool manager:", tools::TOOL_MANAGERS)?,
    };

    fs::create_dir_all(&path).with_context(|| format!("could not create {}", path.display()))?;
    let vars = tools::vars(&name, &package_manager, &tool_manager);
    template::render(&template::DEFAULT, &path, &vars)?;

    if !tool_manager.manifest.is_empty() {
        let manifest = tools::tool_manifest(&package_manager, &tool_manager);
        fs::write(path.join(tool_manager.manifest), manifest)
            .with_context(|| format!("could not write {}", tool_manager.manifest))?;
    }

    // Tool manager first: it puts the package manager on the PATH.
    let to_install: [&dyn Tool; 2] = [&tool_manager, &package_manager];
    let path_first: Vec<PathBuf> = package_manager.bin_path().into_iter().collect();
    install::run_all(&to_install, &vars, &path, &path_first).with_context(|| {
        format!(
            "project files were created in {}, but installation is incomplete",
            path.display()
        )
    })?;

    println!(
        "\nCreated {} ({} + {})",
        path.display(),
        package_manager.name,
        tool_manager.name
    );
    println!("\nNext steps:");
    if path != cwd {
        let folder = path.strip_prefix(&cwd).unwrap_or(&path);
        println!("  cd {}", folder.display());
    }
    println!("  rojo serve");

    if !process::works("rojo --version", &path) {
        println!("\n{}", rojo_note(&package_manager, &tool_manager));
    }

    Ok(())
}

/// What to do when `rojo` does not run in the new project, which depends on
/// who was meant to install it.
fn rojo_note(package_manager: &tools::PackageManager, tool_manager: &tools::ToolManager) -> String {
    let headline = "Note: `rojo` does not run in this folder yet.";

    if tool_manager.name == package_manager.name {
        let bin = package_manager.bin_path().map_or_else(
            || package_manager.bin_dir.to_string(),
            |p| p.display().to_string(),
        );
        return format!(
            "{headline}\n{} installed it in {bin}, but that folder is missing from PATH or another `rojo` comes first.\nPut {bin} first on PATH, then open a new terminal.",
            package_manager.name
        );
    }

    if tool_manager.binary.is_none() {
        return format!(
            "{headline}\nWith no tool manager, install Rojo yourself: https://rojo.space/docs/v7/getting-started/installation/"
        );
    }

    format!(
        "{headline}\nCheck that {} is set up: {}",
        tool_manager.name, tool_manager.homepage
    )
}

/// Where the project goes and what it is called. A folder that is not empty
/// is refused as soon as it is known, before any prompt, unless `force` is set.
///
/// `.` means the current folder, with the name prompted for and the folder's
/// own name suggested. Otherwise the target is a new folder under the current
/// one, prompted for when omitted, and the name defaults to it.
fn resolve_target(
    cwd: &Path,
    target: Option<String>,
    name: Option<String>,
    force: bool,
) -> Result<(PathBuf, String)> {
    if target.as_deref().is_some_and(is_current_dir) {
        ensure_available(cwd, force)?;
        let suggested = cwd
            .file_name()
            .and_then(|folder| normalize_name(&folder.to_string_lossy()));
        let name = match name {
            Some(name) => name,
            None => prompt::text("Project name:", suggested.as_deref())?,
        };
        validate_name(&name)?;
        return Ok((cwd.to_path_buf(), name));
    }

    let folder = match (target, &name) {
        (Some(folder), _) => folder,
        (None, Some(name)) => name.clone(),
        (None, None) => prompt::text("Project name:", None)?,
    };
    let name = name.unwrap_or_else(|| folder.clone());
    validate_name(&folder)?;
    validate_name(&name)?;
    let path = cwd.join(folder);
    ensure_available(&path, force)?;
    Ok((path, name))
}

/// Whether the target is `.` in any spelling, such as `./`.
fn is_current_dir(target: &str) -> bool {
    let mut components = Path::new(target).components();
    matches!(
        (components.next(), components.next()),
        (Some(Component::CurDir), None)
    )
}

/// A folder name made valid as a project name: lowercased, with every run of
/// other characters turned into one dash. `None` when nothing valid is left.
fn normalize_name(folder: &str) -> Option<String> {
    let mut name = String::new();
    for c in folder.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' {
            name.push(c);
        } else if !name.is_empty() && !name.ends_with('-') {
            name.push('-');
        }
    }
    let name = name.trim_end_matches('-');
    (!name.is_empty()).then(|| name.to_string())
}

/// Lowercase letters, digits, dashes and underscores only.
/// Keeps the folder name usable as a package name later.
fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("project name cannot be empty");
    }

    let valid = name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');

    if !valid {
        bail!("project name may only contain lowercase letters, digits, '-' and '_'");
    }

    Ok(())
}

/// Refuses a folder that already has something in it, unless `force` is set.
fn ensure_available(path: &Path, force: bool) -> Result<()> {
    if force || !path.exists() {
        return Ok(());
    }

    let mut entries =
        fs::read_dir(path).with_context(|| format!("could not read {}", path.display()))?;
    if entries.next().is_some() {
        bail!(
            "{} is not empty; pass --force to set up the project in it anyway",
            path.display()
        );
    }

    Ok(())
}
