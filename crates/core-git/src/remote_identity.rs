//! The hosting identity of a git remote: host, provider, owner and repository, read from the shape of its URL over
//! https, ssh and scp-style addresses (decision 0114).

use std::path::Path;

use massorca_protocol::methods::git::remotes::{Provider, RemoteIdentity};

use crate::error::GitError;
use crate::git::Git;

impl Git {
    /// The network remotes of the repository at `dir`, in git's order. A remote whose URL is not a network address is
    /// left out; `origin`, else the first remote, is primary; `origin` is a fork when `upstream` names the same
    /// repository under another owner.
    pub fn remote_identities(&self, dir: &Path) -> Result<Vec<RemoteIdentity>, GitError> {
        let captured = self.run(Some(dir), ["remote", "-v"])?;
        captured.require_success()?;
        let text = captured.text()?;
        let mut remotes: Vec<RemoteIdentity> = fetch_urls(text)
            .filter_map(|(name, url)| named_identity(name, url))
            .collect();
        mark_primary(&mut remotes);
        mark_fork(&mut remotes);
        Ok(remotes)
    }

    /// The owner of the repository's primary remote: `origin`, else the first remote in git's order. `None`
    /// when there is no such remote, git fails or the URL does not parse.
    pub fn primary_remote_owner(&self, repo: &Path) -> Option<String> {
        let captured = self.run(Some(repo), ["remote", "-v"]).ok()?;
        if !captured.succeeded() {
            return None;
        }
        let text = captured.text().ok()?;
        let (_, url) = fetch_urls(text)
            .find(|(name, _)| *name == "origin")
            .or_else(|| fetch_urls(text).next())?;
        parse_remote(url).map(|parsed| parsed.owner)
    }
}

/// The name and fetch URL of each remote in `git remote -v` output; push URLs are not listed.
fn fetch_urls(text: &str) -> impl Iterator<Item = (&str, &str)> {
    text.lines().filter_map(|line| {
        let (name, rest) = line.split_once('\t')?;
        Some((name, rest.strip_suffix(" (fetch)")?))
    })
}

fn named_identity(remote: &str, url: &str) -> Option<RemoteIdentity> {
    let parsed = parse_remote(url)?;
    Some(RemoteIdentity {
        remote: remote.to_owned(),
        host: parsed.host,
        provider: parsed.provider,
        owner: parsed.owner,
        repo: parsed.repo,
        web_url: parsed.web_url,
        primary: false,
        fork_of: None,
    })
}

/// Marks `origin`, or the first remote when there is none, as the primary one.
fn mark_primary(remotes: &mut [RemoteIdentity]) {
    let index = remotes
        .iter()
        .position(|remote| remote.remote == "origin")
        .unwrap_or(0);
    if let Some(remote) = remotes.get_mut(index) {
        remote.primary = true;
    }
}

/// Marks `origin` as a fork of `upstream` when both name the same repository on the same host under other owners.
fn mark_fork(remotes: &mut [RemoteIdentity]) {
    let Some(upstream) = remotes
        .iter()
        .find(|remote| remote.remote == "upstream")
        .cloned()
    else {
        return;
    };
    let Some(origin) = remotes.iter_mut().find(|remote| remote.remote == "origin") else {
        return;
    };
    let same_repository =
        origin.host == upstream.host && origin.repo.eq_ignore_ascii_case(&upstream.repo);
    if same_repository && !origin.owner.eq_ignore_ascii_case(&upstream.owner) {
        origin.fork_of = Some(format!("{}/{}", upstream.owner, upstream.repo));
    }
}

/// What a network remote URL names: its host, provider, owner and repository, and its web address when known.
struct ParsedRemote {
    host: String,
    provider: Provider,
    owner: String,
    repo: String,
    web_url: Option<String>,
}

/// Parses a network remote URL. `None` for a local path, a `file://` URL, or a URL without an owner and a repository.
fn parse_remote(url: &str) -> Option<ParsedRemote> {
    let link = split_remote(url.trim())?;
    let path = link.path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let segments: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let (owner, repo, project) = if is_azure_host(&link.host) {
        azure_path(&link.host, &segments)?
    } else {
        let (repo, owner) = segments.split_last()?;
        if owner.is_empty() {
            return None;
        }
        (owner.join("/"), (*repo).to_owned(), None)
    };
    let provider = provider_of(&link.host);
    let web_url = web_url(&link, provider, &owner, &repo, project.as_deref());
    Some(ParsedRemote {
        host: link.host,
        provider,
        owner,
        repo,
        web_url,
    })
}

