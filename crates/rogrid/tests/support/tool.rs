//! A small native stand-in for external installers and Rojo. Compiled by the
//! integration tests so the same process-boundary checks run on every platform.
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

fn executable(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}{}", env::consts::EXE_SUFFIX))
}

fn main() {
    let exe = env::current_exe().unwrap();
    let tool = exe.file_stem().unwrap().to_str().unwrap();
    let args = env::args().skip(1).collect::<Vec<_>>().join(" ");
    let invocation = format!("{tool} {args}");
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(env::var_os("ROGRID_TEST_LOG").unwrap())
        .unwrap();
    writeln!(log, "{invocation}\t{}", exe.display()).unwrap();
    if env::var("ROGRID_TEST_FAIL").is_ok_and(|fail| invocation == fail) {
        eprintln!("simulated failure: {invocation}");
        std::process::exit(1);
    }
    match (tool, args.as_str()) {
        (_, "--version") => println!("{tool} test-version"),
        ("rokit", args) if args.starts_with("trust ") => {}
        ("rokit", "install") => {
            let bin = exe.parent().unwrap();
            for name in ["rojo", &env::var("ROGRID_TEST_PM").unwrap()] {
                fs::copy(&exe, executable(bin, name)).unwrap();
            }
        }
        ("pesde", "install") => {
            let bin = PathBuf::from(env::var_os("ROGRID_TEST_HOME").unwrap()).join(".pesde/bin");
            fs::create_dir_all(&bin).unwrap();
            fs::copy(&exe, executable(&bin, "rojo")).unwrap();
        }
        ("wally", "install") => {}
        ("rojo", "sourcemap default.project.json") => {
            match env::var_os("ROGRID_TEST_SOURCEMAP") {
                Some(path) => print!("{}", fs::read_to_string(path).unwrap()),
                None => print!("{}", include_str!("sourcemap.json")),
            }
        }
        ("rojo", "serve default.project.json") => {
            let control = PathBuf::from(env::var_os("ROGRID_TEST_CONTROL").unwrap());
            fs::write(control.join("rojo.pid"), std::process::id().to_string()).unwrap();
            // The marker is outside the watched project. Tests choose the exit status.
            loop {
                if let Ok(status) = fs::read_to_string(control.join("rojo.exit")) {
                    std::process::exit(status.trim().parse().unwrap());
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
        _ => panic!("unexpected test tool command: {invocation}"),
    }
}
