use chrono::Utc;
use flex::{persist_localized_values, prepare_attached_values_create, resolve_attached_payload};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, Statement, TransactionTrait,
};
use serde_json::Value;
use std::collections::HashSet;
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

use rustok_core::field_schema::{CustomFieldsSchema, FieldDefinition, FieldType, ValidationRule};
use rustok_core::{generate_id, normalize_locale_tag, PLATFORM_FALLBACK_LOCALE};
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::dto::{
    CreateOrderAdjustmentInput, CreateOrderInput, CreateOrderLineItemInput,
    CreateOrderTaxLineInput, ListOrdersInput, OrderAdjustmentResponse, OrderLineItemResponse,
    OrderResponse, OrderTaxLineResponse,
};
use crate::entities;
use crate::error::{OrderError, OrderResult};

const STATUS_PENDING: &str = "pending";
const STATUS_CONFIRMED: &str = "confirmed";
const STATUS_PAID: &str = "paid";
const STATUS_SHIPPED: &str = "shipped";
const STATUS_DELIVERED: &str = "delivered";
const STATUS_CANCELLED: &str = "cancelled";

mod order_field_definitions_storage {
    rustok_core::define_field_definitions_entity!("order_field_definitions");
}

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
        let preferred_locale = Self::preferred_order_locale_from_metadata(&input.metadata)
            .unwrap_or(load_tenant_default_locale(&self.db, tenant_id).await?);
        let prepared_custom_fields = self
            .prepare_order_custom_fields_for_create(
                tenant_id,
                preferred_locale.as_str(),
                input.metadata.clone(),
            )
            .await?;
        let order_metadata = prepared_custom_fields
            .metadata
            .clone()
            .unwrap_or_else(|| serde_json::json!({}));

        let mut subtotal_amount = Decimal::ZERO;
        for item in &input.line_items {
            Self::validate_line_item(item)?;
            subtotal_amount += item.unit_price * Decimal::from(item.quantity);
        }
        if input.shipping_total < Decimal::ZERO {
            return Err(OrderError::Validation(
                "shipping_total cannot be negative".to_string(),
            ));
        }
        let adjustment_total =
            Self::validate_adjustments(&input.adjustments, input.line_items.len())?;
        let tax_total = Self::validate_tax_lines(
            &input.tax_lines,
            input.line_items.len(),
            currency_code.as_str(),
        )?;
        let tax_included = input.tax_lines.iter().any(|line| {
            line.metadata
                .get("tax_included")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        });
        if adjustment_total > subtotal_amount {
            return Err(OrderError::Validation(
                "adjustment total cannot exceed order subtotal".to_string(),
            ));
        }
        let base_total = subtotal_amount - adjustment_total + input.shipping_total;
        let total_amount = if tax_included {
            base_total
        } else {
            base_total + tax_total
        };

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
            shipping_total: Set(input.shipping_total),
            total_amount: Set(total_amount),
            tax_total: Set(tax_total),
            tax_included: Set(tax_included),
            metadata: Set(order_metadata),
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

        if let (Some(locale), Some(values)) = (
            prepared_custom_fields.locale.as_deref(),
            prepared_custom_fields.localized_values.as_ref(),
        ) {
            persist_localized_values(&txn, tenant_id, "order", order_id, locale, values)
                .await
                .map_err(|error| OrderError::Validation(error.to_string()))?;
        }

        let mut order_line_item_ids = Vec::with_capacity(input.line_items.len());
        for item in &input.line_items {
            let order_line_item_id = generate_id();
            let item_metadata = sanitize_line_item_metadata(item.metadata.clone());
            entities::order_line_item::ActiveModel {
                id: Set(order_line_item_id),
                order_id: Set(order_id),
                product_id: Set(item.product_id),
                variant_id: Set(item.variant_id),
                shipping_profile_slug: Set(item.shipping_profile_slug.clone()),
                seller_id: Set(normalize_seller_id(item.seller_id.as_deref())),
                sku: Set(item.sku.clone()),
                quantity: Set(item.quantity),
                unit_price: Set(item.unit_price),
                total_price: Set(item.unit_price * Decimal::from(item.quantity)),
                currency_code: Set(currency_code.clone()),
                metadata: Set(item_metadata),
                created_at: Set(now.into()),
            }
            .insert(&txn)
            .await?;
            order_line_item_ids.push(order_line_item_id);

            entities::order_line_item_translation::ActiveModel {
                id: Set(generate_id()),
                order_line_item_id: Set(order_line_item_id),
                locale: Set(preferred_locale.clone()),
                title: Set(item.title.clone()),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(&txn)
            .await?;
        }

        for adjustment in &input.adjustments {
            entities::order_adjustment::ActiveModel {
                id: Set(generate_id()),
                order_id: Set(order_id),
                order_line_item_id: Set(adjustment
                    .line_item_index
                    .map(|index| order_line_item_ids[index])),
                source_type: Set(normalize_adjustment_source_type(&adjustment.source_type)?),
                source_id: Set(normalize_adjustment_source_id(
                    adjustment.source_id.as_deref(),
                )),
                amount: Set(adjustment.amount),
                currency_code: Set(currency_code.clone()),
                metadata: Set(sanitize_adjustment_metadata(adjustment.metadata.clone())),
                created_at: Set(now.into()),
            }
            .insert(&txn)
            .await?;
        }

        for tax_line in &input.tax_lines {
            entities::order_tax_line::ActiveModel {
                id: Set(generate_id()),
                order_id: Set(order_id),
                order_line_item_id: Set(tax_line
                    .line_item_index
                    .map(|index| order_line_item_ids[index])),
                shipping_option_id: Set(tax_line.shipping_option_id),
                description: Set(normalize_tax_line_description(
                    tax_line.description.as_deref(),
                )),
                provider_id: Set(normalize_tax_provider_id(&tax_line.provider_id)?),
                rate: Set(tax_line.rate),
                amount: Set(tax_line.amount),
                currency_code: Set(currency_code.clone()),
                metadata: Set(sanitize_tax_line_metadata(tax_line.metadata.clone())),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
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
        self.get_order_with_locale_fallback(tenant_id, order_id, preferred_locale.as_str(), None)
            .await
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, order_id = %order_id))]
    pub async fn get_order(&self, tenant_id: Uuid, order_id: Uuid) -> OrderResult<OrderResponse> {
        let default_locale = load_tenant_default_locale(&self.db, tenant_id).await?;
        self.get_order_with_locale_fallback(tenant_id, order_id, default_locale.as_str(), None)
            .await
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, order_id = %order_id, locale = %locale, fallback_locale = ?fallback_locale))]
    pub async fn get_order_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> OrderResult<OrderResponse> {
        let order = self.load_order_model(tenant_id, order_id).await?;
        self.build_response(order, locale, fallback_locale).await
    }

    pub async fn list_orders(
        &self,
        tenant_id: Uuid,
        input: ListOrdersInput,
    ) -> OrderResult<(Vec<OrderResponse>, u64)> {
        let default_locale = load_tenant_default_locale(&self.db, tenant_id).await?;
        self.list_orders_with_locale_fallback(tenant_id, input, default_locale.as_str(), None)
            .await
    }

    pub async fn list_orders_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        input: ListOrdersInput,
        locale: &str,
        fallback_locale: Option<&str>,
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
            items.push(self.build_response(order, locale, fallback_locale).await?);
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
        let preferred_locale = Self::preferred_order_locale_from_metadata(&existing.metadata)
            .unwrap_or(load_tenant_default_locale(&txn, tenant_id).await?);
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
        self.get_order_with_locale_fallback(tenant_id, order_id, preferred_locale.as_str(), None)
            .await
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
        let preferred_locale = Self::preferred_order_locale_from_metadata(&existing.metadata)
            .unwrap_or(load_tenant_default_locale(&txn, tenant_id).await?);
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
        self.get_order_with_locale_fallback(tenant_id, order_id, preferred_locale.as_str(), None)
            .await
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
        let preferred_locale = Self::preferred_order_locale_from_metadata(&existing.metadata)
            .unwrap_or(load_tenant_default_locale(&txn, tenant_id).await?);
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
        self.get_order_with_locale_fallback(tenant_id, order_id, preferred_locale.as_str(), None)
            .await
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

    async fn build_response(
        &self,
        order: entities::order::Model,
        preferred_locale: &str,
        fallback_locale: Option<&str>,
    ) -> OrderResult<OrderResponse> {
        let line_items = entities::order_line_item::Entity::find()
            .filter(entities::order_line_item::Column::OrderId.eq(order.id))
            .order_by_asc(entities::order_line_item::Column::CreatedAt)
            .all(&self.db)
            .await?;
        let title_map =
            load_line_item_titles(&self.db, &line_items, preferred_locale, fallback_locale).await?;
        let adjustments = entities::order_adjustment::Entity::find()
            .filter(entities::order_adjustment::Column::OrderId.eq(order.id))
            .order_by_asc(entities::order_adjustment::Column::CreatedAt)
            .all(&self.db)
            .await?;
        let tax_lines = entities::order_tax_line::Entity::find()
            .filter(entities::order_tax_line::Column::OrderId.eq(order.id))
            .order_by_asc(entities::order_tax_line::Column::CreatedAt)
            .all(&self.db)
            .await?;
        let resolved_metadata = self
            .resolve_order_metadata(
                order.tenant_id,
                order.id,
                preferred_locale,
                fallback_locale,
                &order.metadata,
            )
            .await?;
        let subtotal_amount = subtotal_amount(&line_items);
        let adjustment_total = adjustment_total(&adjustments);

        Ok(OrderResponse {
            id: order.id,
            tenant_id: order.tenant_id,
            channel_id: order.channel_id,
            channel_slug: order.channel_slug,
            customer_id: order.customer_id,
            status: order.status,
            currency_code: order.currency_code,
            subtotal_amount,
            adjustment_total,
            shipping_total: order.shipping_total,
            total_amount: order.total_amount,
            tax_total: order.tax_total,
            tax_included: order.tax_included,
            metadata: resolved_metadata,
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
                    shipping_profile_slug: item.shipping_profile_slug,
                    seller_id: item.seller_id,
                    sku: item.sku,
                    title: title_map.get(&item.id).cloned().unwrap_or_default(),
                    quantity: item.quantity,
                    unit_price: item.unit_price,
                    total_price: item.total_price,
                    currency_code: item.currency_code,
                    metadata: item.metadata,
                    created_at: item.created_at.with_timezone(&Utc),
                })
                .collect(),
            adjustments: adjustments
                .into_iter()
                .map(|adjustment| OrderAdjustmentResponse {
                    id: adjustment.id,
                    order_id: adjustment.order_id,
                    line_item_id: adjustment.order_line_item_id,
                    source_type: adjustment.source_type,
                    source_id: adjustment.source_id,
                    amount: adjustment.amount,
                    currency_code: adjustment.currency_code,
                    metadata: adjustment.metadata,
                    created_at: adjustment.created_at.with_timezone(&Utc),
                })
                .collect(),
            tax_lines: tax_lines
                .into_iter()
                .map(|line| OrderTaxLineResponse {
                    id: line.id,
                    order_id: line.order_id,
                    line_item_id: line.order_line_item_id,
                    shipping_option_id: line.shipping_option_id,
                    description: line.description,
                    provider_id: line.provider_id,
                    rate: line.rate,
                    amount: line.amount,
                    currency_code: line.currency_code,
                    metadata: line.metadata,
                    created_at: line.created_at.with_timezone(&Utc),
                    updated_at: line.updated_at.with_timezone(&Utc),
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

    fn validate_adjustments(
        adjustments: &[CreateOrderAdjustmentInput],
        line_item_count: usize,
    ) -> OrderResult<Decimal> {
        let mut adjustment_total = Decimal::ZERO;
        for adjustment in adjustments {
            adjustment
                .validate()
                .map_err(|error| OrderError::Validation(error.to_string()))?;
            if adjustment.amount <= Decimal::ZERO {
                return Err(OrderError::Validation(
                    "adjustment amount must be greater than zero".to_string(),
                ));
            }
            if let Some(index) = adjustment.line_item_index {
                if index >= line_item_count {
                    return Err(OrderError::Validation(format!(
                        "adjustment line_item_index {index} is out of range"
                    )));
                }
            }
            adjustment_total += adjustment.amount;
        }
        Ok(adjustment_total)
    }

    fn validate_tax_lines(
        tax_lines: &[CreateOrderTaxLineInput],
        line_item_count: usize,
        currency_code: &str,
    ) -> OrderResult<Decimal> {
        let mut tax_total = Decimal::ZERO;
        for tax_line in tax_lines {
            tax_line
                .validate()
                .map_err(|error| OrderError::Validation(error.to_string()))?;
            if tax_line.amount <= Decimal::ZERO {
                return Err(OrderError::Validation(
                    "tax line amount must be greater than zero".to_string(),
                ));
            }
            if tax_line.rate < Decimal::ZERO {
                return Err(OrderError::Validation(
                    "tax line rate must be zero or greater".to_string(),
                ));
            }
            if let Some(index) = tax_line.line_item_index {
                if index >= line_item_count {
                    return Err(OrderError::Validation(format!(
                        "tax line line_item_index {index} is out of range"
                    )));
                }
            }
            if tax_line.currency_code.trim().to_ascii_uppercase() != currency_code {
                return Err(OrderError::Validation(
                    "tax line currency_code must match order currency".to_string(),
                ));
            }
            tax_total += tax_line.amount;
        }
        Ok(tax_total)
    }

    async fn prepare_order_custom_fields_for_create(
        &self,
        tenant_id: Uuid,
        locale: &str,
        metadata: Value,
    ) -> OrderResult<flex::PreparedAttachedValuesWrite> {
        let schema = load_order_custom_fields_schema(&self.db, tenant_id).await?;
        if schema.active_definitions().is_empty() {
            return prepare_attached_values_create(schema, Some(metadata), locale)
                .map_err(|error| OrderError::Validation(error.to_string()));
        }

        let (reserved_metadata, custom_fields) = split_order_metadata_payload(&schema, &metadata);
        let prepared =
            prepare_attached_values_create(schema, Some(Value::Object(custom_fields)), locale)
                .map_err(|error| OrderError::Validation(error.to_string()))?;

        Ok(flex::PreparedAttachedValuesWrite {
            metadata: Some(merge_reserved_order_metadata(
                reserved_metadata,
                prepared.metadata,
            )),
            localized_values: prepared.localized_values,
            locale: prepared.locale,
        })
    }

    async fn resolve_order_metadata(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
        preferred_locale: &str,
        fallback_locale: Option<&str>,
        metadata: &Value,
    ) -> OrderResult<Value> {
        let schema = load_order_custom_fields_schema(&self.db, tenant_id).await?;
        let resolved = resolve_attached_payload(
            &self.db,
            flex::AttachedEntityRef {
                tenant_id,
                entity_type: "order",
                entity_id: order_id,
            },
            schema,
            metadata,
            preferred_locale,
            fallback_locale.unwrap_or(PLATFORM_FALLBACK_LOCALE),
        )
        .await
        .map_err(|error| OrderError::Validation(error.to_string()))?;

        Ok(resolved.unwrap_or_else(|| serde_json::json!({})))
    }

    fn preferred_order_locale_from_metadata(metadata: &Value) -> Option<String> {
        metadata
            .get("locale")
            .and_then(Value::as_str)
            .or_else(|| metadata.get("locale_code").and_then(Value::as_str))
            .or_else(|| {
                metadata
                    .get("cart_context")
                    .and_then(Value::as_object)
                    .and_then(|context| context.get("locale"))
                    .and_then(Value::as_str)
            })
            .or_else(|| {
                metadata
                    .get("context")
                    .and_then(Value::as_object)
                    .and_then(|context| context.get("locale"))
                    .and_then(Value::as_str)
            })
            .or_else(|| {
                metadata
                    .get("store_context")
                    .and_then(Value::as_object)
                    .and_then(|context| context.get("locale"))
                    .and_then(Value::as_str)
            })
            .and_then(normalize_locale_tag)
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

fn subtotal_amount(line_items: &[entities::order_line_item::Model]) -> Decimal {
    line_items
        .iter()
        .fold(Decimal::ZERO, |acc, item| acc + item.total_price)
}

fn adjustment_total(adjustments: &[entities::order_adjustment::Model]) -> Decimal {
    adjustments
        .iter()
        .fold(Decimal::ZERO, |acc, adjustment| acc + adjustment.amount)
}

async fn load_order_custom_fields_schema(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> OrderResult<CustomFieldsSchema> {
    let rows = order_field_definitions_storage::Entity::find()
        .filter(order_field_definitions_storage::Column::TenantId.eq(tenant_id))
        .filter(order_field_definitions_storage::Column::IsActive.eq(true))
        .order_by_asc(order_field_definitions_storage::Column::Position)
        .all(db)
        .await?;

    let definitions = rows
        .into_iter()
        .filter_map(order_field_definition_from_row)
        .collect();

    Ok(CustomFieldsSchema::new(definitions))
}

fn order_field_definition_from_row(
    row: order_field_definitions_storage::Model,
) -> Option<FieldDefinition> {
    let field_type: FieldType =
        serde_json::from_value(serde_json::Value::String(row.field_type.clone())).ok()?;
    let label = serde_json::from_value(row.label).unwrap_or_default();
    let description = row
        .description
        .and_then(|value| serde_json::from_value(value).ok());
    let validation: Option<ValidationRule> = row
        .validation
        .and_then(|value| serde_json::from_value(value).ok());

    Some(FieldDefinition {
        field_key: row.field_key,
        field_type,
        label,
        description,
        is_localized: row.is_localized,
        is_required: row.is_required,
        default_value: row.default_value,
        validation,
        position: row.position,
        is_active: row.is_active,
    })
}

fn split_order_metadata_payload(
    schema: &CustomFieldsSchema,
    metadata: &Value,
) -> (
    serde_json::Map<String, Value>,
    serde_json::Map<String, Value>,
) {
    let known_keys = schema
        .active_definitions()
        .into_iter()
        .map(|definition| definition.field_key.as_str())
        .collect::<HashSet<_>>();
    let mut reserved = serde_json::Map::new();
    let mut custom_fields = serde_json::Map::new();

    for (key, value) in metadata.as_object().cloned().unwrap_or_default() {
        if known_keys.contains(key.as_str()) {
            custom_fields.insert(key, value);
        } else {
            reserved.insert(key, value);
        }
    }

    (reserved, custom_fields)
}

fn merge_reserved_order_metadata(
    mut reserved: serde_json::Map<String, Value>,
    custom_fields: Option<Value>,
) -> Value {
    if let Some(custom_fields) = custom_fields.and_then(|value| value.as_object().cloned()) {
        for (key, value) in custom_fields {
            reserved.insert(key, value);
        }
    }

    Value::Object(reserved)
}

fn normalize_seller_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_owned())
}

fn normalize_adjustment_source_type(value: &str) -> OrderResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized.len() > 64 {
        return Err(OrderError::Validation(
            "adjustment source_type must be 1-64 characters".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_adjustment_source_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_owned())
}

fn sanitize_adjustment_metadata(metadata: Value) -> Value {
    let mut metadata = match metadata {
        Value::Object(object) => object,
        value => return value,
    };

    metadata.remove("label");
    metadata.remove("display_label");
    metadata.remove("localized_label");

    Value::Object(metadata)
}

fn sanitize_line_item_metadata(metadata: Value) -> Value {
    let mut metadata = match metadata {
        Value::Object(object) => object,
        value => return value,
    };

    metadata.remove("seller_label");

    if let Some(Value::Object(mut seller)) = metadata.remove("seller") {
        seller.remove("label");
        metadata.insert("seller".to_string(), Value::Object(seller));
    }

    Value::Object(metadata)
}

fn sanitize_tax_line_metadata(metadata: Value) -> Value {
    let mut metadata = match metadata {
        Value::Object(object) => object,
        value => return value,
    };

    metadata.remove("label");
    metadata.remove("display_label");
    metadata.remove("localized_label");
    metadata.remove("title");
    metadata.remove("name");
    metadata.remove("description");
    metadata.remove("seller_label");

    Value::Object(metadata)
}

fn normalize_tax_line_description(value: Option<&str>) -> Option<String> {
    let normalized = value.map(str::trim)?.to_ascii_lowercase();
    if normalized.is_empty() || normalized.len() > 64 {
        return None;
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    {
        return None;
    }
    Some(normalized)
}

fn normalize_tax_provider_id(value: &str) -> OrderResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized.len() > 64 {
        return Err(OrderError::Validation(
            "tax line provider_id must be 1-64 characters".to_string(),
        ));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        return Err(OrderError::Validation(
            "tax line provider_id must use lowercase ASCII, digits, underscore, or hyphen"
                .to_string(),
        ));
    }
    Ok(normalized)
}

async fn load_line_item_titles<C>(
    conn: &C,
    line_items: &[entities::order_line_item::Model],
    preferred_locale: &str,
    fallback_locale: Option<&str>,
) -> OrderResult<std::collections::HashMap<Uuid, String>>
where
    C: sea_orm::ConnectionTrait,
{
    let mut titles = std::collections::HashMap::new();
    if line_items.is_empty() {
        return Ok(titles);
    }
    let line_item_ids = line_items.iter().map(|item| item.id).collect::<Vec<_>>();
    let rows = entities::order_line_item_translation::Entity::find()
        .filter(
            entities::order_line_item_translation::Column::OrderLineItemId
                .is_in(line_item_ids.clone()),
        )
        .all(conn)
        .await?;

    let mut rows_by_item =
        std::collections::HashMap::<Uuid, Vec<entities::order_line_item_translation::Model>>::new();
    for row in rows {
        rows_by_item
            .entry(row.order_line_item_id)
            .or_default()
            .push(row);
    }

    for line_item in line_items {
        if let Some(title) = rows_by_item
            .remove(&line_item.id)
            .and_then(|rows| select_order_line_item_title(&rows, preferred_locale, fallback_locale))
        {
            titles.insert(line_item.id, title);
        }
    }

    Ok(titles)
}

fn select_order_line_item_title(
    rows: &[entities::order_line_item_translation::Model],
    preferred_locale: &str,
    fallback_locale: Option<&str>,
) -> Option<String> {
    let preferred_locale = normalize_locale_tag(preferred_locale)?;
    let fallback_locale = fallback_locale.and_then(normalize_locale_tag);

    rows.iter()
        .find(|row| normalize_locale_tag(&row.locale).as_deref() == Some(preferred_locale.as_str()))
        .or_else(|| {
            fallback_locale.as_deref().and_then(|fallback_locale| {
                rows.iter().find(|row| {
                    normalize_locale_tag(&row.locale).as_deref() == Some(fallback_locale)
                })
            })
        })
        .or_else(|| rows.first())
        .map(|row| row.title.clone())
}

async fn load_tenant_default_locale<C>(conn: &C, tenant_id: Uuid) -> OrderResult<String>
where
    C: ConnectionTrait,
{
    let row = conn
        .query_one(Statement::from_sql_and_values(
            conn.get_database_backend(),
            "SELECT default_locale FROM tenants WHERE id = ?",
            vec![tenant_id.into()],
        ))
        .await?;

    let default_locale = row
        .and_then(|row| row.try_get::<String>("", "default_locale").ok())
        .and_then(|locale| normalize_locale_tag(&locale))
        .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());

    Ok(default_locale)
}
