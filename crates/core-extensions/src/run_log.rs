//! The run log of a package, `logs/<id>-runs.json`: its install, update, rollback and remove runs, for `extensions.log`.
//! It is best effort: the lockfile is what is installed, so a run that cannot be logged never fails the change.

use std::fs;
use std::path::Path;

use massorca_ext_package::ids::is_extension_id;
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::lockfile::write_atomic;
use crate::service::{Extensions, now_ms};

/// The newest runs kept per package.
pub const RUN_LIMIT: usize = 20;
const RUN_FILE_VERSION: u32 = 1;

/// What a run did to a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunAction {
    Install,
    Update,
    Rollback,
    Remove,
}

/// How a run ended. A run that failed after its vendor command ran is `failed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunOutcome {
    Ok,
    Failed,
}

/// One run in the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRecord {
    pub at_ms: u64,
    pub action: RunAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_version: Option<String>,
    pub outcome: RunOutcome,
    /// The tail of the output this run's vendor command wrote, redacted and capped; empty when none ran.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct RunFile {
    version: u32,
    runs: Vec<RunRecord>,
}

impl Extensions {
    /// The runs of a package, oldest first. A package never installed has none; an id that is not one is refused.
    pub fn runs(&self, id: &str) -> Result<Vec<RunRecord>, Error> {
        if !is_extension_id(id) {
            return Err(Error::NotFound(id.to_owned()));
        }
        Ok(read_runs(&self.layout.run_log(id)))
    }

    /// Records a run of `id` under the change lock. A run that could not be logged changes nothing else.
    pub(crate) fn log_run(
        &self,
        id: &str,
        action: RunAction,
        from_version: Option<String>,
        to_version: Option<String>,
        ok: bool,
        output: Vec<String>,
    ) {
        let _changes = self.lock_changes();
        self.note_run(id, action, from_version, to_version, ok, output);
    }

    /// Like `log_run`, for a caller that already holds the change lock.
    pub(crate) fn note_run(
        &self,
        id: &str,
        action: RunAction,
        from_version: Option<String>,
        to_version: Option<String>,
        ok: bool,
        output: Vec<String>,
    ) {
        let run = RunRecord {
            at_ms: now_ms(),
            action,
            from_version,
            to_version,
            outcome: if ok {
                RunOutcome::Ok
            } else {
                RunOutcome::Failed
            },
            output,
        };
        append_run(&self.layout.run_log(id), run);
    }
}

/// The runs in a log file, oldest first. A missing or unreadable file has none.
fn read_runs(path: &Path) -> Vec<RunRecord> {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<RunFile>(&bytes).ok())
        .map(|file| file.runs)
        .unwrap_or_default()
}

/// Adds a run, dropping the oldest past `RUN_LIMIT`. A log that cannot be written is left as it was.
fn append_run(path: &Path, run: RunRecord) {
    let mut runs = read_runs(path);
    runs.push(run);
    let excess = runs.len().saturating_sub(RUN_LIMIT);
    runs.drain(..excess);
    let file = RunFile {
        version: RUN_FILE_VERSION,
        runs,
    };
    if let Ok(bytes) = serde_json::to_vec_pretty(&file) {
        let _ = write_atomic(path, &bytes);
    }
}

/// The size of a file in bytes, or zero when it does not exist: where a command's output starts.
pub(crate) fn log_len(path: &Path) -> u64 {
    fs::metadata(path).map_or(0, |metadata| metadata.len())
}
