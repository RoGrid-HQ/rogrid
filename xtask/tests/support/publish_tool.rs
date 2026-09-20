//! Inert command stand-in: all registry data comes from the fixture directory.
#[path = "../../../config.rs"]
mod config;

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(env::var_os("ROGRID_PUBLISH_TEST_ROOT").unwrap());
    let scenario = env::var("ROGRID_PUBLISH_SCENARIO").unwrap();
    let exe = env::current_exe().unwrap();
    let tool = exe.file_stem().unwrap().to_str().unwrap();
    let args: Vec<_> = env::args().skip(1).collect();
    let log = |entry: &str| {
        writeln!(OpenOptions::new().create(true).append(true).open(root.join("commands.log")).unwrap(), "{entry}").unwrap();
    };
    if tool == "curl" {
        let connect_timeout = args.windows(2).find(|pair| pair[0] == "--connect-timeout").expect("registry downloads must use the configured connection timeout");
        assert_eq!(connect_timeout[1], config::REGISTRY_CONNECT_TIMEOUT_SECS.to_string());
        let download_timeout = args.windows(2).find(|pair| pair[0] == "--max-time").expect("registry downloads must use the configured total timeout");
        assert_eq!(download_timeout[1], config::REGISTRY_DOWNLOAD_TIMEOUT_SECS.to_string());
        let retries = args.windows(2).find(|pair| pair[0] == "--retry").expect("registry downloads must use the configured retry count");
        assert_eq!(retries[1], config::REGISTRY_DOWNLOAD_RETRIES.to_string());
        let max_filesize = args.windows(2).find(|pair| pair[0] == "--max-filesize").expect("registry downloads must use the configured size limit");
        assert_eq!(max_filesize[1], config::REGISTRY_DOWNLOAD_MAX_BYTES.to_string());
        let destination = PathBuf::from(&args[args.iter().position(|arg| arg == "--output").unwrap() + 1]);
        let url = args.last().unwrap();
        if url.contains("raw.githubusercontent.com/pesde-pkg/index/") {
            fs::write(destination, "api = 'https://registry.test/pesde'\n").unwrap();
            print!("200");
        } else if url.contains("raw.githubusercontent.com/UpliftGames/wally-index/") {
            fs::write(destination, "{\"api\":\"https://registry.test/wally\"}").unwrap();
            print!("200");
        } else {
            assert!(url.starts_with("https://registry.test/"));
            let registry = if url.contains("/pesde/") { "pesde" } else { "wally" };
            log(&format!("check {registry}"));
            if scenario == "http-error" && registry == "wally" { print!("503"); return; }
            if scenario == "conflict" && registry == "wally" {
                fs::copy(root.join("fixtures/conflict.zip"), destination).unwrap(); print!("200"); return;
            }
            let published = root.join(format!("{registry}.published"));
            if !published.exists() || scenario == "never-visible" { print!("404"); return; }
            let pending = root.join(format!("{registry}.pending"));
            if pending.exists() { fs::remove_file(pending).unwrap(); print!("404"); return; }
            fs::copy(root.join("fixtures").join(if registry == "pesde" { "pesde.tar.gz" } else { "wally.zip" }), destination).unwrap();
            print!("200");
        }
    } else if tool == "pesde" && args == ["publish", "--dry-run", "--yes"] {
        log("bundle pesde");
        fs::copy(root.join("fixtures/pesde.tar.gz"), env::current_dir().unwrap().join("package.tar.gz")).unwrap();
    } else if tool == "wally" && args.first().map(String::as_str) == Some("package") {
        log("bundle wally");
        fs::copy(root.join("fixtures/wally.zip"), args.last().unwrap()).unwrap();
    } else if args.iter().any(|arg| arg == "login") {
        log(&format!("login {tool}"));
        // The actual publisher must suppress token-bearing tool output.
        println!("inert-test-token"); eprintln!("inert-test-token");
        if scenario == "auth-error" && tool == "wally" { std::process::exit(1); }
    } else if args.first().map(String::as_str) == Some("publish") {
        log(&format!("publish {tool}"));
        if scenario == "publish-error" && tool == "wally" { std::process::exit(1); }
        fs::write(root.join(format!("{tool}.published")), "yes").unwrap();
        if scenario == "delayed" && tool == "pesde" { fs::write(root.join("pesde.pending"), "yes").unwrap(); }
    } else { panic!("unexpected test tool invocation"); }
}
