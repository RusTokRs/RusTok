use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;
use validator::Validate;

use rustok_fulfillment::FulfillmentService;
use rustok_fulfillment::providers::{
    FulfillmentProviderOperationRequest, FulfillmentProviderRegistry,
    MANUAL_FULFILLMENT_PROVIDER_ID,
};

use crate::{
    dto::{CreateFulfillmentInput, FulfillmentResponse, ShippingOptionResponse},
    storefront_shipping::{
        is_shipping_option_compatible_with_profiles, normalize_shipping_profile_slug,
    },
};

#[derive(Debug, Error)]
pub enum FulfillmentOrchestrationError {
    #[error("Order not found: {0}")]
    OrderNotFound(Uuid),

    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    #[error("Fulfillment error: {0}")]
    Fulfillment(#[from] rustok_fulfillment::error::FulfillmentError),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error(
        "fulfillment provider `{operation}` failed after fulfillment {fulfillment_id} was persisted: {source}"
    )]
    ProviderAfterPersistence {
        fulfillment_id: Uuid,
        operation: &'static str,
        #[source]
        source: rustok_fulfillment::error::FulfillmentError,
    },

    #[error(
        "fulfillment provider `{operation}` succeeded for fulfillment {fulfillment_id}, but local persistence failed: {source}"
    )]
    PersistenceAfterProvider {
        fulfillment_id: Uuid,
        operation: &'static str,
        #[source]
        source: rustok_fulfillment::error::FulfillmentError,
    },
}

pub type FulfillmentOrchestrationResult<T> = Result<T, FulfillmentOrchestrationError>;

pub struct FulfillmentOrchestrationService {
    db: DatabaseConnection,
    fulfillment_provider_registry: FulfillmentProviderRegistry,
}

