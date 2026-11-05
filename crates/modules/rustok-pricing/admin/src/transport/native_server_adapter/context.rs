#![allow(unused_imports, clippy::empty_line_after_outer_attr)]
use leptos::prelude::*;
use rustok_api::HostRuntimeContext;
use rustok_ui_core::normalize_optional_ui_text;
use uuid::Uuid;

use crate::core::{parse_optional_currency_code, text_or_none};

pub(crate) fn parse_required_currency_code(currency_code: String) -> Result<String, ServerFnError> {
    parse_optional_currency_code(Some(currency_code))
        .map_err(|error| ServerFnError::new(error.to_string()))?
        .ok_or_else(|| ServerFnError::new("currency_code must be a 3-letter code"))
}

#[cfg(feature = "ssr")]
pub(crate) fn parse_decimal(
    value: &str,
    field_name: &str,
) -> Result<rust_decimal::Decimal, ServerFnError> {
    <rust_decimal::Decimal as std::str::FromStr>::from_str(value.trim())
        .map_err(|_| ServerFnError::new(format!("Invalid {field_name}")))
}

#[cfg(feature = "ssr")]
pub(crate) fn parse_optional_decimal(
    value: &str,
    field_name: &str,
) -> Result<Option<rust_decimal::Decimal>, ServerFnError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parse_decimal(trimmed, field_name)?))
    }
}

#[cfg(feature = "ssr")]
pub(crate) fn parse_optional_quantity(
    value: &str,
    field_name: &str,
) -> Result<Option<i32>, ServerFnError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let parsed = trimmed
        .parse::<i32>()
        .map_err(|_| ServerFnError::new(format!("Invalid {field_name}")))?;
    if parsed <= 0 {
        return Err(ServerFnError::new(format!(
            "{field_name} must be greater than zero"
        )));
    }

    Ok(Some(parsed))
}

#[cfg(feature = "ssr")]
pub(crate) fn parse_optional_uuid(
    value: &str,
    field_name: &str,
) -> Result<Option<Uuid>, ServerFnError> {
    let Some(value) = text_or_none(value.to_string()) else {
        return Ok(None);
    };

    let parsed = Uuid::parse_str(value.as_str())
        .map_err(|_| ServerFnError::new(format!("Invalid {field_name}")))?;

    if parsed.is_nil() {
        return Err(ServerFnError::new(format!("Invalid {field_name}")));
    }

    Ok(Some(parsed))
}

#[cfg(feature = "ssr")]
pub(crate) fn resolve_requested_locale(
    requested: Option<String>,
    request_context_locale: Option<&str>,
    tenant_default_locale: &str,
) -> String {
    normalize_optional_ui_text(requested)
        .or_else(|| {
            request_context_locale
                .and_then(|value| normalize_optional_ui_text(Some(value.to_string())))
        })
        .or_else(|| normalize_optional_ui_text(Some(tenant_default_locale.to_string())))
        .unwrap_or_default()
}

#[cfg(feature = "ssr")]
pub(crate) fn ensure_permission(
    permissions: &[rustok_api::Permission],
    required: &[rustok_api::Permission],
    message: &str,
) -> Result<(), ServerFnError> {
    if !rustok_api::has_any_effective_permission(permissions, required) {
        return Err(ServerFnError::new(format!("Permission denied: {message}")));
    }

    Ok(())
}

#[cfg(feature = "ssr")]
pub(crate) fn parse_product_id(value: &str) -> Result<Uuid, ServerFnError> {
    Uuid::parse_str(value.trim()).map_err(|_| ServerFnError::new("Invalid product_id"))
}

#[cfg(feature = "ssr")]

pub(crate) fn transactional_event_bus_from_runtime(
    runtime_ctx: &HostRuntimeContext,
) -> Result<rustok_outbox::TransactionalEventBus, ServerFnError> {
    runtime_ctx
        .shared_get::<rustok_outbox::TransactionalEventBus>()
        .ok_or_else(|| {
            ServerFnError::new(
                "Pricing admin requires TransactionalEventBus in host runtime context",
            )
        })
}

#[cfg(feature = "ssr")]
pub(crate) fn product_read_runtime(
    app_ctx: &HostRuntimeContext,
) -> Result<rustok_product::ProductCatalogReadRuntime, ServerFnError> {
    if let Some(runtime) = app_ctx.shared_get::<rustok_product::ProductCatalogReadRuntime>() {
        return Ok(runtime);
    }
    // Only a host with no selection uses the explicit embedded baseline.
    Ok(rustok_product::ProductCatalogReadRuntime::in_process(
        app_ctx.db_clone(),
        transactional_event_bus_from_runtime(app_ctx)?,
    ))
}

#[cfg(feature = "ssr")]
pub(crate) fn pricing_admin_read_context(
    tenant: &rustok_api::TenantContext,
    auth: &rustok_api::AuthContext,
    request: &rustok_api::RequestContext,
    locale: &str,
) -> Result<rustok_api::PortContext, ServerFnError> {
    if tenant.id.is_nil()
        || auth.user_id.is_nil()
        || auth.tenant_id != tenant.id
        || request.tenant_id != tenant.id
        || request
            .user_id
            .is_some_and(|user_id| user_id != auth.user_id)
    {
        return Err(ServerFnError::new("Pricing request context is unavailable"));
    }
    let mut context = rustok_api::PortContext::new(
        tenant.id.to_string(),
        rustok_api::PortActor::user(auth.user_id.to_string()),
        locale,
        request.correlation_id.clone(),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    context.channel = request.channel_slug.clone();
    context.claims = auth.scopes.clone();
    context
        .claims
        .extend(auth.permissions.iter().map(ToString::to_string));
    Ok(context)
}
