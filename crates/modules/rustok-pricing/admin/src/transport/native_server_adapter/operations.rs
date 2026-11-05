use leptos::prelude::*;
use rustok_api::{AuthContext, HostRuntimeContext, Permission, TenantContext};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::str::FromStr;
use uuid::Uuid;

use crate::core::{parse_optional_uuid_string, sanitize_channel_slug, text_or_none};
use crate::model::{
    PricingAdjustmentPreview, PricingChannelOption, PricingDiscountDraft, PricingPriceDraft,
    PricingPriceListOption, PricingPriceListRuleDraft, PricingPriceListScopeDraft,
};
use super::context::*;
use super::mappers::*;

pub(crate) async fn update_variant_price_native_with_context(
    app_ctx: &HostRuntimeContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    variant_id: String,
    payload: PricingPriceDraft,
) -> Result<(), ServerFnError> {
    use rustok_api::Permission;
    use rustok_pricing::PricingService;

    ensure_permission(
        &auth.permissions,
        &[Permission::PRODUCTS_UPDATE],
        "products:update required",
    )?;

    let variant_id = parse_product_id(&variant_id)?;
    let currency_code = parse_required_currency_code(payload.currency_code)?;
    let amount = parse_decimal(&payload.amount, "amount")?;
    let compare_at_amount =
        parse_optional_decimal(&payload.compare_at_amount, "compare_at_amount")?;
    let price_list_id = parse_optional_uuid(&payload.price_list_id, "price_list_id")?;
    let channel_id = parse_optional_uuid(&payload.channel_id, "channel_id")?;
    let channel_slug = sanitize_channel_slug(text_or_none(payload.channel_slug));
    let min_quantity = parse_optional_quantity(&payload.min_quantity, "min_quantity")?;
    let max_quantity = parse_optional_quantity(&payload.max_quantity, "max_quantity")?;

    let service = PricingService::new(
        app_ctx.db_clone(),
        transactional_event_bus_from_runtime(app_ctx)?,
    )
    .with_product_catalog_read_runtime(product_read_runtime(app_ctx)?);
    if let Some(price_list_id) = price_list_id {
        service
            .set_price_list_tier_with_channel(
                tenant.id,
                auth.user_id,
                variant_id,
                price_list_id,
                currency_code.as_str(),
                amount,
                compare_at_amount,
                channel_id,
                channel_slug,
                min_quantity,
                max_quantity,
            )
            .await
            .map_err(ServerFnError::new)?;
    } else {
        service
            .set_price_tier_with_channel(
                tenant.id,
                auth.user_id,
                variant_id,
                currency_code.as_str(),
                amount,
                compare_at_amount,
                channel_id,
                channel_slug,
                min_quantity,
                max_quantity,
            )
            .await
            .map_err(ServerFnError::new)?;
    }

    Ok(())
}

