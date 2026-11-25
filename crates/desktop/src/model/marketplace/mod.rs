//! The Marketplace as plain data: catalogue items, hosts and agent accounts, behind a source trait
//! that the sample implements. No GPUI types here, so the rules test as Rust.

mod accounts;
mod agent_links;
mod catalogue;
mod extension_items;
mod extension_refusals;
mod host_target;
mod hosted_services;
mod install_details;
mod install_review;
mod item_images;
mod item_page;
mod package_problem;
mod remove_review;
mod rollback_review;
mod run_log;
mod sample;
mod service_accounts;
mod service_source;
mod skills;
mod source;
mod types;

pub use accounts::{
    AccountActivity, AccountView, TerminalSignIn, account_block, account_views, busy_refusal,
    connection_rows, import_outcome, import_update_sentence, terminal_sign_in,
};
pub use agent_links::{AgentLink, installed_agent_links};
pub use catalogue::{count_line, counts, heading, intro, listed, no_match_text, status_word};
pub use extension_items::risk_word;
pub use extension_refusals::extension_refusal_text;
pub use host_target::HostTarget;
pub use hosted_services::{
    ConnectFields, HostedServicesView, ServiceAction, ServiceRow, ServicesState, connect_fields,
    service_row,
};
pub use install_details::review_details;
pub use install_review::{ReviewOutcome, installer_runs, review_outcome};
pub use item_images::{ImageKey, ImageSlots};
pub use item_page::{
    CLI_REMOVAL_SENTENCE, PARTS, Part, absent_reason, advanced_rows, banner_palette_slot,
    banner_tag, information, information_links, offers_cli_removal, settings_rows, state_chip,
    uninstall_body, uninstall_body_for,
};
pub use package_problem::{
    PackageAction, ProblemAlert, ProblemTone, package_refusal, problem_alert,
};
pub use remove_review::{RemoveOutcome, remove_details, remove_outcome};
pub use rollback_review::{RollbackOutcome, rollback_details, rollback_outcome};
pub use run_log::{RunRow, run_log_text, run_row};
pub use sample::SampleMarketplaceSource;
pub use service_accounts::ServiceAccounts;
pub use service_source::{ServiceMarketplace, agent_snapshot};
pub use skills::{RootRow, SkillRow, root_rows, search_skills, skill_count_line, skill_rows};
pub use source::{MarketplaceSource, SourceError, UnavailableMarketplace};
pub use types::{
    AccountId, AccountState, AgentAccount, DEFAULT_AGENT_KEY, Host, HostId, HostState, ItemKind,
    ItemState, MarketplaceItem, MarketplaceItemId, MarketplaceSnapshot, SignInFlow, SignInStep,
};
