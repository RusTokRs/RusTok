#[cfg(feature = "ssr")]
fn map_core_error(error: rustok_core::Error) -> ServerFnError {
    ServerFnError::new(error.to_string())
}

#[cfg(feature = "ssr")]
fn normalize_analytics_days(value: Option<i32>) -> u32 {
    value.unwrap_or(7).clamp(1, 30) as u32
}

#[cfg(feature = "ssr")]
fn normalize_analytics_limit(value: Option<i32>) -> usize {
    value.unwrap_or(10).clamp(1, 25) as usize
}

#[cfg(feature = "ssr")]
fn normalize_limit(value: Option<i32>, default: i32, max: i32) -> usize {
    value.unwrap_or(default).clamp(1, max) as usize
}

#[cfg(feature = "ssr")]
fn ensure_search_admin_tenant_scope(
    auth: &rustok_api::AuthContext,
    resolved_tenant_id: uuid::Uuid,
) -> Result<(), ServerFnError> {
    if auth.tenant_id == resolved_tenant_id {
        return Ok(());
    }

    tracing::warn!(
        auth_tenant_id = %auth.tenant_id,
        resolved_tenant_id = %resolved_tenant_id,
        code = "search.admin_tenant_scope_mismatch",
        boundary = "search_admin_native_transport",
        "search admin permissions cannot cross the resolved tenant boundary"
    );
    Err(ServerFnError::new("Search admin access is denied"))
}

#[cfg(feature = "ssr")]
fn ensure_settings_read_permission(
    auth: &rustok_api::AuthContext,
    resolved_tenant_id: uuid::Uuid,
) -> Result<(), ServerFnError> {
    ensure_search_admin_tenant_scope(auth, resolved_tenant_id)?;
    if !rustok_api::has_effective_permission(
        &auth.permissions,
        &rustok_api::Permission::SETTINGS_READ,
    ) {
        return Err(ServerFnError::new("settings:read required"));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn ensure_settings_manage_permission(
    auth: &rustok_api::AuthContext,
    resolved_tenant_id: uuid::Uuid,
) -> Result<(), ServerFnError> {
    ensure_search_admin_tenant_scope(auth, resolved_tenant_id)?;
    if !rustok_api::has_effective_permission(
        &auth.permissions,
        &rustok_api::Permission::SETTINGS_MANAGE,
    ) {
        return Err(ServerFnError::new("settings:manage required"));
    }
    Ok(())
}

#[cfg(feature = "ssr")]
fn parse_required_uuid(value: &str, field_name: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim())
        .map_err(|_| ServerFnError::new(format!("Invalid {field_name}")))
}

#[cfg(feature = "ssr")]
fn parse_optional_uuid(value: Option<&str>) -> Result<Option<uuid::Uuid>, ServerFnError> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            uuid::Uuid::parse_str(value.trim()).map_err(|_| ServerFnError::new("Invalid UUID"))
        })
        .transpose()
}

#[cfg(feature = "ssr")]
fn parse_engine(
    value: &str,
    field_name: &str,
) -> Result<rustok_search::SearchEngineKind, ServerFnError> {
    rustok_search::SearchEngineKind::try_from_str(value)
        .ok_or_else(|| ServerFnError::new(format!("Invalid {field_name}: unsupported engine")))
}

#[cfg(feature = "ssr")]
fn ensure_engine_available(engine: rustok_search::SearchEngineKind) -> Result<(), ServerFnError> {
    let module = rustok_search::SearchModule;
    if module
        .available_engines()
        .into_iter()
        .any(|descriptor| descriptor.enabled && descriptor.kind == engine)
    {
        Ok(())
    } else {
        Err(ServerFnError::new(format!(
            "Engine '{}' is not installed in the current runtime",
            engine.as_str()
        )))
    }
}
