use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

use rustok_core::{generate_id, normalize_locale_tag};

use crate::dto::{
    CancelFulfillmentInput, CreateFulfillmentInput, CreateShippingOptionInput,
    DeliverFulfillmentInput, FulfillmentItemQuantityInput, FulfillmentItemResponse,
    FulfillmentResponse, ListFulfillmentsInput, ReopenFulfillmentInput, ReshipFulfillmentInput,
    ShipFulfillmentInput, ShippingOptionResponse, ShippingOptionTranslationInput,
    ShippingOptionTranslationResponse, UpdateShippingOptionInput,
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
            translations,
            currency_code,
            amount,
            provider_id,
            allowed_shipping_profile_slugs,
            metadata,
        } = input;

        let translations = normalize_translation_inputs(translations)?;
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

        insert_translations(&self.db, shipping_option_id, &translations).await?;

        self.get_shipping_option(tenant_id, shipping_option_id, None, None)
            .await
    }

    pub async fn list_shipping_options(
        &self,
        tenant_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> FulfillmentResult<Vec<ShippingOptionResponse>> {
        let rows = entities::shipping_option::Entity::find()
            .filter(entities::shipping_option::Column::TenantId.eq(tenant_id))
            .filter(entities::shipping_option::Column::Active.eq(true))
            .order_by_asc(entities::shipping_option::Column::CreatedAt)
            .all(&self.db)
            .await?;

        load_shipping_options_with_translations(
            &self.db,
            rows,
            requested_locale,
            tenant_default_locale,
        )
        .await
    }

    pub async fn list_all_shipping_options(
        &self,
        tenant_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> FulfillmentResult<Vec<ShippingOptionResponse>> {
        let rows = entities::shipping_option::Entity::find()
            .filter(entities::shipping_option::Column::TenantId.eq(tenant_id))
            .order_by_asc(entities::shipping_option::Column::CreatedAt)
            .all(&self.db)
            .await?;

        load_shipping_options_with_translations(
            &self.db,
            rows,
            requested_locale,
            tenant_default_locale,
        )
        .await
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
            translations,
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

        if let Some(translations) = translations {
            let normalized = normalize_translation_inputs(translations)?;
            replace_translations(&self.db, shipping_option_id, &normalized).await?;
        }

        self.get_shipping_option(tenant_id, shipping_option_id, None, None)
            .await
    }

    pub async fn get_shipping_option(
        &self,
        tenant_id: Uuid,
        shipping_option_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> FulfillmentResult<ShippingOptionResponse> {
        let option = entities::shipping_option::Entity::find_by_id(shipping_option_id)
            .filter(entities::shipping_option::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(FulfillmentError::ShippingOptionNotFound(shipping_option_id))?;
        let items = load_shipping_options_with_translations(
            &self.db,
            vec![option],
            requested_locale,
            tenant_default_locale,
        )
        .await?;
        items
            .into_iter()
            .next()
            .ok_or(FulfillmentError::ShippingOptionNotFound(shipping_option_id))
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
            self.get_shipping_option(tenant_id, shipping_option_id, None, None)
                .await?;
        }
        validate_fulfillment_items(input.items.as_deref())?;

        let fulfillment_id = generate_id();
        let now = Utc::now();
        let txn = self.db.begin().await?;
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
        .insert(&txn)
        .await?;

        if let Some(items) = input.items {
            for item in items {
                entities::fulfillment_item::ActiveModel {
                    id: Set(generate_id()),
                    fulfillment_id: Set(fulfillment_id),
                    order_line_item_id: Set(item.order_line_item_id),
                    quantity: Set(item.quantity),
                    shipped_quantity: Set(0),
                    delivered_quantity: Set(0),
                    metadata: Set(item.metadata),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&txn)
                .await?;
            }
        }

        txn.commit().await?;

        self.get_fulfillment(tenant_id, fulfillment_id).await
    }

    pub async fn get_fulfillment(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
    ) -> FulfillmentResult<FulfillmentResponse> {
        let fulfillment = self.load_fulfillment(tenant_id, fulfillment_id).await?;
        self.build_fulfillment_response(fulfillment).await
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

        match fulfillment {
            Some(fulfillment) => Ok(Some(self.build_fulfillment_response(fulfillment).await?)),
            None => Ok(None),
        }
    }

    pub async fn list_by_order(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
    ) -> FulfillmentResult<Vec<FulfillmentResponse>> {
        let rows = entities::fulfillment::Entity::find()
            .filter(entities::fulfillment::Column::TenantId.eq(tenant_id))
            .filter(entities::fulfillment::Column::OrderId.eq(order_id))
            .order_by_asc(entities::fulfillment::Column::CreatedAt)
            .all(&self.db)
            .await?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            items.push(self.build_fulfillment_response(row).await?);
        }
        Ok(items)
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

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            items.push(self.build_fulfillment_response(row).await?);
        }

        Ok((items, total))
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
        if !matches!(fulfillment.status.as_str(), STATUS_PENDING | STATUS_SHIPPED) {
            return Err(FulfillmentError::InvalidTransition {
                from: fulfillment.status,
                to: STATUS_SHIPPED.to_string(),
            });
        }
        let items = self.load_fulfillment_items(fulfillment.id).await?;
        if items.is_empty() {
            if fulfillment.status != STATUS_PENDING {
                return Err(FulfillmentError::InvalidTransition {
                    from: fulfillment.status,
                    to: STATUS_SHIPPED.to_string(),
                });
            }

            let mut active: entities::fulfillment::ActiveModel = fulfillment.into();
            let now = Utc::now();
            let carrier = input.carrier.clone();
            let tracking_number = input.tracking_number.clone();
            let metadata = active.metadata.clone().take().unwrap_or_default();
            active.status = Set(STATUS_SHIPPED.to_string());
            active.carrier = Set(Some(carrier.clone()));
            active.tracking_number = Set(Some(tracking_number.clone()));
            active.metadata = Set(append_audit_event(
                merge_metadata(metadata, input.metadata),
                build_fulfillment_audit_event(
                    FulfillmentItemAction::Ship,
                    now,
                    &[],
                    Some(carrier),
                    Some(tracking_number),
                    STATUS_SHIPPED,
                ),
            ));
            active.shipped_at = Set(Some(now.into()));
            active.updated_at = Set(now.into());
            active.update(&self.db).await?;

            return self.get_fulfillment(tenant_id, fulfillment_id).await;
        }

        validate_item_quantity_adjustments(input.items.as_deref())?;
        let now = Utc::now();
        let adjustment_plan =
            resolve_item_adjustments(&items, input.items.as_deref(), FulfillmentItemAction::Ship)?;
        let txn = self.db.begin().await?;
        let mut adjusted_items = Vec::with_capacity(items.len());
        let adjustment_lookup = adjustment_plan.into_iter().collect::<BTreeMap<Uuid, i32>>();
        let mut adjusted_entries = Vec::new();
        for item in items {
            let adjustment = adjustment_lookup.get(&item.id).copied().unwrap_or_default();
            let mut active: entities::fulfillment_item::ActiveModel = item.clone().into();
            if adjustment > 0 {
                let shipped_quantity = item.shipped_quantity + adjustment;
                active.shipped_quantity = Set(shipped_quantity);
                active.metadata = Set(append_audit_event(
                    item.metadata.clone(),
                    build_item_audit_event(FulfillmentItemAction::Ship, now, adjustment),
                ));
                active.updated_at = Set(now.into());
                adjusted_entries.push((item.id, item.order_line_item_id, adjustment));
            }
            let updated = active.update(&txn).await?;
            adjusted_items.push(updated);
        }

        let all_items_delivered = adjusted_items
            .iter()
            .all(|item| item.delivered_quantity >= item.quantity);
        let mut active: entities::fulfillment::ActiveModel = fulfillment.into();
        let metadata = active.metadata.clone().take().unwrap_or_default();
        active.status = Set(if all_items_delivered {
            STATUS_DELIVERED.to_string()
        } else {
            STATUS_SHIPPED.to_string()
        });
        active.carrier = Set(Some(input.carrier.clone()));
        active.tracking_number = Set(Some(input.tracking_number.clone()));
        active.metadata = Set(append_audit_event(
            merge_metadata(metadata, input.metadata),
            build_fulfillment_audit_event(
                FulfillmentItemAction::Ship,
                now,
                &adjusted_entries,
                Some(input.carrier),
                Some(input.tracking_number),
                active.status.clone().take().unwrap_or_default().as_str(),
            ),
        ));
        if active.shipped_at.clone().take().is_none() {
            active.shipped_at = Set(Some(now.into()));
        }
        active.updated_at = Set(now.into());
        active.update(&txn).await?;
        txn.commit().await?;

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
        let items = self.load_fulfillment_items(fulfillment.id).await?;
        if items.is_empty() {
            let mut active: entities::fulfillment::ActiveModel = fulfillment.into();
            let now = Utc::now();
            let metadata = active.metadata.clone().take().unwrap_or_default();
            active.status = Set(STATUS_DELIVERED.to_string());
            active.delivered_note = Set(input.delivered_note.clone());
            active.metadata = Set(append_audit_event(
                merge_metadata(metadata, input.metadata),
                build_fulfillment_audit_event(
                    FulfillmentItemAction::Deliver,
                    now,
                    &[],
                    None,
                    None,
                    STATUS_DELIVERED,
                ),
            ));
            active.delivered_at = Set(Some(now.into()));
            active.updated_at = Set(now.into());
            active.update(&self.db).await?;

            return self.get_fulfillment(tenant_id, fulfillment_id).await;
        }

        validate_item_quantity_adjustments(input.items.as_deref())?;
        let now = Utc::now();
        let adjustment_plan = resolve_item_adjustments(
            &items,
            input.items.as_deref(),
            FulfillmentItemAction::Deliver,
        )?;
        let txn = self.db.begin().await?;
        let mut adjusted_items = Vec::with_capacity(items.len());
        let adjustment_lookup = adjustment_plan.into_iter().collect::<BTreeMap<Uuid, i32>>();
        let mut adjusted_entries = Vec::new();
        for item in items {
            let adjustment = adjustment_lookup.get(&item.id).copied().unwrap_or_default();
            let mut active: entities::fulfillment_item::ActiveModel = item.clone().into();
            if adjustment > 0 {
                let delivered_quantity = item.delivered_quantity + adjustment;
                active.delivered_quantity = Set(delivered_quantity);
                active.metadata = Set(append_audit_event(
                    item.metadata.clone(),
                    build_item_audit_event(FulfillmentItemAction::Deliver, now, adjustment),
                ));
                active.updated_at = Set(now.into());
                adjusted_entries.push((item.id, item.order_line_item_id, adjustment));
            }
            let updated = active.update(&txn).await?;
            adjusted_items.push(updated);
        }

        let all_items_delivered = adjusted_items
            .iter()
            .all(|item| item.delivered_quantity >= item.quantity);
        let mut active: entities::fulfillment::ActiveModel = fulfillment.into();
        let metadata = active.metadata.clone().take().unwrap_or_default();
        active.status = Set(if all_items_delivered {
            STATUS_DELIVERED.to_string()
        } else {
            STATUS_SHIPPED.to_string()
        });
        active.delivered_note = Set(input.delivered_note.clone());
        active.metadata = Set(append_audit_event(
            merge_metadata(metadata, input.metadata),
            build_fulfillment_audit_event(
                FulfillmentItemAction::Deliver,
                now,
                &adjusted_entries,
                None,
                None,
                active.status.clone().take().unwrap_or_default().as_str(),
            ),
        ));
        if all_items_delivered {
            active.delivered_at = Set(Some(now.into()));
        }
        active.updated_at = Set(now.into());
        active.update(&txn).await?;
        txn.commit().await?;

        self.get_fulfillment(tenant_id, fulfillment_id).await
    }

    pub async fn reopen_fulfillment(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        input: ReopenFulfillmentInput,
    ) -> FulfillmentResult<FulfillmentResponse> {
        let fulfillment = self.load_fulfillment(tenant_id, fulfillment_id).await?;
        let now = Utc::now();

        match fulfillment.status.as_str() {
            STATUS_CANCELLED => {
                let items = self.load_fulfillment_items(fulfillment.id).await?;
                let mut active: entities::fulfillment::ActiveModel = fulfillment.into();
                let metadata = active.metadata.clone().take().unwrap_or_default();
                let status_after = reopened_status_for_cancelled(&items, &active);
                active.status = Set(status_after.to_string());
                active.cancellation_reason = Set(None);
                active.cancelled_at = Set(None);
                active.metadata = Set(append_audit_event(
                    merge_metadata(metadata, input.metadata),
                    build_fulfillment_audit_event(
                        FulfillmentItemAction::Reopen,
                        now,
                        &[],
                        None,
                        None,
                        status_after,
                    ),
                ));
                active.updated_at = Set(now.into());
                active.update(&self.db).await?;

                self.get_fulfillment(tenant_id, fulfillment_id).await
            }
            STATUS_DELIVERED => {
                let items = self.load_fulfillment_items(fulfillment.id).await?;
                if items.is_empty() {
                    let mut active: entities::fulfillment::ActiveModel = fulfillment.into();
                    let metadata = active.metadata.clone().take().unwrap_or_default();
                    active.status = Set(STATUS_SHIPPED.to_string());
                    active.delivered_note = Set(None);
                    active.delivered_at = Set(None);
                    active.metadata = Set(append_audit_event(
                        merge_metadata(metadata, input.metadata),
                        build_fulfillment_audit_event(
                            FulfillmentItemAction::Reopen,
                            now,
                            &[],
                            None,
                            None,
                            STATUS_SHIPPED,
                        ),
                    ));
                    active.updated_at = Set(now.into());
                    active.update(&self.db).await?;

                    return self.get_fulfillment(tenant_id, fulfillment_id).await;
                }

                validate_item_quantity_adjustments(input.items.as_deref())?;
                let adjustment_plan = resolve_item_adjustments(
                    &items,
                    input.items.as_deref(),
                    FulfillmentItemAction::Reopen,
                )?;
                let txn = self.db.begin().await?;
                let adjustment_lookup =
                    adjustment_plan.into_iter().collect::<BTreeMap<Uuid, i32>>();
                let mut adjusted_entries = Vec::new();
                for item in items {
                    let adjustment = adjustment_lookup.get(&item.id).copied().unwrap_or_default();
                    let mut active: entities::fulfillment_item::ActiveModel = item.clone().into();
                    if adjustment > 0 {
                        active.delivered_quantity = Set(item.delivered_quantity - adjustment);
                        active.metadata = Set(append_audit_event(
                            item.metadata.clone(),
                            build_item_audit_event(FulfillmentItemAction::Reopen, now, adjustment),
                        ));
                        active.updated_at = Set(now.into());
                        adjusted_entries.push((item.id, item.order_line_item_id, adjustment));
                    }
                    active.update(&txn).await?;
                }

                let mut active: entities::fulfillment::ActiveModel = fulfillment.into();
                let metadata = active.metadata.clone().take().unwrap_or_default();
                active.status = Set(STATUS_SHIPPED.to_string());
                active.delivered_note = Set(None);
                active.delivered_at = Set(None);
                active.metadata = Set(append_audit_event(
                    merge_metadata(metadata, input.metadata),
                    build_fulfillment_audit_event(
                        FulfillmentItemAction::Reopen,
                        now,
                        &adjusted_entries,
                        None,
                        None,
                        STATUS_SHIPPED,
                    ),
                ));
                active.updated_at = Set(now.into());
                active.update(&txn).await?;
                txn.commit().await?;

                self.get_fulfillment(tenant_id, fulfillment_id).await
            }
            status => Err(FulfillmentError::InvalidTransition {
                from: status.to_string(),
                to: "reopened".to_string(),
            }),
        }
    }

    pub async fn reship_fulfillment(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        input: ReshipFulfillmentInput,
    ) -> FulfillmentResult<FulfillmentResponse> {
        input
            .validate()
            .map_err(|error| FulfillmentError::Validation(error.to_string()))?;

        let fulfillment = self.load_fulfillment(tenant_id, fulfillment_id).await?;
        if fulfillment.status != STATUS_DELIVERED {
            return Err(FulfillmentError::InvalidTransition {
                from: fulfillment.status,
                to: STATUS_SHIPPED.to_string(),
            });
        }

        let items = self.load_fulfillment_items(fulfillment.id).await?;
        let now = Utc::now();
        if items.is_empty() {
            let mut active: entities::fulfillment::ActiveModel = fulfillment.into();
            let metadata = active.metadata.clone().take().unwrap_or_default();
            active.status = Set(STATUS_SHIPPED.to_string());
            active.carrier = Set(Some(input.carrier.clone()));
            active.tracking_number = Set(Some(input.tracking_number.clone()));
            active.delivered_note = Set(None);
            active.delivered_at = Set(None);
            active.metadata = Set(append_audit_event(
                merge_metadata(metadata, input.metadata),
                build_fulfillment_audit_event(
                    FulfillmentItemAction::Reship,
                    now,
                    &[],
                    Some(input.carrier),
                    Some(input.tracking_number),
                    STATUS_SHIPPED,
                ),
            ));
            active.updated_at = Set(now.into());
            active.update(&self.db).await?;

            return self.get_fulfillment(tenant_id, fulfillment_id).await;
        }

        validate_item_quantity_adjustments(input.items.as_deref())?;
        let adjustment_plan = resolve_item_adjustments(
            &items,
            input.items.as_deref(),
            FulfillmentItemAction::Reship,
        )?;
        let txn = self.db.begin().await?;
        let adjustment_lookup = adjustment_plan.into_iter().collect::<BTreeMap<Uuid, i32>>();
        let mut adjusted_entries = Vec::new();
        for item in items {
            let adjustment = adjustment_lookup.get(&item.id).copied().unwrap_or_default();
            let mut active: entities::fulfillment_item::ActiveModel = item.clone().into();
            if adjustment > 0 {
                active.delivered_quantity = Set(item.delivered_quantity - adjustment);
                active.metadata = Set(append_audit_event(
                    item.metadata.clone(),
                    build_item_audit_event(FulfillmentItemAction::Reship, now, adjustment),
                ));
                active.updated_at = Set(now.into());
                adjusted_entries.push((item.id, item.order_line_item_id, adjustment));
            }
            active.update(&txn).await?;
        }

        let mut active: entities::fulfillment::ActiveModel = fulfillment.into();
        let metadata = active.metadata.clone().take().unwrap_or_default();
        active.status = Set(STATUS_SHIPPED.to_string());
        active.carrier = Set(Some(input.carrier.clone()));
        active.tracking_number = Set(Some(input.tracking_number.clone()));
        active.delivered_note = Set(None);
        active.delivered_at = Set(None);
        active.metadata = Set(append_audit_event(
            merge_metadata(metadata, input.metadata),
            build_fulfillment_audit_event(
                FulfillmentItemAction::Reship,
                now,
                &adjusted_entries,
                Some(input.carrier),
                Some(input.tracking_number),
                STATUS_SHIPPED,
            ),
        ));
        active.updated_at = Set(now.into());
        active.update(&txn).await?;
        txn.commit().await?;

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
        active.metadata = Set(append_audit_event(
            merge_metadata(metadata, input.metadata),
            build_fulfillment_audit_event(
                FulfillmentItemAction::Cancel,
                now,
                &[],
                None,
                None,
                STATUS_CANCELLED,
            ),
        ));
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

    async fn build_fulfillment_response(
        &self,
        fulfillment: entities::fulfillment::Model,
    ) -> FulfillmentResult<FulfillmentResponse> {
        let items = entities::fulfillment_item::Entity::find()
            .filter(entities::fulfillment_item::Column::FulfillmentId.eq(fulfillment.id))
            .order_by_asc(entities::fulfillment_item::Column::CreatedAt)
            .all(&self.db)
            .await?;

        Ok(map_fulfillment(fulfillment, items))
    }

    async fn load_fulfillment_items(
        &self,
        fulfillment_id: Uuid,
    ) -> FulfillmentResult<Vec<entities::fulfillment_item::Model>> {
        entities::fulfillment_item::Entity::find()
            .filter(entities::fulfillment_item::Column::FulfillmentId.eq(fulfillment_id))
            .order_by_asc(entities::fulfillment_item::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(Into::into)
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

        self.get_shipping_option(tenant_id, shipping_option_id, None, None)
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

fn validate_fulfillment_items(
    items: Option<&[crate::dto::CreateFulfillmentItemInput]>,
) -> FulfillmentResult<()> {
    let Some(items) = items else {
        return Ok(());
    };

    let mut seen_line_items = BTreeSet::new();
    for item in items {
        item.validate()
            .map_err(|error| FulfillmentError::Validation(error.to_string()))?;
        if !seen_line_items.insert(item.order_line_item_id) {
            return Err(FulfillmentError::Validation(format!(
                "duplicate fulfillment item for order_line_item_id {}",
                item.order_line_item_id
            )));
        }
    }

    Ok(())
}

fn validate_item_quantity_adjustments(
    items: Option<&[FulfillmentItemQuantityInput]>,
) -> FulfillmentResult<()> {
    let Some(items) = items else {
        return Ok(());
    };

    let mut seen_items = BTreeSet::new();
    for item in items {
        item.validate()
            .map_err(|error| FulfillmentError::Validation(error.to_string()))?;
        if !seen_items.insert(item.fulfillment_item_id) {
            return Err(FulfillmentError::Validation(format!(
                "duplicate fulfillment item adjustment for item {}",
                item.fulfillment_item_id
            )));
        }
    }

    Ok(())
}

#[derive(Clone, Copy)]
enum FulfillmentItemAction {
    Ship,
    Deliver,
    Reopen,
    Reship,
    Cancel,
}

fn resolve_item_adjustments(
    items: &[entities::fulfillment_item::Model],
    requested: Option<&[FulfillmentItemQuantityInput]>,
    action: FulfillmentItemAction,
) -> FulfillmentResult<Vec<(Uuid, i32)>> {
    let planned = if let Some(requested) = requested {
        requested
            .iter()
            .map(|item| (item.fulfillment_item_id, item.quantity))
            .collect::<Vec<_>>()
    } else {
        items
            .iter()
            .filter_map(|item| {
                let quantity = match action {
                    FulfillmentItemAction::Ship => item.quantity - item.shipped_quantity,
                    FulfillmentItemAction::Deliver => {
                        item.shipped_quantity - item.delivered_quantity
                    }
                    FulfillmentItemAction::Reopen | FulfillmentItemAction::Reship => {
                        item.delivered_quantity
                    }
                    FulfillmentItemAction::Cancel => 0,
                };
                (quantity > 0).then_some((item.id, quantity))
            })
            .collect::<Vec<_>>()
    };

    if planned.is_empty() {
        return Err(FulfillmentError::Validation(match action {
            FulfillmentItemAction::Ship => {
                "fulfillment has no remaining item quantity to ship".to_string()
            }
            FulfillmentItemAction::Deliver => {
                "fulfillment has no remaining shipped item quantity to deliver".to_string()
            }
            FulfillmentItemAction::Reopen => {
                "fulfillment has no delivered item quantity to reopen".to_string()
            }
            FulfillmentItemAction::Reship => {
                "fulfillment has no delivered item quantity to reship".to_string()
            }
            FulfillmentItemAction::Cancel => {
                "fulfillment has no cancellable item quantity".to_string()
            }
        }));
    }

    let items_by_id = items
        .iter()
        .map(|item| (item.id, item))
        .collect::<BTreeMap<_, _>>();
    for (item_id, quantity) in &planned {
        let item = items_by_id.get(item_id).ok_or_else(|| {
            FulfillmentError::Validation(format!(
                "fulfillment item {item_id} does not belong to this fulfillment"
            ))
        })?;
        let remaining_quantity = match action {
            FulfillmentItemAction::Ship => item.quantity - item.shipped_quantity,
            FulfillmentItemAction::Deliver => item.shipped_quantity - item.delivered_quantity,
            FulfillmentItemAction::Reopen | FulfillmentItemAction::Reship => {
                item.delivered_quantity
            }
            FulfillmentItemAction::Cancel => 0,
        };
        if *quantity > remaining_quantity {
            return Err(FulfillmentError::Validation(format!(
                "{} quantity {} exceeds remaining quantity {} for fulfillment item {}",
                action.as_str(),
                quantity,
                remaining_quantity,
                item_id
            )));
        }
    }

    Ok(planned)
}

fn build_item_audit_event(
    action: FulfillmentItemAction,
    at: chrono::DateTime<Utc>,
    quantity: i32,
) -> Value {
    serde_json::json!({
        "type": action.as_str(),
        "at": at.to_rfc3339(),
        "quantity": quantity,
    })
}

fn build_fulfillment_audit_event(
    action: FulfillmentItemAction,
    at: chrono::DateTime<Utc>,
    items: &[(Uuid, Uuid, i32)],
    carrier: Option<String>,
    tracking_number: Option<String>,
    status_after: &str,
) -> Value {
    serde_json::json!({
        "type": action.as_str(),
        "at": at.to_rfc3339(),
        "status_after": status_after,
        "carrier": carrier,
        "tracking_number": tracking_number,
        "items": items
            .iter()
            .map(|(fulfillment_item_id, order_line_item_id, quantity)| {
                serde_json::json!({
                    "fulfillment_item_id": fulfillment_item_id,
                    "order_line_item_id": order_line_item_id,
                    "quantity": quantity,
                })
            })
            .collect::<Vec<_>>(),
    })
}

fn append_audit_event(metadata: Value, event: Value) -> Value {
    let mut metadata_object = match metadata {
        Value::Object(object) => object,
        _ => Map::new(),
    };
    let mut audit = match metadata_object.remove("audit") {
        Some(Value::Object(object)) => object,
        _ => Map::new(),
    };
    let mut events = match audit.remove("events") {
        Some(Value::Array(items)) => items,
        _ => Vec::new(),
    };
    events.push(event);
    audit.insert("events".to_string(), Value::Array(events));
    metadata_object.insert("audit".to_string(), Value::Object(audit));
    Value::Object(metadata_object)
}

impl FulfillmentItemAction {
    fn as_str(self) -> &'static str {
        match self {
            FulfillmentItemAction::Ship => "ship",
            FulfillmentItemAction::Deliver => "deliver",
            FulfillmentItemAction::Reopen => "reopen",
            FulfillmentItemAction::Reship => "reship",
            FulfillmentItemAction::Cancel => "cancel",
        }
    }
}

fn reopened_status_for_cancelled(
    items: &[entities::fulfillment_item::Model],
    fulfillment: &entities::fulfillment::ActiveModel,
) -> &'static str {
    if items.iter().any(|item| item.shipped_quantity > 0)
        || fulfillment.shipped_at.clone().take().is_some()
    {
        STATUS_SHIPPED
    } else {
        STATUS_PENDING
    }
}

async fn load_shipping_options_with_translations(
    db: &DatabaseConnection,
    rows: Vec<entities::shipping_option::Model>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> FulfillmentResult<Vec<ShippingOptionResponse>> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let translations = entities::shipping_option_translation::Entity::find()
        .filter(entities::shipping_option_translation::Column::ShippingOptionId.is_in(ids.clone()))
        .all(db)
        .await?;

    let mut translations_by_option: HashMap<
        Uuid,
        Vec<entities::shipping_option_translation::Model>,
    > = HashMap::new();
    for translation in translations {
        translations_by_option
            .entry(translation.shipping_option_id)
            .or_default()
            .push(translation);
    }

    Ok(rows
        .into_iter()
        .map(|row| {
            let translations = translations_by_option.remove(&row.id).unwrap_or_default();
            map_shipping_option(row, translations, requested_locale, tenant_default_locale)
        })
        .collect())
}

fn map_shipping_option(
    option: entities::shipping_option::Model,
    translations: Vec<entities::shipping_option_translation::Model>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> ShippingOptionResponse {
    let available_locales = translations
        .iter()
        .map(|translation| translation.locale.clone())
        .collect::<Vec<_>>();
    let requested_locale = requested_locale
        .and_then(normalize_locale_tag)
        .filter(|value| !value.is_empty());
    let (resolved, effective_locale) = resolve_translation(
        &translations,
        requested_locale.as_deref(),
        tenant_default_locale,
    );
    let name = resolved
        .map(|translation| translation.name.clone())
        .unwrap_or_default();

    ShippingOptionResponse {
        id: option.id,
        tenant_id: option.tenant_id,
        name,
        currency_code: option.currency_code,
        amount: option.amount,
        provider_id: option.provider_id,
        active: option.active,
        allowed_shipping_profile_slugs: extract_allowed_shipping_profile_slugs(&option.metadata),
        metadata: option.metadata,
        created_at: option.created_at.with_timezone(&Utc),
        updated_at: option.updated_at.with_timezone(&Utc),
        requested_locale,
        effective_locale,
        available_locales,
        translations: translations
            .into_iter()
            .map(|translation| ShippingOptionTranslationResponse {
                locale: translation.locale,
                name: translation.name,
            })
            .collect(),
    }
}

fn normalize_translation_inputs(
    translations: Vec<ShippingOptionTranslationInput>,
) -> FulfillmentResult<Vec<ShippingOptionTranslationInput>> {
    if translations.is_empty() {
        return Err(FulfillmentError::Validation(
            "At least one translation is required".to_string(),
        ));
    }
    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(translations.len());
    for translation in translations {
        let locale = normalize_locale_tag(&translation.locale)
            .ok_or_else(|| FulfillmentError::Validation("Invalid locale".to_string()))?;
        if !seen.insert(locale.clone()) {
            return Err(FulfillmentError::Validation(
                "Duplicate locale in shipping option translations".to_string(),
            ));
        }
        let name = translation.name.trim();
        if name.is_empty() {
            return Err(FulfillmentError::Validation(
                "Shipping option name cannot be empty".to_string(),
            ));
        }
        normalized.push(ShippingOptionTranslationInput {
            locale,
            name: name.to_string(),
        });
    }
    Ok(normalized)
}

async fn insert_translations(
    db: &DatabaseConnection,
    shipping_option_id: Uuid,
    translations: &[ShippingOptionTranslationInput],
) -> FulfillmentResult<()> {
    for translation in translations {
        entities::shipping_option_translation::ActiveModel {
            id: Set(generate_id()),
            shipping_option_id: Set(shipping_option_id),
            locale: Set(translation.locale.clone()),
            name: Set(translation.name.clone()),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

async fn replace_translations(
    db: &DatabaseConnection,
    shipping_option_id: Uuid,
    translations: &[ShippingOptionTranslationInput],
) -> FulfillmentResult<()> {
    entities::shipping_option_translation::Entity::delete_many()
        .filter(
            entities::shipping_option_translation::Column::ShippingOptionId.eq(shipping_option_id),
        )
        .exec(db)
        .await?;
    insert_translations(db, shipping_option_id, translations).await
}

fn resolve_translation<'a>(
    translations: &'a [entities::shipping_option_translation::Model],
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> (
    Option<&'a entities::shipping_option_translation::Model>,
    Option<String>,
) {
    let mut lookup = HashMap::new();
    for translation in translations {
        if let Some(normalized) = normalize_locale_tag(&translation.locale) {
            lookup.insert(normalized, translation);
        }
    }

    if let Some(locale) = requested_locale.and_then(normalize_locale_tag) {
        if let Some(found) = lookup.get(&locale) {
            return (Some(*found), Some(found.locale.clone()));
        }
    }
    if let Some(locale) = tenant_default_locale.and_then(normalize_locale_tag) {
        if let Some(found) = lookup.get(&locale) {
            return (Some(*found), Some(found.locale.clone()));
        }
    }
    translations
        .first()
        .map(|item| (Some(item), Some(item.locale.clone())))
        .unwrap_or((None, None))
}

fn map_fulfillment(
    fulfillment: entities::fulfillment::Model,
    items: Vec<entities::fulfillment_item::Model>,
) -> FulfillmentResponse {
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
        items: items.into_iter().map(map_fulfillment_item).collect(),
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

fn map_fulfillment_item(item: entities::fulfillment_item::Model) -> FulfillmentItemResponse {
    FulfillmentItemResponse {
        id: item.id,
        fulfillment_id: item.fulfillment_id,
        order_line_item_id: item.order_line_item_id,
        quantity: item.quantity,
        shipped_quantity: item.shipped_quantity,
        delivered_quantity: item.delivered_quantity,
        metadata: item.metadata,
        created_at: item.created_at.with_timezone(&Utc),
        updated_at: item.updated_at.with_timezone(&Utc),
    }
}
