use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

use rustok_core::generate_id;

use crate::dto::{
    CancelFulfillmentInput, CreateFulfillmentInput, CreateShippingOptionInput,
    DeliverFulfillmentInput, FulfillmentResponse, ListFulfillmentsInput, ShipFulfillmentInput,
    ShippingOptionResponse, UpdateShippingOptionInput,
};
use crate::entities;
use crate::error::{FulfillmentError, FulfillmentResult};

const STATUS_PENDING: &str = "pending";
const STATUS_SHIPPED: &str = "shipped";
const STATUS_DELIVERED: &str = "delivered";
const STATUS_CANCELLED: &str = "cancelled";
const MANUAL_PROVIDER_ID: &str = "manual";

pub struct FulfillmentService {
    db: DatabaseConnection,
}

impl FulfillmentService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id))]
    pub async fn create_shipping_option(
        &self,
        tenant_id: Uuid,
        input: CreateShippingOptionInput,
    ) -> FulfillmentResult<ShippingOptionResponse> {
        input
            .validate()
            .map_err(|error| FulfillmentError::Validation(error.to_string()))?;

        let CreateShippingOptionInput {
            name,
            currency_code,
            amount,
            provider_id,
            allowed_shipping_profile_slugs,
            metadata,
        } = input;

        let currency_code = normalize_currency_code(&currency_code)?;
        if amount < Decimal::ZERO {
            return Err(FulfillmentError::Validation(
                "amount cannot be negative".to_string(),
            ));
        }
        let provider_id = provider_id
            .map(|provider_id| provider_id.trim().to_string())
            .filter(|provider_id| !provider_id.is_empty())
            .unwrap_or_else(|| MANUAL_PROVIDER_ID.to_string());
        let allowed_shipping_profile_slugs =
            normalize_allowed_shipping_profile_slugs(allowed_shipping_profile_slugs);
        let metadata =
            apply_allowed_shipping_profiles_to_metadata(metadata, allowed_shipping_profile_slugs);

        let shipping_option_id = generate_id();
        let now = Utc::now();

        entities::shipping_option::ActiveModel {
            id: Set(shipping_option_id),
            tenant_id: Set(tenant_id),
            name: Set(name),
            currency_code: Set(currency_code),
            amount: Set(amount),
            provider_id: Set(provider_id),
            active: Set(true),
            metadata: Set(metadata),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&self.db)
        .await?;

        self.get_shipping_option(tenant_id, shipping_option_id)
            .await
    }

    pub async fn list_shipping_options(
        &self,
        tenant_id: Uuid,
    ) -> FulfillmentResult<Vec<ShippingOptionResponse>> {
        let rows = entities::shipping_option::Entity::find()
            .filter(entities::shipping_option::Column::TenantId.eq(tenant_id))
            .filter(entities::shipping_option::Column::Active.eq(true))
            .order_by_asc(entities::shipping_option::Column::CreatedAt)
            .all(&self.db)
            .await?;

        Ok(rows.into_iter().map(map_shipping_option).collect())
    }

    pub async fn list_all_shipping_options(
        &self,
        tenant_id: Uuid,
    ) -> FulfillmentResult<Vec<ShippingOptionResponse>> {
        let rows = entities::shipping_option::Entity::find()
            .filter(entities::shipping_option::Column::TenantId.eq(tenant_id))
            .order_by_asc(entities::shipping_option::Column::CreatedAt)
            .all(&self.db)
            .await?;

        Ok(rows.into_iter().map(map_shipping_option).collect())
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, shipping_option_id = %shipping_option_id))]
    pub async fn update_shipping_option(
        &self,
        tenant_id: Uuid,
        shipping_option_id: Uuid,
        input: UpdateShippingOptionInput,
    ) -> FulfillmentResult<ShippingOptionResponse> {
        input
            .validate()
            .map_err(|error| FulfillmentError::Validation(error.to_string()))?;

        let UpdateShippingOptionInput {
            name,
            currency_code,
            amount,
            provider_id,
            allowed_shipping_profile_slugs,
            metadata,
        } = input;

        if let Some(amount) = amount {
            if amount < Decimal::ZERO {
                return Err(FulfillmentError::Validation(
                    "amount cannot be negative".to_string(),
                ));
            }
        }

        let shipping_option = entities::shipping_option::Entity::find_by_id(shipping_option_id)
            .filter(entities::shipping_option::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(FulfillmentError::ShippingOptionNotFound(shipping_option_id))?;
        let mut active: entities::shipping_option::ActiveModel = shipping_option.into();

        if let Some(name) = name {
            active.name = Set(name);
        }
        if let Some(currency_code) = currency_code {
            active.currency_code = Set(normalize_currency_code(&currency_code)?);
        }
        if let Some(amount) = amount {
            active.amount = Set(amount);
        }
        if let Some(provider_id) = provider_id {
            let provider_id = Some(provider_id)
                .map(|provider_id| provider_id.trim().to_string())
                .filter(|provider_id| !provider_id.is_empty())
                .unwrap_or_else(|| MANUAL_PROVIDER_ID.to_string());
            active.provider_id = Set(provider_id);
        }
        if metadata.is_some() || allowed_shipping_profile_slugs.is_some() {
            let current_metadata = active.metadata.clone().take().unwrap_or_default();
            let metadata = match metadata {
                Some(patch) => merge_metadata(current_metadata, patch),
                None => current_metadata,
            };
            active.metadata = Set(apply_allowed_shipping_profiles_to_metadata(
                metadata,
                normalize_allowed_shipping_profile_slugs(allowed_shipping_profile_slugs),
            ));
        }

        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        self.get_shipping_option(tenant_id, shipping_option_id)
            .await
    }

    pub async fn get_shipping_option(
        &self,
        tenant_id: Uuid,
        shipping_option_id: Uuid,
    ) -> FulfillmentResult<ShippingOptionResponse> {
        let option = entities::shipping_option::Entity::find_by_id(shipping_option_id)
            .filter(entities::shipping_option::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(FulfillmentError::ShippingOptionNotFound(shipping_option_id))?;
        Ok(map_shipping_option(option))
    }

    pub async fn deactivate_shipping_option(
        &self,
        tenant_id: Uuid,
        shipping_option_id: Uuid,
    ) -> FulfillmentResult<ShippingOptionResponse> {
        self.set_shipping_option_active(tenant_id, shipping_option_id, false)
            .await
    }

    pub async fn reactivate_shipping_option(
        &self,
        tenant_id: Uuid,
        shipping_option_id: Uuid,
    ) -> FulfillmentResult<ShippingOptionResponse> {
        self.set_shipping_option_active(tenant_id, shipping_option_id, true)
            .await
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id))]
    pub async fn create_fulfillment(
        &self,
        tenant_id: Uuid,
        input: CreateFulfillmentInput,
    ) -> FulfillmentResult<FulfillmentResponse> {
        input
            .validate()
            .map_err(|error| FulfillmentError::Validation(error.to_string()))?;

        if let Some(shipping_option_id) = input.shipping_option_id {
            self.get_shipping_option(tenant_id, shipping_option_id)
                .await?;
        }

        let fulfillment_id = generate_id();
        let now = Utc::now();
        entities::fulfillment::ActiveModel {
            id: Set(fulfillment_id),
            tenant_id: Set(tenant_id),
            order_id: Set(input.order_id),
            shipping_option_id: Set(input.shipping_option_id),
            customer_id: Set(input.customer_id),
            status: Set(STATUS_PENDING.to_string()),
            carrier: Set(input.carrier),
            tracking_number: Set(input.tracking_number),
            delivered_note: Set(None),
            cancellation_reason: Set(None),
            metadata: Set(input.metadata),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            shipped_at: Set(None),
            delivered_at: Set(None),
            cancelled_at: Set(None),
        }
        .insert(&self.db)
        .await?;

        self.get_fulfillment(tenant_id, fulfillment_id).await
    }

    pub async fn get_fulfillment(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
    ) -> FulfillmentResult<FulfillmentResponse> {
        let fulfillment = self.load_fulfillment(tenant_id, fulfillment_id).await?;
        Ok(map_fulfillment(fulfillment))
    }

    pub async fn find_by_order(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
    ) -> FulfillmentResult<Option<FulfillmentResponse>> {
        let fulfillment = entities::fulfillment::Entity::find()
            .filter(entities::fulfillment::Column::TenantId.eq(tenant_id))
            .filter(entities::fulfillment::Column::OrderId.eq(order_id))
            .order_by_desc(entities::fulfillment::Column::CreatedAt)
            .one(&self.db)
            .await?;

        Ok(fulfillment.map(map_fulfillment))
    }

    pub async fn list_fulfillments(
        &self,
        tenant_id: Uuid,
        input: ListFulfillmentsInput,
    ) -> FulfillmentResult<(Vec<FulfillmentResponse>, u64)> {
        let page = input.page.max(1);
        let per_page = input.per_page.clamp(1, 100);
        let offset = (page.saturating_sub(1)) * per_page;

        let mut query = entities::fulfillment::Entity::find()
            .filter(entities::fulfillment::Column::TenantId.eq(tenant_id));

        if let Some(status) = input.status {
            query = query.filter(entities::fulfillment::Column::Status.eq(status));
        }
        if let Some(order_id) = input.order_id {
            query = query.filter(entities::fulfillment::Column::OrderId.eq(order_id));
        }
        if let Some(customer_id) = input.customer_id {
            query = query.filter(entities::fulfillment::Column::CustomerId.eq(customer_id));
        }

        let total = query.clone().count(&self.db).await?;
        let rows = query
            .order_by_desc(entities::fulfillment::Column::CreatedAt)
            .offset(offset)
            .limit(per_page)
            .all(&self.db)
            .await?;

        Ok((rows.into_iter().map(map_fulfillment).collect(), total))
    }

    pub async fn ship_fulfillment(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        input: ShipFulfillmentInput,
    ) -> FulfillmentResult<FulfillmentResponse> {
        input
            .validate()
            .map_err(|error| FulfillmentError::Validation(error.to_string()))?;

        let fulfillment = self.load_fulfillment(tenant_id, fulfillment_id).await?;
        if fulfillment.status != STATUS_PENDING {
            return Err(FulfillmentError::InvalidTransition {
                from: fulfillment.status,
                to: STATUS_SHIPPED.to_string(),
            });
        }

        let mut active: entities::fulfillment::ActiveModel = fulfillment.into();
        let now = Utc::now();
        let metadata = active.metadata.clone().take().unwrap_or_default();
        active.status = Set(STATUS_SHIPPED.to_string());
        active.carrier = Set(Some(input.carrier));
        active.tracking_number = Set(Some(input.tracking_number));
        active.metadata = Set(merge_metadata(metadata, input.metadata));
        active.shipped_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());
        active.update(&self.db).await?;

        self.get_fulfillment(tenant_id, fulfillment_id).await
    }

    pub async fn deliver_fulfillment(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        input: DeliverFulfillmentInput,
    ) -> FulfillmentResult<FulfillmentResponse> {
        let fulfillment = self.load_fulfillment(tenant_id, fulfillment_id).await?;
        if fulfillment.status != STATUS_SHIPPED {
            return Err(FulfillmentError::InvalidTransition {
                from: fulfillment.status,
                to: STATUS_DELIVERED.to_string(),
            });
        }

        let mut active: entities::fulfillment::ActiveModel = fulfillment.into();
        let now = Utc::now();
        let metadata = active.metadata.clone().take().unwrap_or_default();
        active.status = Set(STATUS_DELIVERED.to_string());
        active.delivered_note = Set(input.delivered_note);
        active.metadata = Set(merge_metadata(metadata, input.metadata));
        active.delivered_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());
        active.update(&self.db).await?;

        self.get_fulfillment(tenant_id, fulfillment_id).await
    }

    pub async fn cancel_fulfillment(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        input: CancelFulfillmentInput,
    ) -> FulfillmentResult<FulfillmentResponse> {
        let fulfillment = self.load_fulfillment(tenant_id, fulfillment_id).await?;
        if fulfillment.status == STATUS_DELIVERED || fulfillment.status == STATUS_CANCELLED {
            return Err(FulfillmentError::InvalidTransition {
                from: fulfillment.status,
                to: STATUS_CANCELLED.to_string(),
            });
        }

        let mut active: entities::fulfillment::ActiveModel = fulfillment.into();
        let now = Utc::now();
        let metadata = active.metadata.clone().take().unwrap_or_default();
        active.status = Set(STATUS_CANCELLED.to_string());
        active.cancellation_reason = Set(input.reason);
        active.metadata = Set(merge_metadata(metadata, input.metadata));
        active.cancelled_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());
        active.update(&self.db).await?;

        self.get_fulfillment(tenant_id, fulfillment_id).await
    }

    async fn load_fulfillment(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
    ) -> FulfillmentResult<entities::fulfillment::Model> {
        entities::fulfillment::Entity::find_by_id(fulfillment_id)
            .filter(entities::fulfillment::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(FulfillmentError::FulfillmentNotFound(fulfillment_id))
    }

    async fn set_shipping_option_active(
        &self,
        tenant_id: Uuid,
        shipping_option_id: Uuid,
        active: bool,
    ) -> FulfillmentResult<ShippingOptionResponse> {
        let shipping_option = entities::shipping_option::Entity::find_by_id(shipping_option_id)
            .filter(entities::shipping_option::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(FulfillmentError::ShippingOptionNotFound(shipping_option_id))?;

        let mut option: entities::shipping_option::ActiveModel = shipping_option.into();
        option.active = Set(active);
        option.updated_at = Set(Utc::now().into());
        option.update(&self.db).await?;

        self.get_shipping_option(tenant_id, shipping_option_id)
            .await
    }
}

fn normalize_currency_code(value: &str) -> FulfillmentResult<String> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.len() != 3 {
        return Err(FulfillmentError::Validation(
            "currency_code must be a 3-letter code".to_string(),
        ));
    }
    Ok(normalized)
}

