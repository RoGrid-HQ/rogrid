use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};
use clap::ValueEnum;
use semver::Version;
use toml_edit::{DocumentMut, value};

use crate::output;

const CLI: &str = "crates/rogrid/Cargo.toml";
const PESDE: &str = "packages/rogrid/pesde.toml";
const WALLY: &str = "packages/rogrid/wally.toml";
const RUNTIME: &str = "packages/rogrid/src/init.luau";
const TOOLS: &str = "crates/rogrid/src/tools/mod.rs";
const VERSION_LINE: &str = "RoGrid.version = ";
const PIN_LINE: &str = "pub const FRAMEWORK_VERSION: &str = ";

pub struct Versions {
    pub cli: Version,
    pub runtime: Version,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Bump {
    Keep,
    Patch,
    Minor,
    Major,
}

impl Bump {
    fn apply(self, version: &Version) -> Result<Version> {
        ensure!(
            version.pre.is_empty() && version.build.is_empty(),
            "prereleases require a separate release policy"
        );
        let mut next = version.clone();
        match self {
            Self::Keep => {}
            Self::Patch => next.patch = next.patch.checked_add(1).context("version overflow")?,
            Self::Minor => {
                next.minor = next.minor.checked_add(1).context("version overflow")?;
                next.patch = 0;
            }
            Self::Major => {
                next.major = next.major.checked_add(1).context("version overflow")?;
                next.minor = 0;
                next.patch = 0;
            }
        }
        Ok(next)
    }
}

fn manifest(root: &Path, file: &str) -> Result<DocumentMut> {
    fs::read_to_string(root.join(file))?
        .parse()
        .with_context(|| format!("invalid {file}"))
}

fn read_version(value: Option<&str>) -> Result<Version> {
    value
        .context("missing version")?
        .parse()
        .context("invalid version")
}

fn source_version(source: &str, prefix: &str) -> Result<Version> {
    let mut lines = source.lines().filter_map(|line| line.strip_prefix(prefix));
    let line = lines.next().context("missing version assignment")?;
    ensure!(lines.next().is_none(), "duplicate version assignment");
    read_version(line.strip_prefix('"').and_then(|s| s.split('"').next()))
}

pub fn check(root: &Path, base: Option<&str>) -> Result<Versions> {
    let cli = read_version(manifest(root, CLI)?["package"]["version"].as_str())?;
    let runtime = read_version(manifest(root, PESDE)?["version"].as_str())?;
    for (file, version) in [
        (
            WALLY,
            read_version(manifest(root, WALLY)?["package"]["version"].as_str())?,
        ),
        (
            "packages/rogrid/pesde.lock",
            read_version(manifest(root, "packages/rogrid/pesde.lock")?["version"].as_str())?,
        ),
        (
            RUNTIME,
            source_version(&fs::read_to_string(root.join(RUNTIME))?, VERSION_LINE)?,
        ),
        (
            TOOLS,
            source_version(&fs::read_to_string(root.join(TOOLS))?, PIN_LINE)?,
        ),
    ] {
        ensure!(
            version == runtime,
            "{file} has {version}; expected runtime {runtime}"
        );
    }
    let lock = manifest(root, "Cargo.lock")?;
    let packages = lock["package"]
        .as_array_of_tables()
        .context("invalid Cargo.lock")?;
    let package = packages
        .iter()
        .find(|p| p["name"].as_str() == Some("rogrid"))
        .context("rogrid missing from Cargo.lock")?;
    ensure!(
        read_version(package["version"].as_str())? == cli,
        "Cargo.lock does not match CLI {cli}"
    );
    if let Some(base) = base {
        ensure!(!base.starts_with('-'), "invalid base revision");
        let old_cli: toml::Value =
            toml::from_str(&git(root, &["show", &format!("{base}:{CLI}")])?)?;
        let old_runtime: toml::Value =
            toml::from_str(&git(root, &["show", &format!("{base}:{PESDE}")])?)?;
        ensure!(
            cli > read_version(old_cli["package"]["version"].as_str())?,
            "CLI version must increase"
        );
        let old_runtime = read_version(old_runtime["version"].as_str())?;
        ensure!(runtime >= old_runtime, "runtime version cannot decrease");
        if runtime == old_runtime {
            ensure!(
                git(
                    root,
                    &["diff", "--name-only", base, "--", "packages/rogrid"]
                )?
                .trim()
                .is_empty(),
                "runtime package files changed; bump the runtime version too"
            );
        }
    }
    Ok(Versions { cli, runtime })
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    output(Command::new("git").current_dir(root).args(args))
}

fn replace_assignment(source: &str, prefix: &str, version: &Version) -> Result<String> {
    let previous = source_version(source, prefix)?;
    let from = format!("{prefix}\"{previous}\"");
    Ok(source.replacen(&from, &format!("{prefix}\"{version}\""), 1))
}

/// Compute every edit before writing, so an unexpected file layout fails without partial edits.
fn edits(
    root: &Path,
    cli_bump: Bump,
    runtime_bump: Bump,
) -> Result<(Versions, Vec<(String, String)>)> {
    let current = check(root, None)?;
    let next = Versions {
        cli: cli_bump.apply(&current.cli)?,
        runtime: runtime_bump.apply(&current.runtime)?,
    };
    ensure!(
        next.cli > current.cli,
        "choose a CLI bump; every release updates the CLI"
    );
    let mut changes = Vec::new();
    for (file, table, version) in [
        (CLI, Some("package"), &next.cli),
        (PESDE, None, &next.runtime),
        (WALLY, Some("package"), &next.runtime),
        ("packages/rogrid/pesde.lock", None, &next.runtime),
    ] {
        let mut doc = manifest(root, file)?;
        let field = match table {
            Some(table) => &mut doc[table]["version"],
            None => &mut doc["version"],
        };
        *field = value(version.to_string());
        changes.push((file.to_string(), doc.to_string()));
    }
    let mut lock = manifest(root, "Cargo.lock")?;
    for package in lock["package"]
        .as_array_of_tables_mut()
        .context("invalid Cargo.lock")?
        .iter_mut()
    {
        if package["name"].as_str() == Some("rogrid") {
            package["version"] = value(next.cli.to_string());
        }
    }
    changes.push(("Cargo.lock".into(), lock.to_string()));
    for (file, prefix) in [(RUNTIME, VERSION_LINE), (TOOLS, PIN_LINE)] {
        changes.push((
            file.into(),
            replace_assignment(&fs::read_to_string(root.join(file))?, prefix, &next.runtime)?,
        ));
    }
    Ok((next, changes))
}

pub fn prepare(root: &Path, cli: Bump, runtime: Bump, since: Option<String>) -> Result<()> {
    ensure!(
        git(root, &["status", "--porcelain"])?.trim().is_empty(),
        "commit or stash local changes before preparing a release"
    );
    let current = check(root, None)?;
    let since = since.unwrap_or_else(|| format!("v{}", current.cli));
    ensure!(!since.starts_with('-'), "invalid previous tag");
    git(root, &["merge-base", "--is-ancestor", &since, "HEAD"])?;
    if matches!(runtime, Bump::Keep)
        && !git(
            root,
            &["diff", "--name-only", &since, "--", "packages/rogrid"],
        )?
        .trim()
        .is_empty()
    {
        bail!("runtime package files changed since {since}; choose a runtime bump");
    }
    let (next, mut changes) = edits(root, cli, runtime)?;
    let notes_file = format!("releases/v{}.md", next.cli);
    ensure!(
        !root.join(&notes_file).exists(),
        "release notes already exist for {}",
        next.cli
    );
    let log = git(
        root,
        &[
            "log",
            "--reverse",
            "--format=- %s (%h)",
            &format!("{since}..HEAD"),
        ],
    )?;
    changes.push((
        notes_file,
        format!(
            "# RoGrid CLI {}\n\nRuntime: {} (Pesde and Wally).\n\n## Changes\n\n{}\n",
            next.cli,
            next.runtime,
            log.trim()
        ),
    ));
    for (file, contents) in changes {
        let path = root.join(file);
        fs::create_dir_all(path.parent().context("file has no parent")?)?;
        fs::write(path, contents)?;
    }
    check(root, Some(&since))?;
    println!(
        "Prepared CLI {} and runtime {}. Review the diff and releases/v{}.md.",
        next.cli, next.runtime, next.cli
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (file, text) in [
            (
                CLI,
                "[package]\nname = \"rogrid\"\nversion = \"0.4.0\"\n[dependencies]\nother = \"0.4.0\"\n",
            ),
            (PESDE, "name = \"rogrid/rogrid\"\nversion = \"0.2.1\"\n"),
            (
                WALLY,
                "[package]\nname = \"rogrid-hq/rogrid\"\nversion = \"0.2.1\"\n",
            ),
            ("packages/rogrid/pesde.lock", "version = \"0.2.1\"\n"),
            (
                "Cargo.lock",
                "[[package]]\nname = \"other\"\nversion = \"0.4.0\"\n[[package]]\nname = \"rogrid\"\nversion = \"0.4.0\"\n",
            ),
            (
                RUNTIME,
                "RoGrid.version = \"0.2.1\"\nRoGrid._protocol = 2\n",
            ),
            (TOOLS, "pub const FRAMEWORK_VERSION: &str = \"0.2.1\";\n"),
        ] {
            let path = dir.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        dir
    }

    #[test]
    fn bumps_only_owned_versions_and_preserves_protocol_and_dependencies() {
        let dir = fixture();
        let (next, changes) = edits(dir.path(), Bump::Minor, Bump::Patch).unwrap();
        assert_eq!(next.cli.to_string(), "0.5.0");
        assert_eq!(next.runtime.to_string(), "0.2.2");
        for (file, text) in changes {
            fs::write(dir.path().join(file), text).unwrap();
        }
        check(dir.path(), None).unwrap();
        assert_eq!(
            manifest(dir.path(), CLI).unwrap()["dependencies"]["other"].as_str(),
            Some("0.4.0")
        );
        assert!(
            fs::read_to_string(dir.path().join(RUNTIME))
                .unwrap()
                .contains("RoGrid._protocol = 2")
        );
        assert_eq!(
            manifest(dir.path(), "Cargo.lock").unwrap()["package"][0]["version"].as_str(),
            Some("0.4.0")
        );
    }

    #[test]
    fn cli_only_release_keeps_runtime_and_inconsistent_metadata_fails_before_edits() {
        let dir = fixture();
        let (next, _) = edits(dir.path(), Bump::Patch, Bump::Keep).unwrap();
        assert_eq!(next.runtime.to_string(), "0.2.1");
        fs::write(dir.path().join(RUNTIME), "RoGrid.version = \"9.0.0\"\n").unwrap();
        assert!(edits(dir.path(), Bump::Patch, Bump::Patch).is_err());
        assert_eq!(
            manifest(dir.path(), CLI).unwrap()["package"]["version"].as_str(),
            Some("0.4.0")
        );
    }

    #[test]
    fn semver_bumps_reset_lower_components_and_reject_unhandled_prereleases() {
        let version = Version::parse("1.2.3").unwrap();
        for (bump, expected) in [
            (Bump::Keep, "1.2.3"),
            (Bump::Patch, "1.2.4"),
            (Bump::Minor, "1.3.0"),
            (Bump::Major, "2.0.0"),
        ] {
            assert_eq!(bump.apply(&version).unwrap().to_string(), expected);
        }
        assert!(
            Bump::Patch
                .apply(&Version::parse("1.0.0-beta.1").unwrap())
                .is_err()
        );
        assert!(replace_assignment("no version here", VERSION_LINE, &version).is_err());
    }

    fn git_fixture() -> tempfile::TempDir {
        let dir = fixture();
        for args in [
            vec!["init", "-q"],
            vec!["config", "user.name", "Release test"],
            vec!["config", "user.email", "release-test@example.invalid"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["config", "tag.gpgsign", "false"],
            vec!["config", "core.autocrlf", "false"],
            vec!["config", "core.hooksPath", ".git/no-hooks"],
            vec!["add", "."],
            vec!["commit", "-qm", "Initial release"],
            vec!["tag", "v0.4.0"],
        ] {
            git(dir.path(), &args).unwrap();
        }
        dir
    }

    #[test]
    fn prepare_creates_a_consistent_reviewable_diff_without_committing() {
        let dir = git_fixture();
        let original = git(dir.path(), &["rev-parse", "HEAD"]).unwrap();
        prepare(dir.path(), Bump::Minor, Bump::Patch, None).unwrap();
        let next = check(dir.path(), Some("v0.4.0")).unwrap();
        assert_eq!(next.cli.to_string(), "0.5.0");
        assert_eq!(next.runtime.to_string(), "0.2.2");
        assert!(dir.path().join("releases/v0.5.0.md").is_file());
        assert_eq!(git(dir.path(), &["rev-parse", "HEAD"]).unwrap(), original);
        // A second preparation must not overwrite unreviewed edits.
        assert!(prepare(dir.path(), Bump::Patch, Bump::Keep, None).is_err());
    }

    #[test]
    fn keeping_runtime_with_changed_package_files_fails_without_version_edits() {
        let dir = git_fixture();
        fs::write(dir.path().join("packages/rogrid/README.md"), "New docs\n").unwrap();
        git(dir.path(), &["add", "."]).unwrap();
        git(dir.path(), &["commit", "-qm", "Update runtime docs"]).unwrap();
        assert!(prepare(dir.path(), Bump::Patch, Bump::Keep, None).is_err());
        assert_eq!(check(dir.path(), None).unwrap().cli.to_string(), "0.4.0");
        assert!(!dir.path().join("releases").exists());
    }
}
