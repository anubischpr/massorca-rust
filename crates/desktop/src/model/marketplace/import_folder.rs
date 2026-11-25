//! The folder an account import starts from, as the service detects it (capability `agent-import-detect/1`).

pub use massorca_protocol::methods::agents::{CandidateFile, ImportCandidate, ImportCandidates};

/// The candidate the service listed for `kind`, if any.
pub fn candidate_for<'a>(found: &'a ImportCandidates, kind: &str) -> Option<&'a ImportCandidate> {
    found
        .candidates
        .iter()
        .find(|candidate| candidate.kind == kind)
}

/// The folder field's placeholder when no folder is expected: the wording the dialog has always had.
pub const MANUAL_PLACEHOLDER: &str = "Folder that holds the login files";

/// What the import dialog shows and sends for one kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderField {
    /// The field's text: the detected folder when it is ready, else empty; what the person typed once edited.
    pub value: String,
    /// The folder the import expects, else the manual prompt.
    pub placeholder: String,
    /// "Found: <file>", "Missing: <file>" or "Cannot use: <file> (<reason>)", while the field still holds what detection gave.
    pub status: Option<String>,
    /// The folder Import sends; `None` while the detected folder is kept, so the service finds it itself.
    pub from: Option<String>,
}

/// The folder field of one kind: `edited` is `None` on open, then the typed text.
pub fn folder_field(
    capability: bool,
    candidate: Option<&ImportCandidate>,
    edited: Option<&str>,
) -> FolderField {
    let detected = candidate.filter(|_| capability);
    let ready = detected.is_some_and(|candidate| candidate.ready);
    let prefill = detected
        .filter(|candidate| candidate.ready)
        .and_then(|candidate| candidate.folder.clone())
        .unwrap_or_default();
    let placeholder = detected
        .and_then(|candidate| candidate.folder.clone())
        .unwrap_or_else(|| MANUAL_PLACEHOLDER.to_owned());
    let kept = edited.is_none_or(|text| text.trim() == prefill);
    let status = if kept {
        detected.and_then(status_line)
    } else {
        None
    };
    let value = edited.map_or_else(|| prefill, str::to_owned);
    let from = (!(ready && kept)).then(|| value.trim().to_owned());
    FolderField {
        value,
        placeholder,
        status,
        from,
    }
}

