//! `git.diff`, `git.diff_file` (decision 0011) and `git.file_content` (decision 0103).

use base64::{Engine as _, engine::general_purpose::STANDARD};
use massorca_core_git::{FileContentError, SideContent};
use massorca_protocol::methods::git::diff::{
    self, DEFAULT_CONTENT_BYTES, DEFAULT_DIFF_LIMIT, DEFAULT_PATCH_BYTES, DiffTarget, GitDiff,
    GitDiffFile, GitDiffFileParams, GitDiffParams, GitFileContent, GitFileContentParams,
    MAX_CONTENT_BYTES, MAX_DIFF_LIMIT, MAX_PATCH_BYTES, preview_mime,
};
use massorca_protocol::{ErrorBody, capabilities, codes};

use super::{Repository, blocking, git_error, relative_path, repository, require_commit};
use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(super) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![
        registries.methods.register(
            Owner::Core,
            diff::DIFF,
            capabilities::GIT,
            Permission::Read,
            list,
        )?,
        registries.methods.register(
            Owner::Core,
            diff::DIFF_FILE,
            capabilities::GIT,
            Permission::Read,
            file,
        )?,
        registries.methods.register(
            Owner::Core,
            diff::FILE_CONTENT,
            capabilities::GIT,
            Permission::Read,
            file_content,
        )?,
    ])
}

async fn list(ctx: RequestCtx, params: GitDiffParams) -> Result<GitDiff, ErrorBody> {
    let limit = params
        .limit
        .unwrap_or(DEFAULT_DIFF_LIMIT)
        .clamp(1, MAX_DIFF_LIMIT);
    let repository = repository(&ctx, params.workspace).await?;
    let target = params.target;
    blocking(move || {
        check_target(&repository, &target)?;
        repository
            .git
            .diff(&repository.dir, &target, limit as usize)
            .map_err(git_error)
    })
    .await
}

async fn file(ctx: RequestCtx, params: GitDiffFileParams) -> Result<GitDiffFile, ErrorBody> {
    let max_bytes = params
        .max_bytes
        .unwrap_or(DEFAULT_PATCH_BYTES)
        .clamp(1, MAX_PATCH_BYTES);
    relative_path(&params.path)?;
    if let Some(original) = &params.original_path {
        relative_path(original)?;
    }
    let repository = repository(&ctx, params.workspace).await?;
    blocking(move || {
        check_target(&repository, &params.target)?;
        repository
            .git
            .diff_file(
                &repository.dir,
                &params.target,
                &params.path,
                params.original_path.as_deref(),
                max_bytes as usize,
            )
            .map_err(git_error)
    })
    .await
}

async fn file_content(
    ctx: RequestCtx,
    params: GitFileContentParams,
) -> Result<GitFileContent, ErrorBody> {
    let max_bytes = params
        .max_bytes
        .unwrap_or(DEFAULT_CONTENT_BYTES)
        .clamp(1, MAX_CONTENT_BYTES);
    relative_path(&params.path)?;
    let repository = repository(&ctx, params.workspace).await?;
    blocking(move || {
        let content = repository
            .git
            .file_content(
                &repository.dir,
                &params.side,
                &params.path,
                max_bytes as usize,
            )
            .map_err(file_content_error)?;
        Ok(file_content_result(params.path, content))
    })
    .await
}

/// A range's revisions must name commits; the other modes compare fixed sides.
fn check_target(repository: &Repository, target: &DiffTarget) -> Result<(), ErrorBody> {
    if let DiffTarget::Range { from, to } = target {
        require_commit(repository, from)?;
        if let Some(to) = to {
            require_commit(repository, to)?;
        }
    }
    Ok(())
}

/// One side as the protocol reports it: its size, its bytes unless they are over the cap, and the type of a
/// previewable image.
fn file_content_result(path: String, content: SideContent) -> GitFileContent {
    let mime = preview_mime(&path).map(str::to_owned);
    let (size, too_large, content_base64) = match content {
        SideContent::Absent => (None, false, None),
        SideContent::TooLarge { size } => (Some(size), true, None),
        SideContent::Bytes(bytes) => (
            Some(bytes.len() as u64),
            false,
            Some(STANDARD.encode(&bytes)),
        ),
    };
    GitFileContent {
        path,
        size,
        too_large,
        content_base64,
        mime,
    }
}

fn file_content_error(error: FileContentError) -> ErrorBody {
    match error {
        FileContentError::Refused { code, message } => ErrorBody::new(code, message),
        unreadable @ FileContentError::Unreadable { .. } => {
            ErrorBody::new(codes::GIT_FAILED, unreadable.to_string())
        }
        FileContentError::Git(error) => git_error(error),
    }
}