/// Owner, repository and project of an Azure DevOps path: `org/project/_git/repo`, `v3/org/project/repo` over ssh, or
/// the legacy `org.visualstudio.com/project/_git/repo`.
fn azure_path(host: &str, segments: &[&str]) -> Option<(String, String, Option<String>)> {
    let (org, project, repo) = match (host, segments) {
        ("ssh.dev.azure.com", ["v3", org, project, repo])
        | ("dev.azure.com", [org, project, "_git", repo]) => (*org, *project, *repo),
        (host, [project, "_git", repo]) if host.ends_with(".visualstudio.com") => (
            host.split('.').next().filter(|org| !org.is_empty())?,
            *project,
            *repo,
        ),
        _ => return None,
    };
    Some((org.to_owned(), repo.to_owned(), Some(project.to_owned())))
}

/// A remote address split into its host, the port of an http or https address (the web port) and its path.
struct Link {
    host: String,
    web_port: Option<String>,
    path: String,
}

/// The parts of a network remote address; `None` for an address that is not a network remote.
fn split_remote(url: &str) -> Option<Link> {
    if let Some((scheme, rest)) = url.split_once("://") {
        let scheme = scheme.to_ascii_lowercase();
        if !matches!(
            scheme.as_str(),
            "https" | "http" | "ssh" | "git" | "git+ssh" | "ssh+git"
        ) {
            return None;
        }
        let (authority, path) = rest.split_once('/')?;
        let (host, port) = host_and_port(authority);
        let web_port = matches!(scheme.as_str(), "https" | "http")
            .then_some(port)
            .flatten();
        return Some(Link {
            host,
            web_port,
            path: path.to_owned(),
        });
    }
    // scp-style `[user@]host:path`; a drive letter such as `C:` is a local path.
    let (authority, path) = url.split_once(':')?;
    if authority.is_empty() || authority.contains(['/', '\\']) || path.starts_with(['/', '\\']) {
        return None;
    }
    let (host, _) = host_and_port(authority);
    (host.len() > 1).then(|| Link {
        host,
        web_port: None,
        path: path.to_owned(),
    })
}

/// The lowercased host and the port of an authority `[user@]host[:port]`; a port is kept only when it is digits.
fn host_and_port(authority: &str) -> (String, Option<String>) {
    let without_user = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let (host, port) = match without_user.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (without_user, None),
    };
    let port = port
        .filter(|port| !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()))
        .map(str::to_owned);
    (host.to_ascii_lowercase(), port)
}

fn provider_of(host: &str) -> Provider {
    match host {
        "github.com" => Provider::Github,
        "gitlab.com" => Provider::Gitlab,
        "bitbucket.org" => Provider::Bitbucket,
        "codeberg.org" => Provider::Gitea,
        _ if is_azure_host(host) => Provider::AzureDevops,
        _ if host.contains("gitlab") => Provider::Gitlab,
        _ if host.contains("gitea") => Provider::Gitea,
        _ => Provider::Unknown,
    }
}

fn is_azure_host(host: &str) -> bool {
    host == "dev.azure.com" || host == "ssh.dev.azure.com" || host.ends_with(".visualstudio.com")
}

