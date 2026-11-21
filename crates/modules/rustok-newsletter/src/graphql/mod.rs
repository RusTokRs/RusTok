//! GraphQL adapters for the newsletter module.

mod mutation;
mod query;
mod types;

pub use mutation::NewsletterMutation;
pub use query::NewsletterQuery;

use async_graphql::Error;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

/// Runtime data attached to the GraphQL schema for newsletter resolvers.
pub struct NewsletterGraphqlRuntimeData {
    pub db: DatabaseConnection,
}

impl NewsletterGraphqlRuntimeData {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

/// Resolve the tenant for an administrative newsletter operation.
///
/// This GraphQL surface is exported for hosts to mount, so it must not trust
/// a client-supplied tenant id: the caller must be an authenticated actor
/// bound to the resolved tenant, hold the required permission, and the
/// requested tenant (when supplied) must be that same tenant. Public
/// subscribe/confirm/unsubscribe flows use [`public_tenant_id`] instead.
pub(super) fn require_newsletter_admin(
    ctx: &async_graphql::Context<'_>,
    requested: Option<uuid::Uuid>,
    permissions: &[rustok_api::Permission],
) -> async_graphql::Result<uuid::Uuid> {
    use async_graphql::FieldError;
    use rustok_api::graphql::GraphQLError;

    let auth = ctx
        .data::<rustok_api::AuthContext>()
        .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
    let tenant = ctx.data::<rustok_api::TenantContext>()?;

    if tenant.id.is_nil() || auth.tenant_id.is_nil() || auth.user_id.is_nil() {
        return Err(<FieldError as GraphQLError>::unauthenticated());
    }
    if auth.tenant_id != tenant.id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Authenticated actor is not bound to the current tenant",
        ));
    }
    if let Some(requested) = requested {
        if requested.is_nil() {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Requested tenant ID must not be nil",
            ));
        }
        if requested != tenant.id {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Newsletter operations must use the current tenant",
            ));
        }
    }
    if !rustok_api::has_any_effective_permission(&auth.permissions, permissions) {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "Permission denied: newsletter access required",
        ));
    }

    Ok(tenant.id)
}

/// Resolve the tenant for the intentionally public subscribe/confirm/
/// unsubscribe flows. When the host resolved a tenant for the request, a
/// conflicting client-supplied tenant id is rejected; hosts without tenant
/// resolution keep the explicit argument.
pub(super) fn public_tenant_id(
    ctx: &async_graphql::Context<'_>,
    requested: Option<uuid::Uuid>,
) -> async_graphql::Result<uuid::Uuid> {
    use async_graphql::FieldError;
    use rustok_api::graphql::GraphQLError;

    let tenant_id = requested.ok_or_else(|| Error::new("tenant_id is required"))?;
    if tenant_id.is_nil() {
        return Err(Error::new("tenant_id must not be nil"));
    }
    if let Some(tenant) = ctx.data_opt::<rustok_api::TenantContext>() {
        if tenant.id.is_nil() || tenant_id != tenant.id {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "Newsletter operations must use the current tenant",
            ));
        }
    }
    Ok(tenant_id)
}

/// Error type for newsletter GraphQL runtime initialization.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct NewsletterGraphqlRuntimeError(pub String);

impl From<&str> for NewsletterGraphqlRuntimeError {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for NewsletterGraphqlRuntimeError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<NewsletterGraphqlRuntimeError> for String {
    fn from(e: NewsletterGraphqlRuntimeError) -> Self {
        e.0
    }
}

/// Factory function to attach newsletter runtime data to the GraphQL context.
pub fn attach_schema_data(
    inputs: &rustok_api::graphql::GraphqlRuntimeInputs,
) -> Result<Arc<NewsletterGraphqlRuntimeData>, NewsletterGraphqlRuntimeError> {
    Ok(Arc::new(NewsletterGraphqlRuntimeData::new(
        inputs.db_clone(),
    )))
}
