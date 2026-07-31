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

pub(super) fn require_host_authority<'a>(
    ctx: &'a async_graphql::Context<'_>,
    required: rustok_api::HostAuthority,
) -> async_graphql::Result<&'a rustok_api::HostAuthorityContext> {
    use rustok_api::graphql::GraphQLError;

    ctx.data_opt::<rustok_api::HostAuthorityContext>()
        .filter(|authority| authority.allows(required))
        .ok_or_else(|| {
            <async_graphql::FieldError as GraphQLError>::permission_denied(
                "host-global authority required",
            )
        })
}

pub(super) fn require_host_actor<'a>(
    ctx: &'a async_graphql::Context<'_>,
    required: rustok_api::HostAuthority,
) -> async_graphql::Result<(
    &'a rustok_api::HostAuthorityContext,
    &'a crate::context::AuthContext,
)> {
    use rustok_api::graphql::GraphQLError;

    let authority = require_host_authority(ctx, required)?;
    let auth = ctx
        .data::<crate::context::AuthContext>()
        .map_err(|_| <async_graphql::FieldError as GraphQLError>::unauthenticated())?;
    if authority.actor_id() != auth.user_id {
        return Err(<async_graphql::FieldError as GraphQLError>::permission_denied(
            "host-global authority required",
        ));
    }
    Ok((authority, auth))
}
