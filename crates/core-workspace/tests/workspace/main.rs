//! Projects and workspaces against real git repositories and a real store (ADR 0008).

mod branch_prefix;
mod concurrency;
mod create_folder;
mod create_worktree;
mod errors;
mod fixture;
mod generated_names;
mod interrupted_create;
mod names;
mod names_and_paths;
mod projects;
mod read;
mod removal_edges;
mod remove;
mod rollback;
mod selectors;
