//! The files a prompt sends with it (decision 0172). Each names a file of the session's workspace. The names are checked
//! when a prompt is sent and again when a queued prompt is typed. A terminal run types a mention for each file; a chat run
//! mentions its text files the same way.

use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use massorca_core_workspace::{FileRefusal, regular_file_inside};
use massorca_protocol::codes;
use serde_json::json;

use crate::sessions::AgentError;

/// The most files one prompt sends (decision 0172).
pub const MAX_ATTACHMENTS: usize = 10;
/// The most bytes of one image a chat run sends (decision 0172). Its base64 form, which is what the API counts, stays under
/// 5 MB, the per-image limit of Amazon Bedrock (the first-party API allows 10 MB).
pub const MAX_IMAGE_BYTES: u64 = 3 * 1024 * 1024;
/// The most bytes of any other file a prompt names (decision 0172).
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;

/// The image formats a chat run takes as image blocks, by extension, with their media types (decision 0172).
const IMAGE_TYPES: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
];

/// Why a terminal run refuses a prompt that names an image (decision 0172).
pub const TERMINAL_IMAGE_REFUSAL: &str =
    "a terminal run takes no images; attach text files, or use a chat session";

/// Image formats that no run takes, so a prompt naming one is refused (decision 0172).
const REFUSED_IMAGE_EXTENSIONS: &[&str] = &["avif", "bmp", "heic", "heif", "ico", "tif", "tiff"];

/// How a run takes a file (decision 0172).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A file the agent reads from its mention.
    File,
    /// An image: a terminal run refuses it, and a chat run sends it as a block of its own, with its media type.
    Image { media_type: &'static str },
}

/// A file of a prompt, resolved inside the session's workspace (decision 0172).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    /// The path from the workspace root, with `/` between names: the spelling a mention uses.
    pub relative: String,
    /// The canonical path of the file. A chat run reads it only to send an image.
    pub path: PathBuf,
    pub kind: Kind,
}

/// Resolves the files a prompt names against the session's workspace `root`. A prompt that names more than
/// [`MAX_ATTACHMENTS`] files is refused, and so is the first name that is not a regular file inside the root, that is
/// over its size cap, or that is an image format no run takes. The message names no path; the error's data does.
pub fn resolve(root: &Path, names: &[String]) -> Result<Vec<Attachment>, AgentError> {
    if names.len() > MAX_ATTACHMENTS {
        return Err(AgentError::new(
            codes::AGENT_ATTACHMENT_INVALID,
            format!(
                "a prompt takes at most {MAX_ATTACHMENTS} files, and this one names {}",
                names.len()
            ),
        ));
    }
    names.iter().map(|name| resolve_one(root, name)).collect()
}

fn resolve_one(root: &Path, name: &str) -> Result<Attachment, AgentError> {
    // A control character would break the composed text: a newline typed in plain paste mode is an Enter, which would
    // submit part of the prompt, or type a line of its own.
    if has_control(name) {
        return Err(invalid(name, CONTROL));
    }
    let file =
        regular_file_inside(root, name).map_err(|refusal| invalid(name, refusal_text(refusal)))?;
    // The name the file is mentioned by is the one its link reaches, which the person did not type.
    if has_control(&file.relative) {
        return Err(invalid(name, CONTROL));
    }
    let kind = kind_of(&file.path, name)?;
    let bytes = std::fs::metadata(&file.path)
        .map_err(|_| invalid(name, "cannot be read"))?
        .len();
    let cap = match kind {
        Kind::Image { .. } => MAX_IMAGE_BYTES,
        Kind::File => MAX_FILE_BYTES,
    };
    if bytes > cap {
        return Err(invalid(
            name,
            &format!("is over the {cap}-byte limit for its kind"),
        ));
    }
    Ok(Attachment {
        relative: file.relative,
        path: file.path,
        kind,
    })
}

/// The reason of a path that holds a control character.
const CONTROL: &str = "has a control character in its path";

/// Whether `text` holds a control character: C0 (newline, carriage return and tab included), DEL or C1.
fn has_control(text: &str) -> bool {
    text.chars().any(char::is_control)
}

fn refusal_text(refusal: FileRefusal) -> &'static str {
    match refusal {
        FileRefusal::Outside => "is outside the session's workspace",
        FileRefusal::Missing => "does not exist",
        FileRefusal::NotRegularFile => "is not a regular file",
        FileRefusal::NotText => "has a name that is not valid text",
    }
}

