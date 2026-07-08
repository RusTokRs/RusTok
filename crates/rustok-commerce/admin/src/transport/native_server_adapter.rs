use leptos::prelude::*;
#[cfg(feature = "ssr")]
use rustok_ui_core::normalize_ui_text as optional_text;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{
    CommerceAdminCartSnapshot, CommerceCartPromotionDraft, CommerceCartPromotionPreview,
    CommerceOrderChange, CommerceOrderChangeActionDraft, CommerceOrderChangeList,
};
#[cfg(feature = "ssr")]
use crate::model::{CommerceCartPromotionKind, CommerceCartPromotionScope};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    Graphql(String),
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphql(error) => write!(f, "{error}"),
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

pub async fn fetch_order_changes(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    tenant_id: String,
    order_id: Option<String>,
    status: Option<String>,
) -> Result<CommerceOrderChangeList, ApiError> {
    commerce_admin_order_changes_native(tenant_id, order_id, status)
        .await
        .map_err(Into::into)
}

pub async fn apply_order_change(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    draft: CommerceOrderChangeActionDraft,
) -> Result<CommerceOrderChange, ApiError> {
    commerce_admin_apply_order_change_native(tenant_id, id, draft)
        .await
        .map_err(Into::into)
}

pub async fn cancel_order_change(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    draft: CommerceOrderChangeActionDraft,
) -> Result<CommerceOrderChange, ApiError> {
    commerce_admin_cancel_order_change_native(tenant_id, id, draft)
        .await
        .map_err(Into::into)
}

#[allow(dead_code)]
pub async fn preview_cart_promotion(
    cart_id: String,
    payload: CommerceCartPromotionDraft,
) -> Result<CommerceCartPromotionPreview, ApiError> {
    commerce_admin_preview_cart_promotion_native(cart_id, payload)
        .await
        .map_err(Into::into)
}

#[allow(dead_code)]
pub async fn apply_cart_promotion(
    cart_id: String,
    payload: CommerceCartPromotionDraft,
) -> Result<CommerceAdminCartSnapshot, ApiError> {
    commerce_admin_apply_cart_promotion_native(cart_id, payload)
        .await
        .map_err(Into::into)
}

#[cfg(feature = "ssr")]
fn ensure_permission(
    permissions: &[rustok_api::Permission],
    required: &[rustok_api::Permission],
    message: &str,
) -> Result<(), ServerFnError> {
    if required
        .iter()
        .any(|permission| permissions.iter().any(|value| value == permission))
    {
        Ok(())
    } else {
        Err(ServerFnError::new(format!("Permission denied: {message}")))
    }
}

#[cfg(feature = "ssr")]
fn parse_cart_id(value: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim()).map_err(|_| ServerFnError::new("Invalid cart_id"))
}

#[cfg(feature = "ssr")]
fn parse_optional_line_item_id(
    value: &str,
    scope: &CommerceCartPromotionScope,
) -> Result<Option<uuid::Uuid>, ServerFnError> {
    let trimmed = value.trim();
    match scope {
        CommerceCartPromotionScope::Cart | CommerceCartPromotionScope::Shipping => {
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Err(ServerFnError::new(
                    "line_item_id is allowed only for line_item scope",
                ))
            }
        }
        CommerceCartPromotionScope::LineItem => {
            if trimmed.is_empty() {
                return Err(ServerFnError::new(
                    "line_item_id is required for line_item scope",
                ));
            }
            uuid::Uuid::parse_str(trimmed)
                .map(Some)
                .map_err(|_| ServerFnError::new("Invalid line_item_id"))
        }
    }
}

#[cfg(feature = "ssr")]
fn parse_decimal(value: &str, field_name: &str) -> Result<rust_decimal::Decimal, ServerFnError> {
    value
        .trim()
        .parse::<rust_decimal::Decimal>()
        .map_err(|_| ServerFnError::new(format!("Invalid {field_name}")))
}

#[cfg(feature = "ssr")]
fn parse_required_decimal(
    value: &str,
    field_name: &str,
) -> Result<rust_decimal::Decimal, ServerFnError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ServerFnError::new(format!(
            "{field_name} is required for the selected promotion kind"
        )));
    }
    parse_decimal(trimmed, field_name)
}

#[cfg(feature = "ssr")]
fn ensure_unused_decimal(value: &str, field_name: &str) -> Result<(), ServerFnError> {
    if value.trim().is_empty() {
        Ok(())
    } else {
        Err(ServerFnError::new(format!(
            "{field_name} must be omitted for the selected promotion kind"
        )))
    }
}

