//! The secret vault's building blocks (decision 0071): the redacted [`SecretValue`], name and scope rules, the
//! XChaCha20-Poly1305 cipher and the OS-protected master key. No value, key byte or ciphertext reaches any error,
//! `Debug` output or log line.

mod cipher;
mod key;
mod names;
mod value;
mod vault;

pub use cipher::{Aad, AadError, NONCE_LEN, OpenError, SealError, Sealed, open, seal};
pub use key::{KeyError, KeyStore, LockReason, MasterKey, Protection, UnknownProtection};
pub use names::{Scope, ScopeError, is_valid_name, is_valid_reason, is_valid_session};
pub use value::{MAX_VALUE_BYTES, SecretValue, ValueTooLarge};
pub use vault::{Vault, VaultEntry, VaultError, VaultStatus, Written};
