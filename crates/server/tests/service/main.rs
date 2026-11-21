//! Service integration tests: the service runs in-process and is driven through the client crate; raw frames cover
//! the cases the client cannot produce.

mod capabilities;
mod common;
mod concurrency;
mod config;
mod core_methods;
mod events;
mod handshake;
mod lifecycle;
mod registries;
