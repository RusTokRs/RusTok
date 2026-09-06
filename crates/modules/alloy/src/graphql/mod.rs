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
    let tenant = ctx
        .data::<TenantContext>()
        .map_err(|_| async_graphql::Error::new("Tenant context is unavailable"))?;
    if auth.tenant_id != tenant.id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Authenticated tenant does not match the request tenant",
        ));
    }

    let required = Permission::new(Resource::Scripts, Action::Manage);
    if !has_any_effective_permission(&auth.permissions, &[required]) {
        return Err(<FieldError as GraphQLError>::permission_denied("Forbidden"));
    }

    Ok(auth)
}

pub(crate) async fn require_release_admin(ctx: &Context<'_>) -> Result<AuthContext> {
    let auth = require_admin(ctx).await?;
    let required = Permission::new(Resource::Modules, Action::Manage);
    if !has_any_effective_permission(&auth.permissions, &[required]) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Alloy release staging requires modules.manage permission",
        ));
    }
    Ok(auth)
}

pub(crate) fn release_governance_from_graphql_ctx(
    ctx: &Context<'_>,
) -> Result<crate::AlloyReleaseGovernanceHandle> {
    let handle = ctx
        .data::<crate::AlloyReleaseGovernanceHandle>()
        .map_err(|_| async_graphql::Error::new("Alloy release governance is unavailable"))?;
    Ok(handle.clone())
}

pub(crate) fn published_rhai_source_from_graphql_ctx(
    ctx: &Context<'_>,
) -> Result<crate::AlloyPublishedRhaiSourceProviderHandle> {
    let handle = ctx
        .data::<crate::AlloyPublishedRhaiSourceProviderHandle>()
        .map_err(|_| async_graphql::Error::new("Alloy published Rhai source is unavailable"))?;
    Ok(handle.clone())
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

#[cfg(test)]
mod tests {
    use async_graphql::{Context, EmptyMutation, EmptySubscription, Object, Request, Schema};
    use rustok_api::{Action, AuthContext, Permission, Resource};
    use uuid::Uuid;

    use super::{TenantContext, require_admin};

    struct TestQuery;

    #[Object]
    impl TestQuery {
        async fn requires_alloy_admin(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
            require_admin(ctx).await?;
            Ok(true)
        }
    }

    fn tenant(id: Uuid) -> TenantContext {
        TenantContext {
            id,
            name: "test tenant".to_string(),
            slug: "test-tenant".to_string(),
            domain: None,
            settings: serde_json::Value::Null,
            default_locale: "en".to_string(),
            is_active: true,
        }
    }

    fn auth(tenant_id: Uuid, permissions: Vec<Permission>) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "password".to_string(),
        }
    }

    #[tokio::test]
    async fn require_admin_rejects_a_cross_tenant_principal() {
        let tenant_id = Uuid::new_v4();
        let schema = Schema::build(TestQuery, EmptyMutation, EmptySubscription)
            .data(tenant(tenant_id))
            .data(auth(
                Uuid::new_v4(),
                vec![Permission::new(Resource::Scripts, Action::Manage)],
            ))
            .finish();

        let response = schema.execute(Request::new("{ requiresAlloyAdmin }")).await;

        assert!(!response.errors.is_empty());
    }

    #[tokio::test]
    async fn require_admin_accepts_a_matching_scripts_manage_principal() {
        let tenant_id = Uuid::new_v4();
        let schema = Schema::build(TestQuery, EmptyMutation, EmptySubscription)
            .data(tenant(tenant_id))
            .data(auth(
                tenant_id,
                vec![Permission::new(Resource::Scripts, Action::Manage)],
            ))
            .finish();

        let response = schema.execute(Request::new("{ requiresAlloyAdmin }")).await;

        assert!(response.errors.is_empty());
    }
}
