use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use inquire::Text;

use crate::install;
use crate::prompt;
use crate::template;
use crate::tools::{self, Tool};

/// Each choice comes from its flag when given, otherwise from a prompt.
pub fn run(
    name: Option<String>,
    package_manager: Option<String>,
    tool_manager: Option<String>,
) -> Result<()> {
    let name = resolve_name(name)?;
    let cwd = env::current_dir().context("could not read the current directory")?;
    let path = project_path(&cwd, &name)?;

    let package_manager = match package_manager {
        Some(name) => tools::find(tools::PACKAGE_MANAGERS, &name)?,
        None => prompt::select("Package manager:", tools::PACKAGE_MANAGERS)?,
    };
    let tool_manager = match tool_manager {
        _ if package_manager.manages_tools => None,
        Some(name) => Some(tools::find(tools::TOOL_MANAGERS, &name)?),
        None => Some(prompt::select("Tool manager:", tools::TOOL_MANAGERS)?),
    };

    fs::create_dir(&path).with_context(|| format!("could not create {}", path.display()))?;
    let vars = tools::vars(&name, &package_manager);
    let skip = tools::other_manifests(&package_manager);
    template::render(&template::DEFAULT, &path, &vars, &skip)?;

    if let Some(tm) = &tool_manager
        && !tm.manifest.is_empty()
    {
        let manifest = tools::tool_manifest(tm, &package_manager);
        fs::write(path.join(tm.manifest), manifest)
            .with_context(|| format!("could not write {}", tm.manifest))?;
    }

    // Tool manager first: it puts the package manager on the PATH.
    let mut to_install: Vec<&dyn Tool> = Vec::new();
    if let Some(tm) = &tool_manager {
        to_install.push(tm);
    }
    to_install.push(&package_manager);
    let failures = install::run_all(&to_install, &vars, &path);

    let tools_via = tool_manager.map_or("tools built in", |tm| tm.name);
    println!(
        "\nCreated {} ({} + {})",
        path.display(),
        package_manager.name,
        tools_via
    );
    println!("\nNext steps:\n  cd {name}\n  rojo serve");

    if failures.is_empty() {
        return Ok(());
    }

    println!("\nThese steps did not complete:");
    for failure in &failures {
        println!("  ✗ {}\n    {}", failure.command, failure.fix);
    }
    bail!("{} install step(s) failed", failures.len());
}

/// Uses the name from the command line, or prompts for one.
fn resolve_name(name: Option<String>) -> Result<String> {
    let name = match name {
        Some(name) => name,
        None => Text::new("Project name:").prompt()?,
    };

    validate_name(&name)?;
    Ok(name)
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

/// Where the project will be created. Refuses a folder that already exists.
fn project_path(parent: &Path, name: &str) -> Result<PathBuf> {
    let path = parent.join(name);

    if path.exists() {
        bail!("{} already exists", path.display());
    }

    Ok(path)
}
