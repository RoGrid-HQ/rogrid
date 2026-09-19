pub mod help;
mod install;
mod local_framework;
mod prompt;

use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::builder::PossibleValuesParser;

use crate::template;
use crate::tools::{self, RojoFrom};
use crate::{codegen, process};

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

    /// Tool manager to use. `none` skips tool-manager setup; package dependencies still install.
    #[arg(long, value_name = "NAME", value_parser = PossibleValuesParser::new(tools::names(tools::TOOL_MANAGERS)))]
    pub tool_manager: Option<String>,

    /// Use a local RoGrid package folder instead of downloading the framework or CLI.
    #[arg(long, value_name = "PATH")]
    pub local_framework: Option<PathBuf>,
}

/// Each choice comes from its flag when given, otherwise from a prompt.
pub fn run(args: Args) -> Result<()> {
    let Args {
        target,
        name,
        force,
        package_manager,
        tool_manager,
        local_framework,
    } = args;
    let local_source = local_framework
        .as_deref()
        .map(local_framework::resolve_source)
        .transpose()?;
    let cwd = env::current_dir().context("could not read the current directory")?;
    let (path, name) = resolve_target(&cwd, target, name, force)?;

    let package_manager = match package_manager {
        Some(name) => tools::find(tools::PACKAGE_MANAGERS, &name)?,
        None => prompt::select("Package manager:", tools::PACKAGE_MANAGERS)?,
    };
    let tool_manager = match tool_manager {
        Some(name) => tools::find(tools::TOOL_MANAGERS, &name)?,
        None => {
            let supported: Vec<_> = tools::TOOL_MANAGERS
                .iter()
                .copied()
                .filter(|tool| tool.supports(&package_manager))
                .collect();
            prompt::select("Tool manager:", &supported)?
        }
    };

    let install_rogrid = local_source.is_none();
    let setup = tools::setup(&name, &package_manager, &tool_manager, install_rogrid)?;
    // The target may not exist yet. Check externally supplied tools from the caller's folder.
    preflight(&cwd, &package_manager, &tool_manager, &setup)?;
    fs::create_dir_all(&path).with_context(|| format!("could not create {}", path.display()))?;
    template::render(&template::DEFAULT, &path, &setup.vars)?;
    for (file, contents) in &setup.files {
        fs::write(path.join(file), contents).with_context(|| format!("could not write {file}"))?;
    }
    if let Some(source) = &local_source {
        local_framework::configure(&path, source)?;
    }
    codegen::prepare(&path)?;
    codegen::invalidate(&path)?;

    // Tool manager first: it puts the package manager on the PATH.
    install::run_all(&setup.steps, &path, &setup.path_first).with_context(|| {
        format!(
            "project files were created in {}, but installation is incomplete",
            path.display()
        )
    })?;

    codegen::generate(&path, setup.rojo_path()).with_context(|| {
        format!("project files were created in {}, but code generation is incomplete; fix the error and run `rogrid dev --once` there", path.display())
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
    if let Some(source) = &local_source {
        println!("  Run `dev` using the same local CLI you used for `init`.");
        println!("\nFramework source: {}", source.display());
        println!("Rojo uses this folder directly. Restart Studio Play after framework edits.");
    } else {
        println!("  rogrid dev");
    }

    if !process::works("rojo --version", &path) {
        println!("\n{}", rojo_note(&package_manager, &tool_manager, &setup));
    }

    Ok(())
}

/// What to do when `rojo` does not run in the new project, which depends on
/// who was meant to install it.
fn rojo_note(
    package_manager: &tools::PackageManager,
    tool_manager: &tools::ToolManager,
    setup: &tools::Setup,
) -> String {
    let headline = "Note: `rojo` does not run in this folder yet.";

    match setup.rojo {
        RojoFrom::PackageManager => {
            let bin = package_manager.bin_path().map_or_else(
                || {
                    package_manager
                        .bin_dir
                        .unwrap_or("the package manager's binary directory")
                        .to_string()
                },
                |p| p.display().to_string(),
            );
            format!(
                "{headline}\n{} installed it in {bin}, but that folder is missing from PATH or another `rojo` comes first.\nPut {bin} first on PATH, then open a new terminal.",
                package_manager.name
            )
        }
        RojoFrom::ToolManager => format!(
            "{headline}\nCheck that {} is set up: {}",
            tool_manager.name, tool_manager.homepage
        ),
        RojoFrom::Path => format!(
            "{headline}\nInstall Rojo and make it available on PATH: https://rojo.space/docs/v7/getting-started/installation/"
        ),
    }
}

/// Verify only tools that setup will not install itself.
fn preflight(
    dir: &Path,
    pm: &tools::PackageManager,
    tm: &tools::ToolManager,
    setup: &tools::Setup,
) -> Result<()> {
    let (binary, homepage, search) = if tm.installs_tools {
        (
            tm.binary.context("tool manager has no executable")?,
            tm.homepage,
            &[][..],
        )
    } else {
        (pm.binary, pm.homepage, setup.path_first.as_slice())
    };
    if !process::works_with_path(&format!("{binary} --version"), dir, search) {
        bail!(
            "`{binary} --version` failed. Install {binary} from {homepage} and make it available on PATH before running init"
        );
    }
    if setup.rojo == RojoFrom::Path && !process::works("rojo --version", dir) {
        bail!(
            "this setup requires Rojo on PATH; `rojo --version` failed. Install Rojo before running init: https://rojo.space/docs/v7/getting-started/installation/"
        );
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_folder_suggestions_without_changing_explicit_names() {
        assert_eq!(normalize_name("My Game__ 2!"), Some("my-game__-2".into()));
        assert_eq!(normalize_name("!!!"), None);
        assert!(validate_name("my_game-v2").is_ok());
        for name in ["", "../game", "my game", "Game", "a/b", "a\\b"] {
            assert!(validate_name(name).is_err(), "{name}");
        }
    }

    #[test]
    fn resolves_current_folder_new_folder_and_name_only_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        for spelling in [".", "./"] {
            let (path, name) = resolve_target(
                dir.path(),
                Some(spelling.into()),
                Some("game".into()),
                false,
            )
            .unwrap();
            assert_eq!(path, dir.path());
            assert_eq!(name, "game");
        }
        for target in [None, Some("game".into())] {
            let (path, name) =
                resolve_target(dir.path(), target, Some("game".into()), false).unwrap();
            assert_eq!(path, dir.path().join("game"));
            assert_eq!(name, "game");
            assert!(!path.exists());
        }
    }
}
