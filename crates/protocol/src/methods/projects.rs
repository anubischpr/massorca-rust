//! Project methods: registered repositories and folders.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Deserializer, Serialize};

/// Params: [`ProjectAddParams`]. Result: [`Project`].
pub const ADD: &str = "project.add";
/// Params: none. Result: [`ProjectList`].
pub const LIST: &str = "project.list";
/// Params: [`ProjectRef`]. Result: [`Project`].
pub const SHOW: &str = "project.show";
/// Params: [`ProjectRemoveParams`]. Result: [`Project`].
pub const REMOVE: &str = "project.remove";
/// Params: [`ProjectInspectParams`]. Result: [`ProjectInspection`]. Read-only: what `project.add` would register.
pub const INSPECT: &str = "project.inspect";
/// Params: [`ProjectSetBaseParams`]. Result: [`Project`].
pub const SET_BASE: &str = "project.set_base";
/// Params: [`ProjectUpdateParams`]. Result: [`Project`]. Capability `project-update/1`.
pub const UPDATE: &str = "project.update";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectKind {
    Git,
    Folder,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Params of `project.add`. Unset options take the service's defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectAddParams {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ProjectKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspaces_root: Option<String>,
    /// The icon to show. Unset, the service takes one from the repository's files when it has one (decision 0091).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<ProjectIcon>,
}

/// Selector for `project.show`: id, then exact name, then path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRef {
    pub project: String,
}

/// Params of `project.set_base`. A `None` base clears the project's base.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSetBaseParams {
    /// Selected as `project.show` selects: id, then exact name, then path.
    pub project: String,
    pub base: Option<String>,
}

/// Params of `project.update`. An absent `icon` keeps the icon, `null` clears it and an icon sets it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectUpdateParams {
    /// Selected as `project.show` selects: id, then exact name, then path.
    pub project: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_icon"
    )]
    pub icon: Option<Option<ProjectIcon>>,
}

/// Params of `project.remove`. Selects by id, then exact name, then path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRemoveParams {
    pub project: String,
    /// Also forgets the project's workspace records. No file, folder, worktree or branch is touched.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub forget_workspaces: bool,
}

/// Params of `project.inspect`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectInspectParams {
    pub path: String,
}

/// Result of `project.inspect`: what `project.add` without a kind would register for a path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectInspection {
    /// The path a project would be registered at: the repository's main worktree, or the folder itself.
    pub path: String,
    /// The kind `project.add` without a kind would register.
    pub kind: ProjectKind,
    /// The id of the project already registered at `path`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registered: Option<String>,
}

/// Result of `project.list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectList {
    pub projects: Vec<Project>,
}

/// A registered project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub kind: ProjectKind,
    pub path: String,
    pub name: String,
    pub workspaces_root: String,
    pub added_at_ms: i64,
    /// The base a new workspace starts from when it names none; absent when the project has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_ref: Option<String>,
    /// The project's icon; absent when it has none (decision 0091).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<ProjectIcon>,
    /// The owner segment of the primary remote's repository (`origin`, else the first remote); absent for a
    /// folder project or a remote that does not parse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_owner: Option<String>,
}

/// A project's icon (decision 0091): a Lucide icon by name, one emoji, or an image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProjectIcon {
    /// A Lucide icon by its name, drawn in the project's colour.
    Lucide { name: String },
    /// One emoji.
    Emoji { emoji: String },
    /// A PNG as a base64 data URI; a `file` icon may also be a WebP.
    Image {
        src: String,
        source: ProjectIconSource,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
}

/// Where an image icon came from: uploaded by its owner, or detected in the repository's files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectIconSource {
    Upload,
    File,
}

/// Largest image an icon carries, before base64 encoding.
pub const MAX_PROJECT_ICON_IMAGE_BYTES: usize = 256 * 1024;
/// Longest data URI an image icon may carry.
pub const MAX_PROJECT_ICON_DATA_URL_LENGTH: usize = 400 * 1024;
const MAX_LUCIDE_NAME_LENGTH: usize = 40;
const MAX_EMOJI_UTF16_UNITS: usize = 16;
const MAX_LABEL_CHARS: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageFormat {
    Png,
    Webp,
}

impl ImageFormat {
    /// The format whose signature `bytes` starts with.
    fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(Self::Png)
        } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(&b"WEBP"[..]) {
            Some(Self::Webp)
        } else {
            None
        }
    }

    fn data_url_prefix(self) -> &'static str {
        match self {
            Self::Png => "data:image/png;base64,",
            Self::Webp => "data:image/webp;base64,",
        }
    }
}

