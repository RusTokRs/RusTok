use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::dto::{CartDeliveryGroupResponse, CartResponse, UpdateCartContextInput};
use crate::{CartError, CartService, CartStatus};

const CART_CHECKOUT_OWNER: &str = "rustok_cart";
const CART_CHECKOUT_BOUNDARY: &str = "cart_checkout_port";
const PREPARE_CHECKOUT_OPERATION: &str = "prepare_checkout";
const READ_CHECKOUT_SNAPSHOT_OPERATION: &str = "read_checkout_snapshot";
const COMPLETE_CHECKOUT_OPERATION: &str = "complete_checkout";
const RELEASE_CHECKOUT_OPERATION: &str = "release_checkout";

/// Immutable, transport-neutral checkout snapshot owned by the cart module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedCartCheckoutSnapshot {
    pub cart: CartResponse,
    pub shipping_address: Option<Value>,
    pub billing_address: Option<Value>,
    pub subtotal: Decimal,
    pub discount_total: Decimal,
    pub tax_total: Decimal,
    pub total: Decimal,
    pub snapshot_hash: String,
    pub projection_hash: String,
    pub status: String,
    pub locked: bool,
    pub delivery_groups: Vec<CartDeliveryGroupResponse>,
    pub tax_context: Option<Value>,
    pub updated_at: chrono::DateTime<chrono::FixedOffset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareCartCheckoutSnapshotRequest {
    pub cart_id: Uuid,
    pub input: UpdateCartContextInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteCartCheckoutRequest {
    pub cart_id: Uuid,
    pub order_id: Uuid,
}

/// Stable owner-side checkout contract consumed by orchestration modules.
#[async_trait]
pub trait CartCheckoutPort: Send + Sync {
    async fn prepare_checkout(
        &self,
        context: PortContext,
        request: PrepareCartCheckoutSnapshotRequest,
    ) -> Result<PreparedCartCheckoutSnapshot, PortError>;

    async fn read_checkout_snapshot(
        &self,
        context: PortContext,
        cart_id: Uuid,
    ) -> Result<PreparedCartCheckoutSnapshot, PortError>;

    async fn complete_checkout(
        &self,
        context: PortContext,
        request: CompleteCartCheckoutRequest,
    ) -> Result<PreparedCartCheckoutSnapshot, PortError>;

    async fn release_checkout(
        &self,
        context: PortContext,
        cart_id: Uuid,
    ) -> Result<PreparedCartCheckoutSnapshot, PortError>;
}

#[derive(Clone)]
pub struct InProcessCartCheckoutPort {
    service: CartService,
}

impl InProcessCartCheckoutPort {
    pub fn new(service: CartService) -> Self {
        Self { service }
    }
}

pub fn in_process_cart_checkout_port(service: CartService) -> Arc<dyn CartCheckoutPort> {
    Arc::new(InProcessCartCheckoutPort::new(service))
}

fn require_cart_checkout_read_admission(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<(), PortError> {
    context
        .require_policy(PortCallPolicy::read())
        .inspect_err(|error| {
            log_cart_checkout_admission_rejection(context, owner_operation, "policy", error);
        })
}

fn require_cart_checkout_write_admission(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<(), PortError> {
    context
        .require_policy(PortCallPolicy::write())
        .inspect_err(|error| {
            log_cart_checkout_admission_rejection(context, owner_operation, "policy", error);
        })?;
    context.require_write_semantics().inspect_err(|error| {
        log_cart_checkout_admission_rejection(context, owner_operation, "write_semantics", error);
    })
}

fn log_cart_checkout_admission_rejection(
    context: &PortContext,
    owner_operation: &'static str,
    admission_phase: &'static str,
    error: &PortError,
) {
    match &error.kind {
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation => {
            tracing::error!(
                error = ?error,
                owner = CART_CHECKOUT_OWNER,
                owner_operation,
                admission_phase,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                actor = ?context.actor,
                channel = ?context.channel,
                locale = %context.locale,
                causation_id = ?context.causation_id,
                traceparent = ?context.traceparent,
                idempotency_key = ?context.idempotency_key,
                deadline_ms = ?context.deadline_ms,
                internal_code = %error.code,
                internal_message = %error.message,
                error_kind = ?error.kind,
                retryable = error.retryable,
                boundary = CART_CHECKOUT_BOUNDARY,
                "cart checkout owner admission failed"
            );
        }
        _ => {
            tracing::warn!(
                error = ?error,
                owner = CART_CHECKOUT_OWNER,
                owner_operation,
                admission_phase,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                actor = ?context.actor,
                channel = ?context.channel,
                locale = %context.locale,
                causation_id = ?context.causation_id,
                traceparent = ?context.traceparent,
                idempotency_key = ?context.idempotency_key,
                deadline_ms = ?context.deadline_ms,
                internal_code = %error.code,
                internal_message = %error.message,
                error_kind = ?error.kind,
                retryable = error.retryable,
                boundary = CART_CHECKOUT_BOUNDARY,
                "cart checkout owner admission was rejected"
            );
        }
    }
}

#[async_trait]
impl CartCheckoutPort for InProcessCartCheckoutPort {
    async fn prepare_checkout(
        &self,
        context: PortContext,
        request: PrepareCartCheckoutSnapshotRequest,
    ) -> Result<PreparedCartCheckoutSnapshot, PortError> {
        require_cart_checkout_write_admission(&context, PREPARE_CHECKOUT_OPERATION)?;
        let tenant_id = parse_tenant_id(&context, PREPARE_CHECKOUT_OPERATION)?;
        let prepare_input_result = (|| {
            validate_prepare_input(&request.input).map_err(cart_error_to_port_error)?;
            Ok::<(), PortError>(())
        })();
        prepare_input_result.map_err(|error| {
            map_cart_checkout_local_port_error(
                &context,
                PREPARE_CHECKOUT_OPERATION,
                "validate_prepare_input",
                error,
            )
        })?;

        let cart = self
            .service
            .get_cart(tenant_id, request.cart_id)
            .await
            .map_err(|error| {
                map_cart_checkout_service_error(
                    &context,
                    PREPARE_CHECKOUT_OPERATION,
                    "get_cart",
                    error,
                )
            })?;
        let status = CartStatus::parse(cart.status.as_str()).ok_or_else(|| {
            map_cart_checkout_local_port_error(
                &context,
                PREPARE_CHECKOUT_OPERATION,
                "parse_cart_status",
                PortError::validation(
                    "cart.invalid_status",
                    format!("invalid cart status `{}`", cart.status),
                ),
            )
        })?;
        match status {
            CartStatus::Active => {
                let _ = self
                    .service
                    .begin_checkout(tenant_id, request.cart_id)
                    .await
                    .map_err(|error| {
                        map_cart_checkout_service_error(
                            &context,
                            PREPARE_CHECKOUT_OPERATION,
                            "begin_checkout",
                            error,
                        )
                    })?;
            }
            CartStatus::CheckingOut => {}
            status => {
                return Err(map_cart_checkout_local_port_error(
                    &context,
                    PREPARE_CHECKOUT_OPERATION,
                    "require_checkout_status",
                    PortError::conflict(
                        "cart.checkout_status_conflict",
                        format!("cart cannot enter checkout from `{}`", status.as_str()),
                    ),
                ));
            }
        }

        let cart = self
            .service
            .update_context(tenant_id, request.cart_id, request.input)
            .await
            .map_err(|error| {
                map_cart_checkout_service_error(
                    &context,
                    PREPARE_CHECKOUT_OPERATION,
                    "update_context",
                    error,
                )
            })?;
        snapshot_from_cart(cart).map_err(|error| {
            map_cart_checkout_local_port_error(
                &context,
                PREPARE_CHECKOUT_OPERATION,
                "snapshot_from_cart",
                error,
            )
        })
    }

    async fn read_checkout_snapshot(
        &self,
        context: PortContext,
        cart_id: Uuid,
    ) -> Result<PreparedCartCheckoutSnapshot, PortError> {
        require_cart_checkout_read_admission(&context, READ_CHECKOUT_SNAPSHOT_OPERATION)?;
        let tenant_id = parse_tenant_id(&context, READ_CHECKOUT_SNAPSHOT_OPERATION)?;
        let snapshot_result = self
            .service
            .get_cart(tenant_id, cart_id)
            .await
            .map_err(|error| {
                map_cart_checkout_service_error(
                    &context,
                    READ_CHECKOUT_SNAPSHOT_OPERATION,
                    "get_cart",
                    error,
                )
            })
            .and_then(snapshot_from_cart);
        snapshot_result.map_err(|error| {
            map_cart_checkout_local_port_error(
                &context,
                READ_CHECKOUT_SNAPSHOT_OPERATION,
                "snapshot_from_cart",
                error,
            )
        })
    }

    async fn complete_checkout(
        &self,
        context: PortContext,
        request: CompleteCartCheckoutRequest,
    ) -> Result<PreparedCartCheckoutSnapshot, PortError> {
        require_cart_checkout_write_admission(&context, COMPLETE_CHECKOUT_OPERATION)?;
        let tenant_id = parse_tenant_id(&context, COMPLETE_CHECKOUT_OPERATION)?;
        let cart = self
            .service
            .complete_cart(tenant_id, request.cart_id)
            .await
            .map_err(|error| {
                map_cart_checkout_service_error(
                    &context,
                    COMPLETE_CHECKOUT_OPERATION,
                    "complete_cart",
                    error,
                )
            })?;
        let mut cart = cart;
        cart.metadata = merge_checkout_order_metadata(cart.metadata, request.order_id);
        snapshot_from_cart(cart).map_err(|error| {
            map_cart_checkout_local_port_error(
                &context,
                COMPLETE_CHECKOUT_OPERATION,
                "snapshot_from_cart",
                error,
            )
        })
    }

    async fn release_checkout(
        &self,
        context: PortContext,
        cart_id: Uuid,
    ) -> Result<PreparedCartCheckoutSnapshot, PortError> {
        require_cart_checkout_write_admission(&context, RELEASE_CHECKOUT_OPERATION)?;
        let tenant_id = parse_tenant_id(&context, RELEASE_CHECKOUT_OPERATION)?;
        let cart = self
            .service
            .abandon_cart(tenant_id, cart_id)
            .await
            .map_err(|error| {
                map_cart_checkout_service_error(
                    &context,
                    RELEASE_CHECKOUT_OPERATION,
                    "abandon_cart",
                    error,
                )
            })?;
        snapshot_from_cart(cart).map_err(|error| {
            map_cart_checkout_local_port_error(
                &context,
                RELEASE_CHECKOUT_OPERATION,
                "snapshot_from_cart",
                error,
            )
        })
    }
}

fn map_cart_checkout_local_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    local_operation: &'static str,
    error: PortError,
) -> PortError {
    match &error.kind {
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation => {
            tracing::error!(
                error = ?error,
                owner = "rustok_cart",
                owner_operation,
                local_operation,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                actor = ?context.actor,
                channel = ?context.channel,
                locale = %context.locale,
                causation_id = ?context.causation_id,
                traceparent = ?context.traceparent,
                idempotency_key = ?context.idempotency_key,
                deadline_ms = ?context.deadline_ms,
                internal_code = %error.code,
                internal_message = %error.message,
                error_kind = ?error.kind,
                retryable = error.retryable,
                boundary = "cart_checkout_port",
                "cart checkout local owner operation failed"
            );
        }
        _ => {
            tracing::warn!(
                error = ?error,
                owner = "rustok_cart",
                owner_operation,
                local_operation,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                actor = ?context.actor,
                channel = ?context.channel,
                locale = %context.locale,
                causation_id = ?context.causation_id,
                traceparent = ?context.traceparent,
                idempotency_key = ?context.idempotency_key,
                deadline_ms = ?context.deadline_ms,
                internal_code = %error.code,
                internal_message = %error.message,
                error_kind = ?error.kind,
                retryable = error.retryable,
                boundary = "cart_checkout_port",
                "cart checkout local owner operation was rejected"
            );
        }
    }

    error
}

fn map_cart_checkout_service_error(
    context: &PortContext,
    owner_operation: &'static str,
    service_operation: &'static str,
    error: CartError,
) -> PortError {
    let (public_code, public_retryable, technical) = match &error {
        CartError::Validation(_) => ("cart.checkout_validation", false, false),
        CartError::CartNotFound(_) => ("cart.not_found", false, false),
        CartError::CartLineItemNotFound(_) => ("cart.line_item_not_found", false, false),
        CartError::InvalidTransition { .. } => ("cart.checkout_status_conflict", false, false),
        CartError::Database(_) => ("cart.database_unavailable", true, true),
        CartError::TaxBoundary {
            kind,
            code,
            retryable,
            ..
        } => (
            code.as_str(),
            *retryable,
            matches!(
                kind,
                PortErrorKind::Unavailable
                    | PortErrorKind::Timeout
                    | PortErrorKind::InvariantViolation
            ),
        ),
    };

    if technical {
        tracing::error!(
            error = ?error,
            owner = CART_CHECKOUT_OWNER,
            owner_operation,
            service_operation,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            channel = ?context.channel,
            locale = %context.locale,
            causation_id = ?context.causation_id,
            traceparent = ?context.traceparent,
            idempotency_key = ?context.idempotency_key,
            deadline_ms = ?context.deadline_ms,
            public_code,
            public_retryable,
            boundary = CART_CHECKOUT_BOUNDARY,
            "cart checkout owner service operation failed"
        );
    } else {
        tracing::warn!(
            error = ?error,
            owner = CART_CHECKOUT_OWNER,
            owner_operation,
            service_operation,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            channel = ?context.channel,
            locale = %context.locale,
            causation_id = ?context.causation_id,
            traceparent = ?context.traceparent,
            idempotency_key = ?context.idempotency_key,
            deadline_ms = ?context.deadline_ms,
            public_code,
            public_retryable,
            boundary = CART_CHECKOUT_BOUNDARY,
            "cart checkout owner service operation was rejected"
        );
    }

    cart_error_to_port_error(error)
}

fn validate_prepare_input(input: &UpdateCartContextInput) -> Result<(), CartError> {
    input.validate().map_err(|error| {
        tracing::warn!(error = ?error, "cart checkout input validation failed");
        CartError::Validation("cart checkout input is invalid".to_string())
    })?;
    Ok(())
}

fn parse_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.as_str()).map_err(|cause| {
        let error = PortError::validation(
            "cart.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for cart checkout",
        );
        tracing::warn!(
            cause = ?cause,
            error = ?error,
            owner = CART_CHECKOUT_OWNER,
            owner_operation,
            validation_phase = "tenant_id",
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            channel = ?context.channel,
            locale = %context.locale,
            causation_id = ?context.causation_id,
            traceparent = ?context.traceparent,
            idempotency_key = ?context.idempotency_key,
            deadline_ms = ?context.deadline_ms,
            internal_code = %error.code,
            internal_message = %error.message,
            error_kind = ?error.kind,
            retryable = error.retryable,
            boundary = CART_CHECKOUT_BOUNDARY,
            "cart checkout owner tenant context was rejected"
        );
        error
    })
}

