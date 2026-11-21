//! The MassOrca protocol: message types, framing, versions and capabilities.
//!
//! Plain data plus optional async framing behind the `tokio` feature; no other IO.

pub mod capabilities;
pub mod codes;
pub mod frame;
pub mod message;
pub mod methods;

pub use frame::{FrameError, FrameKind, MAX_FRAME_LEN_LOCAL, encode_frame};
#[cfg(feature = "tokio")]
pub use frame::{read_message, write_message};
pub use message::{ErrorBody, Message, PeerInfo, ProtocolRange};

/// Highest protocol version this build speaks.
pub const PROTOCOL_VERSION: u32 = 1;

/// Lowest protocol version this build still accepts.
pub const MIN_PROTOCOL_VERSION: u32 = 1;
