//! Coding agents in workspaces (`docs/architecture/agents.md`, decisions 0050 to 0057).
//!
//! Kinds are data: JSON declarations, built in or in the profile, naming mechanisms from a closed set. No protocol IO;
//! the service maps requests onto this crate.

pub mod account_check;
pub mod accounts;
pub mod catalog;
pub mod credentials;
pub mod event;
pub mod kind;
pub mod launch;
mod modes;
pub mod packages;
pub mod paths;
mod process_tree;
pub mod prompt;
pub mod provided_env;
pub mod resolve;
pub mod sessions;
pub mod sign_in;
pub mod status;

pub use catalog::{Catalog, CatalogEntry, OWNER_CORE, OWNER_USER, built_ins};
pub use event::{DoneOutcome, EventTarget};
pub use kind::KindDecl;
pub use packages::{BuiltInPackages, KindPackages, PackageKind, PackageState, used_kinds};
pub use provided_env::{EnvProvider, PROVIDER_TIMEOUT, ProvidedEnv, ProviderError, SecretGrant};
pub use resolve::SearchEnv;
pub use sessions::{
    AgentError, Agents, AgentsConfig, Change, DELEGATION_DEPTH_DEFAULT, DELEGATION_DEPTH_MAX,
    DELEGATION_DEPTH_MIN, DELEGATION_DEPTH_SETTING, Listener, MergeResult, RepoOps,
};