fn kind_of(path: &Path, name: &str) -> Result<Kind, AgentError> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if let Some(media_type) = IMAGE_TYPES
        .iter()
        .find_map(|(known, media_type)| (*known == extension).then_some(*media_type))
    {
        return Ok(Kind::Image { media_type });
    }
    if REFUSED_IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        return Err(refusal(
            codes::AGENT_ATTACHMENT_UNSUPPORTED,
            "is an image format no run takes; use png, jpeg, gif or webp",
            name,
        ));
    }
    Ok(Kind::File)
}

/// Refuses the prompt when it names an image, with `reason` as the message (decision 0172). A terminal run types text
/// only, so it refuses every image.
pub fn refuse_images(files: &[Attachment], reason: &str) -> Result<(), AgentError> {
    match files
        .iter()
        .find(|file| matches!(file.kind, Kind::Image { .. }))
    {
        Some(file) => Err(refusal(
            codes::AGENT_ATTACHMENT_UNSUPPORTED,
            reason,
            &file.relative,
        )),
        None => Ok(()),
    }
}

/// The prompt text with the files named (decision 0172): the text, then a mention for each file that is not an image. A
/// kind with a `mention` template names each file by it, `{path}` standing for the file's path (`@{path}` for Claude).
/// A kind without one gets the plain paths on the same line, `text (attached: a.txt, b/c.md)`: such a kind may type in
/// plain paste mode, where a line break is an Enter. [`resolve`] refuses a path with a control character, so the
/// mentions add no line break. Images are never named in the text.
pub fn with_mentions(text: &str, mention: Option<&str>, files: &[Attachment]) -> String {
    let named: Vec<&str> = files
        .iter()
        .filter(|file| file.kind == Kind::File)
        .map(|file| file.relative.as_str())
        .collect();
    if named.is_empty() {
        return text.to_owned();
    }
    let (separator, mentions) = match mention {
        Some(template) => (
            " ",
            named
                .iter()
                .map(|path| template.replace("{path}", path))
                .collect::<Vec<String>>()
                .join(" "),
        ),
        None => (" ", format!("(attached: {})", named.join(", "))),
    };
    if text.is_empty() {
        mentions
    } else {
        format!("{text}{separator}{mentions}")
    }
}

/// The bytes of an image, base64 encoded for an image block (decision 0172). The file is read when the prompt is sent,
/// through one handle: it must still be the regular file its check found, at the same canonical path, and no larger than
/// the image cap, since it may have changed since it was named.
pub fn image_data(file: &Attachment) -> Result<String, AgentError> {
    let bytes = read_checked(&file.path, MAX_IMAGE_BYTES)
        .map_err(|reason| invalid(&file.relative, reason))?;
    Ok(base64(&bytes))
}

/// The bytes of the file at `path`, the canonical path its containment check found, read through one handle. The handle
/// is opened without following a final link and without blocking on a FIFO. It must be a regular file and still the
/// file at `path` (its final path on Windows, its device and inode on Unix), checked before anything is read, and it is
/// read through `cap + 1` bytes at most, so a file over the cap is refused without being read whole.
fn read_checked(path: &Path, cap: u64) -> Result<Vec<u8>, &'static str> {
    const UNREADABLE: &str = "cannot be read";
    const OVER: &str = "is over the image size limit";
    let file = open_no_follow(path).map_err(|_| UNREADABLE)?;
    let meta = file.metadata().map_err(|_| UNREADABLE)?;
    if crate::credentials::not_regular(&meta).is_err() {
        return Err("is not a regular file");
    }
    if !handle_is_at(&file, &meta, path) {
        return Err("is outside the session's workspace");
    }
    if meta.len() > cap {
        return Err(OVER);
    }
    let mut bytes = Vec::new();
    file.take(cap + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| UNREADABLE)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > cap {
        return Err(OVER);
    }
    Ok(bytes)
}

/// Opens for reading without following a final link: `O_NOFOLLOW` on Unix, with `O_NONBLOCK` so a FIFO put in the
/// file's place never blocks the open; `FILE_FLAG_OPEN_REPARSE_POINT` on Windows, which opens a reparse point itself.
fn open_no_follow(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options.open(path)
}

/// Whether the open `file` is the file at the canonical `path`: the final path the handle reports is `path`. A folder
/// of the path swapped for a junction after the check gives the handle another final path.
#[cfg(windows)]
fn handle_is_at(file: &File, _meta: &std::fs::Metadata, path: &Path) -> bool {
    final_path(file).is_some_and(|reached| reached == path)
}

