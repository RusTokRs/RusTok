use leptos::prelude::*;
use rustok_api::HostRuntimeContext;
use rustok_ui_core::normalize_ui_text as optional_text;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{
    CommerceAdminCartSnapshot, CommerceCartPromotionDraft, CommerceCartPromotionPreview,
    CommerceOrderChange, CommerceOrderChangeActionDraft, CommerceOrderChangeList,
};
use crate::model::{CommerceCartPromotionKind, CommerceCartPromotionScope};

const COMMERCE_ADMIN_PROMOTION_CONSUMER: &str = "rustok_commerce.admin_promotion_transport";
const COMMERCE_ADMIN_PROMOTION_BOUNDARY: &str = "commerce_admin_promotion_native_transport";
const PREVIEW_CART_PROMOTION_OPERATION: &str = "preview_cart_promotion";
const APPLY_CART_PROMOTION_OPERATION: &str = "apply_cart_promotion";

const COMMERCE_ADMIN_ORDER_CHANGE_CONSUMER: &str = "rustok_commerce.admin_order_change_transport";
const COMMERCE_ADMIN_ORDER_CHANGE_BOUNDARY: &str = "commerce_admin_order_change_native_transport";
const LIST_ORDER_CHANGES_OPERATION: &str = "list_order_changes";
const APPLY_ORDER_CHANGE_OPERATION: &str = "apply_order_change";
const CANCEL_ORDER_CHANGE_OPERATION: &str = "cancel_order_change";

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

fn transport_correlation_id(scope: &str, operation: &'static str) -> String {
    format!("{scope}:{operation}:{}", uuid::Uuid::new_v4())
}

#[allow(dead_code)]
fn request_context_fields(
    request_context: Option<&rustok_api::RequestContext>,
) -> (
    Option<uuid::Uuid>,
    Option<uuid::Uuid>,
    Option<uuid::Uuid>,
    Option<&str>,
    Option<&str>,
) {
    (
        request_context.map(|context| context.tenant_id),
        request_context.and_then(|context| context.user_id),
        request_context.and_then(|context| context.channel_id),
        request_context.and_then(|context| context.channel_slug.as_deref()),
        request_context.map(|context| context.locale.as_str()),
    )
}

fn parse_metadata_json(value: &str) -> Result<serde_json::Value, ServerFnError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Ok(serde_json::Value::Object(Default::default()))
    } else {
        serde_json::from_str(trimmed)
            .map_err(|_| ServerFnError::new("Invalid JSON metadata payload"))
    }
}

fn parse_uuid(value: &str, field: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim())
        .map_err(|_| ServerFnError::new(format!("{field} must be a valid UUID")))
}

fn parse_optional_uuid(
    value: Option<String>,
    field: &str,
) -> Result<Option<uuid::Uuid>, ServerFnError> {
    value
        .and_then(|value| optional_text(value.as_str()))
        .map(|value| parse_uuid(value.as_str(), field))
        .transpose()
}

// -----------------------------------------------------------------------------
// Cart-promotion native boundary
// -----------------------------------------------------------------------------

struct PromotionRequestContextFacts {
    request_context_present: bool,
    request_tenant_id_non_nil: Option<bool>,
    request_user_id_present: bool,
    request_user_id_non_nil: Option<bool>,
    channel_id_present: bool,
    channel_id_non_nil: Option<bool>,
    channel_slug_present: bool,
    channel_slug_length: Option<usize>,
    locale_present: bool,
    locale_length: Option<usize>,
}

fn promotion_request_context_facts(
    request_context: Option<&rustok_api::RequestContext>,
) -> PromotionRequestContextFacts {
    PromotionRequestContextFacts {
        request_context_present: request_context.is_some(),
        request_tenant_id_non_nil: request_context.map(|context| !context.tenant_id.is_nil()),
        request_user_id_present: request_context
            .and_then(|context| context.user_id)
            .is_some(),
        request_user_id_non_nil: request_context
            .and_then(|context| context.user_id)
            .map(|value| !value.is_nil()),
        channel_id_present: request_context
            .and_then(|context| context.channel_id)
            .is_some(),
        channel_id_non_nil: request_context
            .and_then(|context| context.channel_id)
            .map(|value| !value.is_nil()),
        channel_slug_present: request_context
            .and_then(|context| context.channel_slug.as_ref())
            .is_some(),
        channel_slug_length: request_context
            .and_then(|context| context.channel_slug.as_ref())
            .map(|value| value.chars().count()),
        locale_present: request_context.is_some(),
        locale_length: request_context.map(|context| context.locale.chars().count()),
    }
}

fn parse_cart_id(value: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim()).map_err(|_| ServerFnError::new("Invalid cart_id"))
}

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

fn parse_decimal(value: &str, field_name: &str) -> Result<rust_decimal::Decimal, ServerFnError> {
    value
        .trim()
        .parse::<rust_decimal::Decimal>()
        .map_err(|_| ServerFnError::new(format!("Invalid {field_name}")))
}

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

