//! Self-update of the service over the paired link (capability `self-update/1`, decision 0132).
//!
//! The client uploads a binary it already has in chunks, then asks the host to check it, swap it in and restart.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use crate::message::ProtocolRange;

/// Params: [`UpdateBeginParams`]. Result: [`UpdateBegun`].
pub const UPDATE_BEGIN: &str = "host.update_begin";
/// Params: [`UpdateChunkParams`]. Result: [`UpdateChunkAccepted`].
pub const UPDATE_CHUNK: &str = "host.update_chunk";
/// Params: [`UpdateFinishParams`]. Result: [`UpdateInstalled`]; the service restarts after answering.
pub const UPDATE_FINISH: &str = "host.update_finish";
/// Params: [`UpdateRollbackParams`]. Result: [`UpdateInstalled`]: the kept previous binary is put back and the service restarts.
pub const UPDATE_ROLLBACK: &str = "host.update_rollback";

/// Largest binary an upload may announce.
pub const MAX_BINARY_BYTES: u64 = 256 * 1024 * 1024;
/// Largest decoded chunk; its base64 stays well inside a network frame (1 MiB).
pub const MAX_CHUNK_BYTES: usize = 512 * 1024;

/// Refusal codes of the update methods.
pub mod codes {
    /// No supervisor can restart this service, so nothing is uploaded or replaced.
    pub const UNSUPPORTED: &str = "update.unsupported";
    /// The announced size is zero or above [`super::MAX_BINARY_BYTES`].
    pub const TOO_LARGE: &str = "update.too_large";
    /// The upload id is not the current upload's (another upload replaced it, or the service restarted).
    pub const UNKNOWN_UPLOAD: &str = "update.unknown_upload";
    /// A chunk does not start where the received bytes end, or runs past the announced size.
    pub const BAD_CHUNK: &str = "update.bad_chunk";
    /// The received bytes do not match the announced length or sha256; the installed binary is untouched.
    pub const CHECKSUM_MISMATCH: &str = "update.checksum_mismatch";
    /// The file does not run here, or its `--version` cannot be read, or its protocol range does not overlap this
    /// service's.
    pub const INCOMPATIBLE: &str = "update.incompatible";
    /// There is no kept previous binary to roll back to.
    pub const NO_PREVIOUS: &str = "update.no_previous";
    /// The binary is older than the running service and `allow_downgrade` was not set.
    pub const DOWNGRADE: &str = "update.downgrade";
    /// The binary has the running service's version and `force` was not set: nothing to do.
    pub const SAME_VERSION: &str = "update.same_version";
    /// The binary reports another version than the `expected_version` the person confirmed.
    pub const VERSION_MISMATCH: &str = "update.version_mismatch";
    /// Staging, swapping or restarting failed; the message names the step.
    pub const FAILED: &str = "update.failed";
}

/// Params of `host.update_begin`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateBeginParams {
    /// Exact length of the binary in bytes.
    pub size: u64,
    /// Lowercase hex sha256 of the binary.
    pub sha256: String,
}

/// Result of `host.update_begin`. A new upload replaces any unfinished one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateBegun {
    pub upload_id: String,
    /// Largest decoded chunk the service takes.
    pub max_chunk_bytes: u32,
    /// What restarts the service after the swap, such as `systemd`.
    pub supervisor: String,
}

/// Params of `host.update_chunk`. `data` is padded standard base64.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateChunkParams {
    pub upload_id: String,
    /// Where the chunk starts; it must equal the bytes received so far.
    pub offset: u64,
    pub data: String,
}

impl UpdateChunkParams {
    /// Encodes `bytes` as a chunk at `offset`.
    pub fn new(upload_id: &str, offset: u64, bytes: &[u8]) -> Self {
        Self {
            upload_id: upload_id.to_owned(),
            offset,
            data: STANDARD.encode(bytes),
        }
    }

    /// The chunk's bytes; `None` on bad base64.
    pub fn decode_data(&self) -> Option<Vec<u8>> {
        STANDARD.decode(&self.data).ok()
    }
}

/// Result of `host.update_chunk`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateChunkAccepted {
    pub received: u64,
}

/// Params of `host.update_finish`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateFinishParams {
    pub upload_id: String,
    /// The version the person confirmed; the binary must report exactly this one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<String>,
    /// Accepts a binary older than the running service.
    #[serde(default)]
    pub allow_downgrade: bool,
    /// Accepts a binary of the running service's own version.
    #[serde(default)]
    pub force: bool,
}