/// Whether the open `file` is the file at the canonical `path`: the path still resolves to itself, so no link was put
/// in it, and it names the device and inode of the handle.
#[cfg(unix)]
fn handle_is_at(_file: &File, meta: &std::fs::Metadata, path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    std::fs::canonicalize(path).is_ok_and(|resolved| resolved == path)
        && std::fs::metadata(path).is_ok_and(|at| at.dev() == meta.dev() && at.ino() == meta.ino())
}

#[cfg(not(any(unix, windows)))]
fn handle_is_at(_file: &File, _meta: &std::fs::Metadata, _path: &Path) -> bool {
    false
}

/// The final path of an open file, from `GetFinalPathNameByHandleW`, in the spelling `dunce::canonicalize` gives: the
/// `\\?\` prefix is dropped from a drive path. None when it cannot be read, or when it is not a drive path (a UNC or a
/// device path), which no canonical workspace file path is.
#[cfg(windows)]
#[allow(unsafe_code)]
fn final_path(file: &File) -> Option<PathBuf> {
    use std::ffi::{OsString, c_void};
    use std::os::windows::ffi::OsStringExt;
    use std::os::windows::io::AsRawHandle;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFinalPathNameByHandleW(
            file: *mut c_void,
            path: *mut u16,
            length: u32,
            flags: u32,
        ) -> u32;
    }
    // FILE_NAME_NORMALIZED | VOLUME_NAME_DOS, the form std's canonicalize asks for.
    const FLAGS: u32 = 0;
    let mut buffer = vec![0u16; 512];
    loop {
        let capacity = u32::try_from(buffer.len()).ok()?;
        // SAFETY: the handle is the open file's own and stays valid while `file` lives; the buffer holds `capacity`
        // u16s, and the call writes at most that many.
        let length = unsafe {
            GetFinalPathNameByHandleW(file.as_raw_handle(), buffer.as_mut_ptr(), capacity, FLAGS)
        };
        let length = usize::try_from(length).ok()?;
        if length == 0 {
            return None;
        }
        if length < buffer.len() {
            buffer.truncate(length);
            break;
        }
        // The buffer was too small: the length is the size needed, its terminating null included.
        buffer.resize(length, 0);
    }
    let text = OsString::from_wide(&buffer).into_string().ok()?;
    let drive = text.strip_prefix(r"\\?\")?;
    let bytes = drive.as_bytes();
    (bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'\\')
        .then(|| PathBuf::from(drive))
}

/// Standard base64 with padding (RFC 4648), the encoding of an image block.
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = u32::from(chunk[0]);
        let second = u32::from(chunk.get(1).copied().unwrap_or(0));
        let third = u32::from(chunk.get(2).copied().unwrap_or(0));
        let group = (first << 16) | (second << 8) | third;
        let symbol = |shift: u32| char::from(ALPHABET[((group >> shift) & 0x3f) as usize]);
        out.push(symbol(18));
        out.push(symbol(12));
        out.push(if chunk.len() > 1 { symbol(6) } else { '=' });
        out.push(if chunk.len() > 2 { symbol(0) } else { '=' });
    }
    out
}

fn invalid(name: &str, reason: &str) -> AgentError {
    refusal(
        codes::AGENT_ATTACHMENT_INVALID,
        &format!("an attachment {reason}"),
        name,
    )
}

