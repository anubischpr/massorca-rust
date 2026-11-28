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
/// not refused for the service's life. A successful probe is kept until the program changes.
pub const FAILED_PROBE_TTL: Duration = Duration::from_secs(60);

/// A probe's identity: the program, its modified time, and the arguments it is run with.
type ProbeKey = (PathBuf, Option<SystemTime>, Vec<String>);

/// The probe outputs of this service.
fn probe_cache() -> &'static Mutex<HashMap<ProbeKey, (RunOutput, Instant)>> {
    static CACHE: OnceLock<Mutex<HashMap<ProbeKey, (RunOutput, Instant)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// What `program`, the kind's resolved program, prints for `args`: run with the names an account command strips removed
/// and no account home, for at most [`PROBE_TIMEOUT`], and kept per program path, modified time and arguments. A
/// replaced program has a new modified time, so it is probed again.
pub(crate) fn cached_probe(decl: &KindDecl, program: &Path, args: &[String]) -> RunOutput {
    probe_with(decl, program, args, PROBE_TIMEOUT, None).run
}

/// [`cached_probe`], for at most `timeout`, with `runner` standing in for the program when it is set, and whether the run
/// was stopped at the time limit. Such a run is no answer yet, so it is not kept: the next probe runs the program again
/// (decision 0059, amended 2026-11-27).
pub(crate) fn probe_with(
    decl: &KindDecl,
    program: &Path,
    args: &[String],
    timeout: Duration,
    runner: Option<&CheckRunner>,
) -> CheckRun {
    let modified = fs::metadata(program)
        .and_then(|metadata| metadata.modified())
        .ok();
    let key = (program.to_path_buf(), modified, args.to_vec());
    if let Some((known, at)) = probe_cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
        && (known.exit == Some(0) || at.elapsed() < FAILED_PROBE_TTL)
    {
        return CheckRun {
            run: known.clone(),
            timed_out: false,
        };
    }
    let probed = match runner {
        Some(runner) => runner(program, args),
        None => run_probe_timed(program, args, &account_strip_names(decl), timeout),
    };
    if !probed.timed_out {
        probe_cache()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key, (probed.run.clone(), Instant::now()));
    }
    probed
}