fn merge_metadata(current: serde_json::Value, patch: serde_json::Value) -> serde_json::Value {
    match (current, patch) {
        (serde_json::Value::Object(mut current), serde_json::Value::Object(patch)) => {
            for (key, value) in patch {
                current.insert(key, value);
            }
            serde_json::Value::Object(current)
        }
        (_, patch) => patch,
    }
}

fn normalize_shipping_profile_slug(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn normalize_allowed_shipping_profile_slugs(values: Option<Vec<String>>) -> Option<Vec<String>> {
    values.map(|values| {
        values
            .into_iter()
            .filter_map(|value| normalize_shipping_profile_slug(&value))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    })
}

fn extract_allowed_shipping_profile_slugs(metadata: &Value) -> Option<Vec<String>> {
    metadata
        .get("shipping_profiles")
        .and_then(|profiles| profiles.get("allowed_slugs"))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .filter_map(normalize_shipping_profile_slug)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect()
        })
}

fn apply_allowed_shipping_profiles_to_metadata(
    metadata: Value,
    allowed_shipping_profile_slugs: Option<Vec<String>>,
) -> Value {
    let Some(allowed_shipping_profile_slugs) = allowed_shipping_profile_slugs else {
        return metadata;
    };

    let mut metadata_object = match metadata {
        Value::Object(object) => object,
        _ => Map::new(),
    };
    let mut shipping_profiles = match metadata_object.remove("shipping_profiles") {
        Some(Value::Object(object)) => object,
        _ => Map::new(),
    };
    shipping_profiles.insert(
        "allowed_slugs".to_string(),
        Value::Array(
            allowed_shipping_profile_slugs
                .into_iter()
                .map(Value::String)
                .collect(),
        ),
    );
    metadata_object.insert(
        "shipping_profiles".to_string(),
        Value::Object(shipping_profiles),
    );
    Value::Object(metadata_object)
}

