//! The MassOrca client for the phone app (decision 0080): a UniFFI binding over the network path of `massorca-client`.
//! Everything that decides correctness stays in Rust; the app owns screens, the camera and the Keystore.

uniffi::setup_scaffolding!();

mod agent_watch;
mod error;
mod git_status;
mod git_watch;
mod massorca;
mod offer;
mod store;
mod watch;
mod wire;
mod workspaces;

pub use agent_watch::{AgentEvent, AgentItem, AgentSessionSummary, AgentWatch};
pub use error::FfiError;
pub use git_status::{GitFileStatus, GitStatusSummary};
pub use git_watch::{GitEvent, GitItem, GitWatch};
pub use massorca::{Host, HostSummary, MassOrca, ProtocolInfo, WhoAmI};
pub use offer::{OfferSummary, parse_offer};
pub use store::{HostStore, SealedHosts, StorageError};
pub use watch::LinkState;
pub use workspaces::WorkspaceSummary;
