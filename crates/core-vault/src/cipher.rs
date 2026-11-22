//! XChaCha20-Poly1305 over one secret, bound to its row, scope and name by the associated data.

use std::fmt;

use chacha20poly1305::aead::AeadInOut;
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use zeroize::{Zeroize, Zeroizing};

use crate::key::MasterKey;
use crate::names::{Scope, is_valid_name};
use crate::value::SecretValue;

/// Bytes in a nonce; a fresh random one is drawn for every seal.
pub const NONCE_LEN: usize = 24;

const TAG_LEN: usize = 16;
const AAD_PREFIX: &[u8] = b"massorca-secret/1";

/// The associated data of one row: `massorca-secret/1`, id, scope and name, separated by NUL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aad(Vec<u8>);

/// Why an [`Aad`] was refused. Parts are NUL-free so the encoding has one reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AadError {
    #[error("the row id is empty or contains NUL")]
    InvalidId,
    #[error("the scope id is invalid")]
    InvalidScope,
    #[error("the secret name is invalid")]
    InvalidName,
}

impl Aad {
    /// Builds the associated data for row `id`.
    ///
    /// # Errors
    /// An [`AadError`] naming the invalid part.
    pub fn new(id: &str, scope: &Scope, name: &str) -> Result<Self, AadError> {
        if id.is_empty() || id.contains('\0') {
            return Err(AadError::InvalidId);
        }
        if !scope.is_valid() {
            return Err(AadError::InvalidScope);
        }
        if !is_valid_name(name) {
            return Err(AadError::InvalidName);
        }
        let scope = scope.to_string();
        let mut bytes =
            Vec::with_capacity(AAD_PREFIX.len() + id.len() + scope.len() + name.len() + 3);
        for part in [AAD_PREFIX, id.as_bytes(), scope.as_bytes(), name.as_bytes()] {
            if !bytes.is_empty() {
                bytes.push(0);
            }
            bytes.extend_from_slice(part);
        }
        Ok(Self(bytes))
    }
}

/// A sealed value as the `secrets` table stores it.
#[derive(Clone, PartialEq, Eq)]
pub struct Sealed {
    pub nonce: [u8; NONCE_LEN],
    pub ciphertext: Vec<u8>,
}

impl fmt::Debug for Sealed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sealed")
            .field("ciphertext_len", &self.ciphertext.len())
            .finish_non_exhaustive()
    }
}

/// Sealing failed; neither the value nor the key is part of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SealError {
    #[error("the operating system's random source is unavailable")]
    Random,
    #[error("the value could not be encrypted")]
    Encrypt,
}

/// The ciphertext did not open: a wrong key, a moved or renamed row, or tampering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the secret could not be decrypted")]
pub struct OpenError;

fn cipher(key: &MasterKey) -> XChaCha20Poly1305 {
    XChaCha20Poly1305::new(key.bytes().into())
}

/// Encrypts `value` under `key` with a fresh random nonce.
///
/// # Errors
/// [`SealError`] when randomness is unavailable or encryption fails.
pub fn seal(key: &MasterKey, aad: &Aad, value: &SecretValue) -> Result<Sealed, SealError> {
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).map_err(|_| SealError::Random)?;
    let plaintext = value.expose().as_bytes();
    // Room for the tag up front: a reallocation would free a plaintext copy without zeroing it.
    let mut buffer = Zeroizing::new(Vec::with_capacity(plaintext.len() + TAG_LEN));
    buffer.extend_from_slice(plaintext);
    cipher(key)
        .encrypt_in_place(&XNonce::from(nonce), &aad.0, &mut *buffer)
        .map_err(|_| SealError::Encrypt)?;
    Ok(Sealed {
        nonce,
        ciphertext: std::mem::take(&mut *buffer),
    })
}

/// Decrypts a sealed value; any mismatch of key, nonce, associated data or bytes fails.
///
/// # Errors
/// [`OpenError`] whatever the cause, so a caller cannot tell a wrong key from tampering.
pub fn open(
    key: &MasterKey,
    aad: &Aad,
    nonce: &[u8],
    ciphertext: &[u8],
) -> Result<SecretValue, OpenError> {
    let nonce = XNonce::try_from(nonce).map_err(|_| OpenError)?;
    let mut buffer = Zeroizing::new(ciphertext.to_vec());
    cipher(key)
        .decrypt_in_place(&nonce, &aad.0, &mut *buffer)
        .map_err(|_| OpenError)?;
    let plaintext = String::from_utf8(std::mem::take(&mut *buffer)).map_err(|error| {
        error.into_bytes().zeroize();
        OpenError
    })?;
    SecretValue::new(plaintext).map_err(|_| OpenError)
}