fn ensure_unused_decimal(value: &str, field_name: &str) -> Result<(), ServerFnError> {
    if value.trim().is_empty() {
        Ok(())
    } else {
        Err(ServerFnError::new(format!(
            "{field_name} must be omitted for the selected promotion kind"
        )))
    }
}

fn normalize_source_id(value: &str) -> Result<String, ServerFnError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(ServerFnError::new("source_id is required"))
    } else {
        Ok(trimmed.to_string())
    }
}

fn promotion_correlation_id(operation: &'static str) -> String {
    transport_correlation_id("commerce-admin-cart-promotion", operation)
}

fn promotion_context_error<E>(
    _error: E,
    operation: &'static str,
    context_kind: &'static str,
    correlation_id: &str,
    code: &'static str,
    public_message: &'static str,
) -> ServerFnError {
    let error_type = std::any::type_name::<E>();
    tracing::error!(
        error_type,
        consumer = COMMERCE_ADMIN_PROMOTION_CONSUMER,
        operation,
        context_kind,
        correlation_id,
        code,
        boundary = COMMERCE_ADMIN_PROMOTION_BOUNDARY,
        "commerce admin promotion request context extraction failed"
    );
    ServerFnError::new(public_message)
}

fn promotion_auth_context_error<E>(
    error: E,
    operation: &'static str,
    correlation_id: &str,
) -> ServerFnError {
    promotion_context_error(
        error,
        operation,
        "auth",
        correlation_id,
        "commerce.admin_promotion_auth_context_unavailable",
        "Commerce admin authentication context is temporarily unavailable",
    )
}

fn promotion_tenant_context_error<E>(
    error: E,
    operation: &'static str,
    correlation_id: &str,
) -> ServerFnError {
    promotion_context_error(
        error,
        operation,
        "tenant",
        correlation_id,
        "commerce.admin_promotion_tenant_context_unavailable",
        "Commerce admin tenant context is temporarily unavailable",
    )
}

async fn optional_promotion_request_context(
    operation: &'static str,
    correlation_id: &str,
) -> Option<rustok_api::RequestContext> {
    match leptos_axum::extract::<rustok_api::RequestContext>().await {
        Ok(context) => Some(context),
        Err(error) => {
            let error_type = std::any::type_name_of_val(&error);
            tracing::warn!(
                error_type,
                consumer = COMMERCE_ADMIN_PROMOTION_CONSUMER,
                operation,
                context_kind = "request",
                correlation_id,
                code = "commerce.admin_promotion_optional_request_context_unavailable",
                boundary = COMMERCE_ADMIN_PROMOTION_BOUNDARY,
                "commerce admin promotion optional request context extraction failed"
            );
            None
        }
    }
}

fn cart_promotion_port_context(
    tenant: &rustok_api::TenantContext,
    auth: &rustok_api::AuthContext,
    request_context: Option<&rustok_api::RequestContext>,
    correlation_id: &str,
    is_write: bool,
) -> rustok_api::PortContext {
    let locale = request_context
        .map(|context| context.locale.as_str())
        .unwrap_or(tenant.default_locale.as_str());
    let channel = request_context.and_then(|context| {
        context
            .channel_slug
            .clone()
            .or_else(|| context.channel_id.map(|value| value.to_string()))
    });

    let mut context = rustok_api::PortContext::new(
        tenant.id.to_string(),
        rustok_api::PortActor::user(auth.user_id.to_string()),
        locale,
        correlation_id.to_string(),
    )
    .with_deadline(std::time::Duration::from_secs(2));

    if let Some(channel) = channel {
        context = context.with_channel(channel);
    }
    if is_write {
        context.with_idempotency_key(correlation_id.to_string())
    } else {
        context
    }
}