fn refusal(code: &'static str, message: &str, name: &str) -> AgentError {
    let mut error = AgentError::new(code, message);
    error.data = Some(json!({ "path": name }));
    error
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// A workspace folder with each `(name, size)` file written into it, its folders made as needed.
    fn workspace(files: &[(&str, usize)]) -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("temp dir");
        for (name, size) in files {
            let path = root.path().join(name);
            fs::create_dir_all(path.parent().expect("a parent")).expect("folder");
            fs::write(&path, vec![b'x'; *size]).expect("file");
        }
        root
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| (*name).to_owned()).collect()
    }

    fn refused(root: &Path, list: &[&str]) -> AgentError {
        resolve(root, &names(list)).expect_err("refused")
    }

    #[test]
    fn a_file_inside_the_root_is_named_by_its_relative_path() {
        let root = workspace(&[("notes/a.txt", 3)]);
        let files = resolve(root.path(), &names(&["notes/a.txt"])).expect("inside");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative, "notes/a.txt");
        assert_eq!(files[0].kind, Kind::File);
    }

    #[test]
    fn an_image_takes_its_media_type_from_its_extension_in_any_case() {
        let root = workspace(&[("shots/a.PNG", 4), ("b.jpeg", 4)]);
        let files = resolve(root.path(), &names(&["shots/a.PNG", "b.jpeg"])).expect("inside");
        assert_eq!(
            files.iter().map(|file| file.kind).collect::<Vec<_>>(),
            [
                Kind::Image {
                    media_type: "image/png"
                },
                Kind::Image {
                    media_type: "image/jpeg"
                },
            ]
        );
    }

    #[test]
    fn base64_matches_the_rfc_4648_vectors() {
        let cases = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (plain, encoded) in cases {
            assert_eq!(base64(plain.as_bytes()), encoded, "{plain:?}");
        }
    }

    #[test]
    fn an_image_is_sent_as_the_base64_of_its_bytes() {
        let root = workspace(&[]);
        fs::write(root.path().join("dot.png"), b"foobar").expect("file");
        let file = resolve(root.path(), &names(&["dot.png"]))
            .expect("inside")
            .remove(0);
        assert_eq!(
            image_data(&file).expect("read"),
            "Zm9vYmFy",
            "the bytes of the file"
        );
    }

    #[test]
    fn an_image_format_no_run_takes_is_refused_as_unsupported() {
        let root = workspace(&[("scan.bmp", 4)]);
        let error = refused(root.path(), &["scan.bmp"]);
        assert_eq!(error.code, codes::AGENT_ATTACHMENT_UNSUPPORTED);
    }

    #[test]
    fn a_path_that_escapes_the_workspace_is_refused_with_the_path_in_its_data() {
        let root = workspace(&[]);
        let error = refused(root.path(), &["../outside.txt"]);
        assert_eq!(error.code, codes::AGENT_ATTACHMENT_INVALID);
        assert_eq!(error.data, Some(json!({ "path": "../outside.txt" })));
    }

    #[test]
    fn a_missing_file_and_a_directory_are_refused_as_invalid() {
        let root = workspace(&[]);
        fs::create_dir(root.path().join("folder")).expect("folder");
        assert_eq!(
            refused(root.path(), &["nope.txt"]).code,
            codes::AGENT_ATTACHMENT_INVALID
        );
        assert_eq!(
            refused(root.path(), &["folder"]).code,
            codes::AGENT_ATTACHMENT_INVALID
        );
    }

    #[test]
    fn a_file_over_its_size_cap_is_refused() {
        let over = MAX_FILE_BYTES as usize + 1;
        let root = workspace(&[("big.txt", over)]);
        assert_eq!(
            refused(root.path(), &["big.txt"]).code,
            codes::AGENT_ATTACHMENT_INVALID
        );
    }

    #[test]
    fn an_image_over_its_size_cap_is_refused_and_one_at_the_cap_is_taken() {
        let root = workspace(&[
            ("over.png", MAX_IMAGE_BYTES as usize + 1),
            ("at.png", MAX_IMAGE_BYTES as usize),
        ]);
        assert_eq!(
            refused(root.path(), &["over.png"]).code,
            codes::AGENT_ATTACHMENT_INVALID
        );
        assert!(resolve(root.path(), &names(&["at.png"])).is_ok());
    }

    #[test]
    fn a_prompt_naming_more_files_than_the_cap_is_refused_before_any_is_read() {
        let root = workspace(&[]);
        let many = vec!["missing.txt"; MAX_ATTACHMENTS + 1];
        let error = resolve(root.path(), &names(&many)).expect_err("refused");
        assert_eq!(error.code, codes::AGENT_ATTACHMENT_INVALID);
        assert!(error.message.contains("at most 10"), "{}", error.message);
    }

    #[test]
    fn a_terminal_run_refuses_every_image_and_takes_text_files() {
        let root = workspace(&[("a.png", 4), ("notes.txt", 4)]);
        let images = resolve(root.path(), &names(&["a.png"])).expect("inside");
        let error = refuse_images(&images, "a terminal run takes no images").expect_err("refused");
        assert_eq!(error.code, codes::AGENT_ATTACHMENT_UNSUPPORTED);
        let text = resolve(root.path(), &names(&["notes.txt"])).expect("inside");
        assert!(refuse_images(&text, "a terminal run takes no images").is_ok());
    }

    #[test]
    fn a_mention_template_names_each_file_and_a_kind_without_one_lists_the_paths_on_the_line() {
        let file = |relative: &str| Attachment {
            relative: relative.to_owned(),
            path: PathBuf::from(relative),
            kind: Kind::File,
        };
        let files = [file("a.txt"), file("b/c.md")];
        assert_eq!(
            with_mentions("look", Some("@{path}"), &files),
            "look @a.txt @b/c.md"
        );
        assert_eq!(with_mentions("", Some("@{path}"), &files), "@a.txt @b/c.md");
        let plain = with_mentions("look", None, &files);
        assert_eq!(plain, "look (attached: a.txt, b/c.md)");
        assert!(
            !plain.contains('\n'),
            "no line break: a plain paste types each as an Enter"
        );
        assert_eq!(with_mentions("", None, &files), "(attached: a.txt, b/c.md)");
        let image = Attachment {
            relative: "shot.png".to_owned(),
            path: PathBuf::from("shot.png"),
            kind: Kind::Image {
                media_type: "image/png",
            },
        };
        assert_eq!(
            with_mentions("look", Some("@{path}"), &[image]),
            "look",
            "an image is not named in the text"
        );
    }

    #[test]
    fn a_path_with_a_control_character_is_refused() {
        let root = workspace(&[("a.txt", 1)]);
        for name in [
            "a\nb.txt",
            "a\rb.txt",
            "a\tb.txt",
            "a\u{7f}.txt",
            "a\u{85}b.txt",
        ] {
            let error = refused(root.path(), &[name]);
            assert_eq!(error.code, codes::AGENT_ATTACHMENT_INVALID, "{name:?}");
            assert!(
                error.message.contains("control character"),
                "{name:?}: {}",
                error.message
            );
            assert_eq!(error.data, Some(json!({ "path": name })));
        }
    }

    /// Links the directory `link` to `target`: a junction on Windows, which needs no privilege, and a symlink elsewhere.
    fn link_directory(target: &Path, link: &Path) -> bool {
        #[cfg(windows)]
        {
            massorca_process::command("cmd")
                .arg("/C")
                .arg("mklink")
                .arg("/J")
                .arg(link)
                .arg(target)
                .output()
                .is_ok_and(|output| output.status.success())
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).is_ok()
        }
    }

    #[test]
    fn an_image_whose_folder_becomes_a_link_out_after_its_check_is_not_read() {
        let root = workspace(&[("shots/a.png", 4)]);
        let elsewhere = workspace(&[("a.png", 6)]);
        let file = resolve(root.path(), &names(&["shots/a.png"]))
            .expect("inside")
            .remove(0);
        fs::rename(root.path().join("shots"), root.path().join("old")).expect("move the folder");
        assert!(
            link_directory(elsewhere.path(), &root.path().join("shots")),
            "a junction or symlink can be made"
        );
        assert!(
            root.path().join("shots").join("a.png").is_file(),
            "the link resolves"
        );
        let error = image_data(&file).expect_err("refused");
        assert_eq!(error.code, codes::AGENT_ATTACHMENT_INVALID);
        assert_eq!(error.data, Some(json!({ "path": "shots/a.png" })));
    }

    #[test]
    fn an_image_that_became_a_folder_or_grew_over_the_cap_after_its_check_is_refused() {
        let root = workspace(&[("dir.png", 4), ("grow.png", 4)]);
        let files = resolve(root.path(), &names(&["dir.png", "grow.png"])).expect("inside");
        fs::remove_file(root.path().join("dir.png")).expect("remove");
        fs::create_dir(root.path().join("dir.png")).expect("folder");
        assert_eq!(
            image_data(&files[0]).expect_err("a folder").code,
            codes::AGENT_ATTACHMENT_INVALID
        );
        fs::write(
            root.path().join("grow.png"),
            vec![b'x'; MAX_IMAGE_BYTES as usize + 1],
        )
        .expect("grow");
        let error = image_data(&files[1]).expect_err("over the cap");
        assert!(error.message.contains("size limit"), "{}", error.message);
    }

    #[cfg(unix)]
    #[test]
    fn an_image_replaced_by_a_fifo_is_refused_without_blocking() {
        let root = workspace(&[("pipe.png", 4)]);
        let file = resolve(root.path(), &names(&["pipe.png"]))
            .expect("inside")
            .remove(0);
        let path = root.path().join("pipe.png");
        fs::remove_file(&path).expect("remove");
        let made = massorca_process::command("mkfifo")
            .arg(&path)
            .status()
            .is_ok_and(|status| status.success());
        assert!(made, "a FIFO can be made");
        let error = image_data(&file).expect_err("a FIFO");
        assert_eq!(error.code, codes::AGENT_ATTACHMENT_INVALID);
    }
}
