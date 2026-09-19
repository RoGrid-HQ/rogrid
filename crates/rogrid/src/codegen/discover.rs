use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use super::{Module, Side, config::Config, parse, sourcemap::Node};

pub fn modules(
    root: &Path,
    config: &Config,
    map: &Node,
    digest: &mut Sha256,
) -> Result<Vec<Module>> {
    let mut modules = Vec::new();
    let mut resolver = super::resolve::Resolver::new(root, map);
    for side in [Side::Server, Side::Client] {
        let mut names = BTreeMap::new();
        for relative in config.events.folders(side) {
            let folder = root.join(relative);
            let mut paths = fs::read_dir(&folder)?
                .map(|entry| entry.map(|e| e.path()))
                .collect::<std::io::Result<Vec<_>>>()?;
            paths.sort();
            for path in paths {
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .context("event filename must be UTF-8")?;
                if filename.starts_with('.') {
                    continue;
                }
                if fs::symlink_metadata(&path)?.file_type().is_symlink() {
                    bail!("{}: event files must not be symbolic links", path.display());
                }
                if path.is_dir() {
                    continue;
                }
                if !matches!(
                    path.extension().and_then(|e| e.to_str()),
                    Some("luau" | "lua")
                ) {
                    continue;
                }
                let name = path.file_stem().and_then(|n| n.to_str()).unwrap();
                if name == "init" || !super::identifier(name) {
                    bail!(
                        "{}: use a plain module filename such as Combat.luau (not init, .server, or .client)",
                        path.display()
                    );
                }
                if let Some(previous) = names.insert(name.to_string(), path.clone()) {
                    bail!(
                        "duplicate {} module {name}: {} and {}; module names must be unique per side",
                        side.name(),
                        previous.display(),
                        path.display()
                    );
                }
                let location = map.location(root, &path, side)?;
                digest.update(serde_json::to_vec(&location)?);
                let source = fs::read_to_string(&path)
                    .with_context(|| format!("could not read {}", path.display()))?;
                digest.update(side.name());
                digest.update([0]);
                digest.update(filename);
                digest.update([0]);
                digest.update(&source);
                digest.update([0]);
                modules.push(parse::module(
                    &path,
                    &source,
                    side,
                    name.to_string(),
                    location,
                    &mut resolver,
                )?);
            }
        }
    }
    let canonical_root = fs::canonicalize(root)?;
    for (path, source) in resolver.dependencies {
        digest.update(
            path.strip_prefix(&canonical_root)?
                .to_string_lossy()
                .replace('\\', "/"),
        );
        digest.update([0]);
        digest.update(source);
        digest.update([0]);
    }
    Ok(modules)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture(files: &[&str], folders: &[&str]) -> (tempfile::TempDir, Config, Node) {
        let dir = tempfile::tempdir().unwrap();
        for folder in folders {
            fs::create_dir_all(dir.path().join(folder)).unwrap();
        }
        let children: Vec<_> = files.iter().map(|file| {
            let path = dir.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, "return { e = RoGrid.event(function(player: Player) end) }").unwrap();
            json!({"name": path.file_stem().unwrap().to_str().unwrap(), "className": "ModuleScript", "filePaths": [file]})
        }).collect();
        let map = serde_json::from_value(json!({"name": "Game", "className": "DataModel", "children": [{"name": "ServerStorage", "className": "ServerStorage", "children": children}]})).unwrap();
        let mut config = Config::default();
        config.events.server = folders.iter().map(Into::into).collect();
        config.events.client.clear();
        (dir, config, map)
    }

    #[test]
    fn discovers_direct_lua_and_luau_modules_in_stable_order() {
        let (dir, config, map) = fixture(
            &[
                "events/Z.luau",
                "events/A.lua",
                "events/.Hidden.luau",
                "events/notes.txt",
                "events/nested/Skipped.luau",
            ],
            &["events"],
        );
        let discover = || {
            let mut digest = Sha256::new();
            let modules = modules(dir.path(), &config, &map, &mut digest).unwrap();
            (
                modules
                    .into_iter()
                    .map(|module| module.name)
                    .collect::<Vec<_>>(),
                digest.finalize(),
            )
        };
        let first = discover();
        assert_eq!(first.0, ["A", "Z"]);
        assert_eq!(first, discover());
    }

    #[test]
    fn rejects_duplicate_module_names_across_extensions_and_folders() {
        for (files, folders) in [
            (
                vec!["events/Combat.lua", "events/Combat.luau"],
                vec!["events"],
            ),
            (
                vec!["combat/Combat.luau", "inventory/Combat.luau"],
                vec!["combat", "inventory"],
            ),
        ] {
            let (dir, config, map) = fixture(&files, &folders);
            let error = modules(dir.path(), &config, &map, &mut Sha256::new())
                .err()
                .unwrap();
            assert!(
                error.to_string().contains("duplicate server module Combat"),
                "{error:#}"
            );
        }
    }

    #[test]
    fn rejects_invalid_receiver_filenames() {
        for file in [
            "init.luau",
            "Combat.server.luau",
            "Combat.client.lua",
            "bad-name.luau",
            "123.luau",
            "end.luau",
        ] {
            let path = format!("events/{file}");
            let (dir, config, map) = fixture(&[&path], &["events"]);
            assert!(
                modules(dir.path(), &config, &map, &mut Sha256::new())
                    .err()
                    .unwrap()
                    .to_string()
                    .contains("plain module filename")
            );
        }
    }

    #[test]
    fn empty_or_disabled_folders_generate_no_modules() {
        let (dir, mut config, map) = fixture(&[], &["events"]);
        assert!(
            modules(dir.path(), &config, &map, &mut Sha256::new())
                .unwrap()
                .is_empty()
        );
        config.events.server.clear();
        fs::remove_dir(dir.path().join("events")).unwrap();
        assert!(
            modules(dir.path(), &config, &map, &mut Sha256::new())
                .unwrap()
                .is_empty()
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_event_files() {
        let (dir, config, map) = fixture(&["Original.luau"], &["events"]);
        std::os::unix::fs::symlink(
            dir.path().join("Original.luau"),
            dir.path().join("events/Linked.luau"),
        )
        .unwrap();
        assert!(
            modules(dir.path(), &config, &map, &mut Sha256::new())
                .err()
                .unwrap()
                .to_string()
                .contains("must not be symbolic links")
        );
    }
}
