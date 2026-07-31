pub mod mutation;
pub mod query;
pub mod types;

pub use mutation::SettingsMutation;
pub use query::SettingsQuery;
pub use types::*;

pub(super) fn require_tenant_settings_scope(
    auth: &crate::context::AuthContext,
    resolved_tenant_id: uuid::Uuid,
) -> async_graphql::Result<()> {
    use rustok_api::graphql::GraphQLError;

    if auth.tenant_id == resolved_tenant_id {
        return Ok(());
    }

    tracing::warn!(
        auth_tenant_id = %auth.tenant_id,
        resolved_tenant_id = %resolved_tenant_id,
        code = "settings.tenant_scope_mismatch",
        boundary = "server_settings_graphql",
        "tenant settings authority cannot cross the resolved tenant boundary"
    );
    Err(<async_graphql::FieldError as GraphQLError>::permission_denied(
        "Settings access is denied",
    ))
}

pub(super) fn require_host_authority(
    ctx: &async_graphql::Context<'_>,
    required: rustok_api::HostAuthority,
) -> async_graphql::Result<rustok_api::HostAuthorityContext> {
    use axum::http::HeaderMap;
    use rustok_api::graphql::GraphQLError;

    let headers = ctx.data_opt::<HeaderMap>().ok_or_else(|| {
        <async_graphql::FieldError as GraphQLError>::permission_denied(
            "host-global authority required",
        )
    })?;
    match crate::host_authority::resolve_host_authority(headers) {
        Ok(Some(authority)) if authority.allows(required) => Ok(authority),
        Ok(_) | Err(crate::error::Error::Unauthorized(_)) => Err(
            <async_graphql::FieldError as GraphQLError>::permission_denied(
                "host-global authority required",
            ),
        ),
        Err(error) => {
            tracing::error!(
                error = %error,
                code = "host_authority.graphql_configuration_invalid",
                "host authority credential configuration is invalid"
            );
            Err(<async_graphql::FieldError as GraphQLError>::internal_error(
                "Host authority configuration is invalid",
            ))
        }
    }
}

/// Iggy connector secrets are still stored under a routed tenant owner. The
/// host credential grants process-wide mutation authority, while ordinary
/// authentication supplies only the tenant secret-owner boundary.
pub(super) fn require_host_actor<'a>(
    ctx: &'a async_graphql::Context<'_>,
    required: rustok_api::HostAuthority,
) -> async_graphql::Result<(
    rustok_api::HostAuthorityContext,
    &'a crate::context::AuthContext,
)> {
    use rustok_api::graphql::GraphQLError;

    let authority = require_host_authority(ctx, required)?;
    let auth = ctx
        .data::<crate::context::AuthContext>()
        .map_err(|_| <async_graphql::FieldError as GraphQLError>::unauthenticated())?;
    let tenant = ctx.data::<crate::context::TenantContext>()?;
    require_tenant_settings_scope(auth, tenant.id)?;
    Ok((authority, auth))
}
