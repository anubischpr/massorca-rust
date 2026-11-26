//! The per-run cache: a second report reuses the first detection, and a refresh detects again.

use std::time::Duration;

use massorca_core_tools::probe::{ToolSpec, probe_tools};
use massorca_core_tools::tool_cache::ToolCache;
use massorca_protocol::methods::service::ToolKind;

use crate::fixture::{env_with_path, runs, versioned};

#[test]
fn a_report_is_reused_until_a_refresh_asks_again() {
    let dir = tempfile::tempdir().expect("temp dir");
    let log = dir.path().join("runs.log");
    versioned(dir.path(), "gh", "gh version 2.40.1", &log);
    let specs = [ToolSpec {
        id: "gh",
        kind: ToolKind::Forge,
        program: "gh",
        version_args: Some(&["--version"]),
    }];
    let env = env_with_path(dir.path());
    let detect = || probe_tools(&specs, &env, Duration::from_secs(5));
    let cache = ToolCache::default();

    let first = cache.report(false, detect);
    let second = cache.report(false, detect);
    assert_eq!(runs(&log), 1);
    assert_eq!(first, second);

    let third = cache.report(true, detect);
    assert_eq!(runs(&log), 2);
    assert_eq!(third[0].version.as_deref(), Some("2.40.1"));
}
