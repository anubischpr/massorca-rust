//! A walk below a folder that never goes back to a path (ADR 0073): each folder is listed through its own handle
//! (`GetFileInformationByHandleEx` with `FileFullDirectoryInfo`), and each entry is opened by name relative to that
//! handle (`NtCreateFile` with a `RootDirectory`) with `FILE_OPEN_REPARSE_POINT`, so renaming a folder or swapping one
//! for a link mid-walk cannot redirect it. Any reparse point below is refused. The walk goes depth first and holds only
//! the folders on the current path open; it refuses a tree deeper than `MAX_DEPTH` or larger than `MAX_ENTRIES`.
//! Every unsafe call states, in its `SAFETY` comment, the invariant it relies on.
#![allow(unsafe_code)]

use std::fs::File;
use std::io;
use std::mem::offset_of;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::Path;
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

/// How many levels below the walked folder an entry may be: an entry 64 levels down is visited, one deeper refused.
pub(crate) const MAX_DEPTH: usize = 64;

/// How many entries a walk visits before it refuses the folder.
pub(crate) const MAX_ENTRIES: usize = 10_000;

/// Whether a walk only reads each entry's DACL or also changes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WalkAccess {
    Read,
    Change,
}

impl WalkAccess {
    /// What an entry is opened with: its attributes and DACL, the right to change the DACL when the walk does, and the
    /// right to list it only for a folder. A file is never opened for its data.
    fn rights(self, is_dir: bool) -> u32 {
        let mut rights = READ_CONTROL | FILE_READ_ATTRIBUTES | SYNCHRONIZE;
        if self == Self::Change {
            rights |= WRITE_DAC;
        }
        if is_dir {
            rights |= FILE_LIST_DIRECTORY;
        }
        rights
    }
}

/// One entry below the walked folder, open while it is visited.
#[derive(Debug)]
pub(crate) struct Entry<'a> {
    /// For messages only; the entry is reached through `file`.
    pub path: &'a Path,
    pub file: &'a File,
    pub is_dir: bool,
}

/// Opens `dir` itself, never a link's target, to walk it. It shares everything, so a pinned folder can be opened this
/// way while its pin is held.
pub(crate) fn open_folder(dir: &Path, access: WalkAccess) -> Result<File, RootAccessError> {
    let file = std::fs::OpenOptions::new()
        .access_mode(access.rights(true))
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

/// Visits every entry below the folder `dir`, depth first, each opened relative to its folder and closed once visited;
/// only the folders on the current path stay open. A reparse point is refused with `UnsafeLocation`, and a tree deeper
/// than `MAX_DEPTH` or with more than `MAX_ENTRIES` entries is refused before the walk goes on.
pub(crate) fn walk_below(
    dir: &File,
    dir_path: &Path,
    access: WalkAccess,
    visit: &mut dyn FnMut(Entry<'_>) -> Result<(), RootAccessError>,
) -> Result<(), RootAccessError> {
    let mut visited = 0;
    walk_folder(dir, dir_path, 1, access, &mut visited, visit)
}

fn walk_folder(
    folder: &File,
    folder_path: &Path,
    depth: usize,
    access: WalkAccess,
    visited: &mut usize,
    visit: &mut dyn FnMut(Entry<'_>) -> Result<(), RootAccessError>,
) -> Result<(), RootAccessError> {
    for (name, listed_attributes) in list_entries(folder)? {
        let path = folder_path.join(String::from_utf16_lossy(&name));
        if depth > MAX_DEPTH {
            return Err(too_large(
                &path,
                &format!("it is nested more than {MAX_DEPTH} levels deep"),
            ));
        }
        *visited += 1;
        if *visited > MAX_ENTRIES {
            return Err(too_large(
                &path,
                &format!("the folder holds more than {MAX_ENTRIES} entries"),
            ));
        }
        if listed_attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(RootAccessError::UnsafeLocation { path });
        }
        let listed_dir = listed_attributes & FILE_ATTRIBUTE_DIRECTORY != 0;
        let file = open_relative(folder, &name, access.rights(listed_dir))?;
        // The entry itself decides, in case it changed since the listing.
        let attributes = file.metadata()?.file_attributes();
        if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(RootAccessError::UnsafeLocation { path });
        }
        let is_dir = attributes & FILE_ATTRIBUTE_DIRECTORY != 0;
        if is_dir != listed_dir {
            return Err(RootAccessError::UnsafeLocation { path });
        }
        visit(Entry {
            path: &path,
            file: &file,
            is_dir,
        })?;
        if is_dir {
            walk_folder(&file, &path, depth + 1, access, visited, visit)?;
        }
    }
    Ok(())
}

fn too_large(path: &Path, why: &str) -> RootAccessError {
    RootAccessError::Io(io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{} is refused: {why}", path.display()),
    ))
}

/// The names and attributes of the entries the folder `folder` holds, read through the handle; `.` and `..` are left
/// out.
fn list_entries(folder: &File) -> io::Result<Vec<(Vec<u16>, u32)>> {
    let handle = folder.as_raw_handle();
    // u64 words keep each FILE_FULL_DIR_INFO record aligned; records are padded to eight bytes.
    let mut buffer = vec![0_u64; 8 * 1024];
    let size = u32::try_from(buffer.len() * size_of::<u64>()).unwrap_or(u32::MAX);
    let mut entries = Vec::new();
    let mut class = FileFullDirectoryRestartInfo;
    loop {
        buffer.fill(0);
        // SAFETY: `handle` is an open folder with FILE_LIST_DIRECTORY, and `buffer` holds `size` writable bytes.
        let read = unsafe {
            GetFileInformationByHandleEx(handle, class, buffer.as_mut_ptr().cast(), size)
        };
        if read == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) {
                return Ok(entries);
            }
            return Err(error);
        }
        class = FileFullDirectoryInfo;
        // SAFETY: the u64 buffer is initialised and `size` bytes long; viewing it as bytes needs no alignment.
        let bytes =
            unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast::<u8>(), size as usize) };
        entries.extend(parse_records(bytes)?);
    }
}

