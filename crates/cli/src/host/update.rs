//! `massorca host update NAME`: updates a paired host's service over its connection (decision 0132). The person
//! confirms each host, version against version; the host re-checks the sha256, the version rule and `--version` before it
//! swaps the binary and restarts.

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_client::{Client, Hosts, validate_host_name};
use massorca_profile::Profile;
use massorca_protocol::capabilities::SELF_UPDATE;
use massorca_protocol::codes::SERVICE_CAPABILITY_MISSING;
use massorca_protocol::methods::self_update::{
    self as methods, MAX_BINARY_BYTES, UpdateBeginParams, UpdateBegun, UpdateChunkAccepted,
    UpdateChunkParams, UpdateFinishParams, UpdateInstalled, UpdateRollbackParams, compare_versions,
    parse_version_line,
};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::args::HostUpdateArgs;
use crate::connect::peer;
use crate::cwd_path;
use crate::error::{CliError, exit};
use crate::output::Output;

/// How long the CLI waits for the restarted service to answer with the new version.
const RESTART_WAIT: Duration = Duration::from_secs(90);
/// Tests only: a shorter wait, in milliseconds.
pub const TEST_WAIT_ENV: &str = "MASSORCA_TEST_UPDATE_WAIT_MS";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// What `host update` did.
#[derive(Debug, Serialize)]
pub struct UpdateReport {
    pub name: String,
    /// `updated` or `rolled_back`.
    pub outcome: &'static str,
    pub previous_version: String,
    pub version: String,
    pub supervisor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// The binary to send, read and checked on this computer before the host is contacted.
struct Candidate {
    bytes: Vec<u8>,
    sha256: String,
    version: String,
}

/// Refuses a host that predates `self-update/1` before anything is uploaded.
pub fn require_self_update(offered: &[String]) -> Result<(), CliError> {
    if offered.iter().any(|name| name == SELF_UPDATE) {
        return Ok(());
    }
    Err(CliError::new(
        SERVICE_CAPABILITY_MISSING,
        format!(
            "the host does not offer {SELF_UPDATE}: its service predates host update; upgrade it once with host add-ssh or by hand"
        ),
        exit::ERROR,
    ))
}

pub async fn execute(
    profile: &Profile,
    args: &HostUpdateArgs,
    output: Output,
) -> Result<(), CliError> {
    validate_host_name(&args.name)?;
    let hosts = Hosts::of(profile);
    if args.rollback {
        return rollback(&hosts, args, output).await;
    }
    let candidate = prepare(args).await?;
    let client = Client::connect_host(&hosts, &args.name, &peer()).await?;
    require_self_update(&client.welcome().capabilities)?;
    let current = client.welcome().server.version.clone();
    let change = version_change(&current, &candidate.version, args)?;
    let review = vec![
        format!("host: {}", args.name),
        format!("current version: {current}"),
        format!("version to install: {}{change}", candidate.version),
        format!(
            "binary: {} bytes, sha256 {}",
            candidate.bytes.len(),
            candidate.sha256
        ),
    ];
    confirm(
        args.yes,
        output,
        &review,
        &args.name,
        &current,
        &candidate.version,
    )?;

    let begun: UpdateBegun = client
        .call(
            methods::UPDATE_BEGIN,
            &UpdateBeginParams {
                size: candidate.bytes.len() as u64,
                sha256: candidate.sha256.clone(),
            },
        )
        .await?;
    let step = (begun.max_chunk_bytes as usize).clamp(1, methods::MAX_CHUNK_BYTES);
    output.progress(&format!("uploading {} bytes", candidate.bytes.len()));
    for (index, part) in candidate.bytes.chunks(step).enumerate() {
        let offset = (index * step) as u64;
        let _: UpdateChunkAccepted = client
            .call(
                methods::UPDATE_CHUNK,
                &UpdateChunkParams::new(&begun.upload_id, offset, part),
            )
            .await?;
    }
    let finish = UpdateFinishParams {
        upload_id: begun.upload_id,
        expected_version: Some(candidate.version.clone()),
        allow_downgrade: args.allow_downgrade,
        force: args.force,
    };
    let installed: UpdateInstalled = client.call(methods::UPDATE_FINISH, &finish).await?;
    drop(client);
    output.progress(&format!(
        "installed {}; waiting for the restarted service",
        installed.version
    ));
    wait_for_version(&hosts, &args.name, &installed.version).await?;
    report(
        output,
        &args.name,
        "updated",
        installed,
        Some(candidate.sha256),
    );
    Ok(())
}

/// Asks the host which version its kept binary is, confirms it, then puts it back.
async fn rollback(hosts: &Hosts, args: &HostUpdateArgs, output: Output) -> Result<(), CliError> {
    let client = Client::connect_host(hosts, &args.name, &peer()).await?;
    require_self_update(&client.welcome().capabilities)?;
    let named: UpdateInstalled = client
        .call(
            methods::UPDATE_ROLLBACK,
            &UpdateRollbackParams {
                expected_version: None,
                dry_run: true,
            },
        )
        .await?;
    let review = vec![
        format!("host: {}", args.name),
        format!("current version: {}", named.previous_version),
        format!(
            "version to restore: {} (the binary the last update replaced)",
            named.version
        ),
    ];
    confirm(
        args.yes,
        output,
        &review,
        &args.name,
        &named.previous_version,
        &named.version,
    )?;
    let params = UpdateRollbackParams {
        expected_version: Some(named.version),
        dry_run: false,
    };
    let installed: UpdateInstalled = client.call(methods::UPDATE_ROLLBACK, &params).await?;
    drop(client);
    output.progress(&format!(
        "restored {}; waiting for the restarted service",
        installed.version
    ));
    wait_for_version(hosts, &args.name, &installed.version).await?;
    report(output, &args.name, "rolled_back", installed, None);
    Ok(())
}

/// Reads the binary, checks its sha256 against `--sha256`, and learns its version from `--version` or `--expect-version`.
async fn prepare(args: &HostUpdateArgs) -> Result<Candidate, CliError> {
    let own = std::env::current_exe()?;
    let path = match &args.binary {
        Some(path) => PathBuf::from(cwd_path::absolute(path)?),
        None => own.clone(),
    };
    if args.sha256.is_none() && !same_file(&path, &own) {
        return Err(CliError::new(
            "usage.sha256_required",
            "--binary needs --sha256 with the checksum its release publishes, unless it is this massorca itself",
            exit::USAGE,
        ));
    }
    let size = std::fs::metadata(&path)?.len();
    if size == 0 || size > MAX_BINARY_BYTES {
        return Err(CliError::new(
            methods::codes::TOO_LARGE,
            format!(
                "a binary is 1 byte to {MAX_BINARY_BYTES} bytes; {} is {size} bytes",
                path.display()
            ),
            exit::ERROR,
        ));
    }
    let (bytes, reported) = tokio::task::spawn_blocking(move || {
        let bytes = std::fs::read(&path)?;
        let reported = massorca_core_remote::self_update::probe_version(&path)
            .ok()
            .and_then(|line| parse_version_line(&line))
            .map(|parsed| parsed.version);
        Ok::<_, io::Error>((bytes, reported))
    })
    .await??;
    let sha256: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if let Some(expected) = &args.sha256
        && !expected.eq_ignore_ascii_case(&sha256)
    {
        return Err(CliError::new(
            methods::codes::CHECKSUM_MISMATCH,
            format!("the binary's sha256 is {sha256}, not the {expected} given; nothing was sent"),
            exit::ERROR,
        ));
    }
    let version = match (reported, &args.expect_version) {
        (Some(reported), Some(expected)) if &reported != expected => {
            return Err(CliError::new(
                methods::codes::VERSION_MISMATCH,
                format!(
                    "the binary reports {reported}, not the {expected} given; nothing was sent"
                ),
                exit::ERROR,
            ));
        }
        (Some(reported), _) => reported,
        (None, Some(expected)) => expected.clone(),
        (None, None) => {
            return Err(CliError::new(
                "usage.version_required",
                "the binary does not run on this computer, so its version is unknown: give it with --expect-version",
                exit::USAGE,
            ));
        }
    };
    Ok(Candidate {
        bytes,
        sha256: sha256.to_ascii_lowercase(),
        version,
    })
}

/// The CLI's own downgrade rule, which the host enforces again. Returns the note the review shows.
fn version_change(
    current: &str,
    version: &str,
    args: &HostUpdateArgs,
) -> Result<&'static str, CliError> {
    match compare_versions(version, current) {
        Some(std::cmp::Ordering::Greater) => Ok(""),
        Some(std::cmp::Ordering::Equal) if args.force => Ok(" (reinstall of the same version)"),
        Some(std::cmp::Ordering::Equal) => Err(CliError::new(
            methods::codes::SAME_VERSION,
            format!(
                "host {} already runs {current}: nothing to do (--force reinstalls it)",
                args.name
            ),
            exit::ERROR,
        )),
        _ if args.allow_downgrade => Ok(" (DOWNGRADE)"),
        _ => Err(CliError::new(
            methods::codes::DOWNGRADE,
            format!(
                "{version} is not newer than the {current} host {} runs; --allow-downgrade installs it",
                args.name
            ),
            exit::ERROR,
        )),
    }
}