impl FulfillmentOrchestrationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            fulfillment_provider_registry: FulfillmentProviderRegistry::with_manual_provider(),
        }
    }

    pub fn with_provider_registry(
        mut self,
        fulfillment_provider_registry: FulfillmentProviderRegistry,
    ) -> Self {
        self.fulfillment_provider_registry = fulfillment_provider_registry;
        self
    }

    pub async fn create_manual_fulfillment(
        &self,
        tenant_id: Uuid,
        input: CreateFulfillmentInput,
    ) -> FulfillmentOrchestrationResult<FulfillmentResponse> {
        input
            .validate()
            .map_err(|error| FulfillmentOrchestrationError::Validation(error.to_string()))?;

        let order = rustok_order::entities::order::Entity::find_by_id(input.order_id)
            .filter(rustok_order::entities::order::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(FulfillmentOrchestrationError::OrderNotFound(input.order_id))?;

        let requested_items = input.items.clone().ok_or_else(|| {
            FulfillmentOrchestrationError::Validation(
                "manual fulfillments require typed items[]".to_string(),
            )
        })?;
        if requested_items.is_empty() {
            return Err(FulfillmentOrchestrationError::Validation(
                "manual fulfillments require at least one item".to_string(),
            ));
        }

        if input.customer_id != order.customer_id && input.customer_id.is_some() {
            return Err(FulfillmentOrchestrationError::Validation(format!(
                "customer_id {:?} does not match order customer {:?}",
                input.customer_id, order.customer_id
            )));
        }

        let order_line_items = rustok_order::entities::order_line_item::Entity::find()
            .filter(rustok_order::entities::order_line_item::Column::OrderId.eq(order.id))
            .order_by_asc(rustok_order::entities::order_line_item::Column::CreatedAt)
            .all(&self.db)
            .await?;
        let order_line_items_by_id = order_line_items
            .iter()
            .cloned()
            .map(|item| (item.id, item))
            .collect::<BTreeMap<_, _>>();

        let fulfillment_service = FulfillmentService::new(self.db.clone());
        let existing_fulfillments = fulfillment_service
            .list_by_order(tenant_id, order.id)
            .await?;
        let mut fulfilled_quantities = BTreeMap::<Uuid, i32>::new();
        for fulfillment in existing_fulfillments {
            if fulfillment.status == "cancelled" {
                continue;
            }
            if fulfillment.items.is_empty() {
                return Err(FulfillmentOrchestrationError::Validation(format!(
                    "existing fulfillment {} has no typed items; post-order manual fulfillment requires typed fulfillment.items[]",
                    fulfillment.id
                )));
            }
            for item in fulfillment.items {
                let entry = fulfilled_quantities
                    .entry(item.order_line_item_id)
                    .or_insert(0);
                *entry = entry.checked_add(item.quantity).ok_or_else(|| {
                    FulfillmentOrchestrationError::Validation(
                        "fulfilled quantity aggregation overflowed".to_string(),
                    )
                })?;
            }
        }

        let requested_group = requested_items
            .iter()
            .map(|item| {
                let line_item = order_line_items_by_id
                    .get(&item.order_line_item_id)
                    .ok_or_else(|| {
                        FulfillmentOrchestrationError::Validation(format!(
                            "order_line_item_id {} does not belong to order {}",
                            item.order_line_item_id, order.id
                        ))
                    })?;
                let shipping_profile_slug =
                    normalize_shipping_profile_slug(line_item.shipping_profile_slug.as_str())
                        .unwrap_or_else(|| "default".to_string());
                let seller_id = normalize_seller_id(
                    line_item
                        .seller_id
                        .clone()
                        .or_else(|| seller_id_from_metadata(&line_item.metadata))
                        .as_deref(),
                );
                Ok(DeliveryGroupKey {
                    shipping_profile_slug,
                    seller_id,
                })
            })
            .collect::<FulfillmentOrchestrationResult<Vec<_>>>()?;
        let canonical_group = requested_group
            .first()
            .cloned()
            .expect("requested items already validated as non-empty");
        if requested_group.iter().any(|group| {
            group.shipping_profile_slug != canonical_group.shipping_profile_slug
                || group.seller_id != canonical_group.seller_id
        }) {
            return Err(FulfillmentOrchestrationError::Validation(
                "manual fulfillment items must belong to a single seller-aware delivery group"
                    .to_string(),
            ));
        }

        let shipping_option = match input.shipping_option_id {
            Some(shipping_option_id) => Some(
                fulfillment_service
                    .get_shipping_option(tenant_id, shipping_option_id, None, None)
                    .await?,
            ),
            None => None,
        };
        if let Some(shipping_option) = shipping_option.as_ref() {
            validate_shipping_option_against_order(
                shipping_option,
                order.currency_code.as_str(),
                canonical_group.shipping_profile_slug.as_str(),
            )?;
        }
        let provider_id = provider_id_for_shipping_option(shipping_option.as_ref());

        let mut items = Vec::with_capacity(requested_items.len());
        for item in requested_items {
            let line_item = order_line_items_by_id
                .get(&item.order_line_item_id)
                .ok_or_else(|| {
                    FulfillmentOrchestrationError::Validation(format!(
                        "order_line_item_id {} does not belong to order {}",
                        item.order_line_item_id, order.id
                    ))
                })?;
            let already_fulfilled = fulfilled_quantities
                .get(&item.order_line_item_id)
                .copied()
                .unwrap_or_default();
            let remaining_quantity = line_item
                .quantity
                .checked_sub(already_fulfilled)
                .ok_or_else(|| {
                    FulfillmentOrchestrationError::Validation(
                        "remaining fulfillment quantity overflowed".to_string(),
                    )
                })?;
            if remaining_quantity <= 0 {
                return Err(FulfillmentOrchestrationError::Validation(format!(
                    "order line item {} has no remaining quantity to fulfill",
                    item.order_line_item_id
                )));
            }
            if item.quantity > remaining_quantity {
                return Err(FulfillmentOrchestrationError::Validation(format!(
                    "requested quantity {} for order line item {} exceeds remaining quantity {}",
                    item.quantity, item.order_line_item_id, remaining_quantity
                )));
            }

            items.push(crate::dto::CreateFulfillmentItemInput {
                order_line_item_id: item.order_line_item_id,
                quantity: item.quantity,
                metadata: merge_metadata(
                    item.metadata,
                    serde_json::json!({
                        "shipping_profile_slug": canonical_group.shipping_profile_slug,
                        "seller_id": canonical_group.seller_id,
                        "post_order": {
                            "manual": true
                        }
                    }),
                ),
            });
        }

        let metadata = merge_metadata(
            input.metadata,
            serde_json::json!({
                "delivery_group": {
                    "shipping_profile_slug": canonical_group.shipping_profile_slug,
                    "seller_id": canonical_group.seller_id,
                    "order_line_item_ids": items
                        .iter()
                        .map(|item| item.order_line_item_id)
                        .collect::<Vec<_>>(),
                },
                "post_order": {
                    "manual": true
                }
            }),
        );

        let fulfillment = fulfillment_service
            .create_fulfillment(
                tenant_id,
                CreateFulfillmentInput {
                    order_id: input.order_id,
                    shipping_option_id: input.shipping_option_id,
                    customer_id: order.customer_id,
                    carrier: input.carrier,
                    tracking_number: input.tracking_number,
                    items: Some(items),
                    metadata,
                },
            )
            .await?;

        self.fulfillment_provider_registry
            .execute_create_label(
                provider_id.as_str(),
                FulfillmentProviderOperationRequest {
                    tenant_id,
                    fulfillment_id: fulfillment.id,
                    idempotency_key: Some(format!("fulfillment:{}:create_label", fulfillment.id)),
                    metadata: merge_metadata(
                        fulfillment.metadata.clone(),
                        serde_json::json!({
                            "commerce_orchestration": {
                                "operation": "create_label"
                            }
                        }),
                    ),
                },
            )
            .await
            .map_err(
                |source| FulfillmentOrchestrationError::ProviderAfterPersistence {
                    fulfillment_id: fulfillment.id,
                    operation: "create_label",
                    source,
                },
            )?;

        Ok(fulfillment)
    }
}

