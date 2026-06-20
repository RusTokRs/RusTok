mod mutation;
mod query;
mod types;

use async_graphql::{Context, FieldError, Result};
use loco_rs::app::AppContext;
use rustok_api::{graphql::GraphQLError, has_any_effective_permission, AuthContext, TenantContext};
use rustok_core::{permissions::Action, Permission, Resource};

pub use mutation::AlloyMutation;
pub use query::{AlloyQuery, EXECUTION_HISTORY_GRAPHQL_FIELDS};
pub use types::*;

pub(crate) async fn require_admin(ctx: &Context<'_>) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();

    let required = Permission::new(Resource::Scripts, Action::Manage);
    if !has_any_effective_permission(&auth.permissions, &[required]) {
        return Err(<FieldError as GraphQLError>::permission_denied("Forbidden"));
    }

    Ok(auth)
}

pub(crate) fn runtime_from_graphql_ctx(
    ctx: &Context<'_>,
) -> Result<crate::runtime::ScopedAlloyRuntime> {
    let app_ctx = ctx
        .data::<AppContext>()
        .map_err(|_| async_graphql::Error::new("Alloy runtime is unavailable"))?;
    let tenant = ctx
        .data::<TenantContext>()
        .map_err(|_| async_graphql::Error::new("Tenant context is unavailable"))?;

    Ok(crate::runtime::scoped_runtime(app_ctx, tenant.id))
}