/// `--yes`, or a `y` typed at a terminal. Anything else prints the review and refuses; nothing has been sent.
fn confirm(
    yes: bool,
    output: Output,
    review: &[String],
    name: &str,
    current: &str,
    version: &str,
) -> Result<(), CliError> {
    if yes {
        return Ok(());
    }
    let interactive = !output.is_json() && io::stdin().is_terminal() && io::stderr().is_terminal();
    if interactive {
        let mut stderr = io::stderr();
        for line in review {
            let _ = writeln!(stderr, "{line}");
        }
        let _ = write!(stderr, "Update host {name}? [y/N] ");
        let _ = stderr.flush();
        let mut answer = String::new();
        io::stdin().lock().read_line(&mut answer)?;
        if matches!(answer.trim(), "y" | "Y" | "yes") {
            return Ok(());
        }
    } else {
        output.text_lines(review);
    }
    let mut error = CliError::new(
        "update.confirmation_required",
        format!("host {name} was not updated: confirm at a terminal, or pass --yes"),
        exit::ERROR,
    );
    error.data = Some(Box::new(
        json!({ "name": name, "current_version": current, "version": version }),
    ));
    Err(error)
}

/// Reconnects until the host answers with `version`. A host that never does is left for `--rollback` or the recovery
/// steps of decision 0132.
async fn wait_for_version(hosts: &Hosts, name: &str, version: &str) -> Result<(), CliError> {
    let wait = std::env::var(TEST_WAIT_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
        .map_or(RESTART_WAIT, Duration::from_millis);
    let deadline = Instant::now() + wait;
    let mut last = String::from("no answer yet");
    while Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(500)).await;
        match tokio::time::timeout(CONNECT_TIMEOUT, Client::connect_host(hosts, name, &peer()))
            .await
        {
            Ok(Ok(client)) if client.welcome().server.version == version => return Ok(()),
            Ok(Ok(client)) => {
                last = format!("it still answers as {}", client.welcome().server.version)
            }
            Ok(Err(error)) => last = error.to_string(),
            Err(_) => last = "the connection timed out".to_owned(),
        }
    }
    Err(CliError::new(
        "update.not_confirmed",
        format!(
            "host {name} did not come back as {version} within {} seconds ({last}); if it stays down, restore the kept binary \
             as decision 0132 describes, or run host update {name} --rollback once it answers",
            wait.as_secs()
        ),
        exit::UNAVAILABLE,
    ))
}

fn report(
    output: Output,
    name: &str,
    outcome: &'static str,
    installed: UpdateInstalled,
    sha256: Option<String>,
) {
    let line = format!(
        "host {name}: {} -> {} (restarted by {})",
        installed.previous_version, installed.version, installed.supervisor
    );
    let report = UpdateReport {
        name: name.to_owned(),
        outcome,
        previous_version: installed.previous_version,
        version: installed.version,
        supervisor: installed.supervisor,
        sha256,
    };
    output.success(&report, &[line]);
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}
