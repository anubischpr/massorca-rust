//! `git.log` (decision 0011).

use massorca_core_git::LogCursor;
use massorca_protocol::methods::git::log::{
    self, DEFAULT_LOG_LIMIT, GitLog, GitLogParams, MAX_LOG_LIMIT,
};
use massorca_protocol::{ErrorBody, capabilities, codes};

use super::{blocking, git_error, repository};
use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(super) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![registries.methods.register(
        Owner::Core,
        log::LOG,
        capabilities::GIT,
        Permission::Read,
        handle,
    )?])
}

async fn handle(ctx: RequestCtx, params: GitLogParams) -> Result<GitLog, ErrorBody> {
    let limit = params
        .limit
        .unwrap_or(DEFAULT_LOG_LIMIT)
        .clamp(1, MAX_LOG_LIMIT) as usize;
    let cursor = params
        .cursor
        .as_deref()
        .map(|text| {
            LogCursor::decode(text).ok_or_else(|| {
                ErrorBody::new(
                    codes::REQUEST_INVALID_PARAMS,
                    "`cursor` is not a cursor this service returned",
                )
            })
        })
        .transpose()?;
    let repository = repository(&ctx, params.workspace).await?;
    blocking(move || {
        let cursor = match cursor {
            Some(cursor) => cursor,
            None => {
                let revision = params.revision.as_deref().unwrap_or("HEAD");
                let resolved = repository
                    .git
                    .resolve_commit(&repository.dir, revision)
                    .map_err(git_error)?;
                match resolved {
                    Some(start) => LogCursor { start, skip: 0 },
                    // A repository before its first commit has no history yet.
                    None if params.revision.is_none() => {
                        return Ok(GitLog {
                            commits: Vec::new(),
                            next_cursor: None,
                        });
                    }
                    None => {
                        return Err(ErrorBody::new(
                            codes::GIT_REVISION_NOT_FOUND,
                            format!("`{revision}` does not name a commit"),
                        ));
                    }
                }
            }
        };
        repository
            .git
            .log(&repository.dir, &cursor, limit)
            .map_err(git_error)
    })
    .await
}