#[cfg(feature = "ssr")]
pub(crate) async fn preview_variant_discount_native_with_context(
    app_ctx: &HostRuntimeContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    request_context: &rustok_api::RequestContext,
    variant_id: String,
    payload: PricingDiscountDraft,
) -> Result<PricingAdjustmentPreview, ServerFnError> {
    use rustok_api::Permission;
    use rustok_pricing::{PreviewVariantDiscountRequest, in_process_pricing_read_port};

    ensure_permission(
        &auth.permissions,
        &[Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
        "products:read required",
    )?;

    let variant_id = parse_product_id(&variant_id)?;
    let currency_code = parse_required_currency_code(payload.currency_code)?;
    let discount_percent = parse_decimal(&payload.discount_percent, "discount_percent")?;
    let price_list_id = parse_optional_uuid(&payload.price_list_id, "price_list_id")?;
    let channel_id = parse_optional_uuid(&payload.channel_id, "channel_id")?;
    let channel_slug = sanitize_channel_slug(text_or_none(payload.channel_slug));
    let mut context =
        pricing_admin_read_context(tenant, auth, request_context, &request_context.locale)?;
    // Explicit selectors never inherit the host's other half.
    if channel_id.is_some() || channel_slug.is_some() {
        context.channel = channel_slug.clone();
    }
    in_process_pricing_read_port(app_ctx.db_clone(), transactional_event_bus_from_runtime(app_ctx)?, product_read_runtime(app_ctx)?)
        .preview_variant_discount(context, PreviewVariantDiscountRequest {
            variant_id, price_list_id, currency_code, discount_percent, channel_id, channel_slug,
        }).await.map(map_adjustment_preview).map_err(|error| {
            tracing::warn!(boundary = "pricing_admin_native_preview", kind = ?error.kind, retryable = error.retryable,
                code_length = error.code.len(), "pricing preview was refused");
            ServerFnError::new(error.message)
        })
}

#[cfg(feature = "ssr")]
pub(crate) async fn apply_variant_discount_native_with_context(
    app_ctx: &HostRuntimeContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    variant_id: String,
    payload: PricingDiscountDraft,
) -> Result<PricingAdjustmentPreview, ServerFnError> {
    use rustok_api::Permission;
    use rustok_pricing::PricingService;

    ensure_permission(
        &auth.permissions,
        &[Permission::PRODUCTS_UPDATE],
        "products:update required",
    )?;

    let variant_id = parse_product_id(&variant_id)?;
    let currency_code = parse_required_currency_code(payload.currency_code)?;
    let discount_percent = parse_decimal(&payload.discount_percent, "discount_percent")?;
    let price_list_id = parse_optional_uuid(&payload.price_list_id, "price_list_id")?;
    let channel_id = parse_optional_uuid(&payload.channel_id, "channel_id")?;
    let channel_slug = sanitize_channel_slug(text_or_none(payload.channel_slug));
    let service = PricingService::new(
        app_ctx.db_clone(),
        transactional_event_bus_from_runtime(app_ctx)?,
    )
    .with_product_catalog_read_runtime(product_read_runtime(app_ctx)?);

    let preview = if let Some(price_list_id) = price_list_id {
        service
            .apply_price_list_percentage_discount_with_channel(
                tenant.id,
                auth.user_id,
                variant_id,
                price_list_id,
                currency_code.as_str(),
                discount_percent,
                channel_id,
                channel_slug,
            )
            .await
    } else {
        service
            .apply_percentage_discount_with_channel(
                tenant.id,
                auth.user_id,
                variant_id,
                currency_code.as_str(),
                discount_percent,
                channel_id,
                channel_slug,
            )
            .await
    };

    preview
        .map(map_adjustment_preview)
        .map_err(ServerFnError::new)
}

#[cfg(feature = "ssr")]

pub(crate) async fn list_active_price_lists_native_with_context(
    app_ctx: &HostRuntimeContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    request_context: &rustok_api::RequestContext,
    channel_id: Option<String>,
    channel_slug: Option<String>,
) -> Result<Vec<PricingPriceListOption>, ServerFnError> {
    use rustok_api::Permission;
    use rustok_pricing::{ActivePriceListProjectionRequest, in_process_pricing_read_port};

    ensure_permission(
        &auth.permissions,
        &[Permission::PRODUCTS_LIST, Permission::PRODUCTS_READ],
        "products:list or products:read required",
    )?;
    if auth.tenant_id != tenant.id || request_context.tenant_id != tenant.id {
        return Err(ServerFnError::new("Pricing request tenant is inconsistent"));
    }

    let requested_locale = resolve_requested_locale(
        None,
        Some(request_context.locale.as_str()),
        tenant.default_locale.as_str(),
    );
    let channel_id = parse_optional_uuid_string(channel_id, "channel_id")
        .map_err(|error| ServerFnError::new(error.to_string()))?;
    let explicit_scope = channel_id.is_some() || channel_slug.is_some();
    let selected_channel_id = if explicit_scope {
        channel_id
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|_| ServerFnError::new("Invalid channel_id"))?
    } else {
        request_context.channel_id
    };
    let selected_channel_slug = if explicit_scope {
        channel_slug.map(|slug| slug.trim().to_ascii_lowercase())
    } else {
        request_context.channel_slug.clone()
    };
    let mut port_context = rustok_api::PortContext::new(
        tenant.id.to_string(),
        rustok_api::PortActor::user(auth.user_id.to_string()),
        requested_locale,
        request_context.correlation_id.clone(),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    port_context.claims = auth.scopes.clone();
    port_context
        .claims
        .extend(auth.permissions.iter().map(ToString::to_string));
    // Explicit ID-only/slug-only requests must not borrow the host's other selector half.
    port_context.channel = selected_channel_slug.clone();
    let port = in_process_pricing_read_port(
        app_ctx.db_clone(),
        transactional_event_bus_from_runtime(app_ctx)?,
        product_read_runtime(app_ctx)?,
    );
    let items = port
        .list_active_price_list_projections(
            port_context,
            ActivePriceListProjectionRequest {
                channel_id: selected_channel_id,
                channel_slug: selected_channel_slug,
                fallback_locale: Some(tenant.default_locale.clone()),
            },
        )
        .await
        .map_err(|error| {
            ServerFnError::new(rustok_pricing::map_pricing_port_error(&error).to_string())
        })?;
    Ok(items
        .into_iter()
        .map(|item| PricingPriceListOption {
            id: item.price_list_id.to_string(),
            name: item.title,
            list_type: item.list_type,
            channel_id: item.channel_id.map(|id| id.to_string()),
            channel_slug: item.channel_slug,
            rule_kind: item.rule_kind,
            adjustment_percent: item
                .adjustment_percent
                .map(|value| value.normalize().to_string()),
        })
        .collect())
}

#[cfg(feature = "ssr")]
pub(crate) async fn update_price_list_rule_native_with_context(
    app_ctx: &HostRuntimeContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    price_list_id: String,
    payload: PricingPriceListRuleDraft,
) -> Result<PricingPriceListOption, ServerFnError> {
    use rustok_api::Permission;
    use rustok_pricing::PricingService;

    ensure_permission(
        &auth.permissions,
        &[Permission::PRODUCTS_UPDATE],
        "products:update required",
    )?;

    let price_list_id = Uuid::parse_str(price_list_id.trim())
        .map_err(|_| ServerFnError::new("Invalid price_list_id"))?;
    validate_active_price_list_for_rule_update(app_ctx.db(), tenant.id, price_list_id).await?;
    let adjustment_percent =
        parse_optional_decimal(&payload.adjustment_percent, "adjustment_percent")?;
    let service = PricingService::new(
        app_ctx.db_clone(),
        transactional_event_bus_from_runtime(app_ctx)?,
    )
    .with_product_catalog_read_runtime(product_read_runtime(app_ctx)?);
    service
        .set_price_list_percentage_rule(tenant.id, auth.user_id, price_list_id, adjustment_percent)
        .await
        .map_err(ServerFnError::new)?;

    let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
        .await
        .ok();
    let requested_locale = resolve_requested_locale(
        None,
        request_context
            .as_ref()
            .map(|context| context.locale.as_str()),
        tenant.default_locale.as_str(),
    );
    let option = service
        .list_active_price_lists(
            tenant.id,
            Some(requested_locale.as_str()),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(ServerFnError::new)?
        .into_iter()
        .find(|item| item.id == price_list_id)
        .ok_or_else(|| ServerFnError::new("price_list_id must reference an active price list"))?;

    Ok(map_native_price_list_option(option))
}

#[cfg(feature = "ssr")]
pub(crate) async fn validate_active_price_list_for_rule_update(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    price_list_id: Uuid,
) -> Result<(), ServerFnError> {
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

    let backend = db.get_database_backend();
    let (sql, values) = match backend {
        DatabaseBackend::Postgres => (
            "SELECT CASE
                WHEN lower(status) != 'active' THEN 'inactive'
                WHEN starts_at IS NOT NULL AND starts_at > CURRENT_TIMESTAMP THEN 'future'
                WHEN ends_at IS NOT NULL AND ends_at < CURRENT_TIMESTAMP THEN 'expired'
                ELSE 'active'
             END AS lifecycle
             FROM price_lists
             WHERE id = $1 AND tenant_id = $2",
            vec![price_list_id.into(), tenant_id.into()],
        ),
        DatabaseBackend::MySql | DatabaseBackend::Sqlite => (
            "SELECT CASE
                WHEN lower(status) != 'active' THEN 'inactive'
                WHEN starts_at IS NOT NULL AND starts_at > CURRENT_TIMESTAMP THEN 'future'
                WHEN ends_at IS NOT NULL AND ends_at < CURRENT_TIMESTAMP THEN 'expired'
                ELSE 'active'
             END AS lifecycle
             FROM price_lists
             WHERE id = ? AND tenant_id = ?",
            vec![price_list_id.into(), tenant_id.into()],
        ),
        _ => {
            return Err(ServerFnError::new(
                "Unsupported database backend for pricing price-list validation",
            ));
        }
    };

    let row = db
        .query_one_raw(Statement::from_sql_and_values(backend, sql, values))
        .await
        .map_err(ServerFnError::new)?
        .ok_or_else(|| ServerFnError::new("price_list_id was not found"))?;

    let lifecycle: String = row
        .try_get("", "lifecycle")
        .map_err(|err| ServerFnError::new(err.to_string()))?;

    match lifecycle.as_str() {
        "active" => Ok(()),
        "inactive" => Err(ServerFnError::new(
            "price_list_id must reference an active price list",
        )),
        "future" => Err(ServerFnError::new("price_list_id is not active yet")),
        "expired" => Err(ServerFnError::new("price_list_id is already expired")),
        _ => Err(ServerFnError::new(
            "price_list_id must reference an active price list",
        )),
    }
}

#[cfg(feature = "ssr")]
pub(crate) async fn update_price_list_scope_native_with_context(
    app_ctx: &HostRuntimeContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    price_list_id: String,
    payload: PricingPriceListScopeDraft,
) -> Result<PricingPriceListOption, ServerFnError> {
    use rustok_api::Permission;
    use rustok_pricing::PricingService;

    ensure_permission(
        &auth.permissions,
        &[Permission::PRODUCTS_UPDATE],
        "products:update required",
    )?;

    let price_list_id = Uuid::parse_str(price_list_id.trim())
        .map_err(|_| ServerFnError::new("Invalid price_list_id"))?;
    let channel_id = parse_optional_uuid(&payload.channel_id, "channel_id")?;
    let channel_slug = sanitize_channel_slug(text_or_none(payload.channel_slug));
    let service = PricingService::new(
        app_ctx.db_clone(),
        transactional_event_bus_from_runtime(app_ctx)?,
    )
    .with_product_catalog_read_runtime(product_read_runtime(app_ctx)?);

    service
        .set_price_list_scope(
            tenant.id,
            auth.user_id,
            price_list_id,
            channel_id,
            channel_slug,
        )
        .await
        .map(map_native_price_list_option)
        .map_err(ServerFnError::new)
}
