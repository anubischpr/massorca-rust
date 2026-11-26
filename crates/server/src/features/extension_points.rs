//! `extensions.points` (capability `extension-points/1`, decision 0153): what each owner registered on the tool, skill,
//! view, theme and status-rule points, for debugging. It syncs the points with the enabled packages first.

use std::sync::Arc;

use massorca_protocol::methods::extension_points::{
    self as wire, ExtensionPointOwner, ExtensionPointProblem, ExtensionPointsList,
};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::extension_points::sync_extension_points;
use crate::features::NoParams;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let cap = capabilities::EXTENSION_POINTS;
    Ok(vec![
        registries.capabilities.register(cap, Owner::Core)?,
        registries
            .methods
            .register(Owner::Core, wire::POINTS, cap, Permission::Read, points)?,
    ])
}

async fn points(ctx: RequestCtx, _params: NoParams) -> Result<ExtensionPointsList, ErrorBody> {
    let (points, extensions) = (
        Arc::clone(&ctx.state.extension_points),
        Arc::clone(&ctx.state.extensions),
    );
    let synced = Arc::clone(&points);
    ctx.state
        .activity
        .blocking(move || sync_extension_points(&synced, &extensions))
        .await
        .map_err(|error| {
            ErrorBody::new(
                codes::INTERNAL,
                format!("the extension points task failed: {error}"),
            )
        })?;
    Ok(ExtensionPointsList {
        owners: points
            .by_owner()
            .into_iter()
            .map(|(owner, held)| ExtensionPointOwner {
                owner,
                tools: held.tools,
                skills: held.skills,
                views: held.views,
                themes: held.themes,
                status_rules: held.status_rules,
            })
            .collect(),
        problems: points
            .problems()
            .into_iter()
            .map(|problem| ExtensionPointProblem {
                id: problem.id,
                code: problem.code,
                message: problem.message,
            })
            .collect(),
    })
}
