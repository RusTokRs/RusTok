mod mutation;
mod query;
mod types;

use async_graphql::{Context, FieldError, Result};
use rustok_api::{Action, Permission, Resource};
use rustok_api::{AuthContext, TenantContext, graphql::GraphQLError, has_any_effective_permission};

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
    let runtime = ctx
        .data::<crate::runtime::SharedAlloyRuntime>()
        .map_err(|_| async_graphql::Error::new("Alloy runtime is unavailable"))?;
    let tenant = ctx
        .data::<TenantContext>()
        .map_err(|_| async_graphql::Error::new("Tenant context is unavailable"))?;

    Ok(runtime.0.scoped(tenant.id))
}