fn snapshot_from_cart(cart: CartResponse) -> Result<PreparedCartCheckoutSnapshot, PortError> {
    let subtotal = cart.subtotal_amount;
    let discount_total = cart.adjustment_total;
    let tax_total = cart.tax_total;
    let total = cart.total_amount;
    let snapshot_hash = cart_snapshot_hash(&cart, subtotal, discount_total, tax_total, total)
        .map_err(cart_error_to_port_error)?;
    let projection_hash = projection_hash(&cart).map_err(cart_error_to_port_error)?;
    let status = CartStatus::parse(cart.status.as_str()).ok_or_else(|| {
        PortError::validation(
            "cart.invalid_status",
            format!("invalid cart status `{}`", cart.status),
        )
    })?;
    let shipping_address = cart.metadata.get("shipping_address").cloned();
    let billing_address = cart.metadata.get("billing_address").cloned();
    let tax_context = cart.metadata.get("tax_context").cloned();
    Ok(PreparedCartCheckoutSnapshot {
        shipping_address,
        billing_address,
        subtotal,
        discount_total,
        tax_total,
        total,
        projection_hash,
        status: status.as_str().to_string(),
        locked: status == CartStatus::CheckingOut,
        delivery_groups: cart.delivery_groups.clone(),
        tax_context,
        updated_at: cart.updated_at.into(),
        cart,
        snapshot_hash,
    })
}

