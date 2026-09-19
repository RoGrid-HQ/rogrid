use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;

use tempfile::TempDir;

fn executable(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

fn tool_binary() -> &'static [u8] {
    static COMPILED: OnceLock<Vec<u8>> = OnceLock::new();
    COMPILED.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/tool.rs");
        let output = Command::new("rustc")
            .arg("--edition=2024")
            .arg(source)
            .arg("-o")
            .arg(executable(dir.path(), "tool"))
            .output()
            .expect("compile test tools with rustc");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        fs::read(executable(dir.path(), "tool")).unwrap()
    })
}

/// Each CLI process has its own home, PATH and working directory. Tests never
/// mutate the parent process environment or invoke the user's installed tools.
pub struct Sandbox {
    pub root: TempDir,
    pub work: PathBuf,
    pub bin: PathBuf,
    pub home: PathBuf,
    pub local_framework: PathBuf,
}

impl Sandbox {
    pub fn new() -> Self {
        let root = tempfile::Builder::new()
            .prefix("rogrid init ")
            .tempdir()
            .unwrap();
        let work = root.path().join("work");
        let bin = root.path().join("bin");
        let home = root.path().join("home");
        let local_framework = root.path().join("local framework");
        for path in [&work, &bin, &home, &local_framework.join("src")] {
            fs::create_dir_all(path).unwrap();
        }
        fs::write(local_framework.join("src/init.luau"), "return {}\n").unwrap();
        Self {
            root,
            work,
            bin,
            home,
            local_framework,
        }
    }

    pub fn provide(&self, name: &str) {
        let path = executable(&self.bin, name);
        fs::write(&path, tool_binary()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    pub fn command(&self, pm: &str, tm: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_rogrid"));
        command
            .current_dir(&self.work)
            .stdin(Stdio::null())
            .env("PATH", &self.bin)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("ROGRID_TEST_HOME", &self.home)
            .env("ROGRID_TEST_PM", pm)
            .env("ROGRID_TEST_LOG", self.root.path().join("commands.log"))
            .env_remove("ROGRID_TEST_FAIL")
            .args(["init", "--package-manager", pm, "--tool-manager", tm]);
        command
    }

    pub fn log(&self) -> Vec<(String, PathBuf)> {
        fs::read_to_string(self.root.path().join("commands.log"))
            .unwrap_or_default()
            .lines()
            .map(|line| {
                let (command, path) = line.split_once('\t').unwrap();
                (command.to_string(), PathBuf::from(path))
            })
            .collect()
    }

    pub fn commands(&self) -> Vec<String> {
        self.log().into_iter().map(|(command, _)| command).collect()
    }
}

pub fn text(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path).unwrap()
}

pub fn success(output: Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    stdout.into_owned()
}

pub fn failure(output: Output, expected: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "unexpected success: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains(expected),
        "expected {expected:?} in:\n{stderr}"
    );
}
