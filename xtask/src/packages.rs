use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Component, Path};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use flate2::read::GzDecoder;

use crate::{output, run, versions};

#[derive(Clone, Copy)]
enum Registry {
    Pesde,
    Wally,
}

impl Registry {
    fn name(self) -> &'static str {
        match self {
            Self::Pesde => "pesde",
            Self::Wally => "wally",
        }
    }
    fn archive(self) -> &'static str {
        match self {
            Self::Pesde => "pesde.tar.gz",
            Self::Wally => "wally.zip",
        }
    }

    /// Resolve the registry's current download endpoint using its public index configuration.
    fn url(self, version: &str) -> Result<String> {
        let dir = tempfile::tempdir()?;
        let config = dir.path().join("config");
        let (url, name) = match self {
            Self::Pesde => (
                "https://raw.githubusercontent.com/pesde-pkg/index/main/config.toml",
                "rogrid%2Frogrid",
            ),
            Self::Wally => (
                "https://raw.githubusercontent.com/UpliftGames/wally-index/main/config.json",
                "rogrid-hq/rogrid",
            ),
        };
        ensure!(
            download(url, &config)? == 200,
            "could not read {} registry configuration",
            self.name()
        );
        let text = fs::read_to_string(config)?;
        let url = match self {
            Self::Pesde => {
                let config: toml::Value = toml::from_str(&text)?;
                let api = config["api"]
                    .as_str()
                    .context("missing Pesde API")?
                    .trim_end_matches('/');
                config.get("download").and_then(|s| s.as_str())
                    .unwrap_or("{API_URL}/v1/packages/{PACKAGE}/{PACKAGE_VERSION}/{PACKAGE_TARGET}/archive")
                    .replace("{API_URL}", api).replace("{PACKAGE}", name)
                    .replace("{PACKAGE_VERSION}", version).replace("{PACKAGE_TARGET}", "roblox")
            }
            Self::Wally => {
                let config: serde_json::Value = serde_json::from_str(&text)?;
                let api = config["api"]
                    .as_str()
                    .context("missing Wally API")?
                    .trim_end_matches('/');
                format!("{api}/v1/package-contents/{name}/{version}")
            }
        };
        ensure!(
            url.starts_with("https://"),
            "registry download must use HTTPS"
        );
        Ok(url)
    }
}

const REGISTRIES: [Registry; 2] = [Registry::Pesde, Registry::Wally];

#[cfg(test)]
#[path = "packages_tests.rs"]
mod workflow_tests;

/// Public, unauthenticated downloads only. HTTP errors must not look like missing versions.
fn download(url: &str, destination: &Path) -> Result<u16> {
    let status = output(
        Command::new("curl")
            .args([
                "--silent",
                "--show-error",
                "--location",
                "--proto",
                "=https",
                "--proto-redir",
                "=https",
                "--connect-timeout",
                "15",
                "--max-time",
                "90",
                "--retry",
                "2",
                "--max-filesize",
                "10485760",
                "--header",
                "Accept: application/octet-stream",
                "--header",
                "Wally-Version: 0.3.2",
                "--output",
            ])
            .arg(destination)
            .args(["--write-out", "%{http_code}", url]),
    )?;
    status.trim().parse().context("invalid HTTP status")
}

type Contents = BTreeMap<String, Vec<u8>>;

fn insert_file(contents: &mut Contents, path: &Path, bytes: Vec<u8>) -> Result<()> {
    ensure!(
        !path.is_absolute()
            && path
                .components()
                .all(|c| matches!(c, Component::Normal(_) | Component::CurDir)),
        "unsafe archive path"
    );
    let name = path
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    ensure!(!name.is_empty(), "empty archive path");
    let bytes = if matches!(name.as_str(), "pesde.toml" | "wally.toml") {
        // Manifest formatting and key order can differ between packaging tools.
        let value: toml::Value = toml::from_str(std::str::from_utf8(&bytes)?)?;
        serde_json::to_vec(&value)?
    } else if name == "LICENSE"
        || [".md", ".luau", ".json"]
            .iter()
            .any(|ext| name.ends_with(ext))
    {
        // Git's Windows checkout conversion must not make an identical release conflict.
        std::str::from_utf8(&bytes)?
            .replace("\r\n", "\n")
            .into_bytes()
    } else {
        bytes
    };
    ensure!(
        contents.insert(name.clone(), bytes).is_none(),
        "duplicate archive entry {name}"
    );
    Ok(())
}

fn read_file(reader: impl Read) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    reader.take(8 * 1024 * 1024 + 1).read_to_end(&mut data)?;
    ensure!(
        data.len() <= 8 * 1024 * 1024,
        "archive file exceeds size limit"
    );
    Ok(data)
}

