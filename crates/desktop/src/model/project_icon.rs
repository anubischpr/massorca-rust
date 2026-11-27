//! What a project's icon draws (decision 0091): the folder glyph, or the icon its owner chose.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use massorca_protocol::methods::projects::{ProjectIcon, ProjectIconSource, ProjectUpdateParams};

use crate::model::canvas::Canvas;

/// What a project's icon draws. A project with no icon draws the folder glyph.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ProjectGlyph {
    #[default]
    Folder,
    /// One emoji, drawn as text.
    Emoji(String),
    /// A Lucide icon by name; a name the bundled icon set lacks draws the folder glyph.
    Lucide(String),
    /// A PNG or WebP image, drawn in a rounded square.
    Image(ProjectImage),
}

/// An image icon's decoded bytes; the renderer builds its image from them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectImage {
    pub format: ProjectImageFormat,
    pub bytes: Arc<[u8]>,
    pub label: Option<String>,
    /// The hash of the format and the bytes: the key a renderer caches the built image under.
    pub key: u64,
}

impl ProjectImage {
    pub fn new(format: ProjectImageFormat, bytes: Vec<u8>, label: Option<String>) -> Self {
        let mut hasher = DefaultHasher::new();
        (format == ProjectImageFormat::Webp).hash(&mut hasher);
        bytes.hash(&mut hasher);
        Self {
            format,
            bytes: bytes.into(),
            label,
            key: hasher.finish(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectImageFormat {
    Png,
    Webp,
}

impl ProjectGlyph {
    /// What `icon` draws. An image whose data URI does not decode draws the folder glyph.
    pub fn of(icon: Option<&ProjectIcon>) -> Self {
        match icon {
            None => Self::Folder,
            Some(ProjectIcon::Emoji { emoji }) => Self::Emoji(emoji.clone()),
            Some(ProjectIcon::Lucide { name, .. }) => Self::Lucide(name.clone()),
            Some(ProjectIcon::Image { src, label, .. }) => {
                image_glyph(src, label.as_deref()).unwrap_or(Self::Folder)
            }
        }
    }
}

/// The glyph of a base64 PNG or WebP data URI, or `None` when `src` is not one.
fn image_glyph(src: &str, label: Option<&str>) -> Option<ProjectGlyph> {
    let (format, payload) = [
        (ProjectImageFormat::Png, "data:image/png;base64,"),
        (ProjectImageFormat::Webp, "data:image/webp;base64,"),
    ]
    .into_iter()
    .find_map(|(format, prefix)| src.strip_prefix(prefix).map(|payload| (format, payload)))?;
    let bytes = STANDARD.decode(payload).ok()?;
    Some(ProjectGlyph::Image(ProjectImage::new(
        format,
        bytes,
        label.map(str::to_owned),
    )))
}

/// The `project.update` params that set `icon` on `project`; `None` clears it, so the folder glyph returns.
pub fn update_params(project: &str, icon: Option<ProjectIcon>) -> ProjectUpdateParams {
    ProjectUpdateParams {
        project: project.to_owned(),
        icon: Some(icon),
    }
}

/// The start heading's project icon popover: the project it is open for, and the reason its last change was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectIconPopover {
    pub project: String,
    /// The reason the last change was refused, drawn under the grid until the next attempt.
    pub refusal: Option<String>,
    /// The Canvas the popover opened on; it closes once the Canvas is another one.
    pub canvas: Canvas,
}

impl ProjectIconPopover {
    /// The popover while the Canvas is still the one it opened on.
    pub fn while_shown(self, canvas: &Canvas) -> Option<Self> {
        (self.canvas == *canvas).then_some(self)
    }
}

/// The icon an uploaded file makes: a PNG of at most 256 KiB, checked here before the service sees it.
pub fn upload_icon(bytes: &[u8], label: Option<String>) -> Result<ProjectIcon, String> {
    ProjectIcon::image_from_bytes(ProjectIconSource::Upload, bytes, label)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_uri(prefix: &str, bytes: &[u8]) -> String {
        format!("{prefix}{}", STANDARD.encode(bytes))
    }

    #[test]
    fn a_project_without_an_icon_draws_the_folder_glyph() {
        assert_eq!(ProjectGlyph::of(None), ProjectGlyph::Folder);
    }

    #[test]
    fn an_emoji_draws_itself_and_a_lucide_icon_draws_its_name() {
        let emoji = ProjectIcon::Emoji {
            emoji: "🚀".into()
        };
        let lucide = ProjectIcon::Lucide {
            name: "code".into(),
            color: None,
        };
        assert_eq!(
            ProjectGlyph::of(Some(&emoji)),
            ProjectGlyph::Emoji("🚀".into())
        );
        assert_eq!(
            ProjectGlyph::of(Some(&lucide)),
            ProjectGlyph::Lucide("code".into())
        );
    }

    #[test]
    fn a_png_draws_its_decoded_bytes_and_label() {
        let bytes = b"\x89PNG\r\n\x1a\npixels";
        let icon = ProjectIcon::Image {
            src: data_uri("data:image/png;base64,", bytes),
            source: massorca_protocol::methods::projects::ProjectIconSource::Upload,
            label: Some("logo".into()),
        };
        assert_eq!(
            ProjectGlyph::of(Some(&icon)),
            ProjectGlyph::Image(ProjectImage::new(
                ProjectImageFormat::Png,
                bytes.to_vec(),
                Some("logo".into()),
            ))
        );
    }

    #[test]
    fn a_webp_file_icon_draws_as_webp() {
        let bytes = b"RIFF\0\0\0\0WEBPvp8 ";
        let icon = ProjectIcon::Image {
            src: data_uri("data:image/webp;base64,", bytes),
            source: massorca_protocol::methods::projects::ProjectIconSource::File,
            label: None,
        };
        let ProjectGlyph::Image(image) = ProjectGlyph::of(Some(&icon)) else {
            panic!("an image glyph");
        };
        assert_eq!(image.format, ProjectImageFormat::Webp);
        assert_eq!(image.label, None);
    }

    #[test]
    fn an_image_that_does_not_decode_draws_the_folder_glyph() {
        let icon = ProjectIcon::Image {
            src: "data:image/png;base64,!!not base64!!".into(),
            source: massorca_protocol::methods::projects::ProjectIconSource::Upload,
            label: None,
        };
        assert_eq!(ProjectGlyph::of(Some(&icon)), ProjectGlyph::Folder);
    }

    #[test]
    fn a_reset_sends_null_and_a_set_sends_the_icon() {
        let reset = serde_json::to_value(update_params("oppm", None)).expect("json");
        assert_eq!(reset, serde_json::json!({"project": "oppm", "icon": null}));
        let set = ProjectIcon::Lucide {
            name: "star".into(),
            color: None,
        };
        let set = serde_json::to_value(update_params("oppm", Some(set))).expect("json");
        assert_eq!(
            set,
            serde_json::json!({"project": "oppm", "icon": {"type": "lucide", "name": "star"}})
        );
    }
}

#[cfg(test)]
mod upload_tests {
    use super::*;

    fn png(len: usize) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.resize(len, 0);
        bytes
    }

    #[test]
    fn a_png_of_up_to_256_kib_becomes_the_icon() {
        let icon = upload_icon(&png(1024), Some("logo.png".into())).expect("a PNG is accepted");
        assert!(matches!(icon, ProjectIcon::Image { .. }));
    }

    #[test]
    fn a_file_that_is_not_a_png_is_refused_with_the_reason() {
        let reason = upload_icon(b"GIF89a....", None).expect_err("a GIF is refused");
        assert!(reason.contains("PNG"), "{reason}");
    }

    #[test]
    fn an_image_over_256_kib_is_refused_with_the_reason() {
        let reason = upload_icon(&png(256 * 1024 + 1), None).expect_err("too large");
        assert!(reason.contains("256 KiB"), "{reason}");
    }
}

#[cfg(test)]
mod canvas_tests {
    use super::*;
    use crate::model::navigation::Mode;

    #[test]
    fn a_popover_closes_once_the_canvas_is_another_page() {
        let popover = ProjectIconPopover {
            project: "oppm".into(),
            refusal: None,
            canvas: Canvas::Home(Mode::Workspaces),
        };
        assert_eq!(
            popover.clone().while_shown(&Canvas::Home(Mode::Workspaces)),
            Some(popover.clone())
        );
        assert_eq!(popover.while_shown(&Canvas::Session("Feed".into())), None);
    }
}

#[cfg(test)]
mod key_tests {
    use super::*;

    #[test]
    fn the_same_bytes_share_a_key_and_other_bytes_do_not() {
        let first = ProjectImage::new(ProjectImageFormat::Png, b"one".to_vec(), None);
        let same = ProjectImage::new(
            ProjectImageFormat::Png,
            b"one".to_vec(),
            Some("label".into()),
        );
        let other = ProjectImage::new(ProjectImageFormat::Png, b"two".to_vec(), None);
        assert_eq!(first.key, same.key);
        assert_ne!(first.key, other.key);
    }
}