fn promotion_port_error(
    error: rustok_api::PortError,
    consumer_operation: &'static str,
    owner_operation: &'static str,
    correlation_id: &str,
    tenant: &rustok_api::TenantContext,
    auth: &rustok_api::AuthContext,
    request_context: Option<&rustok_api::RequestContext>,
    cart_id: uuid::Uuid,
) -> ServerFnError {
    let request_facts = promotion_request_context_facts(request_context);
    let public_message_present = !error.message.trim().is_empty();
    let public_message_length = error.message.chars().count();
    let effective_locale_length = request_context
        .map(|context| context.locale.as_str())
        .unwrap_or(tenant.default_locale.as_str())
        .chars()
        .count();
    let effective_channel_present = request_context
        .and_then(|context| {
            context
                .channel_slug
                .as_ref()
                .map(|_| ())
                .or_else(|| context.channel_id.map(|_| ()))
        })
        .is_some();

    match &error.kind {
        rustok_api::PortErrorKind::Unavailable
        | rustok_api::PortErrorKind::Timeout
        | rustok_api::PortErrorKind::InvariantViolation => {
            tracing::error!(
                owner = "rustok_cart.promotion",
                consumer = COMMERCE_ADMIN_PROMOTION_CONSUMER,
                consumer_operation,
                owner_operation,
                correlation_id,
                tenant_id_non_nil = !tenant.id.is_nil(),
                actor_id_non_nil = !auth.user_id.is_nil(),
                cart_id_non_nil = !cart_id.is_nil(),
                request_context_present = request_facts.request_context_present,
                request_tenant_id_non_nil = ?request_facts.request_tenant_id_non_nil,
                request_user_id_present = request_facts.request_user_id_present,
                request_user_id_non_nil = ?request_facts.request_user_id_non_nil,
                channel_id_present = request_facts.channel_id_present,
                channel_id_non_nil = ?request_facts.channel_id_non_nil,
                channel_slug_present = request_facts.channel_slug_present,
                channel_slug_length = ?request_facts.channel_slug_length,
                locale_present = request_facts.locale_present,
                locale_length = ?request_facts.locale_length,
                effective_locale_length,
                effective_channel_present,
                public_code = %error.code,
                public_message_present,
                public_message_length,
                error_kind = ?error.kind,
                retryable = error.retryable,
                boundary = COMMERCE_ADMIN_PROMOTION_BOUNDARY,
                "commerce admin promotion owner call failed"
            );
        }
        _ => {
            tracing::warn!(
                owner = "rustok_cart.promotion",
                consumer = COMMERCE_ADMIN_PROMOTION_CONSUMER,
                consumer_operation,
                owner_operation,
                correlation_id,
                tenant_id_non_nil = !tenant.id.is_nil(),
                actor_id_non_nil = !auth.user_id.is_nil(),
                cart_id_non_nil = !cart_id.is_nil(),
                request_context_present = request_facts.request_context_present,
                request_tenant_id_non_nil = ?request_facts.request_tenant_id_non_nil,
                request_user_id_present = request_facts.request_user_id_present,
                request_user_id_non_nil = ?request_facts.request_user_id_non_nil,
                channel_id_present = request_facts.channel_id_present,
                channel_id_non_nil = ?request_facts.channel_id_non_nil,
                channel_slug_present = request_facts.channel_slug_present,
                channel_slug_length = ?request_facts.channel_slug_length,
                locale_present = request_facts.locale_present,
                locale_length = ?request_facts.locale_length,
                effective_locale_length,
                effective_channel_present,
                public_code = %error.code,
                public_message_present,
                public_message_length,
                error_kind = ?error.kind,
                retryable = error.retryable,
                boundary = COMMERCE_ADMIN_PROMOTION_BOUNDARY,
                "commerce admin promotion owner call was rejected"
            );
        }
    }

    ServerFnError::new(error.message)
}

fn cart_promotion_request(
    cart_id: uuid::Uuid,
    payload: &CommerceCartPromotionDraft,
    line_item_id: Option<uuid::Uuid>,
    metadata: serde_json::Value,
) -> Result<rustok_cart::CartPromotionRequest, ServerFnError> {
    let source_id = normalize_source_id(&payload.source_id)?;
    let (kind, amount) = match &payload.kind {
        CommerceCartPromotionKind::PercentageDiscount => {
            ensure_unused_decimal(&payload.amount, "amount")?;
            (
                rustok_cart::CartPromotionKindRequest::PercentageDiscount,
                parse_required_decimal(&payload.discount_percent, "discount_percent")?,
            )
        }
        CommerceCartPromotionKind::FixedDiscount => {
            ensure_unused_decimal(&payload.discount_percent, "discount_percent")?;
            (
                rustok_cart::CartPromotionKindRequest::FixedDiscount,
                parse_required_decimal(&payload.amount, "amount")?,
            )
        }
    };
    let scope = match &payload.scope {
        CommerceCartPromotionScope::Cart => rustok_cart::CartPromotionScopeRequest::Cart,
        CommerceCartPromotionScope::LineItem => rustok_cart::CartPromotionScopeRequest::LineItem,
        CommerceCartPromotionScope::Shipping => rustok_cart::CartPromotionScopeRequest::Shipping,
    };
    Ok(rustok_cart::CartPromotionRequest {
        cart_id,
        line_item_id,
        scope,
        kind,
        source_id,
        amount,
        metadata,
    })
}

async fn preview_cart_promotion_native_with_context(
    app_ctx: &HostRuntimeContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    request_context: Option<&rustok_api::RequestContext>,
    correlation_id: &str,
    cart_id: String,
    payload: CommerceCartPromotionDraft,
) -> Result<CommerceCartPromotionPreview, ServerFnError> {
    use rustok_api::Permission;
    use rustok_cart::in_process_cart_promotion_port;

    ensure_permission(
        &auth.permissions,
        &[Permission::ORDERS_READ],
        "orders:read required",
    )?;

    let cart_id = parse_cart_id(&cart_id)?;
    let line_item_id = parse_optional_line_item_id(&payload.line_item_id, &payload.scope)?;
    let request = cart_promotion_request(cart_id, &payload, line_item_id, serde_json::Value::Null)?;
    let preview = in_process_cart_promotion_port(app_ctx.db_clone())
        .read_cart_promotion_preview(
            cart_promotion_port_context(tenant, auth, request_context, correlation_id, false),
            request,
        )
        .await
        .map_err(|error| {
            promotion_port_error(
                error,
                PREVIEW_CART_PROMOTION_OPERATION,
                "read_cart_promotion_preview",
                correlation_id,
                tenant,
                auth,
                request_context,
                cart_id,
            )
        })?;

    Ok(map_cart_promotion_preview(payload.scope, preview))
}