fn cart_snapshot_hash(
    cart: &CartResponse,
    subtotal: Decimal,
    discount_total: Decimal,
    tax_total: Decimal,
    total: Decimal,
) -> Result<String, CartError> {
    let mut value = serde_json::to_value(cart).map_err(|error| {
        tracing::error!(error = ?error, "cart checkout snapshot projection encoding failed");
        CartError::Validation("cart checkout snapshot could not be encoded".to_string())
    })?;
    normalize_snapshot_value(&mut value)?;
    hash_json(serde_json::json!({
        "cart": value,
        "subtotal": subtotal,
        "discount_total": discount_total,
        "tax_total": tax_total,
        "total": total,
    }))
}

fn projection_hash(cart: &CartResponse) -> Result<String, CartError> {
    let mut value = serde_json::to_value(cart).map_err(|error| {
        tracing::error!(error = ?error, "cart checkout projection encoding failed");
        CartError::Validation("cart checkout projection could not be encoded".to_string())
    })?;
    normalize_snapshot_value(&mut value)?;
    hash_json(value)
}

fn normalize_snapshot_value(value: &mut Value) -> Result<(), CartError> {
    let object = value.as_object_mut().ok_or_else(|| {
        CartError::Validation("cart snapshot must serialize as a JSON object".to_string())
    })?;
    for key in [
        "status",
        "created_at",
        "updated_at",
        "completed_at",
        "shipping_address_id",
        "billing_address_id",
    ] {
        object.remove(key);
    }
    for collection in ["line_items", "adjustments", "tax_lines"] {
        if let Some(items) = object.get_mut(collection).and_then(Value::as_array_mut) {
            for item in items.iter_mut() {
                if let Some(item) = item.as_object_mut() {
                    item.remove("created_at");
                    item.remove("updated_at");
                }
            }
            items.sort_by(|left, right| {
                left.get("id")
                    .and_then(Value::as_str)
                    .cmp(&right.get("id").and_then(Value::as_str))
            });
        }
    }
    if let Some(groups) = object
        .get_mut("delivery_groups")
        .and_then(Value::as_array_mut)
    {
        for group in groups.iter_mut() {
            if let Some(group) = group.as_object_mut() {
                group.remove("seller_scope");
                group.remove("available_shipping_options");
                if let Some(line_ids) = group.get_mut("line_item_ids").and_then(Value::as_array_mut)
                {
                    line_ids.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
                }
            }
        }
        groups.sort_by(|left, right| {
            let left_profile = left
                .get("shipping_profile_slug")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let right_profile = right
                .get("shipping_profile_slug")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let left_seller = left
                .get("seller_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let right_seller = right
                .get("seller_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            (left_profile, left_seller).cmp(&(right_profile, right_seller))
        });
    }
    Ok(())
}

fn hash_json(value: Value) -> Result<String, CartError> {
    let canonical = canonicalize_json(value);
    let bytes = serde_json::to_vec(&canonical).map_err(|error| {
        tracing::error!(error = ?error, "cart checkout snapshot hash encoding failed");
        CartError::Validation("cart checkout snapshot could not be encoded".to_string())
    })?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, canonicalize_json(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_json).collect()),
        value => value,
    }
}

