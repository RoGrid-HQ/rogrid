use std::env;

/// Whether an executable with this name can be found on the PATH.
pub fn is_installed(name: &str) -> bool {
    let Some(path) = env::var_os("PATH") else {
        return false;
    };

    env::split_paths(&path).any(|dir| {
        let file = dir.join(name);
        file.is_file() || file.with_extension("exe").is_file()
    })
}