#[derive(Clone)]
struct DeliveryGroupKey {
    shipping_profile_slug: String,
    seller_id: Option<String>,
}

fn provider_id_for_shipping_option(option: Option<&ShippingOptionResponse>) -> String {
    option
        .map(|option| option.provider_id.clone())
        .unwrap_or_else(|| MANUAL_FULFILLMENT_PROVIDER_ID.to_string())
}

fn validate_shipping_option_against_order(
    option: &ShippingOptionResponse,
    order_currency_code: &str,
    required_shipping_profile_slug: &str,
) -> FulfillmentOrchestrationResult<()> {
    if !option
        .currency_code
        .eq_ignore_ascii_case(order_currency_code)
    {
        return Err(FulfillmentOrchestrationError::Validation(format!(
            "shipping option {} uses currency {}, expected {}",
            option.id, option.currency_code, order_currency_code
        )));
    }

    let required_profiles = BTreeSet::from([required_shipping_profile_slug.to_string()]);
    if !is_shipping_option_compatible_with_profiles(option, &required_profiles) {
        return Err(FulfillmentOrchestrationError::Validation(format!(
            "shipping option {} is not compatible with shipping profile {}",
            option.id, required_shipping_profile_slug
        )));
    }

    Ok(())
}

fn merge_metadata(current: Value, patch: Value) -> Value {
    match (current, patch) {
        (Value::Object(mut current), Value::Object(patch)) => {
            for (key, value) in patch {
                current.insert(key, value);
            }
            Value::Object(current)
        }
        (_, patch) => patch,
    }
}

fn normalize_seller_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_owned())
}

fn seller_id_from_metadata(metadata: &Value) -> Option<String> {
    metadata
        .get("seller")
        .and_then(|seller| seller.get("id"))
        .and_then(Value::as_str)
        .and_then(|value| normalize_seller_id(Some(value)))
        .or_else(|| {
            metadata
                .get("seller_id")
                .and_then(Value::as_str)
                .and_then(|value| normalize_seller_id(Some(value)))
        })
}
