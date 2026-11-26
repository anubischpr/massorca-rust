//! The files a prompt sends with it (decision 0172). Each names a file of the session's workspace. The names are checked
//! when a prompt is sent and again when a queued prompt is typed. A terminal run types a mention for each file; a chat run
//! mentions its text files the same way.

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
    let file =
        regular_file_inside(root, name).map_err(|refusal| invalid(name, refusal_text(refusal)))?;
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
/// A kind without one gets the plain paths, one to a line. Images are never named in the text.
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
        None => ("\n", named.join("\n")),
    };
    if text.is_empty() {
        mentions
    } else {
        format!("{text}{separator}{mentions}")
    }
}

/// The bytes of an image, base64 encoded for an image block (decision 0172). The file is read when the prompt is sent, and
/// checked against the image cap again, since it may have grown since it was named.
pub fn image_data(file: &Attachment) -> Result<String, AgentError> {
    let bytes = std::fs::read(&file.path).map_err(|_| invalid(&file.relative, "cannot be read"))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_IMAGE_BYTES {
        return Err(invalid(&file.relative, "is over the image size limit"));
    }
    Ok(base64(&bytes))
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
    fn a_mention_template_names_each_file_and_a_kind_without_one_lists_the_paths() {
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
        assert_eq!(with_mentions("look", None, &files), "look\na.txt\nb/c.md");
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
}