#[cfg(feature = "ssr")]
fn parse_metadata_json(value: &str) -> Result<serde_json::Value, ServerFnError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Ok(serde_json::Value::Object(Default::default()))
    } else {
        serde_json::from_str(trimmed)
            .map_err(|_| ServerFnError::new("Invalid JSON metadata payload"))
    }
}

#[cfg(feature = "ssr")]
fn normalize_source_id(value: &str) -> Result<String, ServerFnError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(ServerFnError::new("source_id is required"))
    } else {
        Ok(trimmed.to_string())
    }
}

#[cfg(feature = "ssr")]
async fn preview_cart_promotion_native_with_context(
    app_ctx: &loco_rs::app::AppContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    cart_id: String,
    payload: CommerceCartPromotionDraft,
) -> Result<CommerceCartPromotionPreview, ServerFnError> {
    use rustok_api::Permission;
    use rustok_cart::CartService;

    ensure_permission(
        &auth.permissions,
        &[Permission::ORDERS_READ],
        "orders:read required",
    )?;

    let cart_id = parse_cart_id(&cart_id)?;
    let line_item_id = parse_optional_line_item_id(&payload.line_item_id, &payload.scope)?;
    let source_id = normalize_source_id(&payload.source_id)?;
    let service = CartService::new(app_ctx.db.clone());

    let preview = match payload.kind {
        CommerceCartPromotionKind::PercentageDiscount => {
            let discount_percent =
                parse_required_decimal(&payload.discount_percent, "discount_percent")?;
            ensure_unused_decimal(&payload.amount, "amount")?;
            match payload.scope {
                CommerceCartPromotionScope::Shipping => {
                    service
                        .preview_percentage_shipping_promotion(
                            tenant.id,
                            cart_id,
                            source_id.as_str(),
                            discount_percent,
                        )
                        .await
                }
                CommerceCartPromotionScope::Cart | CommerceCartPromotionScope::LineItem => {
                    service
                        .preview_percentage_promotion(
                            tenant.id,
                            cart_id,
                            line_item_id,
                            source_id.as_str(),
                            discount_percent,
                        )
                        .await
                }
            }
        }
        CommerceCartPromotionKind::FixedDiscount => {
            let amount = parse_required_decimal(&payload.amount, "amount")?;
            ensure_unused_decimal(&payload.discount_percent, "discount_percent")?;
            match payload.scope {
                CommerceCartPromotionScope::Shipping => {
                    service
                        .preview_fixed_shipping_promotion(
                            tenant.id,
                            cart_id,
                            source_id.as_str(),
                            amount,
                        )
                        .await
                }
                CommerceCartPromotionScope::Cart | CommerceCartPromotionScope::LineItem => {
                    service
                        .preview_fixed_promotion(
                            tenant.id,
                            cart_id,
                            line_item_id,
                            source_id.as_str(),
                            amount,
                        )
                        .await
                }
            }
        }
    }
    .map_err(ServerFnError::new)?;

    Ok(map_cart_promotion_preview(payload.scope, preview))
}

#[cfg(feature = "ssr")]
async fn apply_cart_promotion_native_with_context(
    app_ctx: &loco_rs::app::AppContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    cart_id: String,
    payload: CommerceCartPromotionDraft,
) -> Result<CommerceAdminCartSnapshot, ServerFnError> {
    use rustok_api::Permission;
    use rustok_cart::CartService;

    ensure_permission(
        &auth.permissions,
        &[Permission::ORDERS_UPDATE],
        "orders:update required",
    )?;

    let cart_id = parse_cart_id(&cart_id)?;
    let line_item_id = parse_optional_line_item_id(&payload.line_item_id, &payload.scope)?;
    let source_id = normalize_source_id(&payload.source_id)?;
    let metadata = parse_metadata_json(&payload.metadata_json)?;
    let service = CartService::new(app_ctx.db.clone());

    let cart = match payload.kind {
        CommerceCartPromotionKind::PercentageDiscount => {
            let discount_percent =
                parse_required_decimal(&payload.discount_percent, "discount_percent")?;
            ensure_unused_decimal(&payload.amount, "amount")?;
            match payload.scope {
                CommerceCartPromotionScope::Shipping => {
                    service
                        .apply_percentage_shipping_promotion(
                            tenant.id,
                            cart_id,
                            source_id.as_str(),
                            discount_percent,
                            metadata,
                        )
                        .await
                }
                CommerceCartPromotionScope::Cart | CommerceCartPromotionScope::LineItem => {
                    service
                        .apply_percentage_promotion(
                            tenant.id,
                            cart_id,
                            line_item_id,
                            source_id.as_str(),
                            discount_percent,
                            metadata,
                        )
                        .await
                }
            }
        }
        CommerceCartPromotionKind::FixedDiscount => {
            let amount = parse_required_decimal(&payload.amount, "amount")?;
            ensure_unused_decimal(&payload.discount_percent, "discount_percent")?;
            match payload.scope {
                CommerceCartPromotionScope::Shipping => {
                    service
                        .apply_fixed_shipping_promotion(
                            tenant.id,
                            cart_id,
                            source_id.as_str(),
                            amount,
                            metadata,
                        )
                        .await
                }
                CommerceCartPromotionScope::Cart | CommerceCartPromotionScope::LineItem => {
                    service
                        .apply_fixed_promotion(
                            tenant.id,
                            cart_id,
                            line_item_id,
                            source_id.as_str(),
                            amount,
                            metadata,
                        )
                        .await
                }
            }
        }
    }
    .map_err(ServerFnError::new)?;

    Ok(map_cart_snapshot(cart))
}