/// The records of one `FileFullDirectoryInfo` reply, read field by field from the bytes with every offset and length
/// checked against the buffer first; malformed data is refused, never read.
fn parse_records(bytes: &[u8]) -> io::Result<Vec<(Vec<u16>, u32)>> {
    const NEXT: usize = offset_of!(FILE_FULL_DIR_INFO, NextEntryOffset);
    const ATTRIBUTES: usize = offset_of!(FILE_FULL_DIR_INFO, FileAttributes);
    const NAME_LENGTH: usize = offset_of!(FILE_FULL_DIR_INFO, FileNameLength);
    const NAME: usize = offset_of!(FILE_FULL_DIR_INFO, FileName);
    let malformed = |why: &str| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("a folder listing is malformed: {why}"),
        )
    };
    let field = |at: usize| -> io::Result<u32> {
        bytes
            .get(at..at + 4)
            .and_then(|field| field.try_into().ok())
            .map(u32::from_le_bytes)
            .ok_or_else(|| malformed("a record runs past the buffer"))
    };
    let mut records = Vec::new();
    let mut start = 0_usize;
    loop {
        if start.checked_add(NAME).is_none_or(|end| end > bytes.len()) {
            return Err(malformed("a record runs past the buffer"));
        }
        let next = field(start + NEXT)? as usize;
        let attributes = field(start + ATTRIBUTES)?;
        let name_length = field(start + NAME_LENGTH)? as usize;
        if !name_length.is_multiple_of(2) {
            return Err(malformed("a name has an odd length"));
        }
        let name_start = start + NAME;
        let name_end = name_start
            .checked_add(name_length)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| malformed("a name runs past the buffer"))?;
        if next != 0 && next < NAME + name_length {
            return Err(malformed("a record overlaps the next one"));
        }
        let name: Vec<u16> = bytes[name_start..name_end]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|unit| u16::from_le_bytes(*unit))
            .collect();
        if name != [u16::from(b'.')] && name != [u16::from(b'.'), u16::from(b'.')] {
            records.push((name, attributes));
        }
        if next == 0 {
            return Ok(records);
        }
        start = start
            .checked_add(next)
            .filter(|next_start| *next_start < bytes.len())
            .ok_or_else(|| malformed("the next record starts past the buffer"))?;
    }
}

/// Opens the entry `name` of the folder `parent` held open, by name relative to that handle, never following a link.
fn open_relative(parent: &File, name: &[u16], rights: u32) -> io::Result<File> {
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
            rights,
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

#[cfg(test)]
mod tests {
    use std::mem::offset_of;

    use windows_sys::Win32::Storage::FileSystem::FILE_FULL_DIR_INFO;

    use super::parse_records;

    const NAME: usize = offset_of!(FILE_FULL_DIR_INFO, FileName);

    /// One record: its next offset, attributes, a name length, and the name's units.
    fn record(next: u32, attributes: u32, name_length: u32, name: &[u16]) -> Vec<u8> {
        let mut bytes = vec![0_u8; NAME];
        bytes[offset_of!(FILE_FULL_DIR_INFO, NextEntryOffset)..][..4]
            .copy_from_slice(&next.to_le_bytes());
        bytes[offset_of!(FILE_FULL_DIR_INFO, FileAttributes)..][..4]
            .copy_from_slice(&attributes.to_le_bytes());
        bytes[offset_of!(FILE_FULL_DIR_INFO, FileNameLength)..][..4]
            .copy_from_slice(&name_length.to_le_bytes());
        for unit in name {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        while !bytes.len().is_multiple_of(8) {
            bytes.push(0);
        }
        bytes
    }

    fn units(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    #[test]
    fn well_formed_records_are_read_and_dot_entries_left_out() {
        let dot = record(0, 0x10, 2, &units("."));
        let office = record(0, 0x10, 12, &units("office"));
        let mut chained = record(u32::try_from(dot.len()).unwrap(), 0x10, 2, &units("."));
        chained.extend_from_slice(&office);

        let records = parse_records(&chained).expect("parsed");

        assert_eq!(records, vec![(units("office"), 0x10)]);
    }

    #[test]
    fn malformed_records_are_refused_not_read() {
        let truncated = vec![0_u8; NAME - 1];
        assert!(parse_records(&truncated).is_err(), "a truncated record");

        let overlong = record(0, 0, 400, &units("office"));
        assert!(
            parse_records(&overlong).is_err(),
            "a name longer than the buffer"
        );

        let odd = record(0, 0, 3, &units("office"));
        assert!(parse_records(&odd).is_err(), "a name of an odd length");

        let past = record(4096, 0, 12, &units("office"));
        assert!(
            parse_records(&past).is_err(),
            "a next record past the buffer"
        );

        let overlapping = record(8, 0, 12, &units("office"));
        assert!(
            parse_records(&overlapping).is_err(),
            "a next record inside this one"
        );
    }
}
