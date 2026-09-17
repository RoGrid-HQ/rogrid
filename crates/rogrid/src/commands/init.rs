use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::install;
use crate::prompt;
use crate::template;
use crate::tools::{self, Tool};

/// Each choice comes from its flag when given, otherwise from a prompt.
pub fn run(
    target: Option<String>,
    name: Option<String>,
    force: bool,
    package_manager: Option<String>,
    tool_manager: Option<String>,
) -> Result<()> {
    let cwd = env::current_dir().context("could not read the current directory")?;
    let (path, name) = resolve_target(&cwd, target, name, force)?;

    let package_manager = match package_manager {
        Some(name) => tools::find(tools::PACKAGE_MANAGERS, &name)?,
        None => prompt::select("Package manager:", tools::PACKAGE_MANAGERS)?,
    };
    let options = tools::tool_managers_for(&package_manager);
    let tool_manager = match tool_manager {
        Some(name) => tools::find(&options, &name)?,
        None if options.len() == 1 => options[0],
        None => prompt::select("Tool manager:", &options)?,
    };

    fs::create_dir_all(&path).with_context(|| format!("could not create {}", path.display()))?;
    let vars = tools::vars(&name, &package_manager, &tool_manager);
    let skip = tools::other_manifests(&package_manager);
    template::render(&template::DEFAULT, &path, &vars, &skip)?;

    if !tool_manager.manifest.is_empty() {
        let manifest = tools::tool_manifest(&package_manager, &tool_manager);
        fs::write(path.join(tool_manager.manifest), manifest)
            .with_context(|| format!("could not write {}", tool_manager.manifest))?;
    }

    // Tool manager first: it puts the package manager on the PATH.
    let to_install: [&dyn Tool; 2] = [&tool_manager, &package_manager];
    let failures = install::run_all(&to_install, &vars, &path);

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
    println!("  rogrid dev");

    if failures.is_empty() {
        return Ok(());
    }

    println!("\nThese steps did not complete:");
    for failure in &failures {
        println!("  ✗ {}\n    {}", failure.command, failure.fix);
    }
    bail!("{} install step(s) failed", failures.len());
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