#[cfg(feature = "ssr")]
fn parse_uuid(value: &str, field: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim())
        .map_err(|_| ServerFnError::new(format!("{field} must be a valid UUID")))
}

#[cfg(feature = "ssr")]
fn parse_optional_uuid(
    value: Option<String>,
    field: &str,
) -> Result<Option<uuid::Uuid>, ServerFnError> {
    value
        .and_then(|value| optional_text(value.as_str()))
        .map(|value| parse_uuid(value.as_str(), field))
        .transpose()
}

#[cfg(feature = "ssr")]
fn order_service_from_context(app_ctx: &loco_rs::app::AppContext) -> rustok_order::OrderService {
    rustok_order::OrderService::new(
        app_ctx.db.clone(),
        rustok_outbox::loco::transactional_event_bus_from_context(app_ctx),
    )
}

#[cfg(feature = "ssr")]
fn map_order_change(change: rustok_order::dto::OrderChangeResponse) -> CommerceOrderChange {
    CommerceOrderChange {
        id: change.id.to_string(),
        tenant_id: change.tenant_id.to_string(),
        order_id: change.order_id.to_string(),
        created_by: change.created_by.to_string(),
        change_type: change.change_type,
        status: change.status,
        description: change.description,
        preview: change.preview.to_string(),
        metadata: change.metadata.to_string(),
        created_at: change.created_at.to_rfc3339(),
        updated_at: change.updated_at.to_rfc3339(),
        applied_at: change.applied_at.map(|value| value.to_rfc3339()),
        cancelled_at: change.cancelled_at.map(|value| value.to_rfc3339()),
    }
}

#[cfg(feature = "ssr")]
async fn fetch_order_changes_native_with_context(
    app_ctx: &loco_rs::app::AppContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    tenant_id: String,
    order_id: Option<String>,
    status: Option<String>,
) -> Result<CommerceOrderChangeList, ServerFnError> {
    use rustok_api::Permission;

    ensure_permission(
        &auth.permissions,
        &[Permission::ORDERS_READ],
        "orders:read required",
    )?;
    let requested_tenant_id = parse_uuid(tenant_id.as_str(), "tenant_id")?;
    if requested_tenant_id != tenant.id {
        return Err(ServerFnError::new(
            "tenant_id must match the effective tenant context",
        ));
    }

    let (items, total) = order_service_from_context(app_ctx)
        .list_order_changes(
            tenant.id,
            rustok_order::dto::ListOrderChangesInput {
                page: 1,
                per_page: 20,
                order_id: parse_optional_uuid(order_id, "order_id")?,
                status: status.and_then(|value| optional_text(value.as_str())),
                change_type: None,
            },
        )
        .await
        .map_err(ServerFnError::new)?;

    Ok(CommerceOrderChangeList {
        items: items.into_iter().map(map_order_change).collect(),
        total,
        page: 1,
        per_page: 20,
        has_next: total > 20,
    })
}

#[cfg(feature = "ssr")]
async fn apply_order_change_native_with_context(
    app_ctx: &loco_rs::app::AppContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    tenant_id: String,
    id: String,
    draft: CommerceOrderChangeActionDraft,
) -> Result<CommerceOrderChange, ServerFnError> {
    use rustok_api::Permission;

    ensure_permission(
        &auth.permissions,
        &[Permission::ORDERS_UPDATE],
        "orders:update required",
    )?;
    let requested_tenant_id = parse_uuid(tenant_id.as_str(), "tenant_id")?;
    if requested_tenant_id != tenant.id {
        return Err(ServerFnError::new(
            "tenant_id must match the effective tenant context",
        ));
    }

    let change = order_service_from_context(app_ctx)
        .apply_order_change(
            tenant.id,
            parse_uuid(id.as_str(), "order_change_id")?,
            rustok_order::dto::ApplyOrderChangeInput {
                metadata: parse_metadata_json(&draft.metadata_json)?,
            },
        )
        .await
        .map_err(ServerFnError::new)?;

    Ok(map_order_change(change))
}

