//! The MassOrca protocol: message types, framing, versions and capabilities.
//!
//! Plain data plus optional async framing behind the `tokio` feature; no other IO.

pub mod capabilities;
pub mod codes;
pub mod frame;
pub mod message;
pub mod methods;
pub mod version;

pub use frame::{
    Frame, FrameError, FrameKind, MAX_FRAME_LEN_LOCAL, encode_frame, encode_stream_bytes,
    fits_frame, fits_stream_bytes,
};
#[cfg(feature = "tokio")]
pub use frame::{read_frame, read_message, write_frame, write_message};
pub use message::{Auth, ErrorBody, Message, PeerInfo, ProtocolRange};
pub use version::negotiate;

/// Highest protocol version this build speaks.
pub const PROTOCOL_VERSION: u32 = 1;

/// Lowest protocol version this build still accepts.
pub const MIN_PROTOCOL_VERSION: u32 = 1;