impl ProjectIcon {
    /// The icon with its text trimmed and its label capped, or why it is not a valid icon.
    pub fn normalized(self) -> Result<Self, String> {
        match self {
            Self::Lucide { name } => {
                let name = name.trim().to_owned();
                if is_lucide_name(&name) {
                    Ok(Self::Lucide { name })
                } else {
                    Err(format!(
                        "a Lucide icon name is letters and digits, starting with a letter, at most {MAX_LUCIDE_NAME_LENGTH} characters"
                    ))
                }
            }
            Self::Emoji { emoji } => {
                let emoji = emoji.trim().to_owned();
                if emoji.is_empty() || emoji.encode_utf16().count() > MAX_EMOJI_UTF16_UNITS {
                    Err(format!(
                        "an emoji icon is 1 to {MAX_EMOJI_UTF16_UNITS} UTF-16 units"
                    ))
                } else {
                    Ok(Self::Emoji { emoji })
                }
            }
            Self::Image { src, source, label } => {
                let src = src.trim().to_owned();
                check_image_src(&src, source)?;
                Ok(Self::Image {
                    src,
                    source,
                    label: clean_label(label),
                })
            }
        }
    }

    /// An image icon from the bytes of a PNG, or of a WebP for a `file` icon, within the size limit.
    pub fn image_from_bytes(
        source: ProjectIconSource,
        bytes: &[u8],
        label: Option<String>,
    ) -> Result<Self, String> {
        let format = ImageFormat::sniff(bytes)
            .ok_or_else(|| "the file is not a PNG or WebP image".to_owned())?;
        if source == ProjectIconSource::Upload && format != ImageFormat::Png {
            return Err("an uploaded icon must be a PNG".to_owned());
        }
        if bytes.len() > MAX_PROJECT_ICON_IMAGE_BYTES {
            return Err("the image is larger than 256 KiB".to_owned());
        }
        Ok(Self::Image {
            src: format!("{}{}", format.data_url_prefix(), STANDARD.encode(bytes)),
            source,
            label: clean_label(label),
        })
    }

    /// The JSON a project row keeps for this icon.
    pub fn to_stored(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// The icon a project row's stored JSON holds; `None` for text this build cannot read.
    pub fn from_stored(text: &str) -> Option<Self> {
        serde_json::from_str(text).ok()
    }
}

/// Reads `null` as a present, empty icon; an absent field is `default`, so it stays `None`.
fn present_icon<'de, D>(deserializer: D) -> Result<Option<Option<ProjectIcon>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<ProjectIcon>::deserialize(deserializer).map(Some)
}

fn is_lucide_name(name: &str) -> bool {
    name.len() <= MAX_LUCIDE_NAME_LENGTH
        && name.starts_with(|c: char| c.is_ascii_alphabetic())
        && name.chars().all(|c| c.is_ascii_alphanumeric())
}

fn clean_label(label: Option<String>) -> Option<String> {
    label
        .map(|text| {
            text.trim()
                .chars()
                .take(MAX_LABEL_CHARS)
                .collect::<String>()
        })
        .filter(|text| !text.is_empty())
}

/// Checks that `src` is a base64 data URI of an image format `source` accepts, within the size limits.
fn check_image_src(src: &str, source: ProjectIconSource) -> Result<(), String> {
    if src.len() > MAX_PROJECT_ICON_DATA_URL_LENGTH {
        return Err("the image icon is larger than 400 KiB as a data URI".to_owned());
    }
    let accepted: &[ImageFormat] = match source {
        ProjectIconSource::Upload => &[ImageFormat::Png],
        ProjectIconSource::File => &[ImageFormat::Png, ImageFormat::Webp],
    };
    let (format, payload) = accepted
        .iter()
        .find_map(|format| {
            src.strip_prefix(format.data_url_prefix())
                .map(|payload| (*format, payload))
        })
        .ok_or_else(|| {
            "the image icon must be a base64 PNG data URI (a file icon may also be WebP)".to_owned()
        })?;
    let bytes = STANDARD
        .decode(payload)
        .map_err(|_| "the image icon's data is not base64".to_owned())?;
    if bytes.len() > MAX_PROJECT_ICON_IMAGE_BYTES {
        return Err("the image is larger than 256 KiB".to_owned());
    }
    if ImageFormat::sniff(&bytes) != Some(format) {
        return Err("the image icon's data is not the image format its prefix names".to_owned());
    }
    Ok(())
}
