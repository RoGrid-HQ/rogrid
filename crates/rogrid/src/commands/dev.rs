use std::env;
use std::process::{Child, Command};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::codegen;

#[derive(clap::Args)]
pub struct Args {
    /// Generate once without watching or starting Rojo. Use before a Rojo build.
    #[arg(long)]
    once: bool,
}

enum Change {
    Files(notify::Result<notify::Event>),
    Stop,
}

/// Own the child so every error path also stops Rojo.
struct Rojo(Child);

impl Drop for Rojo {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub fn run(args: Args) -> Result<()> {
    let root = env::current_dir()?;
    // Validate the project before creating generated output in an arbitrary folder.
    if !root.join("default.project.json").is_file() {
        bail!("run rogrid dev in a project containing default.project.json");
    }
    if args.once {
        let report = codegen::generate(&root, &[])?;
        println!("{report}");
        return Ok(());
    }

    let (send, receive) = mpsc::channel();
    let mut sources = match watch::Sources::new(&root, send.clone()) {
        Ok(sources) => sources,
        Err(error) => {
            codegen::invalidate(&root)?;
            return Err(error);
        }
    };
    let stop = send.clone();
    ctrlc::set_handler(move || {
        let _ = stop.send(Change::Stop);
    })?;

    let report = codegen::generate(&root, &[])?;
    println!("{report}");
    let mut rojo = Rojo(
        Command::new("rojo")
            .args(["serve", "default.project.json"])
            .current_dir(&root)
            .spawn()
            .context("could not start rojo; install it and make it available on PATH")?,
    );
    println!(
        "Watching configured event folders and project settings; restart Studio Play after edits. Ctrl+C stops Rojo."
    );

    loop {
        if let Some(status) = rojo.0.try_wait()? {
            if !status.success() {
                bail!("rojo serve exited with {status}");
            }
            return Ok(());
        }
        match receive.recv_timeout(Duration::from_millis(250)) {
            Ok(Change::Stop) => return Ok(()),
            Ok(Change::Files(Ok(event))) if sources.relevant(&event) => {
                // Let an editor finish its save/rename before reading files again.
                let mut deadline = Instant::now() + Duration::from_millis(100);
                loop {
                    match receive.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                        Ok(Change::Stop) => return Ok(()),
                        Ok(Change::Files(Err(error))) => return Err(error.into()),
                        Ok(Change::Files(Ok(event))) if sources.relevant(&event) => {
                            deadline = Instant::now() + Duration::from_millis(100);
                        }
                        Ok(_) => continue,
                        Err(mpsc::RecvTimeoutError::Timeout) => break,
                        Err(error) => return Err(error.into()),
                    }
                }
                // Keep the previous watches if the edited configuration is temporarily invalid.
                match watch::Sources::new(&root, send.clone()) {
                    Ok(updated) => sources = updated,
                    Err(error) => {
                        codegen::invalidate(&root)?;
                        eprintln!(
                            "{error:#}\nCould not update source watches; fix the configuration and save again."
                        );
                        continue;
                    }
                }
                match codegen::generate(&root, &[]) {
                    Ok(report) => {
                        println!("{report}\nRestart Play to load changes.")
                    }
                    Err(error) => {
                        eprintln!("{error:#}\nGeneration blocked; fix the source and save again.")
                    }
                }
            }
            Ok(Change::Files(Err(error))) => return Err(error.into()),
            Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(error) => return Err(error.into()),
        }
    }
}
mod watch;
