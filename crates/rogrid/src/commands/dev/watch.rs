use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use anyhow::{Context, Result};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use super::Change;
use crate::codegen::config;

pub struct Sources {
    _watcher: RecommendedWatcher,
    folders: Vec<PathBuf>,
    root: PathBuf,
}

impl Sources {
    pub fn new(root: &Path, send: Sender<Change>) -> Result<Self> {
        let config = config::load(root)?;
        let folders: Vec<_> = config
            .events
            .server
            .iter()
            .chain(&config.events.client)
            .map(|folder| root.join(folder))
            .collect();
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = send.send(Change::Files(event));
        })?;
        // Watch the root for atomic config saves and creation of missing top-level folders.
        watcher.watch(root, RecursiveMode::NonRecursive)?;
        let mut parents = BTreeSet::new();
        for folder in &folders {
            // Watching the enclosing top-level directory also catches receiver folder renames.
            let first = folder.strip_prefix(root)?.components().next().unwrap();
            let parent = root.join(first.as_os_str());
            if parent.is_dir() && parents.insert(parent.clone()) {
                watcher
                    .watch(&parent, RecursiveMode::Recursive)
                    .with_context(|| format!("could not watch {}", parent.display()))?;
            }
        }
        Ok(Self {
            _watcher: watcher,
            folders,
            root: root.to_path_buf(),
        })
    }

    pub fn relevant(&self, event: &notify::Event) -> bool {
        if event.kind.is_access() {
            return false;
        }
        event.paths.iter().any(|path| {
            path == &self.root.join("rogrid.toml")
                || path == &self.root.join("default.project.json")
                || (path.extension().is_some_and(|ext| ext == "json")
                    && path.to_string_lossy().ends_with(".project.json"))
                || self
                    .folders
                    .iter()
                    .any(|folder| path.starts_with(folder) || folder.starts_with(path))
        })
    }
}
