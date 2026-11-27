//! `git.remotes`: the hosting identity of each network remote of a workspace or project (capability
//! `git-remote-identity/1`, decision 0114). Provider actions such as pull requests and CI stay in a later phase (0010).

use serde::{Deserialize, Serialize};

/// Params: [`GitRemotesParams`]. Result: [`GitRemotes`].
pub const REMOTES: &str = "git.remotes";

/// The repository to read: the checkout of a workspace, or of a project. Give exactly one of them.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct GitRemotesParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// The network remotes of the repository, in git's order. A remote whose URL is a local path is not listed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitRemotes {
    pub remotes: Vec<RemoteIdentity>,
}

/// One network remote, named after its git remote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteIdentity {
    /// The git remote's name, such as `origin`.
    pub remote: String,
    /// The lowercased host of the URL, such as `github.com`. Never a user name or a port.
    pub host: String,
    pub provider: Provider,
    /// A user, an organization, a GitLab group path such as `group/sub`, or an Azure DevOps organization.
    pub owner: String,
    pub repo: String,
    /// The repository's web address, when the host's layout is known. Never carries credentials.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_url: Option<String>,
    /// The remote the repository's identity is taken from: `origin` when it is a network remote, else the first one.
    pub primary: bool,
    /// `owner/repo` of the upstream, when this remote and `upstream` name the same repository under other owners.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fork_of: Option<String>,
}

/// The hosting service a remote's host belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Github,
    Gitlab,
    Bitbucket,
    AzureDevops,
    Gitea,
    /// A self-hosted or unrecognised host. A value from a newer peer that this build does not know lands here too.
    #[serde(other)]
    Unknown,
}