/// "Found: <file>" when ready; else "Cannot use: <file> (<reason>)" and "Missing: <file>" for the required files not found.
/// Unusable optional files are skipped. `None` when no required file is named.
fn status_line(candidate: &ImportCandidate) -> Option<String> {
    if candidate.ready {
        let found = candidate
            .files
            .iter()
            .filter(|file| file.required && file.found)
            .map(|file| file.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return (!found.is_empty()).then(|| format!("Found: {found}"));
    }
    let unusable = candidate
        .files
        .iter()
        .filter(|file| file.required && !file.found)
        .filter_map(|file| Some(format!("{} ({})", file.name, file.reason.as_deref()?)))
        .collect::<Vec<_>>()
        .join(", ");
    let missing = candidate
        .files
        .iter()
        .filter(|file| file.required && !file.found && file.reason.is_none())
        .map(|file| file.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let lines: Vec<String> = [
        (!unusable.is_empty()).then(|| format!("Cannot use: {unusable}")),
        (!missing.is_empty()).then(|| format!("Missing: {missing}")),
    ]
    .into_iter()
    .flatten()
    .collect();
    (!lines.is_empty()).then(|| lines.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FOLDER: &str = "C:/Users/me/.claude";

    fn file(name: &str, required: bool, found: bool) -> CandidateFile {
        CandidateFile {
            name: name.to_owned(),
            required,
            found,
            reason: None,
            path: None,
            env: None,
        }
    }

    fn unusable(name: &str, required: bool, reason: &str) -> CandidateFile {
        CandidateFile {
            reason: Some(reason.to_owned()),
            ..file(name, required, false)
        }
    }

    fn candidate(ready: bool, folder: Option<&str>, files: Vec<CandidateFile>) -> ImportCandidate {
        ImportCandidate {
            kind: "claude".to_owned(),
            ready,
            folder: folder.map(str::to_owned),
            files,
        }
    }

    #[test]
    fn a_ready_candidate_prefills_its_folder_and_says_what_it_found() {
        let ready = candidate(
            true,
            Some(FOLDER),
            vec![file(".credentials.json", true, true)],
        );
        assert_eq!(
            folder_field(true, Some(&ready), None),
            FolderField {
                value: FOLDER.to_owned(),
                placeholder: FOLDER.to_owned(),
                status: Some("Found: .credentials.json".to_owned()),
                from: None,
            }
        );
    }

    #[test]
    fn a_candidate_that_is_not_ready_leaves_the_field_empty_and_names_the_missing_file() {
        let missing = candidate(
            false,
            Some(FOLDER),
            vec![file(".credentials.json", true, false)],
        );
        assert_eq!(
            folder_field(true, Some(&missing), None),
            FolderField {
                value: String::new(),
                placeholder: FOLDER.to_owned(),
                status: Some("Missing: .credentials.json".to_owned()),
                from: Some(String::new()),
            }
        );
    }

    #[test]
    fn a_required_file_that_exists_but_cannot_be_used_says_why() {
        let blocked = candidate(
            false,
            Some(FOLDER),
            vec![unusable(".credentials.json", true, "a link")],
        );
        assert_eq!(
            folder_field(true, Some(&blocked), None),
            FolderField {
                value: String::new(),
                placeholder: FOLDER.to_owned(),
                status: Some("Cannot use: .credentials.json (a link)".to_owned()),
                from: Some(String::new()),
            }
        );
    }

    #[test]
    fn a_missing_required_file_and_an_unusable_one_are_each_named_with_their_own_line() {
        let blocked = candidate(
            false,
            Some(FOLDER),
            vec![
                file("settings.json", true, false),
                unusable(".credentials.json", true, "too large"),
            ],
        );
        assert_eq!(
            folder_field(true, Some(&blocked), None).status,
            Some("Cannot use: .credentials.json (too large); Missing: settings.json".to_owned())
        );
    }

    #[test]
    fn an_unusable_optional_file_is_not_a_blocker_and_is_not_listed() {
        let found = candidate(
            true,
            Some(FOLDER),
            vec![
                file(".credentials.json", true, true),
                unusable("settings.json", false, "a link"),
            ],
        );
        assert_eq!(
            folder_field(true, Some(&found), None),
            FolderField {
                value: FOLDER.to_owned(),
                placeholder: FOLDER.to_owned(),
                status: Some("Found: .credentials.json".to_owned()),
                from: None,
            }
        );
        let missing = candidate(
            false,
            Some(FOLDER),
            vec![
                file(".credentials.json", true, false),
                unusable("settings.json", false, "a link"),
            ],
        );
        assert_eq!(
            folder_field(true, Some(&missing), None).status,
            Some("Missing: .credentials.json".to_owned())
        );
    }

    #[test]
    fn an_edited_folder_is_sent_and_the_found_line_goes_until_the_detected_folder_is_typed_again() {
        let ready = candidate(
            true,
            Some(FOLDER),
            vec![file(".credentials.json", true, true)],
        );
        let other = folder_field(true, Some(&ready), Some("C:/Users/me/other"));
        assert_eq!(other.status, None);
        assert_eq!(other.from, Some("C:/Users/me/other".to_owned()));
        let kept = folder_field(true, Some(&ready), Some(FOLDER));
        assert_eq!(
            (kept.status, kept.from),
            (Some("Found: .credentials.json".to_owned()), None)
        );
    }

    #[test]
    fn without_the_capability_the_field_stays_manual_and_import_sends_the_typed_folder() {
        let ready = candidate(
            true,
            Some(FOLDER),
            vec![file(".credentials.json", true, true)],
        );
        assert_eq!(
            folder_field(false, Some(&ready), None),
            FolderField {
                value: String::new(),
                placeholder: MANUAL_PLACEHOLDER.to_owned(),
                status: None,
                from: Some(String::new()),
            }
        );
        assert_eq!(
            folder_field(false, None, Some(" C:/Users/me/x ")).from,
            Some("C:/Users/me/x".to_owned())
        );
    }
}
