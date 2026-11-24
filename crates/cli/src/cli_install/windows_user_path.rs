//! The signed-in user's PATH in `HKCU\Environment`, read and written through the registry API.
#![allow(unsafe_code)]

use std::ptr;

use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_EXPAND_SZ, REG_SZ, RegCloseKey,
    RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
};

use super::user_path::{NOT_WRITABLE, PathValue, UNREADABLE, UserPath};
use crate::error::{CliError, exit};

const ENVIRONMENT_KEY: &str = "Environment";
const PATH_VALUE: &str = "Path";
/// Longest a hung window may hold up the broadcast before Windows moves on.
const BROADCAST_TIMEOUT_MS: u32 = 5_000;

/// The signed-in user's PATH. Only the `Path` value of `HKCU\Environment` is read or written.
#[derive(Debug)]
pub struct WindowsUserPath;

impl UserPath for WindowsUserPath {
    fn read(&self) -> Result<PathValue, CliError> {
        read_user_path()
    }

    fn write(&self, value: &PathValue) -> Result<(), CliError> {
        write_user_path(value)
    }
}

/// An open registry key, closed when dropped.
struct Key(HKEY);

impl Key {
    /// Opens `HKCU\Environment`; `Ok(None)` when the key does not exist.
    fn open(access: u32) -> Result<Option<Self>, WIN32_ERROR> {
        let name = wide(ENVIRONMENT_KEY);
        let mut key: HKEY = ptr::null_mut();
        // SAFETY: `name` is NUL-terminated and lives across the call; `key` receives the handle on success.
        let status =
            unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, name.as_ptr(), 0, access, &mut key) };
        match status {
            ERROR_SUCCESS => Ok(Some(Self(key))),
            ERROR_FILE_NOT_FOUND => Ok(None),
            failed => Err(failed),
        }
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful RegOpenKeyExW and is closed only here.
        unsafe { RegCloseKey(self.0) };
    }
}

fn read_user_path() -> Result<PathValue, CliError> {
    let empty = PathValue {
        text: String::new(),
        expandable: true,
    };
    let key = match Key::open(KEY_QUERY_VALUE) {
        Ok(Some(key)) => key,
        Ok(None) => return Ok(empty),
        Err(status) => return Err(unreadable(status)),
    };
    let name = wide(PATH_VALUE);
    let mut kind: u32 = 0;
    let mut size: u32 = 0;
    // SAFETY: `name` is NUL-terminated; a null buffer makes Windows report only the size and the type.
    let status = unsafe {
        RegQueryValueExW(
            key.0,
            name.as_ptr(),
            ptr::null(),
            &mut kind,
            ptr::null_mut(),
            &mut size,
        )
    };
    match status {
        ERROR_SUCCESS => {}
        ERROR_FILE_NOT_FOUND => return Ok(empty),
        failed => return Err(unreadable(failed)),
    }
    let mut units = vec![0u16; size.div_ceil(2) as usize];
    let mut bytes = size;
    // SAFETY: `units` holds at least `bytes` bytes, so Windows writes within the buffer.
    let status = unsafe {
        RegQueryValueExW(
            key.0,
            name.as_ptr(),
            ptr::null(),
            &mut kind,
            units.as_mut_ptr().cast::<u8>(),
            &mut bytes,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(unreadable(status));
    }
    let expandable = match kind {
        REG_EXPAND_SZ => true,
        REG_SZ => false,
        _ => {
            return Err(CliError::new(
                UNREADABLE,
                "your user PATH is stored as an unexpected registry type, so nothing was changed",
                exit::ERROR,
            ));
        }
    };
    let end = units
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(units.len());
    Ok(PathValue {
        text: String::from_utf16_lossy(&units[..end]),
        expandable,
    })
}

fn write_user_path(value: &PathValue) -> Result<(), CliError> {
    let key = Key::open(KEY_SET_VALUE)
        .map_err(not_writable)?
        .ok_or_else(|| not_writable(ERROR_FILE_NOT_FOUND))?;
    let name = wide(PATH_VALUE);
    let data = wide(&value.text);
    let kind = if value.expandable {
        REG_EXPAND_SZ
    } else {
        REG_SZ
    };
    let bytes = u32::try_from(data.len() * 2).map_err(|_| {
        CliError::new(
            NOT_WRITABLE,
            "your user PATH is too long to store",
            exit::ERROR,
        )
    })?;
    // SAFETY: `data` is NUL-terminated and lives across the call; `bytes` is its exact byte length.
    let status = unsafe {
        RegSetValueExW(
            key.0,
            name.as_ptr(),
            0,
            kind,
            data.as_ptr().cast::<u8>(),
            bytes,
        )
    };
    drop(key);
    if status != ERROR_SUCCESS {
        return Err(not_writable(status));
    }
    broadcast_environment_change();
    Ok(())
}

/// Tells running programs the environment changed, so new terminals see the PATH without a sign-out.
fn broadcast_environment_change() {
    let environment = wide(ENVIRONMENT_KEY);
    let mut result: usize = 0;
    // SAFETY: `environment` is NUL-terminated and outlives the call; the timeout keeps a hung window from blocking.
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            environment.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            BROADCAST_TIMEOUT_MS,
            &mut result,
        );
    }
}

/// A UTF-16 copy of `text` with its NUL terminator, as the registry API expects.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn unreadable(status: WIN32_ERROR) -> CliError {
    CliError::new(
        UNREADABLE,
        format!("could not read your user PATH (Windows error {status}), so nothing was changed"),
        exit::ERROR,
    )
}

fn not_writable(status: WIN32_ERROR) -> CliError {
    CliError::new(
        NOT_WRITABLE,
        format!("Windows did not let massorca change your user PATH (Windows error {status})"),
        exit::ERROR,
    )
}
