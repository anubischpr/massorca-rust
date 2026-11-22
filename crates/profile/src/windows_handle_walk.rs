//! A walk below a folder that never goes back to a path (ADR 0073): each folder is listed through its own handle
//! (`GetFileInformationByHandleEx` with `FileFullDirectoryInfo`), and each entry is opened by name relative to that
//! handle (`NtCreateFile` with a `RootDirectory`) with `FILE_OPEN_REPARSE_POINT`, so renaming a folder or swapping one
//! for a link mid-walk cannot redirect it. Any reparse point below is refused.
//! Every unsafe call states, in its `SAFETY` comment, the invariant it relies on.
#![allow(unsafe_code)]

use std::fs::File;
use std::io;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::{Path, PathBuf};
use std::ptr;

use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows_sys::Wdk::Storage::FileSystem::{
    FILE_OPEN, FILE_OPEN_FOR_BACKUP_INTENT, FILE_OPEN_REPARSE_POINT, FILE_SYNCHRONOUS_IO_NONALERT,
    NtCreateFile,
};
use windows_sys::Win32::Foundation::{
    ERROR_NO_MORE_FILES, HANDLE, OBJ_CASE_INSENSITIVE, RtlNtStatusToDosError, UNICODE_STRING,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_FULL_DIR_INFO, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FileFullDirectoryInfo,
    FileFullDirectoryRestartInfo, GetFileInformationByHandleEx, READ_CONTROL, SYNCHRONIZE,
    WRITE_DAC,
};
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

use crate::root_access::RootAccessError;

/// What a walk opens every entry with: enough to read and change its DACL, and to list a folder.
const ACCESS: u32 =
    READ_CONTROL | WRITE_DAC | FILE_READ_ATTRIBUTES | FILE_LIST_DIRECTORY | SYNCHRONIZE;

/// One entry below the walked folder, held open.
#[derive(Debug)]
pub(crate) struct Entry {
    /// For messages only; the entry is reached through `file`.
    pub path: PathBuf,
    pub file: File,
    pub is_dir: bool,
}

/// Opens `dir` itself, never a link's target, with the access a walk needs. It shares everything, so a pinned folder
/// can be opened this way while its pin is held.
pub(crate) fn open_folder(dir: &Path) -> Result<File, RootAccessError> {
    let file = std::fs::OpenOptions::new()
        .access_mode(ACCESS)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(dir)?;
    if file.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(RootAccessError::UnsafeLocation {
            path: dir.to_path_buf(),
        });
    }
    Ok(file)
}

/// Every entry below the folder `dir`, held open; a reparse point anywhere below is refused with `UnsafeLocation`
/// before the walk goes on. `dir_path` names the folder in messages.
pub(crate) fn walk_below(dir: &File, dir_path: &Path) -> Result<Vec<Entry>, RootAccessError> {
    let mut entries: Vec<Entry> = Vec::new();
    let mut pending: Vec<Option<usize>> = vec![None];
    while let Some(index) = pending.pop() {
        let (folder, folder_path) = match index {
            None => (dir, dir_path.to_path_buf()),
            Some(index) => (&entries[index].file, entries[index].path.clone()),
        };
        let mut found = Vec::new();
        for name in list_names(folder)? {
            let path = folder_path.join(String::from_utf16_lossy(&name));
            let file = open_relative(folder, &name)?;
            let attributes = file.metadata()?.file_attributes();
            if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                return Err(RootAccessError::UnsafeLocation { path });
            }
            found.push(Entry {
                path,
                file,
                is_dir: attributes & FILE_ATTRIBUTE_DIRECTORY != 0,
            });
        }
        for entry in found {
            let is_dir = entry.is_dir;
            entries.push(entry);
            if is_dir {
                pending.push(Some(entries.len() - 1));
            }
        }
    }
    Ok(entries)
}

/// The names in the folder `folder` holds, read through the handle; `.` and `..` are left out.
fn list_names(folder: &File) -> io::Result<Vec<Vec<u16>>> {
    let handle = folder.as_raw_handle();
    // u64 words keep each FILE_FULL_DIR_INFO record aligned; records are padded to eight bytes.
    let mut buffer = vec![0_u64; 8 * 1024];
    let size = u32::try_from(buffer.len() * size_of::<u64>()).unwrap_or(u32::MAX);
    let mut names = Vec::new();
    let mut class = FileFullDirectoryRestartInfo;
    loop {
        // SAFETY: `handle` is an open folder with FILE_LIST_DIRECTORY, and `buffer` holds `size` writable bytes.
        let read = unsafe {
            GetFileInformationByHandleEx(handle, class, buffer.as_mut_ptr().cast(), size)
        };
        if read == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) {
                return Ok(names);
            }
            return Err(error);
        }
        class = FileFullDirectoryInfo;
        let base = buffer.as_ptr().cast::<u8>();
        let mut offset = 0_usize;
        loop {
            // SAFETY: the call filled the buffer with records; `offset` is the start of one, inside the buffer.
            let record = unsafe { &*base.add(offset).cast::<FILE_FULL_DIR_INFO>() };
            let length = record.FileNameLength as usize / size_of::<u16>();
            // SAFETY: the record's name is `FileNameLength` bytes long and starts at FileName, inside the buffer.
            let name = unsafe { std::slice::from_raw_parts(record.FileName.as_ptr(), length) };
            if name != [u16::from(b'.')] && name != [u16::from(b'.'), u16::from(b'.')] {
                names.push(name.to_vec());
            }
            if record.NextEntryOffset == 0 {
                break;
            }
            offset += record.NextEntryOffset as usize;
        }
    }
}

/// Opens the entry `name` of the folder `parent` held open, by name relative to that handle, never following a link.
fn open_relative(parent: &File, name: &[u16]) -> io::Result<File> {
    let bytes = u16::try_from(std::mem::size_of_val(name))
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a name is too long"))?;
    let object_name = UNICODE_STRING {
        Length: bytes,
        MaximumLength: bytes,
        Buffer: name.as_ptr().cast_mut(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent.as_raw_handle(),
        ObjectName: &object_name,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: ptr::null(),
        SecurityQualityOfService: ptr::null(),
    };
    let mut handle: HANDLE = ptr::null_mut();
    // SAFETY: an all-zero IO_STATUS_BLOCK is a valid value for the call to overwrite.
    let mut status_block: IO_STATUS_BLOCK = unsafe { std::mem::zeroed() };
    // SAFETY: `attributes` points at `object_name` and the parent handle, which outlive the call, and `name` backs the
    // UNICODE_STRING; `handle` and `status_block` are valid out-pointers; no extended attributes are passed.
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            ACCESS,
            &attributes,
            &mut status_block,
            ptr::null(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            FILE_OPEN,
            FILE_OPEN_REPARSE_POINT | FILE_OPEN_FOR_BACKUP_INTENT | FILE_SYNCHRONOUS_IO_NONALERT,
            ptr::null(),
            0,
        )
    };
    if status < 0 {
        // SAFETY: RtlNtStatusToDosError only maps a status value to an error code.
        let code = unsafe { RtlNtStatusToDosError(status) };
        return Err(io::Error::from_raw_os_error(code as i32));
    }
    // SAFETY: the call succeeded, so `handle` is an open handle nothing else owns.
    Ok(unsafe { File::from_raw_handle(handle) })
}