/// Params of `host.update_rollback`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateRollbackParams {
    /// The version the person confirmed; the kept binary must report exactly this one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<String>,
    /// Only checks the kept binary and answers its version: nothing is swapped or restarted.
    #[serde(default)]
    pub dry_run: bool,
}

/// Result of `host.update_finish` and `host.update_rollback`, sent before the restart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateInstalled {
    /// The version of the service that answers.
    pub previous_version: String,
    /// The version the installed binary reports; the restarted service should answer with it.
    pub version: String,
    pub protocol: ProtocolRange,
    pub supervisor: String,
}

/// What `massorca --version` reports: `massorca <version> (protocol <min>..<max>)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryVersion {
    pub version: String,
    pub protocol: ProtocolRange,
}

/// The `--version` text after the program name, as [`parse_version_line`] reads it.
pub fn version_text(version: &str, protocol: ProtocolRange) -> String {
    format!("{version} (protocol {}..{})", protocol.min, protocol.max)
}

/// Reads `massorca <version> (protocol <min>..<max>)` from the first line of `--version` output.
pub fn parse_version_line(output: &str) -> Option<BinaryVersion> {
    let line = output.lines().next()?.trim();
    let rest = line.strip_prefix("massorca ")?;
    let (version, range) = rest.split_once(" (protocol ")?;
    let (min, max) = range.strip_suffix(')')?.split_once("..")?;
    let plain = |part: &str| {
        part.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-+".contains(&b))
    };
    let (min, max) = (min.parse().ok()?, max.parse().ok()?);
    let numbered = version.split(['-', '+']).next()?.split('.').count() == 3;
    (!version.is_empty() && plain(version) && numbered && min <= max).then(|| BinaryVersion {
        version: version.to_owned(),
        protocol: ProtocolRange { min, max },
    })
}

/// Orders `X.Y.Z[-pre][+build]` versions: numbers first, then a release above its pre-releases. `None` when either
/// does not parse.
pub fn compare_versions(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    fn key(version: &str) -> Option<([u64; 3], Option<&str>)> {
        let core = version.split('+').next()?;
        let (numbers, pre) = match core.split_once('-') {
            Some((numbers, pre)) => (numbers, Some(pre)),
            None => (core, None),
        };
        let mut parts = numbers.split('.').map(|part| part.parse::<u64>().ok());
        let triple = [parts.next()??, parts.next()??, parts.next()??];
        parts.next().is_none().then_some((triple, pre))
    }
    let ((a_numbers, a_pre), (b_numbers, b_pre)) = (key(a)?, key(b)?);
    Some(
        a_numbers
            .cmp(&b_numbers)
            .then_with(|| match (a_pre, b_pre) {
                (None, None) => std::cmp::Ordering::Equal,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (Some(_), None) => std::cmp::Ordering::Less,
                (Some(a), Some(b)) => a.cmp(b),
            }),
    )
}

/// Whether two protocol ranges share a version.
pub fn overlaps(a: ProtocolRange, b: ProtocolRange) -> bool {
    a.min.max(b.min) <= a.max.min(b.max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_line_round_trips_and_garbage_is_refused() {
        let range = ProtocolRange { min: 1, max: 2 };
        let line = format!("massorca {}\n", version_text("0.3.1", range));
        assert_eq!(
            parse_version_line(&line),
            Some(BinaryVersion {
                version: "0.3.1".to_owned(),
                protocol: range
            })
        );
        for bad in [
            "massorca 0.3.1",
            "",
            "orca 0.3.1 (protocol 1..1)",
            "massorca 0.3 (protocol 1..1)",
            "massorca 0.3.1 (protocol 2..1)",
            "massorca 0;1.1 (protocol 1..1)",
        ] {
            assert_eq!(parse_version_line(bad), None, "{bad}");
        }
    }

    #[test]
    fn versions_order_by_number_then_release_above_pre_release() {
        use std::cmp::Ordering::{Equal, Greater, Less};
        assert_eq!(compare_versions("0.2.0", "0.10.0"), Some(Less));
        assert_eq!(compare_versions("1.0.0", "1.0.0-rc.1"), Some(Greater));
        assert_eq!(compare_versions("1.0.0+abc", "1.0.0"), Some(Equal));
        assert_eq!(compare_versions("1.0", "1.0.0"), None);
    }

    #[test]
    fn ranges_overlap_only_when_they_share_a_version() {
        let r = |min, max| ProtocolRange { min, max };
        assert!(overlaps(r(1, 1), r(1, 3)));
        assert!(overlaps(r(2, 5), r(1, 2)));
        assert!(!overlaps(r(1, 1), r(2, 3)));
    }
}
