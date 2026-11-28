//! The icon a project takes from its repository's files when it is added without one (decisions 0091 and 0180): the
//! first conventional PNG or WebP image, else the icon an HTML entry page or a JSX route root declares, else the `icon`
//! of `package.json`. Bounded reads, every one inside the repository and outside its `.worktrees`; nothing is spawned.

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use massorca_core_git::WORKTREES_FOLDER;
use massorca_protocol::methods::projects::{
    MAX_PROJECT_ICON_IMAGE_BYTES, ProjectIcon, ProjectIconSource,
};

/// Conventional icon locations in the order they are tried; each is tried as `.png`, then `.webp`.
const CONVENTIONAL_STEMS: [&str; 15] = [
    "favicon",
    "public/favicon",
    "app/favicon",
    "app/icon",
    "src/favicon",
    "src/app/icon",
    "assets/favicon",
    "assets/icon",
    "static/favicon",
    "logo",
    "public/logo",
    "public/icon",
    "src-tauri/icons/icon",
    "app-icon",
    "icon",
];

/// The files whose declared icon names the project's icon, in pv96's order: HTML entry pages, then JSX route roots.
const DECLARING_FILES: [&str; 7] = [
    "index.html",
    "public/index.html",
    "app/routes/__root.tsx",
    "src/routes/__root.tsx",
    "app/root.tsx",
    "src/root.tsx",
    "src/index.html",
];

/// Largest declaring file read for its icon.
const MAX_DECLARING_FILE_BYTES: u64 = 256 * 1024;

/// Largest `package.json` read for its `icon` field.
const MAX_PACKAGE_JSON_BYTES: u64 = 128 * 1024;

/// The icon detected in the repository or folder at `root`, if it has one.
pub(crate) fn detect_project_icon(root: &Path) -> Option<ProjectIcon> {
    let root = dunce::canonicalize(root).ok()?;
    CONVENTIONAL_STEMS
        .iter()
        .flat_map(|stem| [format!("{stem}.png"), format!("{stem}.webp")])
        .find_map(|relative| image_icon(&root, &relative))
        .or_else(|| {
            DECLARING_FILES
                .iter()
                .find_map(|file| declared_icon(&root, file))
        })
        .or_else(|| package_json_icon(&root))
}

/// The image at `root/relative` as a file icon, when it is a PNG or WebP within the size limit.
fn image_icon(root: &Path, relative: &str) -> Option<ProjectIcon> {
    let bytes = read_bounded(root, relative, MAX_PROJECT_ICON_IMAGE_BYTES as u64)?;
    ProjectIcon::image_from_bytes(ProjectIconSource::File, &bytes, Some(relative.to_owned())).ok()
}

/// The icon the file `root/file` declares, resolved against that file as a browser would.
fn declared_icon(root: &Path, file: &str) -> Option<ProjectIcon> {
    let source = String::from_utf8(read_bounded(root, file, MAX_DECLARING_FILE_BYTES)?).ok()?;
    let href = declared_icon_href(&source)?;
    icon_href_candidates(&href, file)
        .iter()
        .find_map(|relative| image_icon(root, relative))
}

/// The icon `package.json` names: a PNG or WebP data URI, or a PNG or WebP file inside the repository.
fn package_json_icon(root: &Path) -> Option<ProjectIcon> {
    let manifest: serde_json::Value =
        serde_json::from_slice(&read_bounded(root, "package.json", MAX_PACKAGE_JSON_BYTES)?)
            .ok()?;
    let icon = manifest.get("icon")?.as_str()?.trim();
    if icon.starts_with("data:") {
        return ProjectIcon::Image {
            src: icon.to_owned(),
            source: ProjectIconSource::File,
            label: Some("package.json".to_owned()),
        }
        .normalized()
        .ok();
    }
    image_icon(root, &repo_relative_path(icon)?)
}

/// The bytes of the regular file `relative` under the canonical `root`, at most `limit` bytes long. `None` for anything
/// else, including a file that a symbolic link leads outside `root`.
fn read_bounded(root: &Path, relative: &str, limit: u64) -> Option<Vec<u8>> {
    let path = dunce::canonicalize(root.join(relative)).ok()?;
    // Another checkout's files are not the project's (decision 0183).
    if !path.starts_with(root) || path.starts_with(root.join(WORKTREES_FOLDER)) {
        return None;
    }
    let metadata = fs::metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > limit {
        return None;
    }
    let mut bytes = Vec::new();
    File::open(&path)
        .ok()?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= limit).then_some(bytes)
}

/// The href a file declares as its icon: the first `<link rel="icon">` tag, else the first `{ rel: "icon", href }`
/// object literal of a route's `links`.
fn declared_icon_href(source: &str) -> Option<String> {
    html_icon_href(source).or_else(|| object_icon_href(source))
}

