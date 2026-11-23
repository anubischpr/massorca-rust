//! The MassOrca client for the phone app (decision 0080): a UniFFI binding over the network path of `massorca-client`.
//! Everything that decides correctness stays in Rust; the app owns screens, the camera and the Keystore.

uniffi::setup_scaffolding!();

mod error;
mod offer;
mod store;

pub use error::FfiError;
pub use offer::{OfferSummary, parse_offer};
pub use store::{HostStore, SealedHosts, StorageError};
