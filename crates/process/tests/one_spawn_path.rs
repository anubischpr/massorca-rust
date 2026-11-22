//! Every child process in the workspace is started through `massorca_process`, so none opens a console window.

use std::fs;
use std::path::{Path, PathBuf};

/// The only file allowed to name the raw calls.
const HELPER: &str = "crates/process/src/lib.rs";
/// Calls that start a process or replace its creation flags without the helper. Split so this file does not match.
const FORBIDDEN: [&str; 2] = [
    concat!("Command", "::new("),
    concat!(".creation", "_flags("),
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if name == "target" || name.to_string_lossy().starts_with('.') {
            continue;
        }
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_crate_starts_a_process_outside_the_helper() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the workspace root");
    let mut files = Vec::new();
    rust_files(&root.join("crates"), &mut files);
    assert!(files.len() > 50, "the crates were found: {}", files.len());
    let mut offenders = Vec::new();
    for file in files {
        let relative = file
            .strip_prefix(root)
            .expect("under the root")
            .to_string_lossy()
            .replace('\\', "/");
        if relative == HELPER {
            continue;
        }
        let text = fs::read_to_string(&file).expect("a readable source file");
        for (number, line) in text.lines().enumerate() {
            if FORBIDDEN.iter().any(|call| line.contains(call)) {
                offenders.push(format!("{relative}:{}: {}", number + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "use massorca_process::command / creation_flags instead:\n{}",
        offenders.join("\n")
    );
}
