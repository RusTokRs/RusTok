mod marketplace_financial;
mod mutations;
mod query;
mod types;

use async_graphql::{Context, ErrorExtensions, FieldError, MergedObject, Result};
use rustok_api::Permission;
use rustok_api::{
    AuthContext, RequestContext, TenantContext, graphql::GraphQLError, has_any_effective_permission,
};
use sea_orm::DatabaseConnection;

use crate::storefront_channel::is_module_enabled_for_request_channel;

pub use marketplace_financial::{
    MarketplaceFinancialOperationGql, MarketplaceFinancialSweepFailureGql,
    MarketplaceFinancialSweepGql, MarketplacePaidEventGql,
};
pub use mutations::CommerceMutation;
pub use types::*;

#[derive(MergedObject, Default)]
pub struct CommerceQueryRoot(
    query::CommerceQuery,
    marketplace_financial::MarketplaceFinancialQuery,
);

pub type CommerceQuery = CommerceQueryRoot;

#[allow(non_upper_case_globals)]
pub const CommerceQuery: CommerceQueryRoot = CommerceQueryRoot(
    query::CommerceQuery,
    marketplace_financial::MarketplaceFinancialQuery,
);

pub(crate) const MODULE_SLUG: &str = "commerce";
pub(crate) const PRODUCT_MODULE_SLUG: &str = "product";

pub(crate) fn map_product_service_error(
    error: rustok_commerce_foundation::CommerceError,
    operation: &'static str,
) -> async_graphql::Error {
    use rustok_core::error::RichError;

    tracing::error!(error = %error, operation, "product service operation failed");
    let rich: RichError = error.into();
    let public_message = rich
        .user_message
        .clone()
        .unwrap_or_else(|| "Product operation failed".to_owned());
    let code = rich
        .error_code
        .clone()
        .unwrap_or_else(|| "PRODUCT_OPERATION_FAILED".to_owned());

    async_graphql::Error::new(public_message)
        .extend_with(|_, extensions| extensions.set("code", code))
}

pub(crate) fn current_tenant_scope(
    ctx: &Context<'_>,
    requested_tenant_id: Option<uuid::Uuid>,
    operation: &str,
) -> Result<uuid::Uuid> {
    let tenant = ctx.data::<TenantContext>().map_err(|_| {
        <FieldError as GraphQLError>::permission_denied("Tenant context is required")
    })?;
    if requested_tenant_id.is_some_and(|requested| requested != tenant.id) {
        let message = format!("{operation} must use the current tenant");
        return Err(<FieldError as GraphQLError>::permission_denied(&message));
    }
    Ok(tenant.id)
}

pub(crate) fn require_commerce_permission(
    ctx: &Context<'_>,
    permissions: &[Permission],
    message: &str,
) -> Result<AuthContext> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?
        .clone();
    let tenant = ctx.data::<TenantContext>().map_err(|_| {
        <FieldError as GraphQLError>::permission_denied("Tenant context is required")
    })?;

    if auth.tenant_id != tenant.id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Authenticated actor is not bound to the current tenant",
        ));
    }
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(<FieldError as GraphQLError>::permission_denied(message));
    }

    Ok(auth)
}

pub(crate) fn product_mutation_actor(ctx: &Context<'_>) -> Result<(uuid::Uuid, uuid::Uuid)> {
    let auth = ctx
        .data::<AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let tenant = ctx.data::<TenantContext>().map_err(|_| {
        <FieldError as GraphQLError>::permission_denied("Tenant context is required")
    })?;

    if tenant.id != auth.tenant_id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Authenticated actor is not bound to the current tenant",
        ));
    }

    Ok((tenant.id, auth.user_id))
}

pub(crate) fn product_query_tenant(
    ctx: &Context<'_>,
    requested_tenant_id: uuid::Uuid,
) -> Result<uuid::Uuid> {
    let tenant = ctx.data::<TenantContext>().map_err(|_| {
        <FieldError as GraphQLError>::permission_denied("Tenant context is required")
    })?;
    if requested_tenant_id != tenant.id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Product reads must use the current tenant",
        ));
    }
    Ok(tenant.id)
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

#[cfg(test)]
mod tests {
    use super::current_tenant_scope;
    use async_graphql::{EmptyMutation, EmptySubscription, Object, Schema};
    use rustok_api::TenantContext;
    use uuid::Uuid;

    struct Query;

    #[Object]
    impl Query {
        async fn tenant(
            &self,
            ctx: &async_graphql::Context<'_>,
            requested: Option<Uuid>,
        ) -> async_graphql::Result<Uuid> {
            current_tenant_scope(ctx, requested, "test")
        }
    }

    #[tokio::test]
    async fn tenant_scope_rejects_cross_tenant_override() {
        let current = Uuid::new_v4();
        let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
            .data(TenantContext {
                id: current,
                name: "Tenant".to_string(),
                slug: "tenant".to_string(),
                domain: None,
                settings: serde_json::json!({}),
                default_locale: "en".to_string(),
                is_active: true,
            })
            .finish();
        let response = schema
            .execute(format!("{{ tenant(requested: \"{}\") }}", Uuid::new_v4()))
            .await;
        assert!(!response.errors.is_empty());
    }
}