fn map_shipping_option(option: entities::shipping_option::Model) -> ShippingOptionResponse {
    ShippingOptionResponse {
        id: option.id,
        tenant_id: option.tenant_id,
        name: option.name,
        currency_code: option.currency_code,
        amount: option.amount,
        provider_id: option.provider_id,
        active: option.active,
        allowed_shipping_profile_slugs: extract_allowed_shipping_profile_slugs(&option.metadata),
        metadata: option.metadata,
        created_at: option.created_at.with_timezone(&Utc),
        updated_at: option.updated_at.with_timezone(&Utc),
    }
}

fn map_fulfillment(fulfillment: entities::fulfillment::Model) -> FulfillmentResponse {
    FulfillmentResponse {
        id: fulfillment.id,
        tenant_id: fulfillment.tenant_id,
        order_id: fulfillment.order_id,
        shipping_option_id: fulfillment.shipping_option_id,
        customer_id: fulfillment.customer_id,
        status: fulfillment.status,
        carrier: fulfillment.carrier,
        tracking_number: fulfillment.tracking_number,
        delivered_note: fulfillment.delivered_note,
        cancellation_reason: fulfillment.cancellation_reason,
        metadata: fulfillment.metadata,
        created_at: fulfillment.created_at.with_timezone(&Utc),
        updated_at: fulfillment.updated_at.with_timezone(&Utc),
        shipped_at: fulfillment
            .shipped_at
            .map(|value| value.with_timezone(&Utc)),
        delivered_at: fulfillment
            .delivered_at
            .map(|value| value.with_timezone(&Utc)),
        cancelled_at: fulfillment
            .cancelled_at
            .map(|value| value.with_timezone(&Utc)),
    }
}
