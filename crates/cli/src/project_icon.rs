//! The `--icon` values of `project add` and `project update` (decision 0091): `emoji:<emoji>`, `lucide:<name>` or
//! `lucide:<name>:<colour>`, or `image:<PNG file>`. An image file is read here, within the size limit, and sent to the
//! service as an upload.

use std::path::Path;

use massorca_protocol::codes;
use massorca_protocol::methods::projects::{
    IconColor, MAX_PROJECT_ICON_IMAGE_BYTES, ProjectIcon, ProjectIconSource,
};

use crate::cwd_path::PathRules;
use crate::error::{CliError, exit};

/// The icon `spec` names. The service checks the text; an image file is read and checked here.
pub fn parse(spec: &str, paths: &PathRules) -> Result<ProjectIcon, CliError> {
    let (kind, value) = spec.split_once(':').ok_or_else(|| usage(spec))?;
    match kind {
        "emoji" => Ok(ProjectIcon::Emoji {
            emoji: value.to_owned(),
        }),
        "lucide" => lucide(value),
        "image" => upload(&paths.path(Path::new(value))?),
        _ => Err(usage(spec)),
    }
}

/// A Lucide icon `name`, optionally `:<colour>` with a usage colour token or `neutral`.
fn lucide(value: &str) -> Result<ProjectIcon, CliError> {
    let (name, color) = match value.split_once(':') {
        Some((name, token)) => {
            let color = IconColor::from_token(token).ok_or_else(|| {
                invalid(&format!(
                    "{token:?} is not an icon colour: use usage-1 to usage-6 or neutral"
                ))
            })?;
            (name, Some(color))
        }
        None => (value, None),
    };
    Ok(ProjectIcon::Lucide {
        name: name.to_owned(),
        color,
    })
}

fn usage(spec: &str) -> CliError {
    CliError::new(
        codes::REQUEST_INVALID_PARAMS,
        format!("{spec:?} is not an icon: use emoji:<emoji>, lucide:<name> or image:<PNG file>"),
        exit::USAGE,
    )
}

/// The PNG file at `path` as an upload. A file over the size limit is refused before it is read.
fn upload(path: &str) -> Result<ProjectIcon, CliError> {
    let path = Path::new(path);
    if std::fs::metadata(path)?.len() > MAX_PROJECT_ICON_IMAGE_BYTES as u64 {
        return Err(invalid("the image is larger than 256 KiB"));
    }
    let bytes = std::fs::read(path)?;
    let label = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    ProjectIcon::image_from_bytes(ProjectIconSource::Upload, &bytes, label)
        .map_err(|reason| invalid(&reason))
}

fn invalid(reason: &str) -> CliError {
    CliError::new(
        codes::REQUEST_INVALID_PARAMS,
        format!("invalid icon: {reason}"),
        exit::USAGE,
    )
}