async fn apply_cart_promotion_native_with_context(
    app_ctx: &HostRuntimeContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    request_context: Option<&rustok_api::RequestContext>,
    correlation_id: &str,
    cart_id: String,
    payload: CommerceCartPromotionDraft,
) -> Result<CommerceAdminCartSnapshot, ServerFnError> {
    use rustok_api::Permission;
    use rustok_cart::in_process_cart_promotion_port;

    ensure_permission(
        &auth.permissions,
        &[Permission::ORDERS_UPDATE],
        "orders:update required",
    )?;

    let cart_id = parse_cart_id(&cart_id)?;
    let line_item_id = parse_optional_line_item_id(&payload.line_item_id, &payload.scope)?;
    let metadata = parse_metadata_json(&payload.metadata_json)?;
    let request = cart_promotion_request(cart_id, &payload, line_item_id, metadata)?;
    let cart = in_process_cart_promotion_port(app_ctx.db_clone())
        .apply_cart_promotion(
            cart_promotion_port_context(tenant, auth, request_context, correlation_id, true),
            request,
        )
        .await
        .map_err(|error| {
            promotion_port_error(
                error,
                APPLY_CART_PROMOTION_OPERATION,
                "apply_cart_promotion",
                correlation_id,
                tenant,
                auth,
                request_context,
                cart_id,
            )
        })?;

    Ok(map_cart_snapshot(cart))
}

// -----------------------------------------------------------------------------
// Order-change native boundary
// -----------------------------------------------------------------------------

struct OrderChangeRequestContextFacts {
    request_context_present: bool,
    request_tenant_id_non_nil: Option<bool>,
    request_user_id_present: bool,
    request_user_id_non_nil: Option<bool>,
    channel_id_present: bool,
    channel_id_non_nil: Option<bool>,
    channel_slug_present: bool,
    channel_slug_length: Option<usize>,
    locale_present: bool,
    locale_length: Option<usize>,
}

fn order_change_request_context_facts(
    request_context: Option<&rustok_api::RequestContext>,
) -> OrderChangeRequestContextFacts {
    OrderChangeRequestContextFacts {
        request_context_present: request_context.is_some(),
        request_tenant_id_non_nil: request_context.map(|context| !context.tenant_id.is_nil()),
        request_user_id_present: request_context
            .and_then(|context| context.user_id)
            .is_some(),
        request_user_id_non_nil: request_context
            .and_then(|context| context.user_id)
            .map(|value| !value.is_nil()),
        channel_id_present: request_context
            .and_then(|context| context.channel_id)
            .is_some(),
        channel_id_non_nil: request_context
            .and_then(|context| context.channel_id)
            .map(|value| !value.is_nil()),
        channel_slug_present: request_context
            .and_then(|context| context.channel_slug.as_ref())
            .is_some(),
        channel_slug_length: request_context
            .and_then(|context| context.channel_slug.as_ref())
            .map(|value| value.chars().count()),
        locale_present: request_context.is_some(),
        locale_length: request_context.map(|context| context.locale.chars().count()),
    }
}

struct OrderChangeOwnerErrorFacts {
    validation_detail_present: bool,
    validation_detail_length: Option<usize>,
    resource_id_present: bool,
    resource_id_non_nil: Option<bool>,
    transition_from_length: Option<usize>,
    transition_to_length: Option<usize>,
    database_cause_present: bool,
    core_cause_present: bool,
}

fn order_change_owner_error_facts(
    error: &rustok_order::error::OrderError,
) -> OrderChangeOwnerErrorFacts {
    match error {
        rustok_order::error::OrderError::Validation(detail) => OrderChangeOwnerErrorFacts {
            validation_detail_present: !detail.trim().is_empty(),
            validation_detail_length: Some(detail.chars().count()),
            resource_id_present: false,
            resource_id_non_nil: None,
            transition_from_length: None,
            transition_to_length: None,
            database_cause_present: false,
            core_cause_present: false,
        },
        rustok_order::error::OrderError::OrderNotFound(id)
        | rustok_order::error::OrderError::OrderReturnNotFound(id)
        | rustok_order::error::OrderError::OrderChangeNotFound(id) => OrderChangeOwnerErrorFacts {
            validation_detail_present: false,
            validation_detail_length: None,
            resource_id_present: true,
            resource_id_non_nil: Some(!id.is_nil()),
            transition_from_length: None,
            transition_to_length: None,
            database_cause_present: false,
            core_cause_present: false,
        },
        rustok_order::error::OrderError::InvalidTransition { from, to } => {
            OrderChangeOwnerErrorFacts {
                validation_detail_present: false,
                validation_detail_length: None,
                resource_id_present: false,
                resource_id_non_nil: None,
                transition_from_length: Some(from.chars().count()),
                transition_to_length: Some(to.chars().count()),
                database_cause_present: false,
                core_cause_present: false,
            }
        }
        rustok_order::error::OrderError::Database(_) => OrderChangeOwnerErrorFacts {
            validation_detail_present: false,
            validation_detail_length: None,
            resource_id_present: false,
            resource_id_non_nil: None,
            transition_from_length: None,
            transition_to_length: None,
            database_cause_present: true,
            core_cause_present: false,
        },
        rustok_order::error::OrderError::Core(_) => OrderChangeOwnerErrorFacts {
            validation_detail_present: false,
            validation_detail_length: None,
            resource_id_present: false,
            resource_id_non_nil: None,
            transition_from_length: None,
            transition_to_length: None,
            database_cause_present: false,
            core_cause_present: true,
        },
    }
}

