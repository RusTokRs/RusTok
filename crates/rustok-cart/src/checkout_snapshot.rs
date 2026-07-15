use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;
use validator::Validate;

use crate::services::cart::helpers::{
    normalize_country_code, normalize_locale_code, normalize_seller_id,
    normalize_shipping_profile_slug,
};
use crate::{
    CartDeliveryGroupResponse, CartError, CartResponse, CartService, CartShippingSelectionInput,
};

const CHECKOUT_SNAPSHOT_SCHEMA: &str = "rustok.cart.checkout_snapshot.v1";

/// Request overlay used to prepare a stable owner-defined checkout snapshot.
///
/// The cart remains the source of truth. Region, country and locale only fill
/// missing cart context. Shipping fields follow the same patch semantics as
/// `CartService::update_context`, without mutating persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareCartCheckoutSnapshotRequest {
    pub cart_id: Uuid,
    pub region_id: Option<Uuid>,
    pub country_code: Option<String>,
    pub locale_code: Option<String>,
    pub selected_shipping_option_id: Option<Uuid>,
    pub shipping_selections: Option<Vec<CartShippingSelectionInput>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedCartCheckoutSnapshot {
    pub cart: CartResponse,
    pub snapshot_hash: String,
}

/// Cart-owned boundary for immutable checkout input snapshots.
///
/// Consumers must persist `snapshot_hash` with their orchestration identity and
/// reject a replay when the same idempotency key resolves to another snapshot.
#[async_trait]
pub trait CartCheckoutSnapshotPort: Send + Sync {
    async fn prepare_checkout_snapshot(
        &self,
        context: PortContext,
        request: PrepareCartCheckoutSnapshotRequest,
    ) -> Result<PreparedCartCheckoutSnapshot, PortError>;
}

pub fn in_process_cart_checkout_snapshot_port(
    db: DatabaseConnection,
) -> Arc<dyn CartCheckoutSnapshotPort> {
    Arc::new(CartService::new(db))
}

#[async_trait]
impl CartCheckoutSnapshotPort for CartService {
    async fn prepare_checkout_snapshot(
        &self,
        context: PortContext,
        request: PrepareCartCheckoutSnapshotRequest,
    ) -> Result<PreparedCartCheckoutSnapshot, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let cart = self
            .get_cart(tenant_id, request.cart_id)
            .await
            .map_err(cart_error_to_port_error)?;

        prepare_snapshot(cart, request).map_err(cart_error_to_port_error)
    }
}

