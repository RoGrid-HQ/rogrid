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

#[cfg(test)]
mod tests {
    use super::*;

    fn configured(source: &str) -> (tempfile::TempDir, Config) {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("rogrid.toml"), source).unwrap();
        let config = load(dir.path()).unwrap();
        (dir, config)
    }

    #[test]
    fn missing_configuration_and_omitted_sides_use_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let config = load(dir.path()).unwrap();
        assert_eq!(config.events.server, [PathBuf::from("src/server/events")]);
        assert_eq!(config.events.client, [PathBuf::from("src/client/events")]);

        let (_dir, config) = configured("[events]\nserver = ['combat', 'inventory']\n");
        assert_eq!(
            config.events.server,
            [PathBuf::from("combat"), PathBuf::from("inventory")]
        );
        assert_eq!(config.events.client, [PathBuf::from("src/client/events")]);
    }

    #[test]
    fn empty_lists_disable_discovery_without_requiring_default_folders() {
        let (dir, config) = configured("[events]\nserver = []\nclient = []\n");
        assert!(config.events.folders(Side::Server).is_empty());
        assert!(config.events.folders(Side::Client).is_empty());
        config.validate_folders(dir.path()).unwrap();
    }

    #[test]
    fn custom_folders_must_exist_and_be_directories() {
        let (dir, config) = configured("[events]\nserver = ['combat', 'inventory']\nclient = []\n");
        fs::create_dir(dir.path().join("combat")).unwrap();
        assert!(
            format!("{:#}", config.validate_folders(dir.path()).unwrap_err())
                .contains("inventory is missing")
        );
        fs::write(dir.path().join("inventory"), "not a directory").unwrap();
        assert!(
            format!("{:#}", config.validate_folders(dir.path()).unwrap_err())
                .contains("directory inside the project")
        );
        fs::remove_file(dir.path().join("inventory")).unwrap();
        fs::create_dir(dir.path().join("inventory")).unwrap();
        config.validate_folders(dir.path()).unwrap();
    }

    #[test]
    fn malformed_configuration_unknown_keys_and_duplicate_folders_are_rejected() {
        for (source, expected) in [
            ("[events", "could not parse rogrid.toml"),
            ("event = {}", "unknown field"),
            ("[events]\nservers = []", "unknown field"),
            ("[events]\nserver = 'combat'", "invalid type"),
            (
                "[events]\nserver = ['combat', 'combat']",
                "listed more than once",
            ),
            (
                "[events]\nserver = ['combat']\nclient = ['combat']",
                "listed more than once",
            ),
        ] {
            let dir = tempfile::tempdir().unwrap();
            fs::write(dir.path().join("rogrid.toml"), source).unwrap();
            let error = load(dir.path()).err().expect(source);
            assert!(
                format!("{error:#}").contains(expected),
                "{source}: {error:#}"
            );
        }
    }

    #[test]
    fn event_paths_reject_absolute_parent_and_reserved_locations() {
        for folder in [
            "",
            ".",
            "./events",
            "/events",
            "../events",
            "src/../events",
            ".git/events",
            ".rogrid/events",
        ] {
            let dir = tempfile::tempdir().unwrap();
            fs::write(
                dir.path().join("rogrid.toml"),
                format!("[events]\nserver = ['{folder}']\nclient = []\n"),
            )
            .unwrap();
            let error = load(dir.path()).err().expect(folder);
            assert!(
                format!("{error:#}").contains("project-relative path"),
                "{folder}: {error:#}"
            );
        }
    }

    #[test]
    fn redundant_internal_dot_components_resolve_to_the_same_folder() {
        for folder in ["src/events", "src/./events", "src/events/."] {
            let (dir, config) =
                configured(&format!("[events]\nserver = ['{folder}']\nclient = []\n"));
            fs::create_dir_all(dir.path().join("src/events")).unwrap();
            config.validate_folders(dir.path()).unwrap();
            assert_eq!(
                fs::canonicalize(dir.path().join(&config.events.server[0])).unwrap(),
                fs::canonicalize(dir.path().join("src/events")).unwrap()
            );
        }
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("rogrid.toml"),
            "[events]\nserver = ['src/events', 'src/./events']\nclient = []\n",
        )
        .unwrap();
        assert!(format!("{:#}", load(dir.path()).err().unwrap()).contains("listed more than once"));
    }

    #[cfg(unix)]
    #[test]
    fn folder_symlinks_cannot_escape_the_project_or_duplicate_another_folder() {
        use std::os::unix::fs::symlink;
        let outside = tempfile::tempdir().unwrap();
        let (dir, config) = configured("[events]\nserver = ['linked']\nclient = []\n");
        symlink(outside.path(), dir.path().join("linked")).unwrap();
        assert!(
            format!("{:#}", config.validate_folders(dir.path()).unwrap_err())
                .contains("inside the project")
        );

        let (dir, config) = configured("[events]\nserver = ['real', 'linked']\nclient = []\n");
        fs::create_dir(dir.path().join("real")).unwrap();
        symlink(dir.path().join("real"), dir.path().join("linked")).unwrap();
        assert!(
            format!("{:#}", config.validate_folders(dir.path()).unwrap_err())
                .contains("already listed")
        );
    }
}
