//! The icon a project takes from its repository's files when it is added without one (decision 0091): the first
//! conventional PNG or WebP image, else the icon an HTML entry page declares. Bounded reads; nothing is spawned.

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

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

/// The HTML entry pages whose `<link rel="icon">` names the project's icon.
const ENTRY_PAGES: [&str; 3] = ["index.html", "public/index.html", "src/index.html"];

/// Largest entry page read for its declared icon.
const MAX_ENTRY_PAGE_BYTES: u64 = 256 * 1024;

/// The icon detected in the repository or folder at `root`, if it has one.
pub(crate) fn detect_project_icon(root: &Path) -> Option<ProjectIcon> {
    CONVENTIONAL_STEMS
        .iter()
        .flat_map(|stem| [format!("{stem}.png"), format!("{stem}.webp")])
        .find_map(|relative| image_icon(root, &relative))
        .or_else(|| {
            ENTRY_PAGES
                .iter()
                .find_map(|page| declared_icon(root, page))
        })
}

/// The image at `root/relative` as a file icon, when it is a PNG or WebP within the size limit.
fn image_icon(root: &Path, relative: &str) -> Option<ProjectIcon> {
    let bytes = read_bounded(&root.join(relative), MAX_PROJECT_ICON_IMAGE_BYTES as u64)?;
    ProjectIcon::image_from_bytes(ProjectIconSource::File, &bytes, Some(relative.to_owned())).ok()
}

/// The icon the entry page `root/page` declares, resolved against that page as a browser would.
fn declared_icon(root: &Path, page: &str) -> Option<ProjectIcon> {
    let html = String::from_utf8(read_bounded(&root.join(page), MAX_ENTRY_PAGE_BYTES)?).ok()?;
    let href = declared_icon_href(&html)?;
    icon_href_candidates(&href, page)
        .iter()
        .find_map(|relative| image_icon(root, relative))
}

/// The bytes of a regular file of at most `limit` bytes; `None` for anything else.
fn read_bounded(path: &Path, limit: u64) -> Option<Vec<u8>> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > limit {
        return None;
    }
    let mut bytes = Vec::new();
    File::open(path)
        .ok()?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= limit).then_some(bytes)
}

/// The href of the first `<link>` whose `rel` is `icon` or `shortcut icon` and whose href has no query.
fn declared_icon_href(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(offset) = lower[from..].find("<link") {
        let start = from + offset;
        let after_name = lower[start + "<link".len()..].chars().next();
        let end = start + lower[start..].find('>')?;
        let name_ends =
            matches!(after_name, Some(c) if c.is_ascii_whitespace() || c == '/' || c == '>');
        if name_ends && let Some(href) = icon_link_href(&html[start..=end]) {
            return Some(href);
        }
        from = end + 1;
    }
    None
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

/// The repository-relative paths an icon `href` declared in `page` may name, in the order to try them.
fn icon_href_candidates(href: &str, page: &str) -> Vec<String> {
    let href = href.trim();
    if href.is_empty() || href.starts_with("//") || has_scheme(href) {
        return Vec::new();
    }
    let parts: Vec<String> = href
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim_start_matches('/')
        .replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .map(str::to_owned)
        .collect();
    // An href never leaves the repository through `..`.
    if parts.is_empty() || parts.iter().any(|part| part == "..") {
        return Vec::new();
    }
    let path = parts.join("/");
    let mut candidates = Vec::new();
    if !href.starts_with('/')
        && let Some((directory, _)) = page.rsplit_once('/')
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
