//! The MassOrca protocol: message types, framing, versions and capabilities.
//!
//! Plain data plus optional async framing behind the `tokio` feature; no other IO.

pub mod capabilities;
pub mod codes;

/// Highest protocol version this build speaks.
pub const PROTOCOL_VERSION: u32 = 1;

/// Lowest protocol version this build still accepts.
pub const MIN_PROTOCOL_VERSION: u32 = 1;
