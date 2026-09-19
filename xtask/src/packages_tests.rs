//! Exercise the real publisher in an isolated subprocess with inert native tools.
//! Neither credentials nor network access are inherited by the stand-ins.
use super::*;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

fn executable(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

fn tool() -> &'static [u8] {
    static BINARY: OnceLock<Vec<u8>> = OnceLock::new();
    BINARY.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        let target = executable(dir.path(), "tool");
        let result = Command::new("rustc")
            .args(["--edition=2024"])
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/publish_tool.rs"))
            .arg("-o")
            .arg(&target)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        fs::read(target).unwrap()
    })
}

struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        for file in [
            "crates/rogrid/Cargo.toml",
            "packages/rogrid/pesde.toml",
            "packages/rogrid/wally.toml",
            "packages/rogrid/pesde.lock",
            "Cargo.lock",
            "packages/rogrid/src/init.luau",
            "crates/rogrid/src/tools/mod.rs",
        ] {
            let target = root.path().join(file);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::copy(repo.join(file), target).unwrap();
        }
        for folder in ["bin", "artifacts", "fixtures", "home"] {
            fs::create_dir(root.path().join(folder)).unwrap();
        }
        for name in ["curl", "pesde", "wally"] {
            let path = executable(&root.path().join("bin"), name);
            fs::write(&path, tool()).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
        // Windows searches the executable's directory before System32 (which
        // contains a real curl). Keep the child runner beside all inert tools.
        fs::copy(
            std::env::current_exe().unwrap(),
            executable(&root.path().join("bin"), "test-runner"),
        )
        .unwrap();
        let files = [
            "src/init.luau",
            "src/runtime.luau",
            "src/validate.luau",
            "README.md",
            "LICENSE",
        ];
        for changed in [false, true] {
            let zip_path = root.path().join(if changed {
                "fixtures/conflict.zip"
            } else {
                "fixtures/wally.zip"
            });
            let mut zip = zip::ZipWriter::new(fs::File::create(zip_path).unwrap());
            for file in files {
                zip.start_file(file, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(if changed { b"changed\n" } else { b"original\n" })
                    .unwrap();
            }
            zip.finish().unwrap();
        }
        let gzip = flate2::write::GzEncoder::new(
            fs::File::create(root.path().join("fixtures/pesde.tar.gz")).unwrap(),
            flate2::Compression::default(),
        );
        let mut tar = tar::Builder::new(gzip);
        for file in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(9);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, file, &b"original\n"[..])
                .unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap();
        for file in ["wally.zip", "pesde.tar.gz"] {
            fs::copy(
                root.path().join("fixtures").join(file),
                root.path().join("artifacts").join(file),
            )
            .unwrap();
        }
        Self { root }
    }

    fn run(&self, scenario: &str, protected: bool) -> std::process::Output {
        let log = self.root.path().join("commands.log");
        fs::write(&log, "").unwrap();
        let stdout = self.root.path().join("stdout");
        let stderr = self.root.path().join("stderr");
        let mut child = Command::new(executable(&self.root.path().join("bin"), "test-runner"))
            .current_dir(self.root.path().join("bin"))
            .args([
                "--exact",
                "packages::workflow_tests::publish_child",
                "--nocapture",
            ])
            .env("ROGRID_PUBLISH_TEST_ROOT", self.root.path())
            .env("ROGRID_PUBLISH_SCENARIO", scenario)
            .env("GITHUB_ACTIONS", if protected { "true" } else { "false" })
            .env("PATH", self.root.path().join("bin"))
            .env("HOME", self.root.path().join("home"))
            .env("USERPROFILE", self.root.path().join("home"))
            .env("PESDE_TOKEN", "inert-test-token")
            .env("WALLY_TOKEN", "inert-test-token")
            .stdout(fs::File::create(&stdout).unwrap())
            .stderr(fs::File::create(&stderr).unwrap())
            .stdin(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(90);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("publisher test timed out");
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let output = std::process::Output {
            status,
            stdout: fs::read(stdout).unwrap(),
            stderr: fs::read(stderr).unwrap(),
        };
        assert!(!String::from_utf8_lossy(&output.stdout).contains("inert-test-token"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("inert-test-token"));
        output
    }

    fn log(&self) -> Vec<String> {
        fs::read_to_string(self.root.path().join("commands.log"))
            .unwrap()
            .lines()
            .map(Into::into)
            .collect()
    }
}

#[test]
fn publish_child() {
    let Some(root) = std::env::var_os("ROGRID_PUBLISH_TEST_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    publish(&root, &root.join("artifacts")).unwrap();
}

#[test]
fn publishing_requires_the_protected_workflow_before_running_tools() {
    let f = Fixture::new();
    let result = f.run("success", false);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("protected GitHub workflow"));
    assert!(f.log().is_empty());
}

#[test]
fn checks_both_registries_and_authenticates_both_before_uploading() {
    let f = Fixture::new();
    let result = f.run("success", true);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let log = f.log();
    let index = |entry| log.iter().position(|line| line == entry).unwrap();
    assert!(index("check wally") < index("login pesde"));
    assert!(index("login wally") < index("publish pesde"));
    assert!(index("publish pesde") < index("publish wally"));
    let result = f.run("success", true);
    assert!(result.status.success());
    assert!(
        f.log()
            .iter()
            .all(|line| !line.starts_with("publish ") && !line.starts_with("login "))
    );
}

#[test]
fn registry_conflicts_http_errors_and_auth_failures_prevent_all_uploads() {
    for (scenario, expected) in [
        ("conflict", "conflicts with this release"),
        ("http-error", "registry returned HTTP 503"),
        ("auth-error", "authentication failed"),
    ] {
        let f = Fixture::new();
        let result = f.run(scenario, true);
        assert!(!result.status.success(), "{scenario}");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(expected),
            "{scenario}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            f.log().iter().all(|line| !line.starts_with("publish ")),
            "{scenario}"
        );
    }
}

#[test]
fn artifacts_must_match_the_checked_out_source_before_uploading() {
    let f = Fixture::new();
    fs::copy(
        f.root.path().join("fixtures/conflict.zip"),
        f.root.path().join("artifacts/wally.zip"),
    )
    .unwrap();
    assert!(!f.run("success", true).status.success());
    assert!(
        f.log()
            .iter()
            .all(|line| !line.starts_with("publish ") && !line.starts_with("login "))
    );
}

#[test]
fn retry_after_partial_publication_skips_the_completed_registry() {
    let f = Fixture::new();
    assert!(!f.run("publish-error", true).status.success());
    assert!(f.root.path().join("pesde.published").exists());
    assert!(!f.root.path().join("wally.published").exists());
    let result = f.run("success", true);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!f.log().contains(&"publish pesde".into()));
    assert!(f.log().contains(&"publish wally".into()));
}

#[test]
fn verification_retries_until_the_uploaded_package_is_available() {
    let f = Fixture::new();
    let result = f.run("delayed", true);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        f.log()
            .iter()
            .filter(|line| line.as_str() == "check pesde")
            .count(),
        3
    );
}

#[test]
fn verification_timeout_rejects_a_success_exit_without_a_published_package() {
    let f = Fixture::new();
    let result = f.run("never-visible", true);
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("could not be verified after publishing")
    );
    assert!(!f.log().contains(&"publish wally".into()));
}
