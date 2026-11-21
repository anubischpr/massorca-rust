//! Protocol version negotiation for the `hello` / `welcome` handshake.

use crate::codes;
use crate::message::{ErrorBody, ProtocolRange};

/// Picks the highest version both ranges contain.
///
/// # Errors
///
/// A `protocol.unsupported_version` [`ErrorBody`] when the ranges do not overlap; the server
/// then answers `rejected` and closes.
pub fn negotiate(client: ProtocolRange, server: ProtocolRange) -> Result<u32, ErrorBody> {
    let highest = client.max.min(server.max);
    let lowest = client.min.max(server.min);
    if highest >= lowest {
        Ok(highest)
    } else {
        Err(ErrorBody::new(
            codes::PROTOCOL_UNSUPPORTED_VERSION,
            format!(
                "client speaks protocol {}..={}, server speaks {}..={}",
                client.min, client.max, server.min, server.max
            ),
        ))
    }
}
