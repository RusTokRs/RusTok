use chrono::Utc;
use rustok_cart::CartMarketplaceLineSnapshot;
use rustok_order::CreateOrderInput;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use thiserror::Error;
use uuid::Uuid;

use crate::dto::StoreContextResponse;
use crate::entities::{checkout_operation, checkout_order_plan};

use super::{CheckoutOperationStage, CheckoutOperationStatus};

const MAX_HASH_LENGTH: usize = 128;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckoutFulfillmentPlanItem {
    pub cart_line_item_id: Uuid,
    pub quantity: i32,
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckoutFulfillmentPlan {
    pub shipping_option_id: Option<Uuid>,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub items: Vec<CheckoutFulfillmentPlanItem>,
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckoutMarketplaceLineSnapshot {
    pub order_line_index: usize,
    pub snapshot: CartMarketplaceLineSnapshot,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckoutOrderPlanPayload {
    pub order_input: CreateOrderInput,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub context: StoreContextResponse,
    pub create_fulfillment: bool,
    #[serde(default)]
    pub fulfillment_plans: Vec<CheckoutFulfillmentPlan>,
    #[serde(default)]
    pub marketplace_lines: Vec<CheckoutMarketplaceLineSnapshot>,
    pub checkout_metadata: Value,
}

#[derive(Clone, Debug)]
pub struct CheckoutOrderPlanRecord {
    pub checkout_operation_id: Uuid,
    pub tenant_id: Uuid,
    pub snapshot_hash: String,
    pub plan_hash: String,
    pub payload: CheckoutOrderPlanPayload,
}

#[derive(Debug, Error)]
pub enum CheckoutOrderPlanError {
    #[error("checkout order plan validation failed: {0}")]
    Validation(String),
    #[error("checkout order plan for operation {0} not found")]
    NotFound(Uuid),
    #[error("checkout order plan conflict: {0}")]
    Conflict(String),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub type CheckoutOrderPlanResult<T> = Result<T, CheckoutOrderPlanError>;

#[derive(Clone)]
pub struct CheckoutOrderPlanJournal {
    db: DatabaseConnection,
}

impl CheckoutOrderPlanJournal {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn persist(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
        snapshot_hash: impl Into<String>,
        payload: CheckoutOrderPlanPayload,
    ) -> CheckoutOrderPlanResult<CheckoutOrderPlanRecord> {
        validate_payload(&payload)?;
        let snapshot_hash = normalize_hash(snapshot_hash.into(), "snapshot_hash")?;
        let payload_value = serde_json::to_value(&payload).map_err(|error| {
            CheckoutOrderPlanError::Validation(format!(
                "failed to serialize checkout order plan: {error}"
            ))
        })?;
        if !payload_value.is_object() {
            return Err(CheckoutOrderPlanError::Validation(
                "checkout order plan payload must serialize as an object".to_string(),
            ));
        }
        let plan_hash = hash_payload(payload_value.clone())?;

        if let Some(existing) = self.find_model(tenant_id, checkout_operation_id).await? {
            return validate_existing(existing, &snapshot_hash, &plan_hash);
        }

        let operation = checkout_operation::Entity::find_by_id(checkout_operation_id)
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                CheckoutOrderPlanError::Conflict(format!(
                    "checkout operation {checkout_operation_id} was not found for tenant {tenant_id}"
                ))
            })?;
        if operation.status != CheckoutOperationStatus::Executing.as_str() {
            return Err(CheckoutOrderPlanError::Conflict(format!(
                "checkout operation {} must be executing, not `{}`",
                operation.id, operation.status
            )));
        }
        if operation.stage != CheckoutOperationStage::CartLocked.as_str() {
            return Err(CheckoutOrderPlanError::Conflict(format!(
                "checkout operation {} cannot persist an order plan from stage `{}`",
                operation.id, operation.stage
            )));
        }
        if operation.snapshot_hash.as_deref() != Some(snapshot_hash.as_str()) {
            return Err(CheckoutOrderPlanError::Conflict(format!(
                "checkout operation {} snapshot hash does not match the order plan",
                operation.id
            )));
        }

        let now = Utc::now();
        let insert = checkout_order_plan::ActiveModel {
            checkout_operation_id: Set(checkout_operation_id),
            tenant_id: Set(tenant_id),
            snapshot_hash: Set(snapshot_hash.clone()),
            plan_hash: Set(plan_hash.clone()),
            payload: Set(payload_value),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&self.db)
        .await;

        match insert {
            Ok(model) => model_to_record(model),
            Err(insert_error) => {
                if let Some(existing) = self.find_model(tenant_id, checkout_operation_id).await? {
                    return validate_existing(existing, &snapshot_hash, &plan_hash);
                }
                Err(insert_error.into())
            }
        }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
    ) -> CheckoutOrderPlanResult<CheckoutOrderPlanRecord> {
        let model = self
            .find_model(tenant_id, checkout_operation_id)
            .await?
            .ok_or(CheckoutOrderPlanError::NotFound(checkout_operation_id))?;
        model_to_record(model)
    }

    async fn find_model(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
    ) -> Result<Option<checkout_order_plan::Model>, sea_orm::DbErr> {
        checkout_order_plan::Entity::find_by_id(checkout_operation_id)
            .filter(checkout_order_plan::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
    }
}

fn validate_payload(payload: &CheckoutOrderPlanPayload) -> CheckoutOrderPlanResult<()> {
    if !payload.create_fulfillment && !payload.fulfillment_plans.is_empty() {
        return Err(CheckoutOrderPlanError::Validation(
            "fulfillment plans require create_fulfillment=true".to_string(),
        ));
    }
    let mut cart_line_item_ids = HashSet::new();
    for (plan_index, plan) in payload.fulfillment_plans.iter().enumerate() {
        if plan.items.is_empty() {
            return Err(CheckoutOrderPlanError::Validation(format!(
                "fulfillment plan {plan_index} must contain at least one item"
            )));
        }
        for item in &plan.items {
            if item.quantity <= 0 {
                return Err(CheckoutOrderPlanError::Validation(format!(
                    "fulfillment item {} must have a positive quantity",
                    item.cart_line_item_id
                )));
            }
            if !cart_line_item_ids.insert(item.cart_line_item_id) {
                return Err(CheckoutOrderPlanError::Validation(format!(
                    "cart line item {} appears in multiple fulfillment plans",
                    item.cart_line_item_id
                )));
            }
        }
    }

    let mut marketplace_indexes = HashSet::new();
    let mut marketplace_cart_lines = HashSet::new();
    for line in &payload.marketplace_lines {
        if !marketplace_indexes.insert(line.order_line_index) {
            return Err(CheckoutOrderPlanError::Validation(format!(
                "order line index {} has multiple marketplace snapshots",
                line.order_line_index
            )));
        }
        if !marketplace_cart_lines.insert(line.snapshot.cart_line_item_id) {
            return Err(CheckoutOrderPlanError::Validation(format!(
                "cart line {} has multiple marketplace snapshots",
                line.snapshot.cart_line_item_id
            )));
        }
        let order_line = payload
            .order_input
            .line_items
            .get(line.order_line_index)
            .ok_or_else(|| {
                CheckoutOrderPlanError::Validation(format!(
                    "marketplace snapshot references missing order line index {}",
                    line.order_line_index
                ))
            })?;
        let expected_subtotal = i64::from(order_line.quantity)
            .checked_mul(line.snapshot.unit_amount)
            .ok_or_else(|| {
                CheckoutOrderPlanError::Validation(format!(
                    "marketplace snapshot subtotal overflow for cart line {}",
                    line.snapshot.cart_line_item_id
                ))
            })?;
        if order_line.product_id != Some(line.snapshot.master_product_id)
            || order_line.variant_id != Some(line.snapshot.master_variant_id)
            || expected_subtotal != line.snapshot.subtotal_amount
        {
            return Err(CheckoutOrderPlanError::Validation(format!(
                "marketplace snapshot for cart line {} does not match its immutable order line",
                line.snapshot.cart_line_item_id
            )));
        }
        let order_seller_id = order_line
            .seller_id
            .as_deref()
            .and_then(|value| Uuid::parse_str(value.trim()).ok());
        if order_seller_id != Some(line.snapshot.seller_id) {
            return Err(CheckoutOrderPlanError::Validation(format!(
                "marketplace snapshot for cart line {} does not match seller identity",
                line.snapshot.cart_line_item_id
            )));
        }
    }
    Ok(())
}

fn validate_existing(
    model: checkout_order_plan::Model,
    snapshot_hash: &str,
    plan_hash: &str,
) -> CheckoutOrderPlanResult<CheckoutOrderPlanRecord> {
    if model.snapshot_hash != snapshot_hash || model.plan_hash != plan_hash {
        return Err(CheckoutOrderPlanError::Conflict(format!(
            "checkout operation {} is already bound to another immutable order plan",
            model.checkout_operation_id
        )));
    }
    model_to_record(model)
}

fn model_to_record(
    model: checkout_order_plan::Model,
) -> CheckoutOrderPlanResult<CheckoutOrderPlanRecord> {
    let payload = serde_json::from_value(model.payload).map_err(|error| {
        CheckoutOrderPlanError::Validation(format!(
            "stored checkout order plan {} is invalid: {error}",
            model.checkout_operation_id
        ))
    })?;
    Ok(CheckoutOrderPlanRecord {
        checkout_operation_id: model.checkout_operation_id,
        tenant_id: model.tenant_id,
        snapshot_hash: model.snapshot_hash,
        plan_hash: model.plan_hash,
        payload,
    })
}

fn normalize_hash(value: String, label: &str) -> CheckoutOrderPlanResult<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() || value.len() > MAX_HASH_LENGTH {
        return Err(CheckoutOrderPlanError::Validation(format!(
            "{label} must contain between 1 and {MAX_HASH_LENGTH} bytes"
        )));
    }
    if !value.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(CheckoutOrderPlanError::Validation(format!(
            "{label} must be hexadecimal"
        )));
    }
    Ok(value)
}

fn hash_payload(value: Value) -> CheckoutOrderPlanResult<String> {
    let canonical = canonicalize_json(value);
    let payload = serde_json::to_vec(&canonical).map_err(|error| {
        CheckoutOrderPlanError::Validation(format!("failed to encode checkout order plan: {error}"))
    })?;
    Ok(Sha256::digest(payload)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_hash_is_independent_of_object_key_order() {
        let first = hash_payload(serde_json::json!({"b": 2, "a": {"d": 4, "c": 3}})).unwrap();
        let second = hash_payload(serde_json::json!({"a": {"c": 3, "d": 4}, "b": 2})).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn hash_validation_rejects_non_hexadecimal_values() {
        assert!(normalize_hash("not-a-hash".to_string(), "snapshot_hash").is_err());
        assert_eq!(
            normalize_hash(" A0B1 ".to_string(), "snapshot_hash").unwrap(),
            "a0b1"
        );
    }
}