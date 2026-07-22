use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use rustok_commerce_foundation::delivery_groups::{
    CheckoutDeliveryGroupSnapshot, CheckoutLineAssignment,
    build_checkout_delivery_group_snapshots,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::dto::{CartAddressResponse, CartResponse, UpdateCartInput};
use crate::{CartError, CartService, CartStatus};

/// Immutable, transport-neutral checkout snapshot owned by the cart module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedCartCheckoutSnapshot {
    pub cart: CartResponse,
    pub shipping_address: Option<CartAddressResponse>,
    pub billing_address: Option<CartAddressResponse>,
    pub subtotal: Decimal,
    pub discount_total: Decimal,
    pub tax_total: Decimal,
    pub total: Decimal,
    pub snapshot_hash: String,
    pub projection_hash: String,
    pub status: String,
    pub locked: bool,
    pub delivery_groups: Vec<CheckoutDeliveryGroupSnapshot>,
    pub tax_context: Option<Value>,
    pub updated_at: chrono::DateTime<chrono::FixedOffset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareCartCheckoutRequest {
    pub cart_id: Uuid,
    pub input: UpdateCartInput,
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
        request: PrepareCartCheckoutRequest,
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

#[async_trait]
impl CartCheckoutPort for InProcessCartCheckoutPort {
    async fn prepare_checkout(
        &self,
        context: PortContext,
        request: PrepareCartCheckoutRequest,
    ) -> Result<PreparedCartCheckoutSnapshot, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_tenant_id(&context)?;
        let actor_id = parse_actor_id(&context)?;
        validate_prepare_input(&request.input)?;

        let cart = self
            .service
            .get_cart(tenant_id, request.cart_id)
            .await
            .map_err(cart_error_to_port_error)?;
        match CartStatus::parse(cart.status.as_str()).map_err(cart_error_to_port_error)? {
            CartStatus::Active => {}
            CartStatus::CheckingOut => {
                return self
                    .service
                    .get_cart_with_addresses(tenant_id, request.cart_id)
                    .await
                    .map_err(cart_error_to_port_error)
                    .and_then(snapshot_from_cart);
            }
            status => {
                return Err(PortError::conflict(
                    "cart.checkout_status_conflict",
                    format!("cart cannot enter checkout from `{}`", status.as_str()),
                ));
            }
        }

        let cart = self
            .service
            .update_cart(tenant_id, actor_id, request.cart_id, request.input)
            .await
            .map_err(cart_error_to_port_error)?;
        snapshot_from_cart(cart)
    }

    async fn read_checkout_snapshot(
        &self,
        context: PortContext,
        cart_id: Uuid,
    ) -> Result<PreparedCartCheckoutSnapshot, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context)?;
        self.service
            .get_cart_with_addresses(tenant_id, cart_id)
            .await
            .map_err(cart_error_to_port_error)
            .and_then(snapshot_from_cart)
    }

    async fn complete_checkout(
        &self,
        context: PortContext,
        request: CompleteCartCheckoutRequest,
    ) -> Result<PreparedCartCheckoutSnapshot, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_tenant_id(&context)?;
        let actor_id = parse_actor_id(&context)?;
        let cart = self
            .service
            .transition_status(
                tenant_id,
                actor_id,
                request.cart_id,
                CartStatus::Completed,
            )
            .await
            .map_err(cart_error_to_port_error)?;
        let mut cart = cart;
        cart.metadata = merge_checkout_order_metadata(cart.metadata, request.order_id);
        snapshot_from_cart(cart)
    }

    async fn release_checkout(
        &self,
        context: PortContext,
        cart_id: Uuid,
    ) -> Result<PreparedCartCheckoutSnapshot, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_tenant_id(&context)?;
        let actor_id = parse_actor_id(&context)?;
        let cart = self
            .service
            .transition_status(tenant_id, actor_id, cart_id, CartStatus::Active)
            .await
            .map_err(cart_error_to_port_error)?;
        snapshot_from_cart(cart)
    }
}