fn order_change_correlation_id(operation: &'static str) -> String {
    transport_correlation_id("commerce-admin-order-change", operation)
}

fn order_change_context_error<E>(
    _error: E,
    operation: &'static str,
    context_kind: &'static str,
    correlation_id: &str,
    code: &'static str,
    public_message: &'static str,
) -> ServerFnError {
    let error_type = std::any::type_name::<E>();
    tracing::error!(
        error_type,
        consumer = COMMERCE_ADMIN_ORDER_CHANGE_CONSUMER,
        operation,
        context_kind,
        correlation_id,
        code,
        boundary = COMMERCE_ADMIN_ORDER_CHANGE_BOUNDARY,
        "commerce admin order-change request context extraction failed"
    );
    ServerFnError::new(public_message)
}

fn order_change_auth_context_error<E>(
    error: E,
    operation: &'static str,
    correlation_id: &str,
) -> ServerFnError {
    order_change_context_error(
        error,
        operation,
        "auth",
        correlation_id,
        "commerce.admin_order_change_auth_context_unavailable",
        "Commerce admin authentication context is temporarily unavailable",
    )
}

fn order_change_tenant_context_error<E>(
    error: E,
    operation: &'static str,
    correlation_id: &str,
) -> ServerFnError {
    order_change_context_error(
        error,
        operation,
        "tenant",
        correlation_id,
        "commerce.admin_order_change_tenant_context_unavailable",
        "Commerce admin tenant context is temporarily unavailable",
    )
}

async fn optional_order_change_request_context(
    operation: &'static str,
    correlation_id: &str,
) -> Option<rustok_api::RequestContext> {
    match leptos_axum::extract::<rustok_api::RequestContext>().await {
        Ok(context) => Some(context),
        Err(error) => {
            let error_type = std::any::type_name_of_val(&error);
            tracing::warn!(
                error_type,
                consumer = COMMERCE_ADMIN_ORDER_CHANGE_CONSUMER,
                operation,
                context_kind = "request",
                correlation_id,
                code = "commerce.admin_order_change_optional_request_context_unavailable",
                boundary = COMMERCE_ADMIN_ORDER_CHANGE_BOUNDARY,
                "commerce admin order-change optional request context extraction failed"
            );
            None
        }
    }
}

fn order_service_from_context(
    runtime_ctx: &HostRuntimeContext,
    operation: &'static str,
    correlation_id: &str,
    tenant: &rustok_api::TenantContext,
    auth: &rustok_api::AuthContext,
    request_context: Option<&rustok_api::RequestContext>,
) -> Result<rustok_order::OrderService, ServerFnError> {
    let event_bus = runtime_ctx
        .shared_get::<rustok_outbox::TransactionalEventBus>()
        .ok_or_else(|| {
            let request_facts = order_change_request_context_facts(request_context);
            tracing::error!(
                owner = "rustok_order",
                consumer = COMMERCE_ADMIN_ORDER_CHANGE_CONSUMER,
                operation,
                correlation_id,
                tenant_id_non_nil = !tenant.id.is_nil(),
                actor_id_non_nil = !auth.user_id.is_nil(),
                request_context_present = request_facts.request_context_present,
                request_tenant_id_non_nil = ?request_facts.request_tenant_id_non_nil,
                request_user_id_present = request_facts.request_user_id_present,
                request_user_id_non_nil = ?request_facts.request_user_id_non_nil,
                channel_id_present = request_facts.channel_id_present,
                channel_id_non_nil = ?request_facts.channel_id_non_nil,
                channel_slug_present = request_facts.channel_slug_present,
                channel_slug_length = ?request_facts.channel_slug_length,
                locale_present = request_facts.locale_present,
                locale_length = ?request_facts.locale_length,
                code = "commerce.admin_order_change_runtime_unavailable",
                boundary = COMMERCE_ADMIN_ORDER_CHANGE_BOUNDARY,
                "commerce admin order-change runtime composition is unavailable"
            );
            ServerFnError::new("Commerce order-change runtime is temporarily unavailable")
        })?;
    Ok(rustok_order::OrderService::new(
        runtime_ctx.db_clone(),
        event_bus,
    ))
}

