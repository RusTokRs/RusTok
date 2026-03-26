use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

use rustok_core::generate_id;

use crate::dto::{
    CancelFulfillmentInput, CreateFulfillmentInput, CreateShippingOptionInput,
    DeliverFulfillmentInput, FulfillmentResponse, ShipFulfillmentInput, ShippingOptionResponse,
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

        let currency_code = normalize_currency_code(&input.currency_code)?;
        if input.amount < Decimal::ZERO {
            return Err(FulfillmentError::Validation(
                "amount cannot be negative".to_string(),
            ));
        }
        let provider_id = input
            .provider_id
            .map(|provider_id| provider_id.trim().to_string())
            .filter(|provider_id| !provider_id.is_empty())
            .unwrap_or_else(|| MANUAL_PROVIDER_ID.to_string());

        let shipping_option_id = generate_id();
        let now = Utc::now();

        entities::shipping_option::ActiveModel {
            id: Set(shipping_option_id),
            tenant_id: Set(tenant_id),
            name: Set(input.name),
            currency_code: Set(currency_code),
            amount: Set(input.amount),
            provider_id: Set(provider_id),
            active: Set(true),
            metadata: Set(input.metadata),
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

fn map_shipping_option(option: entities::shipping_option::Model) -> ShippingOptionResponse {
    ShippingOptionResponse {
        id: option.id,
        tenant_id: option.tenant_id,
        name: option.name,
        currency_code: option.currency_code,
        amount: option.amount,
        provider_id: option.provider_id,
        active: option.active,
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
