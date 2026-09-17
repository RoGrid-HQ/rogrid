//! Reads the Rojo project file, to translate a file on disk into its place in the game.

use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::Value;

const PROJECT_FILE: &str = "default.project.json";

pub struct Project {
    pub root: PathBuf,
    /// Every `$path` in the project: where it is on disk, relative to the root, and
    /// the names leading to it from the DataModel.
    mappings: Vec<(PathBuf, Vec<String>)>,
}

impl Project {
    pub fn load(root: &Path) -> Result<Self> {
        let text = fs::read_to_string(root.join(PROJECT_FILE))
            .with_context(|| format!("no {PROJECT_FILE} here. Run this inside a RoGrid project"))?;
        let json: Value = serde_json::from_str(&text)
            .with_context(|| format!("{PROJECT_FILE} is not valid JSON"))?;

        let tree = json
            .get("tree")
            .with_context(|| format!("{PROJECT_FILE} has no `tree`"))?;
        if tree.get("$className").and_then(Value::as_str) != Some("DataModel") {
            bail!(
                "{PROJECT_FILE} must describe a place: its tree needs `\"$className\": \"DataModel\"`"
            );
        }

        let mut mappings = Vec::new();
        collect(tree, &mut Vec::new(), &mut mappings);
        Ok(Self {
            root: root.to_path_buf(),
            mappings,
        })
    }

    /// The `@game/...` require path of a file or folder, or `None` when the project does not map it.
    pub fn game_path(&self, path: &Path) -> Option<String> {
        let relative = path.strip_prefix(&self.root).unwrap_or(path);

        // The deepest mapping that contains the path wins.
        let (mapped, instance) = self
            .mappings
            .iter()
            .filter(|(mapped, _)| relative.starts_with(mapped))
            .max_by_key(|(mapped, _)| mapped.components().count())?;

        let mut names = instance.clone();
        for component in relative.strip_prefix(mapped).ok()?.components() {
            let Component::Normal(name) = component else {
                return None;
            };
            names.push(name.to_str()?.to_string());
        }

        // A module's name is its file name without the extension. `init` is the folder itself.
        if let Some(last) = names.last_mut() {
            let stem = last
                .strip_suffix(".luau")
                .or_else(|| last.strip_suffix(".lua"));
            if let Some(stem) = stem {
                *last = stem.to_string();
                if last == "init" {
                    names.pop();
                }
            }
        }

        Some(format!("@game/{}", names.join("/")))
    }

    /// The `@game/...` require path of the installed rogrid library, wherever the
    /// package manager put it.
    pub fn library(&self) -> Option<String> {
        self.mappings.iter().find_map(|(mapped, _)| {
            ["rogrid.luau", "rogrid.lua", "rogrid"]
                .iter()
                .map(|name| self.root.join(mapped).join(name))
                .find(|candidate| candidate.exists())
                .and_then(|found| self.game_path(&found))
        })
    }
}

/// Walks the project tree. Keys starting with `$` are Rojo's own, everything else is a child instance.
fn collect(node: &Value, instance: &mut Vec<String>, mappings: &mut Vec<(PathBuf, Vec<String>)>) {
    let Some(object) = node.as_object() else {
        return;
    };

    // `$path` is a string, or `{ "optional": "..." }`.
    let path = object
        .get("$path")
        .and_then(|path| path.as_str().or_else(|| path.get("optional")?.as_str()));
    if let Some(path) = path {
        mappings.push((PathBuf::from(path), instance.clone()));
    }

    for (key, child) in object {
        if !key.starts_with('$') {
            instance.push(key.clone());
            collect(child, instance, mappings);
            instance.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(tree: &str) -> (tempfile::TempDir, Project) {
        let dir = tempfile::tempdir().unwrap();
        let json = format!(r#"{{ "name": "test", "tree": {tree} }}"#);
        fs::write(dir.path().join(PROJECT_FILE), json).unwrap();
        let project = Project::load(dir.path()).unwrap();
        (dir, project)
    }

    const TREE: &str = r#"{
        "$className": "DataModel",
        "ReplicatedStorage": {
            "Shared": { "$path": "src/shared" },
            "Packages": { "$path": { "optional": "roblox_packages" } }
        },
        "ServerScriptService": { "Server": { "$path": "src/server" } }
    }"#;

    #[test]
    fn maps_files_to_require_paths() {
        let (_dir, project) = project(TREE);

        assert_eq!(
            project
                .game_path(Path::new("src/server/functions/shop/buyItem.luau"))
                .as_deref(),
            Some("@game/ServerScriptService/Server/functions/shop/buyItem")
        );
        assert_eq!(
            project
                .game_path(Path::new("src/shared/events/init.luau"))
                .as_deref(),
            Some("@game/ReplicatedStorage/Shared/events")
        );
        assert_eq!(project.game_path(Path::new("elsewhere/file.luau")), None);
    }

    #[test]
    fn finds_the_library_where_the_package_manager_put_it() {
        let (dir, project) = project(TREE);
        assert_eq!(project.library(), None);

        fs::create_dir_all(dir.path().join("roblox_packages")).unwrap();
        fs::write(dir.path().join("roblox_packages/rogrid.luau"), "").unwrap();
        assert_eq!(
            project.library().as_deref(),
            Some("@game/ReplicatedStorage/Packages/rogrid")
        );
    }

    #[test]
    fn refuses_a_project_that_is_not_a_place() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(PROJECT_FILE),
            r#"{ "name": "lib", "tree": { "$path": "src" } }"#,
        )
        .unwrap();
        assert!(Project::load(dir.path()).is_err());
    }
}
