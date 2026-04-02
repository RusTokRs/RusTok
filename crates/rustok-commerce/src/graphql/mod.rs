mod mutation;
mod query;
mod types;

use async_graphql::{Context, ErrorExtensions, FieldError, Result};
use rustok_api::{
    graphql::GraphQLError, has_any_effective_permission, AuthContext, RequestContext,
};
use rustok_core::Permission;
use sea_orm::DatabaseConnection;

use crate::storefront_channel::is_module_enabled_for_request_channel;

pub use mutation::CommerceMutation;
pub use query::CommerceQuery;
pub use types::*;

pub(crate) const MODULE_SLUG: &str = "commerce";

pub(crate) fn require_commerce_permission(
    ctx: &Context<'_>,
    permissions: &[Permission],
    message: &str,
) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();

    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(<FieldError as GraphQLError>::permission_denied(message));
    }

    Ok(auth)
}

pub(crate) async fn require_storefront_channel_enabled(ctx: &Context<'_>) -> Result<()> {
    let Some(request_context) = ctx.data_opt::<RequestContext>() else {
        return Ok(());
    };

    let db = ctx.data::<DatabaseConnection>()?;
    let enabled = is_module_enabled_for_request_channel(db, request_context, MODULE_SLUG)
        .await
        .map_err(|err| {
            async_graphql::Error::new(format!("Module check failed: {err}"))
                .extend_with(|_, ext| ext.set("code", "INTERNAL_SERVER_ERROR"))
        })?;

    if !enabled {
        return Err(async_graphql::Error::new(format!(
            "Module '{MODULE_SLUG}' is not enabled for channel '{}'",
            request_context.channel_slug.as_deref().unwrap_or("current"),
        ))
        .extend_with(|_, ext| ext.set("code", "MODULE_NOT_ENABLED")));
    }

    Ok(())
}