#[cfg(feature = "ssr")]
async fn cancel_order_change_native_with_context(
    app_ctx: &loco_rs::app::AppContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    tenant_id: String,
    id: String,
    draft: CommerceOrderChangeActionDraft,
) -> Result<CommerceOrderChange, ServerFnError> {
    use rustok_api::Permission;

    ensure_permission(
        &auth.permissions,
        &[Permission::ORDERS_UPDATE],
        "orders:update required",
    )?;
    let requested_tenant_id = parse_uuid(tenant_id.as_str(), "tenant_id")?;
    if requested_tenant_id != tenant.id {
        return Err(ServerFnError::new(
            "tenant_id must match the effective tenant context",
        ));
    }

    let change = order_service_from_context(app_ctx)
        .cancel_order_change(
            tenant.id,
            parse_uuid(id.as_str(), "order_change_id")?,
            rustok_order::dto::CancelOrderChangeInput {
                reason: optional_text(draft.reason.as_str()),
                metadata: parse_metadata_json(&draft.metadata_json)?,
            },
        )
        .await
        .map_err(ServerFnError::new)?;

    Ok(map_order_change(change))
}

#[cfg(feature = "ssr")]
fn map_cart_promotion_preview(
    scope: CommerceCartPromotionScope,
    preview: rustok_cart::services::cart::CartPromotionPreview,
) -> CommerceCartPromotionPreview {
    CommerceCartPromotionPreview {
        kind: match preview.kind {
            rustok_cart::services::cart::CartPromotionKind::PercentageDiscount => {
                CommerceCartPromotionKind::PercentageDiscount
            }
            rustok_cart::services::cart::CartPromotionKind::FixedDiscount => {
                CommerceCartPromotionKind::FixedDiscount
            }
        },
        scope,
        line_item_id: preview.line_item_id.map(|value| value.to_string()),
        currency_code: preview.currency_code,
        base_amount: preview.base_amount.normalize().to_string(),
        adjustment_amount: preview.adjustment_amount.normalize().to_string(),
        adjusted_amount: preview.adjusted_amount.normalize().to_string(),
    }
}

#[cfg(feature = "ssr")]
fn map_cart_snapshot(cart: rustok_cart::dto::CartResponse) -> CommerceAdminCartSnapshot {
    CommerceAdminCartSnapshot {
        id: cart.id.to_string(),
        currency_code: cart.currency_code,
        shipping_total: cart.shipping_total.normalize().to_string(),
        adjustment_total: cart.adjustment_total.normalize().to_string(),
        total_amount: cart.total_amount.normalize().to_string(),
        adjustments: cart
            .adjustments
            .into_iter()
            .map(|adjustment| crate::model::CommerceAdminCartAdjustment {
                id: adjustment.id.to_string(),
                line_item_id: adjustment.line_item_id.map(|value| value.to_string()),
                source_type: adjustment.source_type,
                source_id: adjustment.source_id,
                scope: adjustment
                    .metadata
                    .get("scope")
                    .and_then(|value| value.as_str())
                    .map(ToString::to_string),
                amount: adjustment.amount.normalize().to_string(),
                currency_code: adjustment.currency_code,
                metadata: adjustment.metadata.to_string(),
            })
            .collect(),
    }
}