struct OrderChangeOwnerErrorContext<'a> {
    operation: &'static str,
    correlation_id: &'a str,
    tenant: &'a rustok_api::TenantContext,
    auth: &'a rustok_api::AuthContext,
    request_context: Option<&'a rustok_api::RequestContext>,
    order_id: Option<uuid::Uuid>,
    order_change_id: Option<uuid::Uuid>,
}

fn order_change_owner_error(
    mut context: OrderChangeOwnerErrorContext<'_>,
    error: rustok_order::error::OrderError,
) -> ServerFnError {
    match &error {
        rustok_order::error::OrderError::OrderNotFound(id) => context.order_id = Some(*id),
        rustok_order::error::OrderError::OrderChangeNotFound(id) => {
            context.order_change_id = Some(*id)
        }
        _ => {}
    }

    let (public_code, public_message, error_kind, severe) = match &error {
        rustok_order::error::OrderError::Validation(_) => (
            "commerce.admin_order_change_invalid",
            "Order change request is invalid",
            "validation",
            false,
        ),
        rustok_order::error::OrderError::OrderNotFound(_)
        | rustok_order::error::OrderError::OrderReturnNotFound(_)
        | rustok_order::error::OrderError::OrderChangeNotFound(_) => (
            "commerce.admin_order_change_not_found",
            "Order resource was not found",
            "not_found",
            false,
        ),
        rustok_order::error::OrderError::InvalidTransition { .. } => (
            "commerce.admin_order_change_state_conflict",
            "Order change conflicts with the current order state",
            "state_conflict",
            false,
        ),
        rustok_order::error::OrderError::Database(_) => (
            "commerce.admin_order_change_storage_unavailable",
            "Order storage is temporarily unavailable",
            "database",
            true,
        ),
        rustok_order::error::OrderError::Core(_) => (
            "commerce.admin_order_change_failed",
            "Order change could not be completed safely",
            "core",
            true,
        ),
    };

    let request_facts = order_change_request_context_facts(context.request_context);
    let error_facts = order_change_owner_error_facts(&error);
    let order_id_present = context.order_id.is_some();
    let order_id_non_nil = context.order_id.map(|value| !value.is_nil());
    let order_change_id_present = context.order_change_id.is_some();
    let order_change_id_non_nil = context.order_change_id.map(|value| !value.is_nil());

    if severe {
        tracing::error!(
            owner = "rustok_order",
            consumer = COMMERCE_ADMIN_ORDER_CHANGE_CONSUMER,
            operation = context.operation,
            correlation_id = context.correlation_id,
            tenant_id_non_nil = !context.tenant.id.is_nil(),
            actor_id_non_nil = !context.auth.user_id.is_nil(),
            order_id_present,
            order_id_non_nil = ?order_id_non_nil,
            order_change_id_present,
            order_change_id_non_nil = ?order_change_id_non_nil,
            request_context_present = request_facts.request_context_present,
            request_tenant_id_non_nil = ?request_facts.request_tenant_id_non_nil,
            request_user_id_present = request_facts.request_user_id_present,
            request_user_id_non_nil = ?request_facts.request_user_id_non_nil,
            channel_id_present = request_facts.channel_id_present,
            channel_id_non_nil = ?request_facts.channel_id_non_nil,
            channel_slug_present = request_facts.channel_slug_present,
            channel_slug_length = ?request_facts.channel_slug_length,
            locale_present = request_facts.locale_present,
            locale_length = ?request_facts.locale_length,
            validation_detail_present = error_facts.validation_detail_present,
            validation_detail_length = ?error_facts.validation_detail_length,
            resource_id_present = error_facts.resource_id_present,
            resource_id_non_nil = ?error_facts.resource_id_non_nil,
            transition_from_length = ?error_facts.transition_from_length,
            transition_to_length = ?error_facts.transition_to_length,
            database_cause_present = error_facts.database_cause_present,
            core_cause_present = error_facts.core_cause_present,
            error_kind,
            public_code,
            boundary = COMMERCE_ADMIN_ORDER_CHANGE_BOUNDARY,
            "commerce admin order-change owner operation failed"
        );
    } else {
        tracing::warn!(
            owner = "rustok_order",
            consumer = COMMERCE_ADMIN_ORDER_CHANGE_CONSUMER,
            operation = context.operation,
            correlation_id = context.correlation_id,
            tenant_id_non_nil = !context.tenant.id.is_nil(),
            actor_id_non_nil = !context.auth.user_id.is_nil(),
            order_id_present,
            order_id_non_nil = ?order_id_non_nil,
            order_change_id_present,
            order_change_id_non_nil = ?order_change_id_non_nil,
            request_context_present = request_facts.request_context_present,
            request_tenant_id_non_nil = ?request_facts.request_tenant_id_non_nil,
            request_user_id_present = request_facts.request_user_id_present,
            request_user_id_non_nil = ?request_facts.request_user_id_non_nil,
            channel_id_present = request_facts.channel_id_present,
            channel_id_non_nil = ?request_facts.channel_id_non_nil,
            channel_slug_present = request_facts.channel_slug_present,
            channel_slug_length = ?request_facts.channel_slug_length,
            locale_present = request_facts.locale_present,
            locale_length = ?request_facts.locale_length,
            validation_detail_present = error_facts.validation_detail_present,
            validation_detail_length = ?error_facts.validation_detail_length,
            resource_id_present = error_facts.resource_id_present,
            resource_id_non_nil = ?error_facts.resource_id_non_nil,
            transition_from_length = ?error_facts.transition_from_length,
            transition_to_length = ?error_facts.transition_to_length,
            database_cause_present = error_facts.database_cause_present,
            core_cause_present = error_facts.core_cause_present,
            error_kind,
            public_code,
            boundary = COMMERCE_ADMIN_ORDER_CHANGE_BOUNDARY,
            "commerce admin order-change owner operation was rejected"
        );
    }

    ServerFnError::new(public_message)
}

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

