//! Store rows as protocol types. `missing` is answered by the directory itself (ADR 0008),
//! never by git's prunable line.

use massorca_core_store::{ProjectKind, ProjectRow};
use massorca_protocol::methods::projects::{self, Project};

pub(crate) fn project(row: &ProjectRow) -> Project {
    Project {
        id: row.id.clone(),
        kind: match row.kind {
            ProjectKind::Git => projects::ProjectKind::Git,
            ProjectKind::Folder => projects::ProjectKind::Folder,
        },
        path: row.path.clone(),
        name: row.name.clone(),
        workspaces_root: row.workspaces_root.clone(),
        added_at_ms: row.added_at,
    }
}

