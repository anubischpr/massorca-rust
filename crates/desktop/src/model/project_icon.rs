//! What a project's icon draws (decision 0091): the folder glyph, or the icon its owner chose.

use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use massorca_protocol::methods::projects::{ProjectIcon, ProjectUpdateParams};

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
            Some(ProjectIcon::Lucide { name }) => Self::Lucide(name.clone()),
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
    Some(ProjectGlyph::Image(ProjectImage {
        format,
        bytes: bytes.into(),
        label: label.map(str::to_owned),
    }))
}

/// The `project.update` params that set `icon` on `project`; `None` clears it, so the folder glyph returns.
pub fn update_params(project: &str, icon: Option<ProjectIcon>) -> ProjectUpdateParams {
    ProjectUpdateParams {
        project: project.to_owned(),
        icon: Some(icon),
    }
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
            ProjectGlyph::Image(ProjectImage {
                format: ProjectImageFormat::Png,
                bytes: bytes.to_vec().into(),
                label: Some("logo".into()),
            })
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
        };
        let set = serde_json::to_value(update_params("oppm", Some(set))).expect("json");
        assert_eq!(
            set,
            serde_json::json!({"project": "oppm", "icon": {"type": "lucide", "name": "star"}})
        );
    }
}
