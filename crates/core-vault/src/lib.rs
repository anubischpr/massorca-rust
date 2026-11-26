//! The secret vault's building blocks (decision 0071): the redacted [`SecretValue`], name and scope rules, the
//! XChaCha20-Poly1305 cipher and the OS-protected master key. No value, key byte or ciphertext reaches any error,
//! `Debug` output or log line.

mod cipher;
mod env_policy;
mod key;
mod names;
mod value;
mod vault;

pub use cipher::{Aad, AadError, NONCE_LEN, OpenError, SealError, Sealed, open, seal};
pub use env_policy::is_dangerous_env_name;
pub use key::{KeyError, KeyStore, LockReason, MasterKey, Protection, UnknownProtection};
pub use names::{Scope, ScopeError, is_valid_name, is_valid_reason, is_valid_session};
pub use value::{MAX_VALUE_BYTES, SecretValue, ValueTooLarge};
pub use vault::{
    ANSWERED_RETENTION_DAYS, MAX_PENDING_REQUESTS, MAX_REQUESTS_PER_MINUTE, PENDING_EXPIRY_DAYS,
    ScopedName, ServiceConnection, ServiceField, StoredService, Vault, VaultEntry, VaultError,
    VaultStatus, Written,
};