#[server(prefix = "/api/fn", endpoint = "commerce/admin/order-changes")]
async fn commerce_admin_order_changes_native(
    tenant_id: String,
    order_id: Option<String>,
    status: Option<String>,
) -> Result<CommerceOrderChangeList, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        fetch_order_changes_native_with_context(
            &app_ctx, &auth, &tenant, tenant_id, order_id, status,
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (tenant_id, order_id, status);
        Err(ServerFnError::new(
            "commerce/admin/order-changes requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "commerce/admin/apply-order-change")]
async fn commerce_admin_apply_order_change_native(
    tenant_id: String,
    id: String,
    draft: CommerceOrderChangeActionDraft,
) -> Result<CommerceOrderChange, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        apply_order_change_native_with_context(&app_ctx, &auth, &tenant, tenant_id, id, draft).await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (tenant_id, id, draft);
        Err(ServerFnError::new(
            "commerce/admin/apply-order-change requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "commerce/admin/cancel-order-change")]
async fn commerce_admin_cancel_order_change_native(
    tenant_id: String,
    id: String,
    draft: CommerceOrderChangeActionDraft,
) -> Result<CommerceOrderChange, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        cancel_order_change_native_with_context(&app_ctx, &auth, &tenant, tenant_id, id, draft)
            .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (tenant_id, id, draft);
        Err(ServerFnError::new(
            "commerce/admin/cancel-order-change requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "commerce/admin/preview-cart-promotion")]
async fn commerce_admin_preview_cart_promotion_native(
    cart_id: String,
    payload: CommerceCartPromotionDraft,
) -> Result<CommerceCartPromotionPreview, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        preview_cart_promotion_native_with_context(&app_ctx, &auth, &tenant, cart_id, payload).await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (cart_id, payload);
        Err(ServerFnError::new(
            "commerce/admin/preview-cart-promotion requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "commerce/admin/apply-cart-promotion")]
async fn commerce_admin_apply_cart_promotion_native(
    cart_id: String,
    payload: CommerceCartPromotionDraft,
) -> Result<CommerceAdminCartSnapshot, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        apply_cart_promotion_native_with_context(&app_ctx, &auth, &tenant, cart_id, payload).await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (cart_id, payload);
        Err(ServerFnError::new(
            "commerce/admin/apply-cart-promotion requires the `ssr` feature",
        ))
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;
    use loco_rs::app::{AppContext, SharedStore};
    use loco_rs::cache;
    use loco_rs::environment::Environment;
    use loco_rs::storage::{self, Storage};
    use loco_rs::tests_cfg::config::test_config;
    use rustok_api::Permission;
    use rustok_api::{AuthContext, TenantContext};
    use rustok_cart::dto::{AddCartLineItemInput, CreateCartInput};
    use rustok_cart::CartService;
    use rustok_core::events::EventTransport;
    use rustok_fulfillment::dto::CreateShippingOptionInput;
    use rustok_fulfillment::FulfillmentService;
    use rustok_order::dto::{CreateOrderChangeInput, CreateOrderInput, CreateOrderLineItemInput};
    use rustok_test_utils::db::setup_test_db;
    use rustok_test_utils::{mock_transactional_event_bus, MockEventTransport};
    use serde_json::json;
    use std::sync::Arc;

    mod support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../rustok-commerce/tests/support.rs"
        ));
    }

    fn test_app_context(db: sea_orm::DatabaseConnection) -> AppContext {
        let shared_store = Arc::new(SharedStore::default());
        let event_transport: Arc<dyn EventTransport> = Arc::new(MockEventTransport::new());
        shared_store.insert(event_transport);

        AppContext {
            environment: Environment::Test,
            db,
            queue_provider: None,
            config: test_config(),
            mailer: None,
            storage: Storage::single(storage::drivers::mem::new()).into(),
            cache: Arc::new(cache::Cache::new(cache::drivers::null::new())),
            shared_store,
        }
    }

    fn test_tenant() -> TenantContext {
        TenantContext {
            id: uuid::Uuid::new_v4(),
            slug: "acme".to_string(),
            name: "Acme".to_string(),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        }
    }

    fn auth_with_permissions(permissions: Vec<Permission>) -> AuthContext {
        AuthContext {
            user_id: uuid::Uuid::new_v4(),
            session_id: uuid::Uuid::new_v4(),
            tenant_id: uuid::Uuid::new_v4(),
            permissions,
            client_id: None,
            scopes: vec![],
            grant_type: "password".to_string(),
        }
    }

    async fn seed_tenant_context(db: &sea_orm::DatabaseConnection, tenant: &TenantContext) {
        use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO tenants (id, name, slug, domain, settings, default_locale, is_active)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            vec![
                tenant.id.to_string().into(),
                tenant.name.clone().into(),
                tenant.slug.clone().into(),
                tenant.domain.clone().into(),
                tenant.settings.to_string().into(),
                tenant.default_locale.clone().into(),
                i32::from(tenant.is_active).into(),
            ],
        ))
        .await
        .expect("insert tenant");

        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO tenant_locales (id, tenant_id, locale, name, native_name, is_default, is_enabled, fallback_locale)
             VALUES (?, ?, ?, ?, ?, 1, 1, NULL)",
            vec![
                uuid::Uuid::new_v4().to_string().into(),
                tenant.id.to_string().into(),
                tenant.default_locale.clone().into(),
                "English".to_string().into(),
                "English".to_string().into(),
            ],
        ))
        .await
        .expect("insert tenant locale");
    }

    async fn create_shipping_profile_for_cart(
        db: &sea_orm::DatabaseConnection,
        tenant_id: uuid::Uuid,
    ) {
        use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

        rustok_commerce::entities::shipping_profile::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            slug: Set("default".to_string()),
            active: Set(true),
            metadata: Set(json!({})),
            created_at: Set(chrono::Utc::now().into()),
            updated_at: Set(chrono::Utc::now().into()),
        }
        .insert(db)
        .await
        .expect("insert shipping profile");

        rustok_commerce::entities::shipping_profile_translation::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            shipping_profile_id: Set(rustok_commerce::entities::shipping_profile::Entity::find()
                .filter(rustok_commerce::entities::shipping_profile::Column::TenantId.eq(tenant_id))
                .one(db)
                .await
                .expect("load shipping profile")
                .expect("shipping profile exists")
                .id),
            locale: Set("en".to_string()),
            name: Set("Default".to_string()),
            description: Set(Some("Default shipping profile".to_string())),
        }
        .insert(db)
        .await
        .expect("insert shipping profile translation");
    }

    async fn create_shipping_option_for_cart(
        db: &sea_orm::DatabaseConnection,
        tenant_id: uuid::Uuid,
    ) -> uuid::Uuid {
        let service = FulfillmentService::new(db.clone());
        let option = service
            .create_shipping_option(
                tenant_id,
                CreateShippingOptionInput {
                    provider_id: Some("manual".to_string()),
                    amount: rust_decimal::Decimal::new(999, 2),
                    currency_code: "EUR".to_string(),
                    allowed_shipping_profile_slugs: Some(vec!["default".to_string()]),
                    translations: vec![rustok_fulfillment::dto::ShippingOptionTranslationInput {
                        locale: "en".to_string(),
                        name: "Standard shipping".to_string(),
                    }],
                    metadata: json!({}),
                },
            )
            .await
            .expect("create shipping option");
        option.id
    }

    async fn seed_order_change(
        db: &sea_orm::DatabaseConnection,
        tenant: &TenantContext,
        actor_id: uuid::Uuid,
        change_type: &str,
    ) -> (uuid::Uuid, uuid::Uuid) {
        support::ensure_commerce_schema(db).await;
        let order_service =
            rustok_order::OrderService::new(db.clone(), mock_transactional_event_bus());
        let order = order_service
            .create_order(
                tenant.id,
                actor_id,
                CreateOrderInput {
                    customer_id: Some(uuid::Uuid::new_v4()),
                    currency_code: "usd".to_string(),
                    shipping_total: rust_decimal::Decimal::ZERO,
                    line_items: vec![CreateOrderLineItemInput {
                        product_id: None,
                        variant_id: None,
                        shipping_profile_slug: "default".to_string(),
                        seller_id: None,
                        sku: Some("POST-ORDER-1".to_string()),
                        title: "Post-order operator item".to_string(),
                        quantity: 1,
                        unit_price: rust_decimal::Decimal::new(1250, 2),
                        metadata: json!({}),
                    }],
                    adjustments: Vec::new(),
                    tax_lines: Vec::new(),
                    metadata: json!({"source":"commerce-admin-native-order-change-test"}),
                },
            )
            .await
            .expect("create order");
        let change = order_service
            .create_order_change(
                tenant.id,
                actor_id,
                order.id,
                CreateOrderChangeInput {
                    change_type: change_type.to_string(),
                    description: Some("Exchange generated from return decision".to_string()),
                    preview: json!({"replacement_sku":"POST-ORDER-2"}),
                    metadata: json!({"order_return_id": uuid::Uuid::new_v4().to_string()}),
                },
            )
            .await
            .expect("create order change");
        (order.id, change.id)
    }

    async fn seed_cart_with_shipping(
        db: &sea_orm::DatabaseConnection,
        tenant: &TenantContext,
    ) -> (uuid::Uuid, uuid::Uuid) {
        support::ensure_commerce_schema(db).await;
        seed_tenant_context(db, tenant).await;
        create_shipping_profile_for_cart(db, tenant.id).await;

        let cart_service = CartService::new(db.clone());
        let shipping_option_id = create_shipping_option_for_cart(db, tenant.id).await;
        let cart = cart_service
            .create_cart(
                tenant.id,
                CreateCartInput {
                    customer_id: None,
                    email: Some("cart@example.com".to_string()),
                    region_id: None,
                    country_code: Some("DE".to_string()),
                    locale_code: Some("en".to_string()),
                    selected_shipping_option_id: Some(shipping_option_id),
                    currency_code: "EUR".to_string(),
                    metadata: json!({}),
                },
            )
            .await
            .expect("create cart");
        let cart = cart_service
            .add_line_item(
                tenant.id,
                cart.id,
                AddCartLineItemInput {
                    product_id: None,
                    variant_id: None,
                    shipping_profile_slug: Some("default".to_string()),
                    sku: Some("SKU-1".to_string()),
                    title: "Operator line".to_string(),
                    quantity: 1,
                    unit_price: rust_decimal::Decimal::new(1999, 2),
                    metadata: json!({}),
                },
            )
            .await
            .expect("add line item");
        (cart.id, cart.line_items[0].id)
    }

    #[tokio::test]
    async fn fetch_order_changes_native_with_context_filters_pending_changes() {
        let db = setup_test_db().await;
        let app = test_app_context(db.clone());
        let tenant = test_tenant();
        let actor_id = uuid::Uuid::new_v4();
        let auth = auth_with_permissions(vec![Permission::ORDERS_READ]);
        let (order_id, change_id) = seed_order_change(&db, &tenant, actor_id, "exchange").await;

        let list = fetch_order_changes_native_with_context(
            &app,
            &auth,
            &tenant,
            tenant.id.to_string(),
            Some(order_id.to_string()),
            Some("pending".to_string()),
        )
        .await
        .expect("fetch order changes");

        assert_eq!(list.total, 1);
        assert_eq!(list.page, 1);
        assert_eq!(list.per_page, 20);
        assert!(!list.has_next);
        let change = list.items.first().expect("one change");
        assert_eq!(change.id, change_id.to_string());
        assert_eq!(change.order_id, order_id.to_string());
        assert_eq!(change.change_type, "exchange");
        assert_eq!(change.status, "pending");
        assert!(change.preview.contains("replacement_sku"));
        assert!(change.metadata.contains("order_return_id"));
    }

    #[tokio::test]
    async fn apply_order_change_native_with_context_uses_order_service_lifecycle() {
        let db = setup_test_db().await;
        let app = test_app_context(db.clone());
        let tenant = test_tenant();
        let actor_id = uuid::Uuid::new_v4();
        let auth = auth_with_permissions(vec![Permission::ORDERS_UPDATE]);
        let (_, change_id) = seed_order_change(&db, &tenant, actor_id, "claim").await;

        let change = apply_order_change_native_with_context(
            &app,
            &auth,
            &tenant,
            tenant.id.to_string(),
            change_id.to_string(),
            CommerceOrderChangeActionDraft {
                metadata_json: "{\"operator\":\"returns-desk\"}".to_string(),
                reason: String::new(),
            },
        )
        .await
        .expect("apply order change");

        assert_eq!(change.id, change_id.to_string());
        assert_eq!(change.status, "applied");
        assert!(change.applied_at.is_some());
        assert!(change.cancelled_at.is_none());
        assert!(change.metadata.contains("returns-desk"));
    }

    #[tokio::test]
    async fn cancel_order_change_native_with_context_records_reason_patch() {
        let db = setup_test_db().await;
        let app = test_app_context(db.clone());
        let tenant = test_tenant();
        let actor_id = uuid::Uuid::new_v4();
        let auth = auth_with_permissions(vec![Permission::ORDERS_UPDATE]);
        let (_, change_id) = seed_order_change(&db, &tenant, actor_id, "exchange").await;

        let change = cancel_order_change_native_with_context(
            &app,
            &auth,
            &tenant,
            tenant.id.to_string(),
            change_id.to_string(),
            CommerceOrderChangeActionDraft {
                metadata_json: "{\"operator\":\"returns-desk\"}".to_string(),
                reason: "customer withdrew exchange".to_string(),
            },
        )
        .await
        .expect("cancel order change");

        assert_eq!(change.status, "cancelled");
        assert!(change.cancelled_at.is_some());
        assert!(change.metadata.contains("returns-desk"));
        assert!(change.metadata.contains("customer withdrew exchange"));
    }

    #[tokio::test]
    async fn fetch_order_changes_native_with_context_enforces_orders_read_permission() {
        let db = setup_test_db().await;
        let app = test_app_context(db);
        let tenant = test_tenant();
        let auth = auth_with_permissions(Vec::new());

        let error = fetch_order_changes_native_with_context(
            &app,
            &auth,
            &tenant,
            tenant.id.to_string(),
            None,
            None,
        )
        .await
        .expect_err("orders:read must be required");

        assert!(
            error
                .to_string()
                .contains("Permission denied: orders:read required"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn preview_cart_promotion_native_with_context_supports_shipping_scope() {
        let db = setup_test_db().await;
        let app = test_app_context(db.clone());
        let tenant = test_tenant();
        let auth = auth_with_permissions(vec![Permission::ORDERS_READ]);
        let (cart_id, _) = seed_cart_with_shipping(&db, &tenant).await;

        let preview = preview_cart_promotion_native_with_context(
            &app,
            &auth,
            &tenant,
            cart_id.to_string(),
            CommerceCartPromotionDraft {
                kind: CommerceCartPromotionKind::PercentageDiscount,
                scope: CommerceCartPromotionScope::Shipping,
                line_item_id: String::new(),
                source_id: "promo-shipping-native".to_string(),
                discount_percent: "50".to_string(),
                amount: String::new(),
                metadata_json: String::new(),
            },
        )
        .await
        .expect("preview shipping promotion");

        assert_eq!(preview.kind, CommerceCartPromotionKind::PercentageDiscount);
        assert_eq!(preview.scope, CommerceCartPromotionScope::Shipping);
        assert_eq!(preview.line_item_id, None);
        assert_eq!(preview.currency_code, "EUR");
        assert_eq!(preview.base_amount, "9.99");
        assert_eq!(preview.adjustment_amount, "4.99");
        assert_eq!(preview.adjusted_amount, "5");
    }

    #[tokio::test]
    async fn apply_cart_promotion_native_with_context_snapshots_shipping_adjustment() {
        let db = setup_test_db().await;
        let app = test_app_context(db.clone());
        let tenant = test_tenant();
        let auth = auth_with_permissions(vec![Permission::ORDERS_UPDATE]);
        let (cart_id, _) = seed_cart_with_shipping(&db, &tenant).await;

        let cart = apply_cart_promotion_native_with_context(
            &app,
            &auth,
            &tenant,
            cart_id.to_string(),
            CommerceCartPromotionDraft {
                kind: CommerceCartPromotionKind::FixedDiscount,
                scope: CommerceCartPromotionScope::Shipping,
                line_item_id: String::new(),
                source_id: "promo-shipping-native".to_string(),
                discount_percent: String::new(),
                amount: "4.99".to_string(),
                metadata_json: "{\"campaign\":\"native-operator\"}".to_string(),
            },
        )
        .await
        .expect("apply shipping promotion");

        assert_eq!(cart.shipping_total, "9.99");
        assert_eq!(cart.adjustment_total, "4.99");
        assert_eq!(cart.total_amount, "24.99");
        assert_eq!(cart.adjustments.len(), 1);
        let adjustment = &cart.adjustments[0];
        assert_eq!(adjustment.source_type, "promotion");
        assert_eq!(
            adjustment.source_id.as_deref(),
            Some("promo-shipping-native")
        );
        assert_eq!(adjustment.scope.as_deref(), Some("shipping"));
        assert_eq!(adjustment.amount, "4.99");
        assert_eq!(adjustment.currency_code, "EUR");
        assert!(adjustment
            .metadata
            .contains("\"campaign\":\"native-operator\""));
        assert!(adjustment.metadata.contains("\"scope\":\"shipping\""));
    }

    #[tokio::test]
    async fn preview_cart_promotion_native_with_context_rejects_missing_line_item_target() {
        let db = setup_test_db().await;
        let app = test_app_context(db);
        let tenant = test_tenant();
        let auth = auth_with_permissions(vec![Permission::ORDERS_READ]);

        let error = preview_cart_promotion_native_with_context(
            &app,
            &auth,
            &tenant,
            uuid::Uuid::new_v4().to_string(),
            CommerceCartPromotionDraft {
                kind: CommerceCartPromotionKind::FixedDiscount,
                scope: CommerceCartPromotionScope::LineItem,
                line_item_id: String::new(),
                source_id: "promo-line-item".to_string(),
                discount_percent: String::new(),
                amount: "3.00".to_string(),
                metadata_json: String::new(),
            },
        )
        .await
        .expect_err("line item scope must require line_item_id");

        assert!(
            error
                .to_string()
                .contains("line_item_id is required for line_item scope"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn apply_cart_promotion_native_with_context_enforces_orders_update_permission() {
        let db = setup_test_db().await;
        let app = test_app_context(db.clone());
        let tenant = test_tenant();
        let auth = auth_with_permissions(vec![Permission::ORDERS_READ]);
        let (cart_id, _) = seed_cart_with_shipping(&db, &tenant).await;

        let error = apply_cart_promotion_native_with_context(
            &app,
            &auth,
            &tenant,
            cart_id.to_string(),
            CommerceCartPromotionDraft {
                kind: CommerceCartPromotionKind::FixedDiscount,
                scope: CommerceCartPromotionScope::Shipping,
                line_item_id: String::new(),
                source_id: "promo-shipping-native".to_string(),
                discount_percent: String::new(),
                amount: "4.99".to_string(),
                metadata_json: String::new(),
            },
        )
        .await
        .expect_err("orders:update must be required");

        assert!(
            error
                .to_string()
                .contains("Permission denied: orders:update required"),
            "unexpected error: {error}"
        );
    }
}