fn validate_prepare_input(input: &UpdateCartInput) -> Result<(), CartError> {
    input.validate().map_err(|error| {
        tracing::warn!(error = ?error, "cart checkout input validation failed");
        CartError::Validation("cart checkout input is invalid".to_string())
    })?;
    if input.status.as_deref() != Some(CartStatus::CheckingOut.as_str()) {
        return Err(CartError::Validation(
            "checkout preparation requires status=checking_out".to_string(),
        ));
    }
    Ok(())
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.as_str()).map_err(|_| {
        PortError::validation(
            "cart.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for cart checkout",
        )
    })
}

fn parse_actor_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.actor.id.as_str()).map_err(|_| {
        PortError::validation(
            "cart.actor_id_invalid",
            "PortContext.actor.id must be a UUID for cart checkout writes",
        )
    })
}

fn snapshot_from_cart(cart: CartResponse) -> Result<PreparedCartCheckoutSnapshot, PortError> {
    let (subtotal, discount_total, tax_total, total) = calculate_checkout_totals(&cart);
    let snapshot_hash = cart_snapshot_hash(&cart, subtotal, discount_total, tax_total, total)
        .map_err(cart_error_to_port_error)?;
    let projection_hash = projection_hash(&cart).map_err(cart_error_to_port_error)?;
    let status = CartStatus::parse(cart.status.as_str()).map_err(cart_error_to_port_error)?;
    let delivery_groups = delivery_groups_from_cart(&cart).map_err(cart_error_to_port_error)?;
    let tax_context = cart.metadata.get("tax_context").cloned();
    Ok(PreparedCartCheckoutSnapshot {
        shipping_address: cart.shipping_address.clone(),
        billing_address: cart.billing_address.clone(),
        subtotal,
        discount_total,
        tax_total,
        total,
        projection_hash,
        status: status.as_str().to_string(),
        locked: status == CartStatus::CheckingOut,
        delivery_groups,
        tax_context,
        updated_at: cart.updated_at,
        cart,
        snapshot_hash,
    })
}

fn calculate_checkout_totals(cart: &CartResponse) -> (Decimal, Decimal, Decimal, Decimal) {
    let subtotal = cart.line_items.iter().map(|line| line.subtotal).sum();
    let discount_total = cart
        .line_items
        .iter()
        .map(|line| line.discount_total)
        .sum();
    let tax_total = cart.line_items.iter().map(|line| line.tax_total).sum();
    let total = cart.line_items.iter().map(|line| line.total).sum();
    (subtotal, discount_total, tax_total, total)
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

fn delivery_groups_from_cart(
    cart: &CartResponse,
) -> Result<Vec<CheckoutDeliveryGroupSnapshot>, CartError> {
    let assignments = cart
        .line_items
        .iter()
        .map(|line| CheckoutLineAssignment {
            cart_line_item_id: line.id,
            shipping_profile_slug: line.shipping_profile_slug.clone(),
            seller_id: line.seller_id,
        })
        .collect::<Vec<_>>();
    let groups = build_checkout_delivery_group_snapshots(assignments)
        .map_err(|error| CartError::Validation(error.to_string()))?;
    let selected = cart
        .delivery_groups
        .iter()
        .map(|group| {
            (
                (
                    group.shipping_profile_slug.clone(),
                    group.seller_id.unwrap_or(Uuid::nil()),
                ),
                group.selected_shipping_option_id,
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    Ok(groups
        .into_iter()
        .map(|mut group| {
            group.selected_shipping_option_id = selected
                .get(&(
                    group.shipping_profile_slug.clone(),
                    group.seller_id.unwrap_or(Uuid::nil()),
                ))
                .copied()
                .flatten();
            group
        })
        .collect())
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
                if let Some(line_ids) = group
                    .get_mut("line_item_ids")
                    .and_then(Value::as_array_mut)
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
    Ok(hex::encode(Sha256::digest(bytes)))
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
        CartError::CartNotFound(_) => {
            PortError::not_found("cart.not_found", "cart was not found")
        }
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
