//! Service integration tests: the service runs in-process and is driven through the client crate; raw frames cover
//! the cases the client cannot produce.

mod common;
mod config;
mod events;
mod handshake;
mod lifecycle;
mod registries;
