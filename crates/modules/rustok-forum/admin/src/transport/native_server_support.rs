#[cfg(feature = "ssr")]
use leptos::prelude::ServerFnError;

#[cfg(feature = "ssr")]
pub(super) fn require_tenant_scope(
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
) -> Result<(), ServerFnError> {
    if auth.tenant_id == tenant.id {
        Ok(())
    } else {
        Err(ServerFnError::new("Forum admin tenant scope mismatch"))
    }
}

#[cfg(feature = "ssr")]
pub(super) fn require_permission(
    auth: &rustok_api::AuthContext,
    permission: rustok_api::Permission,
    message: &'static str,
) -> Result<(), ServerFnError> {
    if rustok_api::has_any_effective_permission(&auth.permissions, &[permission]) {
        Ok(())
    } else {
        Err(ServerFnError::new(message))
    }
}

#[cfg(feature = "ssr")]
pub(super) async fn require_forum_module_enabled(
    host: &rustok_api::HostRuntimeContext,
    tenant_id: uuid::Uuid,
) -> Result<(), ServerFnError> {
    match rustok_api::is_tenant_module_enabled(host.db(), tenant_id, "forum").await {
        Ok(true) => Ok(()),
        Ok(false) => Err(ServerFnError::new("Forum module is not enabled")),
        Err(error) => Err(ServerFnError::new(format!(
            "Forum module state is unavailable: {error}"
        ))),
    }
}

#[cfg(feature = "ssr")]
pub(super) fn runtime() -> Result<
    (
        rustok_api::HostRuntimeContext,
        rustok_outbox::TransactionalEventBus,
    ),
    ServerFnError,
> {
    use leptos::prelude::expect_context;

    let host = expect_context::<rustok_api::HostRuntimeContext>();
    let event_bus = host
        .shared_get::<rustok_outbox::TransactionalEventBus>()
        .ok_or_else(|| {
            ServerFnError::new("Forum admin requires TransactionalEventBus in host runtime context")
        })?;
    Ok((host, event_bus))
}

#[cfg(feature = "ssr")]
pub(super) fn parse_uuid(value: &str, field: &'static str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim()).map_err(|_| ServerFnError::new(format!("Invalid {field}")))
}