async fn fetch_order_changes_native_with_context(
    app_ctx: &HostRuntimeContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    request_context: Option<&rustok_api::RequestContext>,
    correlation_id: &str,
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
    let order_id = parse_optional_uuid(order_id, "order_id")?;

    let (items, total) = order_service_from_context(
        app_ctx,
        LIST_ORDER_CHANGES_OPERATION,
        correlation_id,
        tenant,
        auth,
        request_context,
    )?
    .list_order_changes(
        tenant.id,
        rustok_order::dto::ListOrderChangesInput {
            page: 1,
            per_page: 20,
            order_id,
            status: status.and_then(|value| optional_text(value.as_str())),
            change_type: None,
        },
    )
    .await
    .map_err(|error| {
        order_change_owner_error(
            OrderChangeOwnerErrorContext {
                operation: LIST_ORDER_CHANGES_OPERATION,
                correlation_id,
                tenant,
                auth,
                request_context,
                order_id,
                order_change_id: None,
            },
            error,
        )
    })?;

    Ok(CommerceOrderChangeList {
        items: items.into_iter().map(map_order_change).collect(),
        total,
        page: 1,
        per_page: 20,
        has_next: total > 20,
    })
}

async fn apply_order_change_native_with_context(
    app_ctx: &HostRuntimeContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    request_context: Option<&rustok_api::RequestContext>,
    correlation_id: &str,
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
    let order_change_id = parse_uuid(id.as_str(), "order_change_id")?;
    let metadata = parse_metadata_json(&draft.metadata_json)?;

    let change = order_service_from_context(
        app_ctx,
        APPLY_ORDER_CHANGE_OPERATION,
        correlation_id,
        tenant,
        auth,
        request_context,
    )?
    .apply_order_change(
        tenant.id,
        order_change_id,
        rustok_order::dto::ApplyOrderChangeInput { metadata },
    )
    .await
    .map_err(|error| {
        order_change_owner_error(
            OrderChangeOwnerErrorContext {
                operation: APPLY_ORDER_CHANGE_OPERATION,
                correlation_id,
                tenant,
                auth,
                request_context,
                order_id: None,
                order_change_id: Some(order_change_id),
            },
            error,
        )
    })?;

    Ok(map_order_change(change))
}

async fn cancel_order_change_native_with_context(
    app_ctx: &HostRuntimeContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    request_context: Option<&rustok_api::RequestContext>,
    correlation_id: &str,
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
    let order_change_id = parse_uuid(id.as_str(), "order_change_id")?;
    let metadata = parse_metadata_json(&draft.metadata_json)?;

    let change = order_service_from_context(
        app_ctx,
        CANCEL_ORDER_CHANGE_OPERATION,
        correlation_id,
        tenant,
        auth,
        request_context,
    )?
    .cancel_order_change(
        tenant.id,
        order_change_id,
        rustok_order::dto::CancelOrderChangeInput {
            reason: optional_text(draft.reason.as_str()),
            metadata,
        },
    )
    .await
    .map_err(|error| {
        order_change_owner_error(
            OrderChangeOwnerErrorContext {
                operation: CANCEL_ORDER_CHANGE_OPERATION,
                correlation_id,
                tenant,
                auth,
                request_context,
                order_id: None,
                order_change_id: Some(order_change_id),
            },
            error,
        )
    })?;

    Ok(map_order_change(change))
}

// -----------------------------------------------------------------------------
// Shared response mapping
// -----------------------------------------------------------------------------

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

// -----------------------------------------------------------------------------
// Mounted native server functions
// -----------------------------------------------------------------------------

