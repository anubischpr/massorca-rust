//! The owner segment of a git remote URL, read from the path shapes of GitHub, GitLab, Bitbucket,
//! Gitea and Azure DevOps over https, ssh and scp-style addresses.

use std::path::Path;

use crate::git::Git;

/// The owner (user, organization or group path) of the repository a remote URL names, or `None` for a
/// local path, a `file://` URL or a URL without an owner segment.
pub fn remote_owner(url: &str) -> Option<String> {
    let (host, path) = split_remote(url.trim())?;
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let segments: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    if is_azure_host(&host) {
        return azure_owner(&host, &segments);
    }
    let (_repository, owner) = segments.split_last()?;
    (!owner.is_empty()).then(|| owner.join("/"))
}

impl Git {
    /// The owner of the repository's primary remote: `origin`, else the first remote in git's order. `None`
    /// when there is no such remote, git fails or the URL does not parse.
    pub fn primary_remote_owner(&self, repo: &Path) -> Option<String> {
        let captured = self.run(Some(repo), ["remote", "-v"]).ok()?;
        if !captured.succeeded() {
            return None;
        }
        let text = captured.text().ok()?;
        let fetch_urls: Vec<(&str, &str)> = text
            .lines()
            .filter_map(|line| {
                let (name, rest) = line.split_once('\t')?;
                Some((name, rest.strip_suffix(" (fetch)")?))
            })
            .collect();
        let &(_, url) = fetch_urls
            .iter()
            .find(|entry| entry.0 == "origin")
            .or(fetch_urls.first())?;
        remote_owner(url)
    }
}

/// The host and path of a remote URL; `None` for an address that is not a network remote.
fn split_remote(url: &str) -> Option<(String, &str)> {
    if let Some((scheme, rest)) = url.split_once("://") {
        let scheme = scheme.to_ascii_lowercase();
        if !matches!(
            scheme.as_str(),
            "https" | "http" | "ssh" | "git" | "git+ssh" | "ssh+git"
        ) {
            return None;
        }
        let (authority, path) = rest.split_once('/')?;
        return Some((host_of(authority), path));
    }
    // scp-style `[user@]host:path`; a drive letter such as `C:` is a local path.
    let (authority, path) = url.split_once(':')?;
    if authority.is_empty() || authority.contains(['/', '\\']) || path.starts_with(['/', '\\']) {
        return None;
    }
    let host = host_of(authority);
    (host.len() > 1).then_some((host, path))
}

/// The lowercased host of an authority `[user@]host[:port]`.
fn host_of(authority: &str) -> String {
    let without_user = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = without_user
        .split_once(':')
        .map_or(without_user, |(host, _)| host);
    host.to_ascii_lowercase()
}

fn is_azure_host(host: &str) -> bool {
    host == "dev.azure.com" || host == "ssh.dev.azure.com" || host.ends_with(".visualstudio.com")
}

/// Azure DevOps: `org/project/_git/repo`, `v3/org/project/repo` over ssh, or the legacy `<org>.visualstudio.com`.
fn azure_owner(host: &str, segments: &[&str]) -> Option<String> {
    match (host, segments) {
        ("ssh.dev.azure.com", ["v3", org, _, _]) | ("dev.azure.com", [org, _, "_git", _]) => {
            Some((*org).to_owned())
        }
        (host, [_, "_git", _]) if host.ends_with(".visualstudio.com") => host
            .split('.')
            .next()
            .filter(|org| !org.is_empty())
            .map(str::to_owned),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::remote_owner;

    #[test]
    fn owner_of_each_url_shape() {
        let cases: &[(&str, Option<&str>)] = &[
            ("https://github.com/acme/widgets.git", Some("acme")),
            ("https://github.com/acme/widgets", Some("acme")),
            ("https://github.com/acme/widgets/", Some("acme")),
            ("https://user@github.com/acme/widgets", Some("acme")),
            ("https://gitea.example.com/Team/Widgets.git", Some("Team")),
            ("https://bitbucket.org/acme/widgets.git", Some("acme")),
            ("git@github.com:acme/widgets.git", Some("acme")),
            ("git@bitbucket.org:acme/widgets.git", Some("acme")),
            ("ssh://git@github.com/acme/widgets.git", Some("acme")),
            (
                "ssh://git@gitea.example.com:2222/acme/widgets.git",
                Some("acme"),
            ),
            (
                "https://gitlab.example.com/group/sub/repo.git",
                Some("group/sub"),
            ),
            (
                "ssh://git@gitlab.example.com/group/sub/repo.git",
                Some("group/sub"),
            ),
            (
                "git@gitlab.example.com:group/sub/repo.git",
                Some("group/sub"),
            ),
            ("https://dev.azure.com/org/project/_git/repo", Some("org")),
            (
                "https://org@dev.azure.com/org/project/_git/repo",
                Some("org"),
            ),
            (
                "https://org.visualstudio.com/project/_git/repo",
                Some("org"),
            ),
            ("git@ssh.dev.azure.com:v3/org/project/repo", Some("org")),
            ("/srv/repo.git", None),
            ("C:\\repos\\x", None),
            ("../x", None),
            ("file:///srv/repo.git", None),
            ("https://github.com/widgets.git", None),
            ("git@github.com:widgets.git", None),
            ("", None),
            ("not a url", None),
        ];
        for (url, expected) in cases {
            assert_eq!(remote_owner(url).as_deref(), *expected, "url {url:?}");
        }
    }
}
