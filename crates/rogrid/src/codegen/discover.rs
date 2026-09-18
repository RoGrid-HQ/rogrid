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
                )?);
            }
        }
    }
    Ok(modules)
}
