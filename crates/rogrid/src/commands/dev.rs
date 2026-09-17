//! `rogrid dev`: keeps `.rogrid` up to date while you work, with `rojo serve` running beside it.
//!
//! Rojo does the syncing. This only rewrites `.rogrid` when a request changes, and
//! Rojo's own watcher carries that into Studio.

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use notify::event::{EventKind, ModifyKind};
use notify::{Event, RecursiveMode, Watcher};

use crate::binaries;
use crate::codegen;
use crate::commands::generate::{MAPPING_HINT, requests};

/// One save fires several file events. Generation waits until they have stopped for this long.
const SETTLE: Duration = Duration::from_millis(100);
/// How often the loop wakes up to see whether it should stop or Rojo has.
const TICK: Duration = Duration::from_millis(250);

const PROJECT_FILE: &str = "default.project.json";

pub fn run() -> Result<()> {
    let root = env::current_dir().context("could not read the current directory")?;
    let inputs = watched_inputs(&root);

    if !binaries::is_installed("rojo") {
        bail!("rojo is not installed. Install it from https://rojo.space, then run this again");
    }

    // Watching starts before the first generation, so a save made during startup is not missed.
    let (sender, events) = mpsc::channel();
    let mut watcher =
        notify::recommended_watcher(sender).context("could not start watching files")?;
    watcher
        .watch(&root.join("src"), RecursiveMode::Recursive)
        .context("could not watch the src folder. Run this inside a RoGrid project")?;
    // The folder and not the project file itself: an editor that saves by replacing the
    // file would end a watch that was placed on the file.
    watcher
        .watch(&root, RecursiveMode::NonRecursive)
        .context("could not watch the project folder")?;

    // Ctrl-C reaches Rojo by itself, because the terminal sends it to both processes.
    // Being terminated any other way does not, and would leave Rojo holding its port.
    // So every way of stopping ends the loop below, and dropping `rojo` stops Rojo.
    let stopping = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stopping);
    ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst))
        .context("could not listen for Ctrl-C")?;

    let mut working = generate(&root, Trigger::Startup);
    println!("Watching {} for changes\n", codegen::FUNCTIONS_DIR);

    let mut rojo = Rojo::start(&root)?;
    let mut due: Option<Instant> = None;

    loop {
        let wait = due.map_or(TICK, |at| at.saturating_duration_since(Instant::now()));
        match events.recv_timeout(wait) {
            Ok(Ok(event)) if affects_output(&event, &inputs) => {
                due = Some(Instant::now() + SETTLE);
            }
            Ok(Ok(_)) | Err(RecvTimeoutError::Timeout) => {}
            Ok(Err(error)) => eprintln!("warning: file watcher: {error}"),
            Err(RecvTimeoutError::Disconnected) => bail!("the file watcher stopped"),
        }

        // Checked before Rojo's status: on Ctrl-C Rojo stops too, and that is not an error.
        if stopping.load(Ordering::SeqCst) {
            return Ok(());
        }

        if due.is_some_and(|at| Instant::now() >= at) {
            due = None;
            working = generate(
                &root,
                Trigger::Change {
                    was_working: working,
                },
            );
        }

        if let Some(status) = rojo.exited()? {
            bail!("rojo serve stopped ({status})");
        }
    }
}

enum Trigger {
    Startup,
    Change { was_working: bool },
}

/// Generates once and reports it. A problem is printed, never fatal: the last good
/// output stays in place and the next save tries again. Returns whether it worked.
fn generate(root: &Path, trigger: Trigger) -> bool {
    match codegen::run(root) {
        Ok(summary) => {
            match trigger {
                Trigger::Startup => {
                    println!("Generated {} into .rogrid", requests(summary.count));
                    if !summary.mapped {
                        println!("{MAPPING_HINT}");
                    }
                }
                // Editing a handler's body changes nothing in the output, so it stays quiet.
                Trigger::Change { was_working } if summary.changed || !was_working => {
                    println!("Generated {}", requests(summary.count));
                }
                Trigger::Change { .. } => {}
            }
            true
        }
        Err(error) => {
            eprintln!("\n{error:#}\n");
            false
        }
    }
}

/// The paths whose changes matter: the functions folder and the Rojo project file.
/// Each is listed as the project was opened and with symlinks resolved, because
/// watchers differ in which form they report.
fn watched_inputs(root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![root.to_path_buf()];
    if let Ok(resolved) = root.canonicalize()
        && resolved != root
    {
        roots.push(resolved);
    }

    roots
        .iter()
        .flat_map(|root| [root.join(codegen::FUNCTIONS_DIR), root.join(PROJECT_FILE)])
        .collect()
}

/// Whether an event can change what gets generated: something in `inputs` was created,
/// edited, renamed, or deleted.
///
/// Opening a file is an event too on Linux. Reacting to those would make generation,
/// which reads every request file, trigger itself forever.
fn affects_output(event: &Event, inputs: &[PathBuf]) -> bool {
    let changes_content = match event.kind {
        EventKind::Create(_) | EventKind::Remove(_) => true,
        EventKind::Modify(ModifyKind::Metadata(_)) => false,
        EventKind::Modify(_) => true,
        _ => false,
    };

    changes_content
        && event
            .paths
            .iter()
            .any(|path| inputs.iter().any(|input| path.starts_with(input)))
}

/// `rojo serve` as a child process. It is stopped when this value goes away, so it is
/// never left running, whether `dev` ends normally, with an error, or by a signal.
struct Rojo(Child);

impl Rojo {
    fn start(root: &Path) -> Result<Self> {
        Command::new("rojo")
            .arg("serve")
            .current_dir(root)
            .spawn()
            .map(Rojo)
            .context("could not start `rojo serve`")
    }

    fn exited(&mut self) -> Result<Option<ExitStatus>> {
        self.0.try_wait().context("could not check on rojo serve")
    }
}

impl Drop for Rojo {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(test)]
mod tests {
    use notify::event::{AccessKind, CreateKind, DataChange, MetadataKind, RemoveKind};

    use super::*;

    fn event(kind: EventKind, path: &str) -> Event {
        Event::new(kind).add_path(PathBuf::from("/game").join(path))
    }

    fn affects(kind: EventKind, path: &str) -> bool {
        affects_output(&event(kind, path), &watched_inputs(Path::new("/game")))
    }

    const EDIT: EventKind = EventKind::Modify(ModifyKind::Data(DataChange::Any));

    #[test]
    fn request_files_and_the_project_file_matter() {
        let request = "src/server/functions/shop/buyItem.luau";

        assert!(affects(EDIT, request));
        assert!(affects(EventKind::Create(CreateKind::File), request));
        assert!(affects(EventKind::Remove(RemoveKind::File), request));
        assert!(affects(EDIT, "default.project.json"));
    }

    #[test]
    fn other_files_do_not() {
        assert!(!affects(EDIT, "src/client/main.client.luau"));
        assert!(!affects(EDIT, "src/server/systems/Shop.luau"));
        assert!(!affects(EDIT, ".rogrid/client/api.luau"));
        assert!(!affects(EDIT, "pesde.toml"));
    }

    #[test]
    fn reading_a_request_file_is_not_a_change() {
        let request = "src/server/functions/shop/buyItem.luau";

        assert!(!affects(EventKind::Access(AccessKind::Any), request));
        assert!(!affects(
            EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)),
            request
        ));
    }
}