fn contents(path: &Path) -> Result<Contents> {
    let bytes = fs::read(path)?;
    let mut result = BTreeMap::new();
    if bytes.starts_with(b"PK") {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
        ensure!(archive.len() <= 256, "too many package entries");
        for i in 0..archive.len() {
            let file = archive.by_index(i)?;
            if file.is_dir() {
                continue;
            }
            ensure!(!file.is_symlink(), "package contains a symlink");
            let name = file.enclosed_name().context("invalid zip entry")?;
            insert_file(&mut result, &name, read_file(file)?)?;
        }
    } else {
        let mut archive = tar::Archive::new(GzDecoder::new(Cursor::new(bytes)));
        for (i, entry) in archive.entries()?.enumerate() {
            ensure!(i < 256, "too many package entries");
            let file = entry?;
            if file.header().entry_type().is_dir() {
                continue;
            }
            ensure!(
                file.header().entry_type().is_file(),
                "package contains a non-regular file"
            );
            let name = file.path()?.into_owned();
            insert_file(&mut result, &name, read_file(file)?)?;
        }
    }
    for required in ["src/init.luau", "src/runtime.luau", "README.md", "LICENSE"] {
        ensure!(
            result.contains_key(required),
            "package is missing {required}"
        );
    }
    Ok(result)
}

fn compare(expected: &Contents, actual: &Contents) -> Result<()> {
    let differences: Vec<_> = expected
        .keys()
        .chain(actual.keys())
        .filter(|name| expected.get(*name) != actual.get(*name))
        .collect();
    ensure!(
        differences.is_empty(),
        "published package conflicts with this release: {differences:?}"
    );
    Ok(())
}

pub fn bundle(root: &Path, destination: &Path) -> Result<()> {
    versions::check(root, None)?;
    fs::create_dir_all(destination)?;
    let destination = fs::canonicalize(destination)?;
    let package = root.join("packages/rogrid");
    run(Command::new("pesde")
        .current_dir(&package)
        .args(["publish", "--dry-run", "--yes"]))?;
    fs::copy(
        package.join("package.tar.gz"),
        destination.join(Registry::Pesde.archive()),
    )?;
    run(Command::new("wally")
        .current_dir(&package)
        .args(["package", "--output"])
        .arg(destination.join(Registry::Wally.archive())))?;
    for registry in REGISTRIES {
        contents(&destination.join(registry.archive()))?;
    }
    Ok(())
}

fn existing(url: &str, expected: &Contents, path: &Path) -> Result<bool> {
    check_response(download(url, path)?, expected, path)
}

fn check_response(status: u16, expected: &Contents, path: &Path) -> Result<bool> {
    match status {
        200 => {
            compare(expected, &contents(path)?)?;
            Ok(true)
        }
        404 => Ok(false),
        status => {
            bail!("registry returned HTTP {status}; refusing to assume the version is missing")
        }
    }
}

fn login(registry: Registry, package: &Path) -> Result<()> {
    let variable = match registry {
        Registry::Pesde => "PESDE_TOKEN",
        Registry::Wally => "WALLY_TOKEN",
    };
    let token = std::env::var(variable).with_context(|| format!("missing secret {variable}"))?;
    ensure!(!token.is_empty(), "empty secret {variable}");
    let mut command = Command::new(registry.name());
    command.current_dir(package);
    if matches!(registry, Registry::Pesde) {
        command.arg("auth");
    }
    // Auth output is discarded, including error output. Never log token-bearing arguments.
    let status = command
        .args(["login", "--token"])
        .arg(token)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    ensure!(
        status.success(),
        "{} authentication failed",
        registry.name()
    );
    Ok(())
}

pub fn verify(root: &Path, artifacts: &Path) -> Result<()> {
    let version = versions::check(root, None)?.runtime.to_string();
    let dir = tempfile::tempdir()?;
    for registry in REGISTRIES {
        let expected = contents(&artifacts.join(registry.archive()))?;
        ensure!(
            existing(
                &registry.url(&version)?,
                &expected,
                &dir.path().join(registry.archive())
            )?,
            "{} {version} is not available",
            registry.name()
        );
        println!("Verified {} runtime {version}", registry.name());
    }
    Ok(())
}