fn prepare_snapshot(
    mut cart: CartResponse,
    request: PrepareCartCheckoutSnapshotRequest,
) -> Result<PreparedCartCheckoutSnapshot, CartError> {
    if cart.region_id.is_none() {
        cart.region_id = request.region_id;
    }
    if cart.country_code.is_none() {
        cart.country_code = request
            .country_code
            .as_deref()
            .map(normalize_country_code)
            .transpose()?;
    }
    if cart.locale_code.is_none() {
        cart.locale_code = request
            .locale_code
            .as_deref()
            .map(normalize_locale_code)
            .transpose()?;
    }

    if request.shipping_selections.is_some() || request.selected_shipping_option_id.is_some() {
        apply_shipping_overlay(
            &mut cart,
            request.selected_shipping_option_id,
            request.shipping_selections,
        )?;
    }

    let snapshot_hash = hash_cart_snapshot(&cart)?;
    Ok(PreparedCartCheckoutSnapshot {
        cart,
        snapshot_hash,
    })
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SnapshotGroupKey {
    shipping_profile_slug: String,
    seller_id: Option<String>,
    seller_scope: Option<String>,
}

impl From<&CartDeliveryGroupResponse> for SnapshotGroupKey {
    fn from(group: &CartDeliveryGroupResponse) -> Self {
        Self {
            shipping_profile_slug: normalize_shipping_profile_slug(Some(
                group.shipping_profile_slug.as_str(),
            )),
            seller_id: normalize_seller_id(group.seller_id.as_deref()),
            // Seller scope is retained in the snapshot identity but intentionally
            // ignored when matching the current compatibility request shape.
            seller_scope: group.seller_scope.clone(),
        }
    }
}

fn apply_shipping_overlay(
    cart: &mut CartResponse,
    selected_shipping_option_id: Option<Uuid>,
    shipping_selections: Option<Vec<CartShippingSelectionInput>>,
) -> Result<(), CartError> {
    let available_groups = cart
        .delivery_groups
        .iter()
        .map(SnapshotGroupKey::from)
        .collect::<Vec<_>>();
    let mut desired = cart
        .delivery_groups
        .iter()
        .map(|group| {
            (
                SnapshotGroupKey::from(group),
                group.selected_shipping_option_id,
            )
        })
        .collect::<BTreeMap<_, _>>();

    if let Some(shipping_selections) = shipping_selections {
        desired.clear();
        for selection in shipping_selections {
            selection
                .validate()
                .map_err(|error| CartError::Validation(error.to_string()))?;
            let shipping_profile_slug =
                normalize_shipping_profile_slug(Some(selection.shipping_profile_slug.as_str()));
            let seller_id = normalize_seller_id(selection.seller_id.as_deref());

            for key in available_groups.iter().filter(|key| {
                key.shipping_profile_slug == shipping_profile_slug
                    && match seller_id.as_deref() {
                        Some(seller_id) => key.seller_id.as_deref() == Some(seller_id),
                        None => key.seller_id.is_none(),
                    }
            }) {
                desired.insert(key.clone(), selection.selected_shipping_option_id);
            }
        }
    } else if available_groups.len() <= 1 {
        if let Some(group) = available_groups.first() {
            desired.insert(group.clone(), selected_shipping_option_id);
        } else {
            desired.clear();
        }
    } else if selected_shipping_option_id != cart.selected_shipping_option_id
        && selected_shipping_option_id.is_some()
    {
        return Err(CartError::Validation(
            "selected_shipping_option_id can only be used for carts with a single delivery group"
                .to_string(),
        ));
    }

    for group in &mut cart.delivery_groups {
        group.selected_shipping_option_id = desired
            .get(&SnapshotGroupKey::from(&*group))
            .copied()
            .flatten();
    }

    cart.selected_shipping_option_id = match cart.delivery_groups.len() {
        0 => selected_shipping_option_id,
        1 => cart.delivery_groups[0].selected_shipping_option_id,
        _ => None,
    };

    Ok(())
}

fn hash_cart_snapshot(cart: &CartResponse) -> Result<String, CartError> {
    let mut value = serde_json::to_value(cart).map_err(|error| {
        CartError::Validation(format!(
            "failed to serialize cart checkout snapshot: {error}"
        ))
    })?;
    normalize_snapshot_value(&mut value)?;
    let canonical = canonicalize_json(value);
    let payload = serde_json::to_vec(&serde_json::json!({
        "schema": CHECKOUT_SNAPSHOT_SCHEMA,
        "cart": canonical,
    }))
    .map_err(|error| {
        CartError::Validation(format!("failed to encode cart checkout snapshot: {error}"))
    })?;

    Ok(Sha256::digest(payload)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn normalize_snapshot_value(value: &mut Value) -> Result<(), CartError> {
    let cart = value.as_object_mut().ok_or_else(|| {
        CartError::Validation("cart checkout snapshot must serialize as an object".to_string())
    })?;
    remove_fields(
        cart,
        &["status", "created_at", "updated_at", "completed_at"],
    );

    normalize_record_array(cart.get_mut("line_items"), true)?;
    normalize_record_array(cart.get_mut("adjustments"), true)?;
    normalize_record_array(cart.get_mut("tax_lines"), true)?;

    if let Some(groups) = cart.get_mut("delivery_groups") {
        let groups = groups.as_array_mut().ok_or_else(|| {
            CartError::Validation(
                "cart checkout delivery groups must serialize as an array".to_string(),
            )
        })?;
        for group in groups.iter_mut() {
            let group = group.as_object_mut().ok_or_else(|| {
                CartError::Validation(
                    "cart checkout delivery group must serialize as an object".to_string(),
                )
            })?;
            group.remove("available_shipping_options");
            if let Some(line_item_ids) =
                group.get_mut("line_item_ids").and_then(Value::as_array_mut)
            {
                line_item_ids.sort_by_key(value_sort_key);
            }
        }
        groups.sort_by_key(delivery_group_sort_key);
    }

    Ok(())
}

fn normalize_record_array(
    value: Option<&mut Value>,
    remove_timestamps: bool,
) -> Result<(), CartError> {
    let Some(value) = value else {
        return Ok(());
    };
    let records = value.as_array_mut().ok_or_else(|| {
        CartError::Validation("cart checkout records must serialize as an array".to_string())
    })?;
    for record in records.iter_mut() {
        let record = record.as_object_mut().ok_or_else(|| {
            CartError::Validation("cart checkout record must serialize as an object".to_string())
        })?;
        if remove_timestamps {
            remove_fields(record, &["created_at", "updated_at"]);
        }
    }
    records.sort_by_key(|record| {
        record
            .get("id")
            .map(value_sort_key)
            .unwrap_or_else(|| value_sort_key(record))
    });
    Ok(())
}

fn remove_fields(object: &mut serde_json::Map<String, Value>, fields: &[&str]) {
    for field in fields {
        object.remove(*field);
    }
}

fn delivery_group_sort_key(value: &Value) -> String {
    let Some(group) = value.as_object() else {
        return value_sort_key(value);
    };
    format!(
        "{}\u{0}{}\u{0}{}",
        group
            .get("shipping_profile_slug")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        group
            .get("seller_id")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        group
            .get("seller_scope")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )
}

fn value_sort_key(value: &Value) -> String {
    value
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| value.to_string())
}

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Object(values) => {
            let ordered = values
                .into_iter()
                .map(|(key, value)| (key, canonicalize_json(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(ordered.into_iter().collect())
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_json).collect()),
        value => value,
    }
}

fn parse_port_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.trim()).map_err(|_| {
        PortError::validation(
            "cart.invalid_tenant_id",
            "cart checkout snapshot requires a UUID tenant_id",
        )
    })
}

fn cart_error_to_port_error(error: CartError) -> PortError {
    match error {
        CartError::Validation(message) => {
            PortError::validation("cart.checkout_snapshot_validation", message)
        }
        CartError::CartNotFound(cart_id) => {
            PortError::not_found("cart.not_found", format!("cart {cart_id} not found"))
        }
        CartError::CartLineItemNotFound(line_item_id) => PortError::not_found(
            "cart.line_item_not_found",
            format!("cart line item {line_item_id} not found"),
        ),
        CartError::InvalidTransition { from, to } => PortError::conflict(
            "cart.invalid_transition",
            format!("invalid cart status transition: {from} -> {to}"),
        ),
        CartError::Database(_) => PortError::unavailable(
            "cart.checkout_snapshot_storage_unavailable",
            "cart checkout snapshot storage is unavailable",
        ),
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