fn merge_checkout_order_metadata(metadata: Value, order_id: Uuid) -> Value {
    let mut root = match metadata {
        Value::Object(root) => root,
        _ => Default::default(),
    };
    let mut checkout = root
        .remove("checkout")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    checkout.insert("order_id".to_string(), Value::String(order_id.to_string()));
    root.insert("checkout".to_string(), Value::Object(checkout));
    Value::Object(root)
}

fn cart_error_to_port_error(error: CartError) -> PortError {
    match error {
        CartError::Validation(message) => {
            tracing::warn!(message = %message, "cart checkout owner validation failed");
            PortError::validation(
                "cart.checkout_validation",
                "cart checkout request or projection is invalid",
            )
        }
        CartError::CartNotFound(_) => PortError::not_found("cart.not_found", "cart was not found"),
        CartError::CartLineItemNotFound(_) => {
            PortError::not_found("cart.line_item_not_found", "cart line item was not found")
        }
        CartError::InvalidTransition { .. } => PortError::conflict(
            "cart.checkout_status_conflict",
            "cart status transition conflicts with checkout lifecycle",
        ),
        CartError::Database(error) => {
            tracing::error!(error = ?error, "cart checkout storage operation failed");
            PortError::unavailable(
                "cart.database_unavailable",
                "cart storage is temporarily unavailable",
            )
        }
        CartError::TaxBoundary {
            kind,
            code,
            message,
            retryable,
        } => PortError::new(kind, code, message, retryable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_json_is_independent_of_object_key_order() {
        let first = canonicalize_json(serde_json::json!({
            "b": 2,
            "a": {"d": 4, "c": 3}
        }));
        let second = canonicalize_json(serde_json::json!({
            "a": {"c": 3, "d": 4},
            "b": 2
        }));
        assert_eq!(first, second);
    }

    #[test]
    fn snapshot_normalization_removes_volatile_projection_fields() {
        let mut value = serde_json::json!({
            "id": Uuid::nil(),
            "status": "active",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-02T00:00:00Z",
            "completed_at": null,
            "line_items": [{
                "id": "b",
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-02T00:00:00Z"
            }],
            "adjustments": [],
            "tax_lines": [],
            "delivery_groups": [{
                "shipping_profile_slug": "default",
                "seller_id": null,
                "seller_scope": null,
                "line_item_ids": ["b", "a"],
                "selected_shipping_option_id": null,
                "available_shipping_options": [{"id": "volatile"}]
            }]
        });

        normalize_snapshot_value(&mut value).expect("normalize snapshot");
        let object = value.as_object().expect("snapshot object");
        assert!(!object.contains_key("status"));
        assert!(!object.contains_key("updated_at"));
        let group = object["delivery_groups"][0]
            .as_object()
            .expect("delivery group");
        assert!(!group.contains_key("available_shipping_options"));
        assert_eq!(group["line_item_ids"], serde_json::json!(["a", "b"]));
    }
}