pub fn publish(root: &Path, artifacts: &Path) -> Result<()> {
    ensure!(
        std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true"),
        "publishing is reserved for the protected GitHub workflow"
    );
    let version = versions::check(root, None)?.runtime.to_string();
    let dir = tempfile::tempdir()?;
    let package = root.join("packages/rogrid");
    let repacked = dir.path().join("repacked");
    bundle(root, &repacked)?;
    let mut pending = Vec::new();
    // Check BOTH registries before the first upload, including conflicting existing versions.
    for registry in REGISTRIES {
        let expected = contents(&artifacts.join(registry.archive()))?;
        compare(&expected, &contents(&repacked.join(registry.archive()))?)?;
        let url = registry.url(&version)?;
        if existing(&url, &expected, &dir.path().join(registry.archive()))? {
            println!(
                "{} {version} already matches; skipping upload",
                registry.name()
            );
        } else {
            pending.push((registry, url, expected));
        }
    }
    // Authenticate all missing destinations before uploading to either one.
    for (registry, _, _) in &pending {
        login(*registry, &package)?;
    }
    for (registry, url, expected) in pending {
        let mut command = Command::new(registry.name());
        command
            .current_dir(&package)
            .arg("publish")
            .stdin(Stdio::null());
        if matches!(registry, Registry::Pesde) {
            command.arg("--yes");
        }
        run(&mut command)?;
        // Wally 0.3.2 can return exit code zero for an HTTP failure. Check the registry.
        let mut available = false;
        for attempt in 0..12 {
            if attempt > 0 {
                std::thread::sleep(Duration::from_secs(5));
            }
            if existing(&url, &expected, &dir.path().join(registry.archive()))? {
                available = true;
                break;
            }
        }
        ensure!(
            available,
            "{} {version} could not be verified after publishing; rerun this release to retry",
            registry.name()
        );
    }
    Ok(())
}

pub fn smoke(cli: &Path) -> Result<()> {
    let cli = fs::canonicalize(cli)?;
    let dir = tempfile::tempdir()?;
    for manager in ["pesde", "wally"] {
        run(Command::new(&cli)
            .current_dir(dir.path())
            .args([
                "init",
                manager,
                "--package-manager",
                manager,
                "--tool-manager",
                "none",
            ])
            .stdin(Stdio::null()))?;
        let project = dir.path().join(manager);
        run(Command::new(&cli)
            .current_dir(&project)
            .args(["dev", "--once"]))?;
        run(Command::new("rojo")
            .current_dir(&project)
            .args(["build", "--output"])
            .arg(dir.path().join(format!("{manager}.rbxl"))))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_payloads_not_archive_metadata_or_checkout_line_endings() {
        let mut a = Contents::new();
        let mut b = Contents::new();
        insert_file(
            &mut a,
            Path::new("./src/init.luau"),
            b"return {}\r\n".to_vec(),
        )
        .unwrap();
        insert_file(&mut b, Path::new("src/init.luau"), b"return {}\n".to_vec()).unwrap();
        insert_file(
            &mut a,
            Path::new("pesde.toml"),
            b"version = '1.0.0'\nname = 'a/b'\n".to_vec(),
        )
        .unwrap();
        insert_file(
            &mut b,
            Path::new("pesde.toml"),
            b"name = 'a/b'\nversion = '1.0.0'\n".to_vec(),
        )
        .unwrap();
        compare(&a, &b).unwrap();
        b.insert("src/init.luau".into(), b"return 1\n".to_vec());
        assert!(compare(&a, &b).is_err());
        b.remove("src/init.luau");
        assert!(compare(&a, &b).is_err());
    }

    #[test]
    fn rejects_unsafe_and_duplicate_archive_paths() {
        let mut contents = Contents::new();
        assert!(insert_file(&mut contents, Path::new("../secret"), vec![]).is_err());
        insert_file(&mut contents, Path::new("a"), vec![]).unwrap();
        assert!(insert_file(&mut contents, Path::new("./a"), vec![]).is_err());
    }

    #[test]
    fn authentication_and_server_failures_are_not_treated_as_missing_versions() {
        let expected = Contents::new();
        let absent = Path::new("does-not-exist.zip");
        assert!(!check_response(404, &expected, absent).unwrap());
        for status in [401, 403, 429, 500, 503] {
            assert!(check_response(status, &expected, absent).is_err());
        }
        // An HTTP success with a missing or corrupt archive is also a failure.
        assert!(check_response(200, &expected, absent).is_err());
    }

    #[test]
    fn existing_identical_archive_is_skipped_but_conflicting_version_is_rejected() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("package.zip");
        let mut writer = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
        for name in ["src/init.luau", "src/runtime.luau", "README.md", "LICENSE"] {
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"original\n").unwrap();
        }
        writer.finish().unwrap();
        let mut expected = contents(&archive).unwrap();
        assert!(check_response(200, &expected, &archive).unwrap());
        expected.insert("src/runtime.luau".into(), b"changed\n".to_vec());
        assert!(check_response(200, &expected, &archive).is_err());
    }
}
