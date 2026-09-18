use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use super::Side;
use crate::process;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    name: String,
    class_name: String,
    #[serde(default)]
    file_paths: Vec<PathBuf>,
    #[serde(default)]
    children: Vec<Node>,
}

pub fn read(root: &Path, path_first: &[PathBuf]) -> Result<Node> {
    let mut command = Command::new("rojo");
    command
        .args(["sourcemap", "default.project.json"])
        .current_dir(root);
    if let Some(path) = process::search_path(path_first) {
        command.env("PATH", path);
    }
    let output = command
        .output()
        .context("could not run rojo sourcemap; install Rojo and make it available on PATH")?;
    if !output.status.success() {
        bail!(
            "rojo sourcemap failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let map: Node =
        serde_json::from_slice(&output.stdout).context("invalid Rojo sourcemap output")?;
    if map.class_name != "DataModel" {
        bail!("default.project.json must describe a DataModel");
    }
    Ok(map)
}

impl Node {
    /// File identities, not directory names, connect declarations to Roblox instances.
    pub fn location(&self, root: &Path, file: &Path, side: Side) -> Result<Vec<String>> {
        let file = fs::canonicalize(file)?;
        let mut matches = Vec::new();
        self.find(root, &file, &mut Vec::new(), &mut matches);
        let [location] = matches.as_slice() else {
            bail!(
                "{} must map to exactly one ModuleScript in default.project.json (found {})",
                file.display(),
                matches.len()
            );
        };
        let mut location = location.clone();
        match side {
            Side::Server
                if matches!(
                    location.first().map(String::as_str),
                    Some("ServerScriptService" | "ServerStorage")
                ) => {}
            Side::Client
                if location
                    .starts_with(&["StarterPlayer".into(), "StarterPlayerScripts".into()]) =>
            {
                location.splice(..2, ["PlayerScripts".to_string()]);
            }
            Side::Client
                if location
                    .first()
                    .is_some_and(|name| name == "ReplicatedStorage") => {}
            _ => bail!(
                "{} maps to {}; server events must be under ServerScriptService or ServerStorage, client events under StarterPlayerScripts or ReplicatedStorage",
                file.display(),
                location.join("/")
            ),
        }
        Ok(location)
    }

    fn find(
        &self,
        root: &Path,
        file: &Path,
        location: &mut Vec<String>,
        found: &mut Vec<Vec<String>>,
    ) {
        if self.class_name == "ModuleScript"
            && self
                .file_paths
                .iter()
                .any(|path| fs::canonicalize(root.join(path)).is_ok_and(|path| path == file))
        {
            found.push(location.clone());
        }
        for child in &self.children {
            location.push(child.name.clone());
            child.find(root, file, location, found);
            location.pop();
        }
    }
}
