use chrono::Utc;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

use rustok_core::generate_id;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::dto::{
    CreateOrderInput, CreateOrderLineItemInput, ListOrdersInput, OrderLineItemResponse,
    OrderResponse,
};
use crate::entities;
use crate::error::{OrderError, OrderResult};

const STATUS_PENDING: &str = "pending";
const STATUS_CONFIRMED: &str = "confirmed";
const STATUS_PAID: &str = "paid";
const STATUS_SHIPPED: &str = "shipped";
const STATUS_DELIVERED: &str = "delivered";
const STATUS_CANCELLED: &str = "cancelled";

pub struct OrderService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl OrderService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id))]
    pub async fn create_order(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: CreateOrderInput,
    ) -> OrderResult<OrderResponse> {
        self.create_order_with_channel(tenant_id, actor_id, input, None, None)
            .await
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, channel_id = ?channel_id, channel_slug = ?channel_slug))]
    pub async fn create_order_with_channel(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: CreateOrderInput,
        channel_id: Option<Uuid>,
        channel_slug: Option<String>,
    ) -> OrderResult<OrderResponse> {
        input
            .validate()
            .map_err(|error| OrderError::Validation(error.to_string()))?;

        let currency_code = input.currency_code.trim().to_ascii_uppercase();
        if currency_code.len() != 3 {
            return Err(OrderError::Validation(
                "currency_code must be a 3-letter code".to_string(),
            ));
        }
        let channel_slug = channel_slug
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let mut total_amount = Decimal::ZERO;
        for item in &input.line_items {
            Self::validate_line_item(item)?;
            total_amount += item.unit_price * Decimal::from(item.quantity);
        }

        let order_id = generate_id();
        let now = Utc::now();
        let txn = self.db.begin().await?;

        entities::order::ActiveModel {
            id: Set(order_id),
            tenant_id: Set(tenant_id),
            channel_id: Set(channel_id),
            channel_slug: Set(channel_slug),
            customer_id: Set(input.customer_id),
            status: Set(STATUS_PENDING.to_string()),
            currency_code: Set(currency_code.clone()),
            total_amount: Set(total_amount),
            metadata: Set(input.metadata.clone()),
            payment_id: Set(None),
            payment_method: Set(None),
            tracking_number: Set(None),
            carrier: Set(None),
            cancellation_reason: Set(None),
            delivered_signature: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            confirmed_at: Set(None),
            paid_at: Set(None),
            shipped_at: Set(None),
            delivered_at: Set(None),
            cancelled_at: Set(None),
        }
        .insert(&txn)
        .await?;

        for item in &input.line_items {
            entities::order_line_item::ActiveModel {
                id: Set(generate_id()),
                order_id: Set(order_id),
                product_id: Set(item.product_id),
                variant_id: Set(item.variant_id),
                sku: Set(item.sku.clone()),
                title: Set(item.title.clone()),
                quantity: Set(item.quantity),
                unit_price: Set(item.unit_price),
                total_price: Set(item.unit_price * Decimal::from(item.quantity)),
                currency_code: Set(currency_code.clone()),
                metadata: Set(item.metadata.clone()),
                created_at: Set(now.into()),
            }
            .insert(&txn)
            .await?;
        }

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                Some(actor_id),
                DomainEvent::OrderPlaced {
                    order_id,
                    customer_id: input.customer_id,
                    total: decimal_to_minor_units(total_amount).unwrap_or(0),
                    currency: currency_code,
                },
            )
            .await?;

        txn.commit().await?;
        self.get_order(tenant_id, order_id).await
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, order_id = %order_id))]
    pub async fn get_order(&self, tenant_id: Uuid, order_id: Uuid) -> OrderResult<OrderResponse> {
        let order = self.load_order_model(tenant_id, order_id).await?;
        self.build_response(order).await
    }

    pub async fn list_orders(
        &self,
        tenant_id: Uuid,
        input: ListOrdersInput,
    ) -> OrderResult<(Vec<OrderResponse>, u64)> {
        let page = input.page.max(1);
        let per_page = input.per_page.clamp(1, 100);

        let mut query =
            entities::order::Entity::find().filter(entities::order::Column::TenantId.eq(tenant_id));

        if let Some(status) = input
            .status
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            query = query.filter(entities::order::Column::Status.eq(status.trim()));
        }
        if let Some(customer_id) = input.customer_id {
            query = query.filter(entities::order::Column::CustomerId.eq(customer_id));
        }

        let total = query.clone().count(&self.db).await?;
        let orders = query
            .order_by_desc(entities::order::Column::CreatedAt)
            .offset((page - 1) * per_page)
            .limit(per_page)
            .all(&self.db)
            .await?;

        let mut items = Vec::with_capacity(orders.len());
        for order in orders {
            items.push(self.build_response(order).await?);
        }

        Ok((items, total))
    }

    pub async fn confirm_order(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        order_id: Uuid,
    ) -> OrderResult<OrderResponse> {
        self.transition_order(
            tenant_id,
            actor_id,
            order_id,
            STATUS_PENDING,
            STATUS_CONFIRMED,
            |active, now| {
                active.confirmed_at = Set(Some(now.into()));
            },
        )
        .await
    }

    pub async fn mark_paid(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        order_id: Uuid,
        payment_id: String,
        payment_method: String,
    ) -> OrderResult<OrderResponse> {
        if payment_id.trim().is_empty() || payment_method.trim().is_empty() {
            return Err(OrderError::Validation(
                "payment_id and payment_method are required".to_string(),
            ));
        }

        self.transition_order(
            tenant_id,
            actor_id,
            order_id,
            STATUS_CONFIRMED,
            STATUS_PAID,
            move |active, now| {
                active.payment_id = Set(Some(payment_id.clone()));
                active.payment_method = Set(Some(payment_method.clone()));
                active.paid_at = Set(Some(now.into()));
            },
        )
        .await
    }

    pub async fn ship_order(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        order_id: Uuid,
        tracking_number: String,
        carrier: String,
    ) -> OrderResult<OrderResponse> {
        if tracking_number.trim().is_empty() || carrier.trim().is_empty() {
            return Err(OrderError::Validation(
                "tracking_number and carrier are required".to_string(),
            ));
        }

        self.transition_order(
            tenant_id,
            actor_id,
            order_id,
            STATUS_PAID,
            STATUS_SHIPPED,
            move |active, now| {
                active.tracking_number = Set(Some(tracking_number.clone()));
                active.carrier = Set(Some(carrier.clone()));
                active.shipped_at = Set(Some(now.into()));
            },
        )
        .await
    }

    pub async fn deliver_order(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        order_id: Uuid,
        delivered_signature: Option<String>,
    ) -> OrderResult<OrderResponse> {
        let txn = self.db.begin().await?;
        let existing = self
            .load_order_model_in_tx(&txn, tenant_id, order_id)
            .await?;
        if existing.status != STATUS_SHIPPED {
            return Err(OrderError::InvalidTransition {
                from: existing.status,
                to: STATUS_DELIVERED.to_string(),
            });
        }

        let mut active: entities::order::ActiveModel = existing.into();
        let old_status = active.status.clone().take().unwrap_or_default();
        let now = Utc::now();
        active.status = Set(STATUS_DELIVERED.to_string());
        active.delivered_signature = Set(delivered_signature);
        active.delivered_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());
        active.update(&txn).await?;

        self.publish_status_changed(
            &txn,
            tenant_id,
            actor_id,
            order_id,
            &old_status,
            STATUS_DELIVERED,
        )
        .await?;
        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                Some(actor_id),
                DomainEvent::OrderCompleted { order_id },
            )
            .await?;

        txn.commit().await?;
        self.get_order(tenant_id, order_id).await
    }

    pub async fn cancel_order(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        order_id: Uuid,
        reason: Option<String>,
    ) -> OrderResult<OrderResponse> {
        let txn = self.db.begin().await?;
        let existing = self
            .load_order_model_in_tx(&txn, tenant_id, order_id)
            .await?;
        if !can_cancel(&existing.status) {
            return Err(OrderError::InvalidTransition {
                from: existing.status,
                to: STATUS_CANCELLED.to_string(),
            });
        }

        let mut active: entities::order::ActiveModel = existing.into();
        let old_status = active.status.clone().take().unwrap_or_default();
        let now = Utc::now();
        let cancel_reason = reason.filter(|value| !value.trim().is_empty());
        active.status = Set(STATUS_CANCELLED.to_string());
        active.cancellation_reason = Set(cancel_reason.clone());
        active.cancelled_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());
        active.update(&txn).await?;

        self.publish_status_changed(
            &txn,
            tenant_id,
            actor_id,
            order_id,
            &old_status,
            STATUS_CANCELLED,
        )
        .await?;
        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                Some(actor_id),
                DomainEvent::OrderCancelled {
                    order_id,
                    reason: cancel_reason,
                },
            )
            .await?;

        txn.commit().await?;
        self.get_order(tenant_id, order_id).await
    }

    async fn transition_order<F>(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        order_id: Uuid,
        expected_from: &str,
        next_status: &str,
        mutate: F,
    ) -> OrderResult<OrderResponse>
    where
        F: FnOnce(&mut entities::order::ActiveModel, chrono::DateTime<Utc>),
    {
        let txn = self.db.begin().await?;
        let existing = self
            .load_order_model_in_tx(&txn, tenant_id, order_id)
            .await?;
        if existing.status != expected_from {
            return Err(OrderError::InvalidTransition {
                from: existing.status,
                to: next_status.to_string(),
            });
        }

        let mut active: entities::order::ActiveModel = existing.into();
        let old_status = active.status.clone().take().unwrap_or_default();
        let now = Utc::now();
        active.status = Set(next_status.to_string());
        active.updated_at = Set(now.into());
        mutate(&mut active, now);
        active.update(&txn).await?;

        self.publish_status_changed(
            &txn,
            tenant_id,
            actor_id,
            order_id,
            &old_status,
            next_status,
        )
        .await?;

        txn.commit().await?;
        self.get_order(tenant_id, order_id).await
    }

    async fn publish_status_changed<C>(
        &self,
        txn: &C,
        tenant_id: Uuid,
        actor_id: Uuid,
        order_id: Uuid,
        old_status: &str,
        new_status: &str,
    ) -> OrderResult<()>
    where
        C: sea_orm::ConnectionTrait,
    {
        self.event_bus
            .publish_in_tx(
                txn,
                tenant_id,
                Some(actor_id),
                DomainEvent::OrderStatusChanged {
                    order_id,
                    old_status: old_status.to_string(),
                    new_status: new_status.to_string(),
                },
            )
            .await?;

        Ok(())
    }

    async fn load_order_model(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
    ) -> OrderResult<entities::order::Model> {
        self.load_order_model_in_tx(&self.db, tenant_id, order_id)
            .await
    }

    async fn load_order_model_in_tx<C>(
        &self,
        conn: &C,
        tenant_id: Uuid,
        order_id: Uuid,
    ) -> OrderResult<entities::order::Model>
    where
        C: sea_orm::ConnectionTrait,
    {
        entities::order::Entity::find_by_id(order_id)
            .filter(entities::order::Column::TenantId.eq(tenant_id))
            .one(conn)
            .await?
            .ok_or(OrderError::OrderNotFound(order_id))
    }

    async fn build_response(&self, order: entities::order::Model) -> OrderResult<OrderResponse> {
        let line_items = entities::order_line_item::Entity::find()
            .filter(entities::order_line_item::Column::OrderId.eq(order.id))
            .order_by_asc(entities::order_line_item::Column::CreatedAt)
            .all(&self.db)
            .await?;

        Ok(OrderResponse {
            id: order.id,
            tenant_id: order.tenant_id,
            channel_id: order.channel_id,
            channel_slug: order.channel_slug,
            customer_id: order.customer_id,
            status: order.status,
            currency_code: order.currency_code,
            total_amount: order.total_amount,
            metadata: order.metadata,
            payment_id: order.payment_id,
            payment_method: order.payment_method,
            tracking_number: order.tracking_number,
            carrier: order.carrier,
            cancellation_reason: order.cancellation_reason,
            delivered_signature: order.delivered_signature,
            created_at: order.created_at.with_timezone(&Utc),
            updated_at: order.updated_at.with_timezone(&Utc),
            confirmed_at: order.confirmed_at.map(|value| value.with_timezone(&Utc)),
            paid_at: order.paid_at.map(|value| value.with_timezone(&Utc)),
            shipped_at: order.shipped_at.map(|value| value.with_timezone(&Utc)),
            delivered_at: order.delivered_at.map(|value| value.with_timezone(&Utc)),
            cancelled_at: order.cancelled_at.map(|value| value.with_timezone(&Utc)),
            line_items: line_items
                .into_iter()
                .map(|item| OrderLineItemResponse {
                    id: item.id,
                    order_id: item.order_id,
                    product_id: item.product_id,
                    variant_id: item.variant_id,
                    sku: item.sku,
                    title: item.title,
                    quantity: item.quantity,
                    unit_price: item.unit_price,
                    total_price: item.total_price,
                    currency_code: item.currency_code,
                    metadata: item.metadata,
                    created_at: item.created_at.with_timezone(&Utc),
                })
                .collect(),
        })
    }

    fn validate_line_item(item: &CreateOrderLineItemInput) -> OrderResult<()> {
        item.validate()
            .map_err(|error| OrderError::Validation(error.to_string()))?;
        if item.unit_price < Decimal::ZERO {
            return Err(OrderError::Validation(
                "unit_price cannot be negative".to_string(),
            ));
        }
        Ok(())
    }
}

fn can_cancel(status: &str) -> bool {
    matches!(
        status,
        STATUS_PENDING | STATUS_CONFIRMED | STATUS_PAID | STATUS_SHIPPED
    )
}

fn decimal_to_minor_units(amount: Decimal) -> Option<i64> {
    (amount.round_dp(2) * Decimal::from(100)).to_i64()
}
