//! Probes of an installed kind's program (`--help` for its instruction channel, decision 0161; `--version` for its chat
//! support, decision 0101): the program run once with fixed arguments, its output kept per program path and modified
//! time, so a probe runs once per installed program and not per launch or listing.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use crate::account_check::{RunOutput, account_strip_names, run_probe};
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
        return known.clone();
    }
    let output = run_probe(program, args, &account_strip_names(decl), PROBE_TIMEOUT);
    probe_cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key, (output.clone(), Instant::now()));
    output
}