/// The href of the first `<link>` whose `rel` is `icon` or `shortcut icon` and whose href has no query.
fn html_icon_href(source: &str) -> Option<String> {
    let lower = source.to_ascii_lowercase();
    let mut from = 0;
    while let Some(offset) = lower[from..].find("<link") {
        let start = from + offset;
        let after_name = lower[start + "<link".len()..].chars().next();
        let end = start + lower[start..].find('>')?;
        let name_ends =
            matches!(after_name, Some(c) if c.is_ascii_whitespace() || c == '/' || c == '>');
        if name_ends && let Some(href) = icon_link_href(&source[start..=end]) {
            return Some(href);
        }
        from = end + 1;
    }
    None
}

/// The href of the first object literal whose `rel` is `icon` or `shortcut icon` and whose href has no query.
/// A `}` closes an object, so its properties are the text since the last `{` before that `}`.
fn object_icon_href(source: &str) -> Option<String> {
    source.split('}').find_map(|segment| {
        let object = segment.rsplit('{').next().unwrap_or_default();
        let rel = quoted_property(object, "rel")?;
        if !rel.eq_ignore_ascii_case("icon") && !rel.eq_ignore_ascii_case("shortcut icon") {
            return None;
        }
        let href = quoted_property(object, "href")?;
        (!href.contains('?')).then(|| href.to_owned())
    })
}

/// The quoted value of the property `name`, written `name: "value"` or `name: 'value'`, matched as a whole word.
fn quoted_property<'a>(object: &'a str, name: &str) -> Option<&'a str> {
    object.match_indices(name).find_map(|(start, _)| {
        let preceded_by_word = object[..start]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
        if preceded_by_word {
            return None;
        }
        let value = object[start + name.len()..]
            .trim_start()
            .strip_prefix(':')?
            .trim_start();
        let quote = value.chars().next().filter(|c| *c == '"' || *c == '\'')?;
        let inner = &value[quote.len_utf8()..];
        inner.find(quote).map(|close| &inner[..close])
    })
}

/// The href of one `<link>` tag that declares an icon with a query-free href.
fn icon_link_href(tag: &str) -> Option<String> {
    let rel = quoted_attribute(tag, "rel")?;
    if !rel.eq_ignore_ascii_case("icon") && !rel.eq_ignore_ascii_case("shortcut icon") {
        return None;
    }
    let href = quoted_attribute(tag, "href")?;
    (!href.contains('?')).then(|| href.to_owned())
}

/// The quoted value of attribute `name` in `tag`, matched case-insensitively.
fn quoted_attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let bytes = tag.as_bytes();
    let name = name.as_bytes();
    let last_start = bytes.len().checked_sub(name.len())?;
    (0..=last_start)
        .filter(|&index| index == 0 || bytes[index - 1].is_ascii_whitespace())
        .filter(|&index| bytes[index..index + name.len()].eq_ignore_ascii_case(name))
        .find_map(|index| {
            let equals = index + name.len();
            if bytes.get(equals) != Some(&b'=') {
                return None;
            }
            let quote = *bytes.get(equals + 1)?;
            if quote != b'"' && quote != b'\'' {
                return None;
            }
            let start = equals + 2;
            let length = tag[start..].find(char::from(quote))?;
            Some(&tag[start..start + length])
        })
}

/// The repository-relative path an icon reference names, or `None` for a URL, or for a path that leaves the repository.
fn repo_relative_path(reference: &str) -> Option<String> {
    let reference = reference.trim();
    if reference.is_empty() || reference.starts_with("//") || has_scheme(reference) {
        return None;
    }
    let path = reference
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim_start_matches('/')
        .replace('\\', "/");
    let parts: Vec<&str> = path
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    // An icon never leaves the repository through `..`.
    if parts.is_empty() || parts.contains(&"..") {
        return None;
    }
    Some(parts.join("/"))
}

/// The repository-relative paths a declared icon `href` in `file` may name, in the order to try them.
fn icon_href_candidates(href: &str, file: &str) -> Vec<String> {
    let Some(path) = repo_relative_path(href) else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    if !href.trim().starts_with('/')
        && let Some((directory, _)) = file.rsplit_once('/')
    {
        candidates.push(format!("{directory}/{path}"));
    }
    candidates.push(format!("public/{path}"));
    candidates.push(path);
    let mut unique: Vec<String> = Vec::new();
    for candidate in candidates {
        if !unique.contains(&candidate) {
            unique.push(candidate);
        }
    }
    unique
}

/// Whether `href` starts with a URL scheme such as `https:`.
fn has_scheme(href: &str) -> bool {
    href.split_once(':').is_some_and(|(scheme, _)| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
    })
}
