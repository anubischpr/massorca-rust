//! Notes on a session's current run (decision 0168), in plain words: what a resume or a fork left out of the stored
//! options, and a first input the agent never took. A session view lists them after its delegation notes. They live in
//! the run directory, one per line, and a run that starts again replaces them. A failure to read or write only loses a
//! note.

use std::fs;
use std::path::Path;

/// The file in a run directory that holds its notes, one per line.
const NOTES_FILE: &str = "run-notes";

/// Replaces the notes of the run in `run_dir` with `notes`; an empty list clears them. A note is one line.
pub(crate) fn record(run_dir: &Path, notes: &[String]) {
    let path = run_dir.join(NOTES_FILE);
    if notes.is_empty() {
        let _ = fs::remove_file(path);
    } else {
        let _ = fs::write(path, notes.join("\n"));
    }
}

/// Adds `note` to the notes of the run in `run_dir`.
pub(crate) fn append(run_dir: &Path, note: &str) {
    let mut notes = read(run_dir);
    notes.push(note.to_owned());
    record(run_dir, &notes);
}

/// The notes of the run in `run_dir`, in the order they were added.
pub(crate) fn read(run_dir: &Path) -> Vec<String> {
    fs::read_to_string(run_dir.join(NOTES_FILE))
        .map(|text| {
            text.lines()
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_are_appended_read_and_replaced_by_the_next_run() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read(dir.path()).is_empty());
        record(dir.path(), &["first".to_owned()]);
        append(dir.path(), "second");
        assert_eq!(read(dir.path()), ["first", "second"]);
        record(dir.path(), &["third".to_owned()]);
        assert_eq!(read(dir.path()), ["third"], "a new run replaces them");
        record(dir.path(), &[]);
        assert!(read(dir.path()).is_empty());
    }
}
