use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use inquire::Text;

use crate::prompt;
use crate::template;
use crate::tools;

pub fn run(name: Option<String>) -> Result<()> {
    let name = resolve_name(name)?;
    let cwd = env::current_dir().context("could not read the current directory")?;
    let path = project_path(&cwd, &name)?;

    let package_manager = prompt::select("Package manager:", tools::PACKAGE_MANAGERS)?;
    let tool_manager = prompt::select("Tool manager:", &tools::tool_managers(package_manager))?;

    fs::create_dir(&path).with_context(|| format!("could not create {}", path.display()))?;
    let vars = tools::template_vars(&name, package_manager)?;
    let skip = tools::unused_files(&[package_manager, tool_manager]);
    template::render(&template::DEFAULT, &path, &vars, &skip)?;

    println!(
        "Created {} ({} + {})",
        path.display(),
        package_manager.name,
        tool_manager.name
    );
    Ok(())
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
