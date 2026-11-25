//! Fast mode of a session's run (decision 0165). A start that asks for it records that in the run directory, where the
//! other per-run notes live, so a session view can say it. Nothing confirms the vendor took it, so the record says only
//! that it was requested. A later change adds the live toggle; this module does not send anything.

use std::fs;
use std::path::Path;

use massorca_protocol::methods::agents::SessionFast;

/// The file in a run directory that marks a run started with fast mode on.
const REQUESTED_FILE: &str = "fast-requested";

/// Records in the run directory whether the current run was started with fast mode on, or clears an earlier run's
/// record. A failure only loses the record.
pub(crate) fn record(run_dir: &Path, on: bool) {
    let path = run_dir.join(REQUESTED_FILE);
    if on {
        let _ = fs::write(path, "requested");
    } else {
        let _ = fs::remove_file(path);
    }
}

/// The fast mode of a session's current or last run: on and not confirmed, or None when the run was not started with it.
pub(crate) fn of_run(run_dir: &Path) -> Option<SessionFast> {
    run_dir
        .join(REQUESTED_FILE)
        .is_file()
        .then_some(SessionFast {
            on: true,
            confirmed: false,
        })
}
