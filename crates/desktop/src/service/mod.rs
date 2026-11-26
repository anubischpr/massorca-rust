//! The desktop's link to the MassOrca service (decision 0028): a bridge that owns the tokio
//! runtime, the connection states, the updates the UI folds into its store, and the typed
//! refusals that pick a dialog.

mod accounts;
mod agent_chat;
mod agent_forget;
mod agent_queue;
mod agents;
mod bridge;
mod connection;
mod extensions;
mod git;
mod hosted_services;
mod hosts;
mod launch;
mod quit;
mod refusal;
mod secrets;
mod skills;
mod team_room;
mod team_routines;
mod teams;
mod terminals;
mod update;
mod workspace_create;

#[cfg(test)]
mod git_tests;
#[cfg(test)]
mod secrets_tests;

#[cfg(test)]
pub(crate) use accounts::AccountCommand;
pub use agent_chat::{ChatUnsupportedReason, KindChat};
pub use agent_forget::ForgetSessionParams;
#[cfg(test)]
pub(crate) use agent_queue::QueueCommand;
#[cfg(test)]
pub(crate) use bridge::{Command, Script};
pub use bridge::{LinkCloser, ServiceBridge, ServiceHandle, connect_options};
pub use connection::{
    BACKOFF_MAX, BACKOFF_START, Backoff, ConnectionState, after_failed_attempt, after_loss,
    reason_text,
};
#[cfg(test)]
pub(crate) use extensions::ExtensionCommand;
pub use extensions::{
    ExtensionHistory, ExtensionsRead, HostListing, HostLists, HostMarketplaceRead, RollbackAnswer,
};
#[cfg(test)]
pub(crate) use hosted_services::ServicesCommand;
pub use hosts::{HostLoss, HostRemoval, SshAdded, SshFailed, SshHost, bundled_program};
pub(crate) use launch::truthy;
pub use launch::{LaunchOptions, autostart_next_to};
pub use quit::{QUIT_BOUND, close_link_on_quit, close_link_within};
pub use refusal::Refusal;
#[cfg(test)]
pub(crate) use secrets::SecretCommand;
pub use skills::{SkillsListing, SkillsRead};
#[cfg(test)]
pub(crate) use team_room::RoomCommand;
pub use team_room::{NO_ROOM, TeamRoomRead, TeamRooms};
#[cfg(test)]
pub(crate) use team_routines::RoutineCommand;
#[cfg(test)]
pub(crate) use teams::TeamCommand;
pub use teams::{ProjectRecord, TeamIds, TeamRecord, TeamsRead};
pub use terminals::{AttachRole, StreamEnd, TerminalStream, TerminalStreamItem, feed_events};
#[cfg(test)]
pub(crate) use terminals::{TerminalCommand, TerminalWrite};
pub use update::{
    GitUpdate, HostEntry, HostLink, SecretsState, SecretsUpdate, StoreState, StoreUpdate,
    WorkspaceGit,
};
pub use workspace_create::create_for_start;

#[cfg(test)]
pub(crate) use update::fixtures;