/// The web address of the repository for the layouts known: GitHub, GitLab, Bitbucket, Gitea and Azure DevOps. A host
/// of another layout has none, so no address is guessed for it.
fn web_url(
    link: &Link,
    provider: Provider,
    owner: &str,
    repo: &str,
    project: Option<&str>,
) -> Option<String> {
    match (provider, project) {
        (Provider::Unknown, _) | (Provider::AzureDevops, None) => None,
        (Provider::AzureDevops, Some(project)) if link.host.ends_with(".visualstudio.com") => {
            Some(format!("https://{}/{project}/_git/{repo}", link.host))
        }
        (Provider::AzureDevops, Some(project)) => Some(format!(
            "https://dev.azure.com/{owner}/{project}/_git/{repo}"
        )),
        _ => {
            let host = match &link.web_port {
                Some(port) => format!("{}:{port}", link.host),
                None => link.host.clone(),
            };
            Some(format!("https://{host}/{owner}/{repo}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::git::remotes::{Provider, RemoteIdentity};

    use super::{mark_fork, mark_primary, parse_remote};

    fn identity(remote: &str, host: &str, owner: &str, repo: &str) -> RemoteIdentity {
        RemoteIdentity {
            remote: remote.to_owned(),
            host: host.to_owned(),
            provider: Provider::Github,
            owner: owner.to_owned(),
            repo: repo.to_owned(),
            web_url: None,
            primary: false,
            fork_of: None,
        }
    }

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
            let actual = parse_remote(url).map(|parsed| parsed.owner);
            assert_eq!(actual.as_deref(), *expected, "url {url:?}");
        }
    }

    #[test]
    fn every_url_shape_names_its_host_owner_and_repository() {
        type Named<'a> = Option<(&'a str, &'a str, &'a str)>;
        let cases: &[(&str, Named<'_>)] = &[
            (
                "https://github.com/acme/widgets.git",
                Some(("github.com", "acme", "widgets")),
            ),
            (
                "https://github.com/acme/widgets",
                Some(("github.com", "acme", "widgets")),
            ),
            (
                "https://github.com/acme/widgets/",
                Some(("github.com", "acme", "widgets")),
            ),
            (
                "https://user@github.com/acme/widgets",
                Some(("github.com", "acme", "widgets")),
            ),
            (
                "https://GitHub.com/acme/widgets",
                Some(("github.com", "acme", "widgets")),
            ),
            (
                "git@github.com:acme/widgets.git",
                Some(("github.com", "acme", "widgets")),
            ),
            (
                "ssh://git@github.com/acme/widgets.git",
                Some(("github.com", "acme", "widgets")),
            ),
            (
                "ssh://git@gitea.example.com:2222/acme/widgets.git",
                Some(("gitea.example.com", "acme", "widgets")),
            ),
            (
                "https://gitea.example.com:3000/Team/Widgets.git",
                Some(("gitea.example.com", "Team", "Widgets")),
            ),
            (
                "https://gitlab.example.com/group/sub/repo.git",
                Some(("gitlab.example.com", "group/sub", "repo")),
            ),
            (
                "ssh://git@gitlab.example.com/group/sub/repo.git",
                Some(("gitlab.example.com", "group/sub", "repo")),
            ),
            (
                "git@gitlab.example.com:group/sub/repo.git",
                Some(("gitlab.example.com", "group/sub", "repo")),
            ),
            (
                "https://dev.azure.com/org/project/_git/repo",
                Some(("dev.azure.com", "org", "repo")),
            ),
            (
                "https://org@dev.azure.com/org/project/_git/repo",
                Some(("dev.azure.com", "org", "repo")),
            ),
            (
                "https://org.visualstudio.com/project/_git/repo",
                Some(("org.visualstudio.com", "org", "repo")),
            ),
            (
                "git@ssh.dev.azure.com:v3/org/project/repo",
                Some(("ssh.dev.azure.com", "org", "repo")),
            ),
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
            let actual = parse_remote(url).map(|parsed| (parsed.host, parsed.owner, parsed.repo));
            let expected = expected
                .map(|(host, owner, repo)| (host.to_owned(), owner.to_owned(), repo.to_owned()));
            assert_eq!(actual, expected, "url {url:?}");
        }
    }

    #[test]
    fn the_provider_is_read_from_the_host() {
        let cases = [
            ("https://github.com/a/b", Provider::Github),
            ("https://gitlab.com/a/b", Provider::Gitlab),
            ("https://gitlab.example.com/a/b", Provider::Gitlab),
            ("https://bitbucket.org/a/b", Provider::Bitbucket),
            ("https://dev.azure.com/o/p/_git/r", Provider::AzureDevops),
            ("git@ssh.dev.azure.com:v3/o/p/r", Provider::AzureDevops),
            ("https://o.visualstudio.com/p/_git/r", Provider::AzureDevops),
            ("https://codeberg.org/a/b", Provider::Gitea),
            ("https://gitea.example.com/a/b", Provider::Gitea),
            ("https://git.example.com/a/b", Provider::Unknown),
            ("https://github.example.com/a/b", Provider::Unknown),
        ];
        for (url, expected) in cases {
            let actual = parse_remote(url).map(|parsed| parsed.provider);
            assert_eq!(actual, Some(expected), "url {url:?}");
        }
    }

    #[test]
    fn the_web_address_is_known_for_known_layouts_and_never_carries_credentials() {
        let cases: &[(&str, Option<&str>)] = &[
            (
                "https://github.com/acme/widgets.git",
                Some("https://github.com/acme/widgets"),
            ),
            (
                "git@github.com:acme/widgets.git",
                Some("https://github.com/acme/widgets"),
            ),
            (
                "https://user:secret@github.com/acme/widgets",
                Some("https://github.com/acme/widgets"),
            ),
            (
                "ssh://git@gitea.example.com:2222/acme/widgets.git",
                Some("https://gitea.example.com/acme/widgets"),
            ),
            (
                "https://gitea.example.com:3000/acme/widgets.git",
                Some("https://gitea.example.com:3000/acme/widgets"),
            ),
            (
                "https://gitlab.example.com/group/sub/repo.git",
                Some("https://gitlab.example.com/group/sub/repo"),
            ),
            (
                "https://dev.azure.com/org/project/_git/repo",
                Some("https://dev.azure.com/org/project/_git/repo"),
            ),
            (
                "git@ssh.dev.azure.com:v3/org/project/repo",
                Some("https://dev.azure.com/org/project/_git/repo"),
            ),
            (
                "https://org.visualstudio.com/project/_git/repo",
                Some("https://org.visualstudio.com/project/_git/repo"),
            ),
            ("https://git.example.com/acme/widgets", None),
        ];
        for (url, expected) in cases {
            let actual = parse_remote(url).and_then(|parsed| parsed.web_url);
            assert_eq!(actual.as_deref(), *expected, "url {url:?}");
        }
    }

    #[test]
    fn origin_is_a_fork_of_an_upstream_on_the_same_host_with_another_owner() {
        let mut remotes = vec![
            identity("origin", "github.com", "acme", "widgets"),
            identity("upstream", "github.com", "ops", "widgets"),
        ];
        mark_primary(&mut remotes);
        mark_fork(&mut remotes);
        assert!(remotes[0].primary);
        assert_eq!(remotes[0].fork_of.as_deref(), Some("ops/widgets"));
        assert_eq!(remotes[1].fork_of, None);
    }

    #[test]
    fn a_remote_on_another_host_or_repository_or_with_the_same_owner_is_no_fork() {
        for upstream in [
            identity("upstream", "gitlab.com", "ops", "widgets"),
            identity("upstream", "github.com", "ops", "gadgets"),
            identity("upstream", "github.com", "acme", "widgets"),
        ] {
            let mut remotes = vec![
                identity("origin", "github.com", "acme", "widgets"),
                upstream,
            ];
            mark_fork(&mut remotes);
            assert_eq!(remotes[0].fork_of, None, "{remotes:?}");
        }
    }

    #[test]
    fn owner_and_repository_case_do_not_make_a_fork_apart() {
        let mut remotes = vec![
            identity("origin", "github.com", "Acme", "widgets"),
            identity("upstream", "github.com", "acme", "Widgets"),
        ];
        mark_fork(&mut remotes);
        assert_eq!(remotes[0].fork_of, None);
    }

    #[test]
    fn the_primary_is_origin_else_the_first_network_remote() {
        let mut remotes = vec![
            identity("backup", "github.com", "acme", "widgets"),
            identity("origin", "github.com", "acme", "widgets"),
        ];
        mark_primary(&mut remotes);
        let marked: Vec<bool> = remotes.iter().map(|remote| remote.primary).collect();
        assert_eq!(marked, [false, true]);

        let mut without_origin = vec![identity("backup", "github.com", "acme", "widgets")];
        mark_primary(&mut without_origin);
        assert!(without_origin[0].primary);

        let mut none: Vec<RemoteIdentity> = Vec::new();
        mark_primary(&mut none);
        assert!(none.is_empty());
    }
}
