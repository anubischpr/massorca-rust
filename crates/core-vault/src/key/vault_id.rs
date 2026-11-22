//! `<vault>/id`: 16 random bytes in hex that name the vault in a keyring, so no item carries a filesystem path.

use std::io;
use std::path::Path;

use super::file;

const ID_FILE: &str = "id";
const ID_BYTES: usize = 16;

/// The recorded id, or `NotFound` when there is none; anything that is not 32 hex digits is `InvalidData`.
pub(super) fn read(vault_dir: &Path) -> io::Result<String> {
    let bytes = file::read_small(&vault_dir.join(ID_FILE))?;
    let text = std::str::from_utf8(&bytes)
        .map(str::trim)
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
    if text.len() == ID_BYTES * 2 && text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(text.to_ascii_lowercase())
    } else {
        Err(io::Error::from(io::ErrorKind::InvalidData))
    }
}

/// The recorded id, or a new one written now; a creation that failed earlier left its id to be reused.
pub(super) fn read_or_create(vault_dir: &Path) -> io::Result<String> {
    match read(vault_dir) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        other => return other,
    }
    let mut bytes = [0u8; ID_BYTES];
    getrandom::fill(&mut bytes).map_err(|_| io::Error::other("no random source"))?;
    let id: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    file::write_new(&vault_dir.join(ID_FILE), format!("{id}\n").as_bytes())?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::{read, read_or_create};

    #[test]
    fn an_id_is_created_once_then_read_back() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            read(dir.path()).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
        let id = read_or_create(dir.path()).expect("created");
        assert_eq!(id.len(), 32);
        assert!(id.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(read_or_create(dir.path()).expect("reused"), id);
        assert_eq!(read(dir.path()).expect("read"), id);
        let path = dir.path().to_string_lossy().into_owned();
        assert!(!id.contains(&path));
    }

    #[test]
    fn a_damaged_id_is_invalid_data_and_never_replaced() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("id"), b"/home/someone/vault\n").expect("write");
        for result in [read(dir.path()), read_or_create(dir.path())] {
            assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
        }
    }
}
