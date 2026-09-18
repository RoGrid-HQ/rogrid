use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use include_dir::{Dir, include_dir};

/// The default place template, embedded at compile time.
pub static DEFAULT: Dir = include_dir!("$CARGO_MANIFEST_DIR/../../templates/places/default");

/// Values substituted for `{{key}}` placeholders in template files.
pub type Vars<'a> = [(&'a str, String)];

/// Copies every file in `template` into `dest`, filling in placeholders.
pub fn render(template: &Dir, dest: &Path, vars: &Vars) -> Result<()> {
    for file in files(template) {
        let relative = file.path();
        let target = dest.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = match file.contents_utf8() {
            Some(text) => fill(text, vars).into_bytes(),
            None => file.contents().to_vec(),
        };

        fs::write(&target, contents)
            .with_context(|| format!("could not write {}", target.display()))?;
    }

    Ok(())
}

/// Replaces every `{{key}}` in `text` with its value.
pub fn fill(text: &str, vars: &Vars) -> String {
    vars.iter().fold(text.to_string(), |text, (key, value)| {
        text.replace(&format!("{{{{{key}}}}}"), value)
    })
}

/// All files in the directory tree, in no particular order.
fn files<'a>(dir: &'a Dir<'a>) -> Vec<&'a include_dir::File<'a>> {
    let mut list: Vec<_> = dir.files().collect();
    for sub in dir.dirs() {
        list.extend(files(sub));
    }
    list
}