#[server(prefix = "/api/fn", endpoint = "commerce/admin/order-changes")]
async fn commerce_admin_order_changes_native(
    tenant_id: String,
    order_id: Option<String>,
    status: Option<String>,
) -> Result<CommerceOrderChangeList, ServerFnError> {
    use leptos::prelude::expect_context;
    use rustok_api::{AuthContext, TenantContext};

    let operation = LIST_ORDER_CHANGES_OPERATION;
    let correlation_id = order_change_correlation_id(operation);
    let app_ctx = expect_context::<HostRuntimeContext>();
    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(|error| order_change_auth_context_error(error, operation, &correlation_id))?;
    let tenant = leptos_axum::extract::<TenantContext>()
        .await
        .map_err(|error| order_change_tenant_context_error(error, operation, &correlation_id))?;
    let request_context = optional_order_change_request_context(operation, &correlation_id).await;

    fetch_order_changes_native_with_context(
        &app_ctx,
        &auth,
        &tenant,
        request_context.as_ref(),
        &correlation_id,
        tenant_id,
        order_id,
        status,
    )
    .await
}

#[server(prefix = "/api/fn", endpoint = "commerce/admin/apply-order-change")]
async fn commerce_admin_apply_order_change_native(
    tenant_id: String,
    id: String,
    draft: CommerceOrderChangeActionDraft,
) -> Result<CommerceOrderChange, ServerFnError> {
    use leptos::prelude::expect_context;
    use rustok_api::{AuthContext, TenantContext};

    let operation = APPLY_ORDER_CHANGE_OPERATION;
    let correlation_id = order_change_correlation_id(operation);
    let app_ctx = expect_context::<HostRuntimeContext>();
    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(|error| order_change_auth_context_error(error, operation, &correlation_id))?;
    let tenant = leptos_axum::extract::<TenantContext>()
        .await
        .map_err(|error| order_change_tenant_context_error(error, operation, &correlation_id))?;
    let request_context = optional_order_change_request_context(operation, &correlation_id).await;

    apply_order_change_native_with_context(
        &app_ctx,
        &auth,
        &tenant,
        request_context.as_ref(),
        &correlation_id,
        tenant_id,
        id,
        draft,
    )
    .await
}

#[server(prefix = "/api/fn", endpoint = "commerce/admin/cancel-order-change")]
async fn commerce_admin_cancel_order_change_native(
    tenant_id: String,
    id: String,
    draft: CommerceOrderChangeActionDraft,
) -> Result<CommerceOrderChange, ServerFnError> {
    use leptos::prelude::expect_context;
    use rustok_api::{AuthContext, TenantContext};

    let operation = CANCEL_ORDER_CHANGE_OPERATION;
    let correlation_id = order_change_correlation_id(operation);
    let app_ctx = expect_context::<HostRuntimeContext>();
    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(|error| order_change_auth_context_error(error, operation, &correlation_id))?;
    let tenant = leptos_axum::extract::<TenantContext>()
        .await
        .map_err(|error| order_change_tenant_context_error(error, operation, &correlation_id))?;
    let request_context = optional_order_change_request_context(operation, &correlation_id).await;

    cancel_order_change_native_with_context(
        &app_ctx,
        &auth,
        &tenant,
        request_context.as_ref(),
        &correlation_id,
        tenant_id,
        id,
        draft,
    )
    .await
}

#[server(prefix = "/api/fn", endpoint = "commerce/admin/preview-cart-promotion")]
async fn commerce_admin_preview_cart_promotion_native(
    cart_id: String,
    payload: CommerceCartPromotionDraft,
) -> Result<CommerceCartPromotionPreview, ServerFnError> {
    use leptos::prelude::expect_context;
    use rustok_api::{AuthContext, TenantContext};

    let operation = PREVIEW_CART_PROMOTION_OPERATION;
    let correlation_id = promotion_correlation_id(operation);
    let app_ctx = expect_context::<HostRuntimeContext>();
    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(|error| promotion_auth_context_error(error, operation, &correlation_id))?;
    let tenant = leptos_axum::extract::<TenantContext>()
        .await
        .map_err(|error| promotion_tenant_context_error(error, operation, &correlation_id))?;
    let request_context = optional_promotion_request_context(operation, &correlation_id).await;

    preview_cart_promotion_native_with_context(
        &app_ctx,
        &auth,
        &tenant,
        request_context.as_ref(),
        &correlation_id,
        cart_id,
        payload,
    )
    .await
}

#[server(prefix = "/api/fn", endpoint = "commerce/admin/apply-cart-promotion")]
async fn commerce_admin_apply_cart_promotion_native(
    cart_id: String,
    payload: CommerceCartPromotionDraft,
) -> Result<CommerceAdminCartSnapshot, ServerFnError> {
    use leptos::prelude::expect_context;
    use rustok_api::{AuthContext, TenantContext};

    let operation = APPLY_CART_PROMOTION_OPERATION;
    let correlation_id = promotion_correlation_id(operation);
    let app_ctx = expect_context::<HostRuntimeContext>();
    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(|error| promotion_auth_context_error(error, operation, &correlation_id))?;
    let tenant = leptos_axum::extract::<TenantContext>()
        .await
        .map_err(|error| promotion_tenant_context_error(error, operation, &correlation_id))?;
    let request_context = optional_promotion_request_context(operation, &correlation_id).await;

    apply_cart_promotion_native_with_context(
        &app_ctx,
        &auth,
        &tenant,
        request_context.as_ref(),
        &correlation_id,
        cart_id,
        payload,
    )
    .await
}
