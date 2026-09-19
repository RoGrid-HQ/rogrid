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
        let mut location = self.source_location(root, file)?;
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

    pub fn module_at(&self, root: &Path, location: &[String]) -> Result<PathBuf> {
        let mut node = self;
        for name in location {
            let matches: Vec<_> = node
                .children
                .iter()
                .filter(|child| &child.name == name)
                .collect();
            let [child] = matches.as_slice() else {
                bail!(
                    "type import {} must map to exactly one ModuleScript",
                    location.join("/")
                );
            };
            node = child;
        }
        let files: Vec<_> = node
            .file_paths
            .iter()
            .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("luau" | "lua")))
            .collect();
        if node.class_name != "ModuleScript" || files.len() != 1 {
            bail!(
                "type import {} must map to one Luau ModuleScript",
                location.join("/")
            );
        }
        Ok(root.join(files[0]))
    }

    pub fn source_location(&self, root: &Path, file: &Path) -> Result<Vec<String>> {
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
        Ok(location.clone())
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn map(location: &[&str], class: &str, files: &[&str]) -> Node {
        let mut node =
            json!({"name": location.last().unwrap(), "className": class, "filePaths": files});
        for name in location[..location.len() - 1].iter().rev() {
            node = json!({"name": name, "className": "Folder", "children": [node]});
        }
        serde_json::from_value(
            json!({"name": "Game", "className": "DataModel", "children": [node]}),
        )
        .unwrap()
    }

    #[test]
    fn uses_mapped_names_and_all_documented_receiver_locations() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Events.luau"), "return {}").unwrap();
        for (location, side, expected) in [
            (
                vec!["ServerScriptService", "Renamed", "Combat"],
                Side::Server,
                vec!["ServerScriptService", "Renamed", "Combat"],
            ),
            (
                vec!["ServerStorage", "Combat"],
                Side::Server,
                vec!["ServerStorage", "Combat"],
            ),
            (
                vec!["ReplicatedStorage", "Combat"],
                Side::Client,
                vec!["ReplicatedStorage", "Combat"],
            ),
            (
                vec!["StarterPlayer", "StarterPlayerScripts", "Combat"],
                Side::Client,
                vec!["PlayerScripts", "Combat"],
            ),
        ] {
            let map = map(&location, "ModuleScript", &["Events.luau"]);
            assert_eq!(
                map.location(dir.path(), &dir.path().join("Events.luau"), side)
                    .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn rejects_wrong_sides_and_unsupported_receiver_locations() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Events.luau"), "return {}").unwrap();
        for (location, side) in [
            (vec!["ReplicatedStorage", "Combat"], Side::Server),
            (vec!["ServerStorage", "Combat"], Side::Client),
            (vec!["Workspace", "Combat"], Side::Server),
            (
                vec!["StarterPlayer", "StarterCharacterScripts", "Combat"],
                Side::Client,
            ),
        ] {
            let map = map(&location, "ModuleScript", &["Events.luau"]);
            assert!(
                map.location(dir.path(), &dir.path().join("Events.luau"), side)
                    .unwrap_err()
                    .to_string()
                    .contains("server events must be under")
            );
        }
    }

    #[test]
    fn requires_exactly_one_module_mapping_for_a_source() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Events.luau"), "return {}").unwrap();
        for class in ["Folder", "Script", "LocalScript"] {
            let map = map(&["ServerStorage", "Combat"], class, &["Events.luau"]);
            assert!(
                map.source_location(dir.path(), &dir.path().join("Events.luau"))
                    .unwrap_err()
                    .to_string()
                    .contains("found 0")
            );
        }
        let mut map = map(
            &["ServerStorage", "Combat"],
            "ModuleScript",
            &["Events.luau"],
        );
        map.children.push(Node {
            name: "Duplicate".into(),
            class_name: "ModuleScript".into(),
            file_paths: vec!["Events.luau".into()],
            children: vec![],
        });
        assert!(
            map.source_location(dir.path(), &dir.path().join("Events.luau"))
                .unwrap_err()
                .to_string()
                .contains("found 2")
        );
    }

    #[test]
    fn imports_require_an_unambiguous_module_with_one_luau_source() {
        let dir = tempfile::tempdir().unwrap();
        let location = ["ReplicatedStorage".into(), "Types".into()];
        for (class, files) in [
            ("Folder", vec!["Types.luau"]),
            ("ModuleScript", vec![]),
            ("ModuleScript", vec!["a.lua", "b.luau"]),
        ] {
            assert!(
                map(&["ReplicatedStorage", "Types"], class, &files)
                    .module_at(dir.path(), &location)
                    .unwrap_err()
                    .to_string()
                    .contains("one Luau ModuleScript")
            );
        }
        let mut map = map(
            &["ReplicatedStorage", "Types"],
            "ModuleScript",
            &["Types.luau", "Types.meta.json"],
        );
        assert_eq!(
            map.module_at(dir.path(), &location).unwrap(),
            dir.path().join("Types.luau")
        );
        assert!(map.module_at(dir.path(), &["Missing".into()]).is_err());
        map.children[0].children.push(Node {
            name: "Types".into(),
            class_name: "ModuleScript".into(),
            file_paths: vec!["Other.luau".into()],
            children: vec![],
        });
        assert!(
            map.module_at(dir.path(), &location)
                .unwrap_err()
                .to_string()
                .contains("exactly one ModuleScript")
        );
    }
}
