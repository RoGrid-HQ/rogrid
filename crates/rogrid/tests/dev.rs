#[allow(dead_code)]
mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use support::{Sandbox, failure, success, text};

struct Project {
    sandbox: Sandbox,
    root: PathBuf,
    map: PathBuf,
}

impl Project {
    fn new() -> Self {
        let sandbox = Sandbox::new();
        sandbox.provide("wally");
        sandbox.provide("rojo");
        success(
            sandbox
                .command("wally", "none")
                .arg("game")
                .output()
                .unwrap(),
        );
        let root = sandbox.work.join("game");
        let map = sandbox.root.path().join("sourcemap.json");
        let mut source: serde_json::Value =
            serde_json::from_str(include_str!("support/sourcemap.json")).unwrap();
        let events = source
            .pointer_mut("/children/1/children/0/children/0/children")
            .unwrap()
            .as_array_mut()
            .unwrap();
        events[0]["filePaths"] = serde_json::json!([
            "src/server/events/Lobby.luau",
            "src/server/events/Lobby.lua"
        ]);
        events.push(serde_json::json!({"name": "Extra", "className": "ModuleScript", "filePaths": ["src/server/events/Extra.luau"]}));
        fs::write(&map, serde_json::to_string(&source).unwrap()).unwrap();
        Self { sandbox, root, map }
    }

    fn dev(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_rogrid"));
        command
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .env("PATH", &self.sandbox.bin)
            .env("HOME", &self.sandbox.home)
            .env("USERPROFILE", &self.sandbox.home)
            .env(
                "ROGRID_TEST_LOG",
                self.sandbox.root.path().join("commands.log"),
            )
            .env("ROGRID_TEST_SOURCEMAP", &self.map)
            .env("ROGRID_TEST_CONTROL", self.sandbox.root.path())
            .arg("dev");
        command
    }

    fn once(&self) -> String {
        success(self.dev().arg("--once").output().unwrap())
    }
    fn revision(&self) -> PathBuf {
        self.root.join(".rogrid/generated/shared/Revision.luau")
    }
    fn caller(&self) -> PathBuf {
        self.root.join(".rogrid/generated/shared/Server.luau")
    }
    fn lobby(&self) -> PathBuf {
        self.root.join("src/server/events/Lobby.luau")
    }
}

