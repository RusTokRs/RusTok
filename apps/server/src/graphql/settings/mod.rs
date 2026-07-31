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
