//! Probes of an installed kind's program (`--help` for its instruction channel, decision 0161; `--version` for its chat
//! support, decision 0101): the program run once with fixed arguments, its output kept per program path and modified
//! time, so a probe runs once per installed program and not per launch or listing.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use crate::account_check::{CheckRun, RunOutput, account_strip_names, run_probe_timed};
use crate::check_retry::CheckRunner;
use crate::kind::KindDecl;

/// How long a probe may run before it counts as failed.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
/// How long a failed probe's output is kept before the program is probed again: a CLI that was busy or slow once is
/// not refused for the service's life. A successful probe is kept until the program changes. A probe stopped at its
/// time limit is kept as long, marked as a timeout, so a read under a gate does not run the program again.
pub const FAILED_PROBE_TTL: Duration = Duration::from_secs(60);

/// A probe's identity: the program, its modified time, and the arguments it is run with.
type ProbeKey = (PathBuf, Option<SystemTime>, Vec<String>);

/// A kept probe: its output, when it ran, and whether it was stopped at its time limit.
type Kept = (RunOutput, Instant, bool);

/// The probe outputs of this service.
fn probe_cache() -> &'static Mutex<HashMap<ProbeKey, Kept>> {
    static CACHE: OnceLock<Mutex<HashMap<ProbeKey, Kept>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// What `program`, the kind's resolved program, prints for `args`: run with the names an account command strips removed
/// and no account home, for at most [`PROBE_TIMEOUT`], and kept per program path, modified time and arguments. A
/// replaced program has a new modified time, so it is probed again.
pub(crate) fn cached_probe(decl: &KindDecl, program: &Path, args: &[String]) -> RunOutput {
    probe_with(decl, program, args, PROBE_TIMEOUT, None, false).run
}

/// [`cached_probe`], for at most `timeout`, with `runner` standing in for the program when it is set, and whether the run
/// was stopped at the time limit. Such a run is no answer yet: it is kept for [`FAILED_PROBE_TTL`] marked as a timeout,
/// and a read of it says so, so a caller can retry it. `fresh` runs the program whatever is kept, as a background retry
/// does (decision 0059, amended 2026-11-27).
pub(crate) fn probe_with(
    decl: &KindDecl,
    program: &Path,
    args: &[String],
    timeout: Duration,
    runner: Option<&CheckRunner>,
    fresh: bool,
) -> CheckRun {
    let modified = fs::metadata(program)
        .and_then(|metadata| metadata.modified())
        .ok();
    let key = (program.to_path_buf(), modified, args.to_vec());
    if !fresh
        && let Some((known, at, timed_out)) = probe_cache()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
        && ((known.exit == Some(0) && !timed_out) || at.elapsed() < FAILED_PROBE_TTL)
    {
        return CheckRun {
            run: known.clone(),
            timed_out: *timed_out,
        };
    }
    let probed = match runner {
        Some(runner) => runner(program, args),
        None => run_probe_timed(program, args, &account_strip_names(decl), timeout),
    };
    probe_cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key, (probed.run.clone(), Instant::now(), probed.timed_out));
    probed
}
