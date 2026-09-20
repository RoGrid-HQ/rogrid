#[allow(dead_code)]
mod support;

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use support::{Sandbox, text};

struct Terminal {
    child: Box<dyn Child + Send + Sync>,
    _master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    output: Arc<Mutex<Vec<u8>>>,
    cursor_queries: usize,
    control: std::path::PathBuf,
}

impl Terminal {
    fn start(sandbox: &Sandbox, args: &[&str]) -> Self {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 30,
                cols: 120,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_rogrid"));
        command.cwd(&sandbox.work);
        command.args(args);
        command.env("PATH", &sandbox.bin);
        command.env("HOME", &sandbox.home);
        command.env("USERPROFILE", &sandbox.home);
        command.env("ROGRID_TEST_HOME", &sandbox.home);
        command.env("ROGRID_TEST_PM", "wally");
        command.env("ROGRID_TEST_CONTROL", sandbox.root.path());
        command.env("ROGRID_TEST_LOG", sandbox.root.path().join("commands.log"));
        command.env("TERM", "xterm-256color");
        let child = pair.slave.spawn_command(command).unwrap();
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().unwrap();
        let writer = pair.master.take_writer().unwrap();
        let output = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&output);
        std::thread::spawn(move || {
            let mut buffer = [0; 4096];
            while let Ok(count) = reader.read(&mut buffer) {
                if count == 0 {
                    break;
                }
                captured.lock().unwrap().extend_from_slice(&buffer[..count]);
            }
        });
        Self {
            child,
            _master: pair.master,
            writer,
            output,
            cursor_queries: 0,
            control: sandbox.root.path().into(),
        }
    }

    fn output(&self) -> String {
        String::from_utf8_lossy(&self.output.lock().unwrap()).into_owned()
    }
    fn send(&mut self, text: &str) {
        self.writer.write_all(text.as_bytes()).unwrap();
        self.writer.flush().unwrap();
    }
    fn answer_cursor_queries(&mut self) {
        // ConPTY and crossterm ask the terminal for its cursor position.
        let queries = self.output().matches("\u{1b}[6n").count();
        while self.cursor_queries < queries {
            self.send("\u{1b}[1;1R");
            self.cursor_queries += 1;
        }
    }
    fn expect(&mut self, expected: &str) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !self.output().contains(expected) {
            self.answer_cursor_queries();
            assert!(
                Instant::now() < deadline,
                "expected {expected}: {}",
                self.output()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn finished(&mut self, successful: bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            self.answer_cursor_queries();
            if let Some(status) = self.child.try_wait().unwrap() {
                assert_eq!(status.success(), successful, "{}", self.output());
                return;
            }
            assert!(
                Instant::now() < deadline,
                "prompt process did not finish: {}",
                self.output()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = std::fs::write(self.control.join("rojo.exit"), "0");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    sandbox.provide("wally");
    sandbox.provide("rojo");
    sandbox
}

#[test]
fn project_name_prompt_creates_the_requested_folder() {
    let sandbox = sandbox();
    let mut terminal = Terminal::start(
        &sandbox,
        &[
            "init",
            "--package-manager",
            "wally",
            "--tool-manager",
            "none",
        ],
    );
    terminal.expect("Project name:");
    terminal.send("my-game\r");
    terminal.finished(true);
    assert!(sandbox.work.join("my-game/wally.toml").is_file());
}

#[test]
fn current_directory_prompt_uses_the_folder_name_on_enter() {
    let sandbox = sandbox();
    let mut terminal = Terminal::start(
        &sandbox,
        &[
            "init",
            ".",
            "--package-manager",
            "wally",
            "--tool-manager",
            "none",
        ],
    );
    terminal.expect("Project name:");
    terminal.send("\r");
    terminal.finished(true);
    assert!(text(sandbox.work.join("wally.toml")).contains("local/work"));
}

#[test]
fn cancellation_and_invalid_names_leave_no_project_files() {
    for input in ["\u{1b}", "\u{3}", "Bad Name\r"] {
        let sandbox = sandbox();
        let mut terminal = Terminal::start(
            &sandbox,
            &[
                "init",
                "--package-manager",
                "wally",
                "--tool-manager",
                "none",
            ],
        );
        terminal.expect("Project name:");
        terminal.send(input);
        terminal.finished(false);
        assert!(std::fs::read_dir(&sandbox.work).unwrap().next().is_none());
        assert!(sandbox.commands().is_empty());
    }
}

#[test]
fn package_prompt_defaults_to_an_installed_manager() {
    let sandbox = sandbox();
    let mut terminal = Terminal::start(&sandbox, &["init", "game", "--tool-manager", "none"]);
    terminal.expect("Package manager:");
    terminal.expect("wally");
    terminal.send("\r");
    terminal.finished(true);
    assert!(sandbox.work.join("game/wally.toml").is_file());
    assert!(!sandbox.work.join("game/pesde.toml").exists());
}

#[test]
fn tool_prompt_only_offers_compatible_managers() {
    let sandbox = sandbox();
    let mut terminal = Terminal::start(&sandbox, &["init", "game", "--package-manager", "wally"]);
    terminal.expect("Tool manager:");
    terminal.expect("none");
    assert!(!terminal.output().contains("pesde"));
    terminal.send("\u{1b}[B\r");
    terminal.finished(true);
    assert!(sandbox.work.join("game/wally.toml").is_file());
    assert!(!sandbox.work.join("game/rokit.toml").exists());
}

#[test]
fn ctrl_c_in_a_terminal_stops_dev_and_reaps_rojo() {
    let sandbox = sandbox();
    support::success(
        sandbox
            .command("wally", "none")
            .args([".", "--name", "work"])
            .output()
            .unwrap(),
    );
    let mut terminal = Terminal::start(&sandbox, &["dev"]);
    terminal.expect("Watching project Luau sources");
    let marker = sandbox.root.path().join("rojo.pid");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !marker.exists() {
        assert!(Instant::now() < deadline, "Rojo did not start");
        std::thread::sleep(Duration::from_millis(20));
    }
    let pid = text(marker).trim().parse::<u32>().unwrap();
    terminal.send("\u{3}");
    terminal.finished(true);
    #[cfg(unix)]
    {
        assert!(
            !std::process::Command::new("kill")
                .args(["-0", &pid.to_string()])
                .stderr(std::process::Stdio::null())
                .status()
                .unwrap()
                .success(),
            "Rojo survived Ctrl+C"
        );
    }
    #[cfg(windows)]
    {
        use std::ffi::c_void;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
            fn WaitForSingleObject(handle: *mut c_void, milliseconds: u32) -> u32;
            fn CloseHandle(handle: *mut c_void) -> i32;
        }
        // SYNCHRONIZE checks whether this specific child has exited; it cannot terminate it.
        unsafe {
            let handle = OpenProcess(0x00100000, 0, pid);
            if !handle.is_null() {
                let status = WaitForSingleObject(handle, 0);
                CloseHandle(handle);
                assert_eq!(status, 0, "Rojo survived Ctrl+C");
            }
        }
    }
}
