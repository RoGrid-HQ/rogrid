use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use anyhow::Result;
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
        // Shared aliases can live anywhere in the project, including a module that
        // does not exist yet. Watching sources also recovers failed/deleted imports.
        watcher.watch(root, RecursiveMode::Recursive)?;
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
            let Ok(relative) = path.strip_prefix(&self.root) else {
                return false;
            };
            if relative
                .components()
                .any(|part| matches!(part.as_os_str().to_str(), Some(".rogrid" | ".git")))
            {
                return false;
            }
            path == &self.root.join("rogrid.toml")
                || path
                    .extension()
                    .is_some_and(|ext| ext == "luau" || ext == "lua")
                || path.extension().is_none()
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

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{AccessKind, CreateKind, ModifyKind, RemoveKind};
    use notify::{Event, EventKind};

    #[test]
    fn shared_type_changes_and_recovery_trigger_generation() {
        let dir = tempfile::tempdir().unwrap();
        let (send, _receive) = std::sync::mpsc::channel();
        let sources = Sources::new(dir.path(), send).unwrap();
        for kind in [
            EventKind::Create(CreateKind::File),
            EventKind::Modify(ModifyKind::Any),
            EventKind::Remove(RemoveKind::File),
        ] {
            for path in [
                "src/shared/Types.luau",
                "types/Imported.lua",
                "Packages/Types.luau",
                "new-types",
                "rogrid.toml",
                "other.project.json",
                "target/fixture.luau",
            ] {
                assert!(
                    sources.relevant(&Event::new(kind).add_path(dir.path().join(path))),
                    "{path}"
                );
            }
            for path in [
                ".rogrid/generated/shared/Server.luau",
                ".git/index",
                "README.md",
            ] {
                assert!(
                    !sources.relevant(&Event::new(kind).add_path(dir.path().join(path))),
                    "{path}"
                );
            }
        }
        assert!(
            !sources.relevant(
                &Event::new(EventKind::Access(AccessKind::Read))
                    .add_path(dir.path().join("types.luau"))
            )
        );
    }
}
