use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

/// Resolve before creating project files. Keep ordinary absolute paths on Windows,
/// rather than canonicalize's verbatim paths, for Rojo and editor tooling.
pub(super) fn resolve_source(package: &Path) -> Result<PathBuf> {
    let source = std::path::absolute(package)?.join("src");
    if !source.join("init.luau").is_file() {
        bail!(
            "local framework {} must contain src/init.luau; pass the RoGrid package folder",
            package.display()
        );
    }
    source
        .to_str()
        .context("local framework path must be valid UTF-8 for the Rojo project")?;
    Ok(source)
}

/// Local and installed packages have the same in-game import path.
pub(super) fn configure(project: &Path, source: &Path) -> Result<()> {
    let path = project.join("default.project.json");
    let mut config: Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
    let packages = config
        .pointer_mut("/tree/ReplicatedStorage/Packages")
        .and_then(Value::as_object_mut)
        .context("starter template is missing ReplicatedStorage.Packages")?;
    // The installed packages directory is optional and may not exist yet.
    packages.insert("$className".into(), json!("Folder"));
    packages.insert("rogrid".into(), json!({ "$path": source }));
    fs::write(&path, serde_json::to_string_pretty(&config)? + "\n")
        .with_context(|| format!("could not write {}", path.display()))
}