fn wait_for(description: &str, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {description}"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

struct Watching {
    child: Child,
    control: PathBuf,
}

impl Watching {
    fn start(project: &Project) -> Self {
        let mut command = project.dev();
        command
            .stdout(fs::File::create(project.sandbox.root.path().join("dev.out")).unwrap())
            .stderr(fs::File::create(project.sandbox.root.path().join("dev.err")).unwrap());
        let watching = Self {
            child: command.spawn().unwrap(),
            control: project.sandbox.root.path().into(),
        };
        wait_for("Rojo startup", || {
            watching.control.join("rojo.pid").is_file()
        });
        watching
    }

    fn exit(&mut self, status: i32) -> std::process::ExitStatus {
        fs::write(self.control.join("rojo.exit"), status.to_string()).unwrap();
        let mut result = None;
        wait_for("dev to exit with Rojo", || {
            result = self.child.try_wait().unwrap();
            result.is_some()
        });
        result.unwrap()
    }
}

impl Drop for Watching {
    fn drop(&mut self) {
        let _ = fs::write(self.control.join("rojo.exit"), "0");
        // Give the stand-in a chance to exit even if an assertion panicked.
        std::thread::sleep(Duration::from_millis(100));
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn regeneration_tracks_event_additions_removals_signatures_and_body_only_edits() {
    let project = Project::new();
    let original = text(project.lobby());
    fs::write(
        project.lobby(),
        "return { renamed = RoGrid.event(function(player: Player, id: string) end) }",
    )
    .unwrap();
    let report = project.once();
    assert!(report.contains("+ server.Lobby.renamed"));
    assert!(report.contains("- server.Lobby.setReady"));
    assert!(text(project.caller()).contains("renamed"));
    assert!(!text(project.caller()).contains("setReady"));
    let revision = text(project.revision());
    fs::write(
        project.lobby(),
        "return { renamed = RoGrid.event(function(player: Player, id: string) print(id) end) }",
    )
    .unwrap();
    assert_eq!(project.once().trim(), "Generated 2 typed events.");
    assert_ne!(text(project.revision()), revision);
    fs::write(
        project.lobby(),
        "return { renamed = RoGrid.event(function(player: Player, id: number) end) }",
    )
    .unwrap();
    assert!(project.once().contains("~ server.Lobby.renamed"));
    fs::remove_file(project.lobby()).unwrap();
    assert!(project.once().contains("- server.Lobby.renamed"));
    assert!(!text(project.caller()).contains("Lobby"));
    fs::write(project.lobby(), original).unwrap();
    assert!(project.once().contains("+ server.Lobby.setReady"));
    fs::write(
        project.root.join("rogrid.toml"),
        "[events]\nserver = []\nclient = []\n",
    )
    .unwrap();
    let report = project.once();
    assert!(report.contains("Generated 0 typed events."));
    assert!(report.contains("- server.Lobby.setReady"));
    assert!(!text(project.caller()).contains("Lobby"));
}

#[test]
fn unchanged_generation_preserves_timestamps_and_removes_only_obsolete_output() {
    let project = Project::new();
    project.once();
    let paths: Vec<_> = [
        project.caller(),
        project.revision(),
        project.root.join(".rogrid/events.json"),
        project.root.join("default.project.json"),
        project.root.join(".vscode/settings.json"),
    ]
    .into_iter()
    .collect();
    let old = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
    for path in &paths {
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(old)
            .unwrap();
    }
    let times: Vec<_> = paths
        .iter()
        .map(|path| fs::metadata(path).unwrap().modified().unwrap())
        .collect();
    let obsolete = project.root.join(".rogrid/generated/obsolete/nested");
    fs::create_dir_all(&obsolete).unwrap();
    fs::write(obsolete.join("Old.luau"), "old generated code").unwrap();
    fs::write(project.root.join("keep.txt"), "user data").unwrap();
    project.once();
    assert!(!project.root.join(".rogrid/generated/obsolete").exists());
    assert_eq!(text(project.root.join("keep.txt")), "user data");
    assert_eq!(
        paths
            .iter()
            .map(|path| fs::metadata(path).unwrap().modified().unwrap())
            .collect::<Vec<_>>(),
        times
    );
}

#[test]
fn absent_or_corrupt_inventory_is_rebuilt_and_failed_writes_block_startup() {
    let project = Project::new();
    let inventory = project.root.join(".rogrid/events.json");
    for content in [None, Some("invalid json"), Some("[]")] {
        if let Some(content) = content {
            fs::write(&inventory, content).unwrap();
        } else {
            fs::remove_file(&inventory).unwrap();
        }
        let report = project.once();
        assert!(report.contains("+ server.Lobby.setReady"));
        assert!(report.contains("+ client.Notifications.show"));
        assert!(
            serde_json::from_str::<serde_json::Value>(&text(&inventory))
                .unwrap()
                .is_object()
        );
    }
    let caller = project.caller();
    fs::remove_file(&caller).unwrap();
    fs::create_dir(&caller).unwrap();
    failure(
        project.dev().arg("--once").output().unwrap(),
        "could not write",
    );
    assert!(text(project.revision()).contains("generation is incomplete"));
    fs::remove_dir(&caller).unwrap();
    project.once();
    assert!(!text(project.revision()).contains("generation is incomplete"));
}

#[test]
fn malformed_or_non_game_sourcemaps_fail_and_invalidate_generation() {
    let project = Project::new();
    for (map, expected) in [
        ("broken", "invalid Rojo sourcemap output"),
        (
            "{\"name\":\"Package\",\"className\":\"ModuleScript\"}",
            "must describe a DataModel",
        ),
    ] {
        fs::write(&project.map, map).unwrap();
        failure(project.dev().arg("--once").output().unwrap(), expected);
        assert!(text(project.revision()).contains("generation is incomplete"));
    }
}

#[test]
fn live_watcher_handles_saves_renames_creation_deletion_and_configuration_recovery() {
    let project = Project::new();
    let mut watching = Watching::start(&project);
    let old = text(project.revision());
    // An editor's atomic save writes a temporary file then renames it into place.
    let temp = project.lobby().with_extension("tmp");
    fs::write(
        &temp,
        "return { changed = RoGrid.event(function(player: Player, value: string) end) }",
    )
    .unwrap();
    fs::remove_file(project.lobby()).unwrap();
    fs::rename(temp, project.lobby()).unwrap();
    wait_for("atomic save generation", || {
        fs::read_to_string(project.caller()).is_ok_and(|s| s.contains("changed"))
            && fs::read_to_string(project.revision())
                .is_ok_and(|s| s != old && !s.contains("incomplete"))
    });
    let renamed = text(project.revision());
    fs::rename(project.lobby(), project.lobby().with_extension("lua")).unwrap();
    wait_for("extension rename generation", || {
        fs::read_to_string(project.revision())
            .is_ok_and(|s| s != renamed && !s.contains("incomplete"))
    });
    let source = project.lobby().with_extension("lua");
    fs::write(
        &source,
        "return { changed = RoGrid.event(function(player: Player, value: any) end) }",
    )
    .unwrap();
    wait_for("invalid source", || {
        fs::read_to_string(project.revision()).is_ok_and(|s| s.contains("incomplete"))
    });
    assert!(watching.child.try_wait().unwrap().is_none());
    fs::create_dir_all(project.root.join("src/shared")).unwrap();
    let alias = project.root.join("src/shared/Id.luau");
    fs::write(&alias, "export type Id = number\nreturn {}\n").unwrap();
    fs::write(&source, "local Types = require('@game/ReplicatedStorage/Shared/Id')\nreturn { changed = RoGrid.event(function(player: Player, value: Types.Id) end) }").unwrap();
    wait_for("source recovery with shared alias", || {
        fs::read_to_string(project.caller()).is_ok_and(|s| s.contains("value: number"))
            && fs::read_to_string(project.revision()).is_ok_and(|s| !s.contains("incomplete"))
    });
    fs::remove_file(&alias).unwrap();
    wait_for("deleted shared alias", || {
        fs::read_to_string(project.revision()).is_ok_and(|s| s.contains("incomplete"))
    });
    fs::write(&alias, "export type Id = string\nreturn {}\n").unwrap();
    wait_for("restored shared alias", || {
        fs::read_to_string(project.caller()).is_ok_and(|s| s.contains("value: string"))
            && fs::read_to_string(project.revision()).is_ok_and(|s| !s.contains("incomplete"))
    });
    let extra = project.root.join("src/server/events/Extra.luau");
    fs::write(
        &extra,
        "return { added = RoGrid.event(function(player: Player) end) }",
    )
    .unwrap();
    wait_for("new module", || {
        fs::read_to_string(project.caller()).is_ok_and(|s| s.contains("Extra"))
    });
    fs::remove_file(extra).unwrap();
    wait_for("removed module", || {
        fs::read_to_string(project.caller()).is_ok_and(|s| !s.contains("Extra"))
    });
    fs::write(project.root.join("rogrid.toml"), "[broken").unwrap();
    wait_for("invalid configuration", || {
        fs::read_to_string(project.revision()).is_ok_and(|s| s.contains("incomplete"))
    });
    assert!(watching.child.try_wait().unwrap().is_none());
    fs::write(project.root.join("rogrid.toml"), "[events]\nclient = []\n").unwrap();
    wait_for("configuration recovery", || {
        fs::read_to_string(project.revision()).is_ok_and(|s| !s.contains("incomplete"))
    });
    let start = text(project.root.join(".rogrid/generated/client/Start.luau"));
    assert!(!start.contains("Notifications"));
    std::thread::sleep(Duration::from_millis(350));
    let calls = project.sandbox.commands().len();
    fs::write(
        project.root.join(".rogrid/generated/ignored.luau"),
        "ignored",
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(350));
    assert_eq!(
        project.sandbox.commands().len(),
        calls,
        "generated writes must not trigger regeneration loops"
    );
    assert!(watching.exit(0).success());
}

#[test]
fn live_dev_reports_a_failed_rojo_process() {
    let project = Project::new();
    let mut watching = Watching::start(&project);
    assert!(!watching.exit(7).success());
    assert!(text(project.sandbox.root.path().join("dev.err")).contains("rojo serve exited with"));
}

#[cfg(unix)]
#[test]
fn ctrl_c_stops_dev_and_its_rojo_child() {
    let project = Project::new();
    let mut watching = Watching::start(&project);
    let rojo = text(watching.control.join("rojo.pid"));
    assert!(
        Command::new("kill")
            .args(["-INT", &watching.child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let mut status = None;
    wait_for("Ctrl+C shutdown", || {
        status = watching.child.try_wait().unwrap();
        status.is_some()
    });
    assert!(status.unwrap().success());
    assert!(
        !Command::new("kill")
            .args(["-0", rojo.trim()])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success(),
        "Rojo must be reaped before dev exits"
    );
}

#[test]
fn dev_outside_a_project_does_not_create_output() {
    let dir = tempfile::tempdir().unwrap();
    failure(
        Command::new(env!("CARGO_BIN_EXE_rogrid"))
            .current_dir(dir.path())
            .args(["dev", "--once"])
            .output()
            .unwrap(),
        "project containing default.project.json",
    );
    assert!(!Path::new(dir.path()).join(".rogrid").exists());
}
