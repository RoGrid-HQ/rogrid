use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use super::Side;

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub events: Events,
}

impl Config {
    pub fn validate_folders(&self, root: &Path) -> Result<()> {
        let root = fs::canonicalize(root)?;
        let mut seen = BTreeSet::new();
        for folder in self.events.server.iter().chain(&self.events.client) {
            let resolved = fs::canonicalize(root.join(folder)).with_context(|| {
                format!(
                    "event folder {} is missing; create it or update [events] in rogrid.toml",
                    folder.display()
                )
            })?;
            if !resolved.starts_with(&root) || !resolved.is_dir() {
                bail!(
                    "event folder {} must resolve to a directory inside the project",
                    folder.display()
                );
            }
            if !seen.insert(resolved) {
                bail!(
                    "event folder {} resolves to a folder already listed",
                    folder.display()
                );
            }
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Events {
    pub server: Vec<PathBuf>,
    pub client: Vec<PathBuf>,
}

impl Default for Events {
    fn default() -> Self {
        Self {
            server: vec!["src/server/events".into()],
            client: vec!["src/client/events".into()],
        }
    }
}

impl Events {
    pub fn folders(&self, side: Side) -> &[PathBuf] {
        match side {
            Side::Server => &self.server,
            Side::Client => &self.client,
        }
    }
}

/// A missing file uses defaults. Omitted sides use defaults; an empty list disables a side.
pub fn load(root: &Path) -> Result<Config> {
    let config: Config = match fs::read_to_string(root.join("rogrid.toml")) {
        Ok(source) => toml::from_str(&source).context("could not parse rogrid.toml")?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Config::default(),
        Err(error) => return Err(error).context("could not read rogrid.toml"),
    };
    let mut seen = BTreeSet::new();
    for folder in config.events.server.iter().chain(&config.events.client) {
        if folder.as_os_str().is_empty()
            || !folder
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
            || folder.starts_with(".rogrid")
            || folder.starts_with(".git")
        {
            bail!(
                "event folder {} must be a project-relative path without . or .. components, outside .rogrid and .git",
                folder.display()
            );
        }
        if !seen.insert(folder) {
            bail!("event folder {} is listed more than once", folder.display());
        }
    }
    Ok(config)
}
