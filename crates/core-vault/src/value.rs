//! The plaintext of one secret, which never prints and is zeroed on drop.

use std::fmt;

use zeroize::{Zeroize, Zeroizing};

/// The largest value `secret.set` accepts, in bytes of UTF-8.
pub const MAX_VALUE_BYTES: usize = 64 * 1024;

/// A secret's plaintext. It has no `Clone` and no `Serialize`, so a copy can only be made on purpose.
pub struct SecretValue(Zeroizing<String>);

/// The value was longer than [`MAX_VALUE_BYTES`]; it was zeroed before this error was returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the secret value is longer than {MAX_VALUE_BYTES} bytes")]
pub struct ValueTooLarge;

impl SecretValue {
    /// Takes ownership of `value`, refusing one over [`MAX_VALUE_BYTES`].
    ///
    /// # Errors
    /// [`ValueTooLarge`] when the value is too long.
    pub fn new(mut value: String) -> Result<Self, ValueTooLarge> {
        if value.len() > MAX_VALUE_BYTES {
            value.zeroize();
            return Err(ValueTooLarge);
        }
        Ok(Self(Zeroizing::new(value)))
    }

    /// The plaintext. This is the only way to the bytes: call it only where the value leaves the service,
    /// into the environment of a process the service starts.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

impl fmt::Display for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}
