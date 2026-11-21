//! The network side of the service (decisions 0040 to 0045): listeners, the server identity, pairing and devices.

mod auth;
mod listen;
mod live;
mod pairing;
mod state;

pub(crate) use auth::authenticate;
pub use listen::{ListenSpec, ListenSpecError, NetworkConfig};
pub(crate) use live::LiveGuard;
pub(crate) use pairing::{DEFAULT_TTL, MAX_TTL};
pub use state::NetworkState;
