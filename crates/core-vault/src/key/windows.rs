//! DPAPI wrapping of the master key: current-user scope, never showing UI.
#![expect(
    unsafe_code,
    reason = "DPAPI is a C API; decision 0007 allows unsafe only in named Windows modules"
)]

use std::io;
use std::ptr;

use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
use zeroize::{Zeroize, Zeroizing};

/// Ties the blob to this use, so another program unprotecting it by accident gets nothing.
const ENTROPY: &[u8] = b"massorca-vault/1";

fn blob_of(bytes: &[u8]) -> io::Result<CRYPT_INTEGER_BLOB> {
    Ok(CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(bytes.len())
            .map_err(|_| io::Error::other("the DPAPI input is too large"))?,
        // DPAPI only reads its input blobs.
        pbData: bytes.as_ptr().cast_mut(),
    })
}

/// Wraps `plain` for the current user.
pub(super) fn protect(plain: &[u8]) -> io::Result<Vec<u8>> {
    let input = blob_of(plain)?;
    let entropy = blob_of(ENTROPY)?;
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };
    // SAFETY: `input` and `entropy` point at live slices of the stated lengths for the whole call; the description,
    // reserved and prompt pointers may be null; `output` is a valid out-parameter DPAPI fills with a LocalAlloc block.
    let ok = unsafe {
        CryptProtectData(
            &input,
            ptr::null(),
            &entropy,
            ptr::null(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    if output.pbData.is_null() {
        return Err(io::Error::other("DPAPI returned no data"));
    }
    // SAFETY: on success `output` holds `cbData` bytes at `pbData`, owned by us until LocalFree below.
    let wrapped =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    // SAFETY: `pbData` came from DPAPI's LocalAlloc and is freed exactly once.
    unsafe { LocalFree(output.pbData.cast()) };
    Ok(wrapped)
}

/// Unwraps a blob made by [`protect`] for the current user.
pub(super) fn unprotect(wrapped: &[u8]) -> io::Result<Zeroizing<Vec<u8>>> {
    let input = blob_of(wrapped)?;
    let entropy = blob_of(ENTROPY)?;
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };
    // SAFETY: as in `protect`; the description out-pointer may be null when the caller does not want it.
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            ptr::null_mut(),
            &entropy,
            ptr::null(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    if output.pbData.is_null() {
        return Err(io::Error::other("DPAPI returned no data"));
    }
    // SAFETY: on success `output` holds `cbData` writable bytes at `pbData`, owned by us until LocalFree below.
    let plain = unsafe { std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize) };
    let copy = Zeroizing::new(plain.to_vec());
    plain.zeroize();
    // SAFETY: `pbData` came from DPAPI's LocalAlloc and is freed exactly once; `plain` is not used after this.
    unsafe { LocalFree(output.pbData.cast()) };
    Ok(copy)
}
