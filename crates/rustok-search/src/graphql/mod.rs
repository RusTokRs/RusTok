mod forum_projection_reconciliation;
mod forum_storefront;
mod mutation;
mod query;
mod rate_limit;
mod types;

pub use forum_projection_reconciliation::{
    ForumSearchProjectionReconciliationQuery, GqlForumSearchProjectionDrift,
    GqlForumSearchProjectionReconciliationStatus,
};
pub use forum_storefront::ForumStorefrontSearchQuery;
pub use mutation::SearchMutationRoot;
pub use query::SearchQueryRoot;
pub use rate_limit::{
    SearchGraphqlRateLimitError, SearchGraphqlRateLimitExceeded, SearchGraphqlRateLimiter,
    SearchGraphqlRateLimiterHandle,
};
pub use types::*;

async fn ensure_search_admin_permission(
    ctx: &async_graphql::Context<'_>,
    permission: &rustok_api::Permission,
) -> async_graphql::Result<()> {
    use rustok_api::graphql::GraphQLError;

    let auth = ctx
        .data::<rustok_api::AuthContext>()
        .map_err(|_| <async_graphql::FieldError as GraphQLError>::unauthenticated())?;
    let tenant = ctx.data::<rustok_api::TenantContext>()?;

    if auth.tenant_id != tenant.id {
        tracing::warn!(
            auth_tenant_id = %auth.tenant_id,
            resolved_tenant_id = %tenant.id,
            code = "search.graphql_tenant_scope_mismatch",
            boundary = "search_graphql_admin",
            "Search GraphQL admin authority cannot cross the resolved tenant boundary"
        );
        return Err(<async_graphql::FieldError as GraphQLError>::permission_denied(
            "Search administration access is denied",
        ));
    }

    if !rustok_api::has_effective_permission(&auth.permissions, permission) {
        return Err(<async_graphql::FieldError as GraphQLError>::permission_denied(
            &format!("{permission} required"),
        ));
    }

    Ok(())
}
