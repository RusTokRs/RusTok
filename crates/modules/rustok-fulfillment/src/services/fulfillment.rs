use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{
    AccessMode, ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection,
    DatabaseTransaction, EntityTrait, IsolationLevel, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait, sea_query::OnConflict,
};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

use rustok_core::generate_id;

use rustok_api::{TenantLocale, UNKNOWN_PROVENANCE_LOCALE, normalize_locale_tag};

use crate::dto::{
    CancelFulfillmentInput, CreateFulfillmentInput, CreateShippingOptionInput,
    DeliverFulfillmentInput, FulfillmentItemQuantityInput, FulfillmentItemResponse,
    FulfillmentResponse, ListFulfillmentsInput, ReopenFulfillmentInput, ReshipFulfillmentInput,
    ShipFulfillmentInput, ShippingOptionResponse, ShippingOptionTranslationInput,
    ShippingOptionTranslationResponse, UpdateShippingOptionInput,
};
use crate::entities;
use crate::error::{FulfillmentError, FulfillmentResult};
use crate::translation_changes::{
    ShippingOptionTranslationChangeLifecycle, record_shipping_option_translation_change_in_tx,
};

use super::shipping_option_translation::{
    ShippingOptionTranslationExactLocaleError,
    resource_revision as shipping_option_translation_resource_revision,
};

const STATUS_PENDING: &str = "pending";
const STATUS_SHIPPED: &str = "shipped";
const STATUS_DELIVERED: &str = "delivered";
const STATUS_CANCELLED: &str = "cancelled";
const MANUAL_PROVIDER_ID: &str = "manual";

#[derive(Debug, Clone)]
struct CheckoutFulfillmentIdentity {
    operation_id: Uuid,
    index: u32,
    plan_hash: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CheckoutFulfillmentRecord {
    pub index: u32,
    pub order_id: Uuid,
    pub plan_hash: Option<String>,
    pub fulfillment: FulfillmentResponse,
}

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
        validate_tenant_id(tenant_id)?;
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
        let provider_id = normalize_provider_id(provider_id)?;
        let allowed_shipping_profile_slugs =
            normalize_allowed_shipping_profile_slugs(allowed_shipping_profile_slugs)?;
        let metadata =
            apply_allowed_shipping_profiles_to_metadata(metadata, allowed_shipping_profile_slugs)?;

        let shipping_option_id = generate_id();
        let now = Utc::now();
        let txn = self.db.begin().await?;

        let option = entities::shipping_option::ActiveModel {
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
        .insert(&txn)
        .await?;

        insert_translations(&txn, shipping_option_id, &translations).await?;
        let translation_rows =
            load_shipping_option_translation_rows(&txn, shipping_option_id).await?;
        let resource_revision =
            shipping_option_translation_resource_revision(&option, &translation_rows);
        record_shipping_option_translation_change_in_tx(
            &txn,
            tenant_id,
            shipping_option_id,
            generate_id(),
            &resource_revision,
            ShippingOptionTranslationChangeLifecycle::Active,
        )
        .await
        .map_err(translation_change_error_to_fulfillment_error)?;
        txn.commit().await?;

        self.get_shipping_option(tenant_id, shipping_option_id, None, None)
            .await
    }

    pub async fn list_shipping_options(
        &self,
        tenant_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> FulfillmentResult<Vec<ShippingOptionResponse>> {
        validate_tenant_id(tenant_id)?;
        let rows = entities::shipping_option::Entity::find()
            .filter(entities::shipping_option::Column::TenantId.eq(tenant_id))
            .filter(entities::shipping_option::Column::Active.eq(true))
            .order_by_asc(entities::shipping_option::Column::CreatedAt)
            .order_by_asc(entities::shipping_option::Column::Id)
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
        validate_tenant_id(tenant_id)?;
        let rows = entities::shipping_option::Entity::find()
            .filter(entities::shipping_option::Column::TenantId.eq(tenant_id))
            .order_by_asc(entities::shipping_option::Column::CreatedAt)
            .order_by_asc(entities::shipping_option::Column::Id)
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
        validate_tenant_id(tenant_id)?;
        input
            .validate()
            .map_err(|error| FulfillmentError::Validation(error.to_string()))?;

        let UpdateShippingOptionInput {
            translations,
            expected_translation_revision,
            currency_code,
            amount,
            provider_id,
            allowed_shipping_profile_slugs,
            metadata,
        } = input;

        if let Some(amount) = amount
            && amount < Decimal::ZERO
        {
            return Err(FulfillmentError::Validation(
                "amount cannot be negative".to_string(),
            ));
        }
        let translations = translations.map(normalize_translation_inputs).transpose()?;
        match (&translations, &expected_translation_revision) {
            (Some(_), Some(revision)) if !revision.trim().is_empty() => {}
            (Some(_), _) => {
                return Err(FulfillmentError::Validation(
                    "expected_translation_revision is required when translations are updated"
                        .to_string(),
                ));
            }
            (None, Some(_)) => {
                return Err(FulfillmentError::Validation(
                    "expected_translation_revision is only valid when translations are updated"
                        .to_string(),
                ));
            }
            (None, None) => {}
        }

        let txn = self.db.begin().await?;
        let shipping_option = entities::shipping_option::Entity::find_by_id(shipping_option_id)
            .filter(entities::shipping_option::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or(FulfillmentError::ShippingOptionNotFound(shipping_option_id))?;

        if let Some(expected_revision) = expected_translation_revision.as_deref() {
            let current_translations =
                load_shipping_option_translation_rows(&txn, shipping_option_id).await?;
            let current_revision = shipping_option_translation_resource_revision(
                &shipping_option,
                &current_translations,
            );
            if expected_revision != current_revision {
                return Err(FulfillmentError::ShippingOptionTranslationRevisionConflict(
                    shipping_option_id,
                ));
            }
        }

        let mut active: entities::shipping_option::ActiveModel = shipping_option.into();

        if let Some(currency_code) = currency_code {
            active.currency_code = Set(normalize_currency_code(&currency_code)?);
        }
        if let Some(amount) = amount {
            active.amount = Set(amount);
        }
        if provider_id.is_some() {
            active.provider_id = Set(normalize_provider_id(provider_id)?);
        }
        if metadata.is_some() || allowed_shipping_profile_slugs.is_some() {
            let current_metadata = active.metadata.clone().take().unwrap_or_default();
            let metadata = match metadata {
                Some(patch) => merge_metadata(current_metadata, patch),
                None => current_metadata,
            };
            active.metadata = Set(apply_allowed_shipping_profiles_to_metadata(
                metadata,
                normalize_allowed_shipping_profile_slugs(allowed_shipping_profile_slugs)?,
            )?);
        }

        active.updated_at = Set(Utc::now().into());
        let option = active.update(&txn).await?;

        let localized_copy_changed = match translations.as_deref() {
            Some(translations) => {
                synchronize_translations(&txn, shipping_option_id, translations).await?
            }
            None => false,
        };
        if localized_copy_changed {
            let translation_rows =
                load_shipping_option_translation_rows(&txn, shipping_option_id).await?;
            let resource_revision =
                shipping_option_translation_resource_revision(&option, &translation_rows);
            record_shipping_option_translation_change_in_tx(
                &txn,
                tenant_id,
                shipping_option_id,
                generate_id(),
                &resource_revision,
                ShippingOptionTranslationChangeLifecycle::from(option.active),
            )
            .await
            .map_err(translation_change_error_to_fulfillment_error)?;
        }
        txn.commit().await?;

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
        validate_tenant_id(tenant_id)?;
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
        validate_tenant_id(tenant_id)?;
        self.set_shipping_option_active(tenant_id, shipping_option_id, false)
            .await
    }

    pub async fn reactivate_shipping_option(
        &self,
        tenant_id: Uuid,
        shipping_option_id: Uuid,
    ) -> FulfillmentResult<ShippingOptionResponse> {
        validate_tenant_id(tenant_id)?;
        self.set_shipping_option_active(tenant_id, shipping_option_id, true)
            .await
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id))]
    pub async fn create_fulfillment(
        &self,
        tenant_id: Uuid,
        input: CreateFulfillmentInput,
    ) -> FulfillmentResult<FulfillmentResponse> {
        validate_tenant_id(tenant_id)?;
        self.create_fulfillment_with_identity(tenant_id, input, None)
            .await
    }

    /// Create a fulfillment with a stable orchestration-owned ID.
    ///
    /// Durable orchestration journals use this ID as their local resource anchor so
    /// retries rebuild the same fulfillment instead of creating a second resource.
    pub(crate) async fn create_fulfillment_with_id(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        input: CreateFulfillmentInput,
    ) -> FulfillmentResult<FulfillmentResponse> {
        validate_tenant_id(tenant_id)?;
        self.create_fulfillment_with_identity_and_id(tenant_id, input, None, fulfillment_id)
            .await
    }

    async fn create_fulfillment_with_identity(
        &self,
        tenant_id: Uuid,
        input: CreateFulfillmentInput,
        identity: Option<CheckoutFulfillmentIdentity>,
    ) -> FulfillmentResult<FulfillmentResponse> {
        let fulfillment_id = generate_id();
        self.create_fulfillment_with_identity_and_id(tenant_id, input, identity, fulfillment_id)
            .await
    }

    pub(crate) async fn ensure_checkout_fulfillment_set(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
        order_id: Uuid,
        customer_id: Option<Uuid>,
        checkout_plan_hash: &str,
        plans: Vec<(u32, CreateFulfillmentInput)>,
    ) -> FulfillmentResult<Vec<CheckoutFulfillmentRecord>> {
        validate_tenant_id(tenant_id)?;
        let checkout_plan_hash =
            validate_checkout_identity(checkout_operation_id, 0, checkout_plan_hash)?;

        let mut requested_indices = BTreeSet::new();
        for (index, input) in &plans {
            if !requested_indices.insert(*index) {
                return Err(FulfillmentError::Validation(
                    "checkout fulfillment plan indexes must be unique".to_string(),
                ));
            }
            if input.order_id != order_id || input.customer_id != customer_id {
                return Err(FulfillmentError::Validation(
                    "checkout fulfillment input does not match the checkout identity".to_string(),
                ));
            }
            self.validate_create_fulfillment_input(tenant_id, input)
                .await?;
        }

        let txn = self.db.begin().await?;
        let anchor = CheckoutFulfillmentIdentity {
            operation_id: checkout_operation_id,
            index: 0,
            plan_hash: checkout_plan_hash.clone(),
        };
        self.ensure_checkout_identity_anchor(
            &txn,
            tenant_id,
            order_id,
            customer_id,
            &anchor,
        )
        .await?;

        let existing_rows = entities::fulfillment::Entity::find()
            .filter(entities::fulfillment::Column::TenantId.eq(tenant_id))
            .filter(
                entities::fulfillment::Column::CheckoutOperationId.eq(checkout_operation_id),
            )
            .lock_exclusive()
            .all(&txn)
            .await?;

        let mut existing_indices = BTreeSet::new();
        for row in &existing_rows {
            let raw_index = row.checkout_fulfillment_index.ok_or_else(|| {
                FulfillmentError::Validation(
                    "checkout fulfillment identity index is missing".to_string(),
                )
            })?;
            let index = u32::try_from(raw_index).map_err(|_| {
                FulfillmentError::Validation(
                    "checkout fulfillment identity index is out of range".to_string(),
                )
            })?;
            if !existing_indices.insert(index) {
                return Err(FulfillmentError::Validation(
                    "checkout fulfillment identity contains duplicate indexes".to_string(),
                ));
            }
            let persisted_plan_hash = row
                .checkout_plan_hash
                .as_deref()
                .map(normalize_checkout_plan_hash)
                .transpose()?;
            if row.order_id != order_id
                || row.customer_id != customer_id
                || persisted_plan_hash.as_deref() != Some(checkout_plan_hash.as_str())
            {
                return Err(FulfillmentError::Validation(
                    "checkout fulfillment child identity does not match the operation anchor"
                        .to_string(),
                ));
            }
        }

        if existing_indices
            .iter()
            .any(|index| !requested_indices.contains(index))
        {
            return Err(FulfillmentError::Validation(
                "checkout fulfillment set already contains an index outside the requested immutable plan"
                    .to_string(),
            ));
        }

        let now = Utc::now();
        for (index, input) in plans {
            if existing_indices.contains(&index) {
                continue;
            }
            let identity = CheckoutFulfillmentIdentity {
                operation_id: checkout_operation_id,
                index,
                plan_hash: checkout_plan_hash.clone(),
            };
            self.insert_fulfillment_in_txn(
                &txn,
                tenant_id,
                generate_id(),
                input,
                Some(&identity),
                now,
            )
            .await?;
        }

        txn.commit().await?;

        self.list_checkout_fulfillments(tenant_id, checkout_operation_id)
            .await
    }

    async fn create_fulfillment_with_identity_and_id(
        &self,
        tenant_id: Uuid,
        input: CreateFulfillmentInput,
        identity: Option<CheckoutFulfillmentIdentity>,
        fulfillment_id: Uuid,
    ) -> FulfillmentResult<FulfillmentResponse> {
        self.validate_create_fulfillment_input(tenant_id, &input)
            .await?;

        let now = Utc::now();
        let txn = self.db.begin().await?;

        if let Some(identity) = identity.as_ref() {
            self.ensure_checkout_identity_anchor(
                &txn,
                tenant_id,
                input.order_id,
                input.customer_id,
                identity,
            )
            .await?;
        }

        self.insert_fulfillment_in_txn(
            &txn,
            tenant_id,
            fulfillment_id,
            input,
            identity.as_ref(),
            now,
        )
        .await?;

        txn.commit().await?;

        self.get_fulfillment(tenant_id, fulfillment_id).await
    }

    async fn validate_create_fulfillment_input(
        &self,
        tenant_id: Uuid,
        input: &CreateFulfillmentInput,
    ) -> FulfillmentResult<()> {
        input
            .validate()
            .map_err(|error| FulfillmentError::Validation(error.to_string()))?;

        if let Some(shipping_option_id) = input.shipping_option_id {
            self.get_shipping_option(tenant_id, shipping_option_id, None, None)
                .await?;
        }
        validate_fulfillment_items(input.items.as_deref())?;
        validate_object_metadata(&input.metadata, "fulfillment")?;
        if let Some(items) = input.items.as_ref() {
            for item in items {
                validate_object_metadata(&item.metadata, "fulfillment item")?;
            }
        }

        Ok(())
    }

    async fn insert_fulfillment_in_txn(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        input: CreateFulfillmentInput,
        identity: Option<&CheckoutFulfillmentIdentity>,
        now: chrono::DateTime<Utc>,
    ) -> FulfillmentResult<()> {
        let CreateFulfillmentInput {
            order_id,
            shipping_option_id,
            customer_id,
            carrier,
            tracking_number,
            items,
            metadata,
        } = input;

        let checkout_operation_id = identity.map(|value| value.operation_id);
        let checkout_fulfillment_index = identity.map(|value| i64::from(value.index));
        let checkout_plan_hash = identity.map(|value| value.plan_hash.clone());

        entities::fulfillment::ActiveModel {
            id: Set(fulfillment_id),
            tenant_id: Set(tenant_id),
            order_id: Set(order_id),
            shipping_option_id: Set(shipping_option_id),
            customer_id: Set(customer_id),
            checkout_operation_id: Set(checkout_operation_id),
            checkout_fulfillment_index: Set(checkout_fulfillment_index),
            checkout_plan_hash: Set(checkout_plan_hash),
            status: Set(STATUS_PENDING.to_string()),
            carrier: Set(carrier),
            tracking_number: Set(tracking_number),
            delivered_note: Set(None),
            cancellation_reason: Set(None),
            metadata: Set(strip_fulfillment_metadata(metadata)?),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            shipped_at: Set(None),
            delivered_at: Set(None),
            cancelled_at: Set(None),
        }
        .insert(txn)
        .await?;

        if let Some(items) = items {
            for item in items {
                entities::fulfillment_item::ActiveModel {
                    id: Set(generate_id()),
                    fulfillment_id: Set(fulfillment_id),
                    order_line_item_id: Set(item.order_line_item_id),
                    quantity: Set(item.quantity),
                    shipped_quantity: Set(0),
                    delivered_quantity: Set(0),
                    metadata: Set(strip_fulfillment_item_metadata(item.metadata)?),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(txn)
                .await?;
            }
        }

        Ok(())
    }

    /// Read one fulfillment projection from one database transaction snapshot.
    ///
    /// PostgreSQL uses repeatable-read/read-only settings; SQLite keeps the snapshot
    /// within the transaction because its driver ignores those configuration knobs.
    /// Fulfillment state and its items are persisted in separate tables but form one
    /// response aggregate. The snapshot prevents a concurrent lifecycle commit from
    /// producing a parent/item combination that never existed atomically.
    pub async fn get_fulfillment(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
    ) -> FulfillmentResult<FulfillmentResponse> {
        validate_tenant_id(tenant_id)?;
        let txn = self.begin_read_transaction().await?;
        let fulfillment = self
            .load_fulfillment(&txn, tenant_id, fulfillment_id)
            .await?;
        let response = self.build_fulfillment_response(&txn, fulfillment).await?;
        txn.commit().await?;
        Ok(response)
    }

    async fn ensure_checkout_identity_anchor(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        order_id: Uuid,
        customer_id: Option<Uuid>,
        identity: &CheckoutFulfillmentIdentity,
    ) -> FulfillmentResult<()> {
        let now = Utc::now();
        let _ = entities::checkout_identity::Entity::insert(
            entities::checkout_identity::ActiveModel {
                tenant_id: Set(tenant_id),
                checkout_operation_id: Set(identity.operation_id),
                order_id: Set(order_id),
                customer_id: Set(customer_id),
                plan_hash: Set(identity.plan_hash.clone()),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            },
        )
        .on_conflict(
            OnConflict::columns([
                entities::checkout_identity::Column::TenantId,
                entities::checkout_identity::Column::CheckoutOperationId,
            ])
            .do_nothing_on([
                entities::checkout_identity::Column::TenantId,
                entities::checkout_identity::Column::CheckoutOperationId,
            ])
            .to_owned(),
        )
        .try_insert()
        .exec(txn)
        .await?;

        let existing = entities::checkout_identity::Entity::find()
            .filter(entities::checkout_identity::Column::TenantId.eq(tenant_id))
            .filter(
                entities::checkout_identity::Column::CheckoutOperationId.eq(identity.operation_id),
            )
            .lock_exclusive()
            .one(txn)
            .await?
            .ok_or_else(|| {
                FulfillmentError::Validation(
                    "checkout operation identity anchor could not be established".to_string(),
                )
            })?;

        if existing.order_id == order_id
            && existing.customer_id == customer_id
            && existing.plan_hash == identity.plan_hash
        {
            Ok(())
        } else {
            Err(FulfillmentError::Validation(
                "checkout operation identity is already bound to a different order, customer, or plan"
                    .to_string(),
            ))
        }
    }

    pub(crate) async fn list_checkout_fulfillments(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
    ) -> FulfillmentResult<Vec<CheckoutFulfillmentRecord>> {
        validate_tenant_id(tenant_id)?;
        let txn = self.begin_read_transaction().await?;
        let rows = entities::fulfillment::Entity::find()
            .filter(entities::fulfillment::Column::TenantId.eq(tenant_id))
            .filter(entities::fulfillment::Column::CheckoutOperationId.eq(checkout_operation_id))
            .order_by_asc(entities::fulfillment::Column::CheckoutFulfillmentIndex)
            .all(&txn)
            .await?;

        let mut records = Vec::with_capacity(rows.len());
        let (rows, fulfillments) = self.build_fulfillment_responses(&txn, rows).await?;

        for (row, fulfillment) in rows.into_iter().zip(fulfillments) {
            let index = row.checkout_fulfillment_index.ok_or_else(|| {
                FulfillmentError::Validation(
                    "checkout fulfillment identity index is missing".to_string(),
                )
            })?;
            let index = u32::try_from(index).map_err(|_| {
                FulfillmentError::Validation(
                    "checkout fulfillment identity index is out of range".to_string(),
                )
            })?;
            records.push(CheckoutFulfillmentRecord {
                index,
                order_id: row.order_id,
                plan_hash: row.checkout_plan_hash,
                fulfillment,
            });
        }
        txn.commit().await?;
        Ok(records)
    }

    pub async fn find_by_order(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
    ) -> FulfillmentResult<Option<FulfillmentResponse>> {
        validate_tenant_id(tenant_id)?;
        let txn = self.begin_read_transaction().await?;
        let fulfillment = entities::fulfillment::Entity::find()
            .filter(entities::fulfillment::Column::TenantId.eq(tenant_id))
            .filter(entities::fulfillment::Column::OrderId.eq(order_id))
            .order_by_desc(entities::fulfillment::Column::CreatedAt)
            .order_by_desc(entities::fulfillment::Column::Id)
            .one(&txn)
            .await?;

        let response = match fulfillment {
            Some(fulfillment) => Some(self.build_fulfillment_response(&txn, fulfillment).await?),
            None => None,
        };
        txn.commit().await?;
        Ok(response)
    }

    pub async fn list_by_order(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
    ) -> FulfillmentResult<Vec<FulfillmentResponse>> {
        validate_tenant_id(tenant_id)?;
        let txn = self.begin_read_transaction().await?;
        let rows = entities::fulfillment::Entity::find()
            .filter(entities::fulfillment::Column::TenantId.eq(tenant_id))
            .filter(entities::fulfillment::Column::OrderId.eq(order_id))
            .order_by_asc(entities::fulfillment::Column::CreatedAt)
            .order_by_asc(entities::fulfillment::Column::Id)
            .all(&txn)
            .await?;

        let (_, items) = self.build_fulfillment_responses(&txn, rows).await?;
        txn.commit().await?;
        Ok(items)
    }

    pub async fn list_fulfillments(
        &self,
        tenant_id: Uuid,
        input: ListFulfillmentsInput,
    ) -> FulfillmentResult<(Vec<FulfillmentResponse>, u64)> {
        validate_tenant_id(tenant_id)?;
        let page = input.page.max(1);
        let per_page = input.per_page.clamp(1, 100);
        let offset = fulfillment_list_offset(page, per_page);

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

        let txn = self.begin_read_transaction().await?;
        let total = query.clone().count(&txn).await?;
        let rows = query
            .order_by_desc(entities::fulfillment::Column::CreatedAt)
            .order_by_desc(entities::fulfillment::Column::Id)
            .offset(offset)
            .limit(per_page)
            .all(&txn)
            .await?;

        let (_, items) = self.build_fulfillment_responses(&txn, rows).await?;
        txn.commit().await?;

        Ok((items, total))
    }

    pub async fn ship_fulfillment(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        input: ShipFulfillmentInput,
    ) -> FulfillmentResult<FulfillmentResponse> {
        self.ship_fulfillment_internal(tenant_id, fulfillment_id, input, None)
            .await
    }

    /// Apply a provider-backed ship result after the provider operation has been journaled.
    ///
    /// The public service entrypoint strips caller-supplied provider receipts; only this
    /// crate-internal path can attach the journal-owned receipt to the lifecycle write.
    pub(crate) async fn ship_fulfillment_with_provider_result(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        input: ShipFulfillmentInput,
        provider_metadata: Value,
        operation_id: Uuid,
    ) -> FulfillmentResult<FulfillmentResponse> {
        self.ship_fulfillment_internal(
            tenant_id,
            fulfillment_id,
            input,
            Some((provider_metadata, operation_id)),
        )
        .await
    }

    async fn ship_fulfillment_internal(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        mut input: ShipFulfillmentInput,
        provider_result: Option<(Value, Uuid)>,
    ) -> FulfillmentResult<FulfillmentResponse> {
        validate_tenant_id(tenant_id)?;
        input
            .validate()
            .map_err(|error| FulfillmentError::Validation(error.to_string()))?;
        input.metadata = match provider_result {
            Some((provider_metadata, operation_id)) => prepare_provider_lifecycle_metadata(
                input.metadata,
                provider_metadata,
                operation_id,
                "ship",
            )?,
            None => strip_provider_operation_metadata(input.metadata),
        };

        let txn = self.db.begin().await?;
        let fulfillment = self
            .load_fulfillment_for_update(&txn, tenant_id, fulfillment_id)
            .await?;
        if !matches!(fulfillment.status.as_str(), STATUS_PENDING | STATUS_SHIPPED) {
            return Err(FulfillmentError::InvalidTransition {
                from: fulfillment.status,
                to: STATUS_SHIPPED.to_string(),
            });
        }
        let items = self.load_fulfillment_items(&txn, fulfillment.id).await?;
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
                merge_fulfillment_metadata(metadata, input.metadata)?,
                build_fulfillment_audit_event(
                    FulfillmentItemAction::Ship,
                    now,
                    &[],
                    Some(carrier),
                    Some(tracking_number),
                    STATUS_SHIPPED,
                ),
            )?);
            active.shipped_at = Set(Some(now.into()));
            active.updated_at = Set(now.into());
            active.update(&txn).await?;
            txn.commit().await?;

            return self.get_fulfillment(tenant_id, fulfillment_id).await;
        }

        validate_item_quantity_adjustments(input.items.as_deref())?;
        let now = Utc::now();
        let adjustment_plan =
            resolve_item_adjustments(&items, input.items.as_deref(), FulfillmentItemAction::Ship)?;
        let mut adjusted_items = Vec::with_capacity(items.len());
        let adjustment_lookup = adjustment_plan.into_iter().collect::<BTreeMap<Uuid, i32>>();
        let mut adjusted_entries = Vec::new();
        for item in items {
            let adjustment = adjustment_lookup.get(&item.id).copied().unwrap_or_default();
            if adjustment == 0 {
                adjusted_items.push(item);
                continue;
            }

            let mut active: entities::fulfillment_item::ActiveModel = item.clone().into();
            let shipped_quantity = item.shipped_quantity + adjustment;
            active.shipped_quantity = Set(shipped_quantity);
            active.metadata = Set(append_audit_event(
                item.metadata.clone(),
                build_item_audit_event(FulfillmentItemAction::Ship, now, adjustment),
            )?);
            active.updated_at = Set(now.into());
            adjusted_entries.push((item.id, item.order_line_item_id, adjustment));
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
            merge_fulfillment_metadata(metadata, input.metadata)?,
            build_fulfillment_audit_event(
                FulfillmentItemAction::Ship,
                now,
                &adjusted_entries,
                Some(input.carrier),
                Some(input.tracking_number),
                active.status.clone().take().unwrap_or_default().as_str(),
            ),
        )?);
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
        mut input: DeliverFulfillmentInput,
    ) -> FulfillmentResult<FulfillmentResponse> {
        validate_tenant_id(tenant_id)?;
        input
            .validate()
            .map_err(|error| FulfillmentError::Validation(error.to_string()))?;
        input.metadata = strip_provider_operation_metadata(input.metadata);
        let txn = self.db.begin().await?;
        let fulfillment = self
            .load_fulfillment_for_update(&txn, tenant_id, fulfillment_id)
            .await?;
        if fulfillment.status != STATUS_SHIPPED {
            return Err(FulfillmentError::InvalidTransition {
                from: fulfillment.status,
                to: STATUS_DELIVERED.to_string(),
            });
        }
        let items = self.load_fulfillment_items(&txn, fulfillment.id).await?;
        if items.is_empty() {
            let mut active: entities::fulfillment::ActiveModel = fulfillment.into();
            let now = Utc::now();
            let metadata = active.metadata.clone().take().unwrap_or_default();
            active.status = Set(STATUS_DELIVERED.to_string());
            active.delivered_note = Set(input.delivered_note.clone());
            active.metadata = Set(append_audit_event(
                merge_fulfillment_metadata(metadata, input.metadata)?,
                build_fulfillment_audit_event(
                    FulfillmentItemAction::Deliver,
                    now,
                    &[],
                    None,
                    None,
                    STATUS_DELIVERED,
                ),
            )?);
            active.delivered_at = Set(Some(now.into()));
            active.updated_at = Set(now.into());
            active.update(&txn).await?;
            txn.commit().await?;

            return self.get_fulfillment(tenant_id, fulfillment_id).await;
        }

        validate_item_quantity_adjustments(input.items.as_deref())?;
        let now = Utc::now();
        let adjustment_plan = resolve_item_adjustments(
            &items,
            input.items.as_deref(),
            FulfillmentItemAction::Deliver,
        )?;
        let mut adjusted_items = Vec::with_capacity(items.len());
        let adjustment_lookup = adjustment_plan.into_iter().collect::<BTreeMap<Uuid, i32>>();
        let mut adjusted_entries = Vec::new();
        for item in items {
            let adjustment = adjustment_lookup.get(&item.id).copied().unwrap_or_default();
            if adjustment == 0 {
                adjusted_items.push(item);
                continue;
            }

            let mut active: entities::fulfillment_item::ActiveModel = item.clone().into();
            let delivered_quantity = item.delivered_quantity + adjustment;
            active.delivered_quantity = Set(delivered_quantity);
            active.metadata = Set(append_audit_event(
                item.metadata.clone(),
                build_item_audit_event(FulfillmentItemAction::Deliver, now, adjustment),
            )?);
            active.updated_at = Set(now.into());
            adjusted_entries.push((item.id, item.order_line_item_id, adjustment));
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
            merge_fulfillment_metadata(metadata, input.metadata)?,
            build_fulfillment_audit_event(
                FulfillmentItemAction::Deliver,
                now,
                &adjusted_entries,
                None,
                None,
                active.status.clone().take().unwrap_or_default().as_str(),
            ),
        )?);
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
        mut input: ReopenFulfillmentInput,
    ) -> FulfillmentResult<FulfillmentResponse> {
        validate_tenant_id(tenant_id)?;
        input.metadata = strip_provider_operation_metadata(input.metadata);
        let txn = self.db.begin().await?;
        let fulfillment = self
            .load_fulfillment_for_update(&txn, tenant_id, fulfillment_id)
            .await?;
        let now = Utc::now();

        match fulfillment.status.as_str() {
            STATUS_CANCELLED => {
                let items = self.load_fulfillment_items(&txn, fulfillment.id).await?;
                let mut active: entities::fulfillment::ActiveModel = fulfillment.into();
                let metadata = active.metadata.clone().take().unwrap_or_default();
                let status_after = reopened_status_for_cancelled(&items, &active);
                active.status = Set(status_after.to_string());
                active.cancellation_reason = Set(None);
                active.cancelled_at = Set(None);
                active.metadata = Set(append_audit_event(
                    merge_fulfillment_metadata(metadata, input.metadata)?,
                    build_fulfillment_audit_event(
                        FulfillmentItemAction::Reopen,
                        now,
                        &[],
                        None,
                        None,
                        status_after,
                    ),
                )?);
                active.updated_at = Set(now.into());
                active.update(&txn).await?;
                txn.commit().await?;

                self.get_fulfillment(tenant_id, fulfillment_id).await
            }
            STATUS_DELIVERED => {
                let items = self.load_fulfillment_items(&txn, fulfillment.id).await?;
                if items.is_empty() {
                    let mut active: entities::fulfillment::ActiveModel = fulfillment.into();
                    let metadata = active.metadata.clone().take().unwrap_or_default();
                    active.status = Set(STATUS_SHIPPED.to_string());
                    active.delivered_note = Set(None);
                    active.delivered_at = Set(None);
                    active.metadata = Set(append_audit_event(
                        merge_fulfillment_metadata(metadata, input.metadata)?,
                        build_fulfillment_audit_event(
                            FulfillmentItemAction::Reopen,
                            now,
                            &[],
                            None,
                            None,
                            STATUS_SHIPPED,
                        ),
                    )?);
                    active.updated_at = Set(now.into());
                    active.update(&txn).await?;
                    txn.commit().await?;

                    return self.get_fulfillment(tenant_id, fulfillment_id).await;
                }

                validate_item_quantity_adjustments(input.items.as_deref())?;
                let adjustment_plan = resolve_item_adjustments(
                    &items,
                    input.items.as_deref(),
                    FulfillmentItemAction::Reopen,
                )?;
                let adjustment_lookup =
                    adjustment_plan.into_iter().collect::<BTreeMap<Uuid, i32>>();
                let mut adjusted_entries = Vec::new();
                for item in items {
                    let adjustment = adjustment_lookup.get(&item.id).copied().unwrap_or_default();
                    if adjustment == 0 {
                        continue;
                    }

                    let mut active: entities::fulfillment_item::ActiveModel = item.clone().into();
                    active.delivered_quantity = Set(item.delivered_quantity - adjustment);
                    active.metadata = Set(append_audit_event(
                        item.metadata.clone(),
                        build_item_audit_event(FulfillmentItemAction::Reopen, now, adjustment),
                    )?);
                    active.updated_at = Set(now.into());
                    adjusted_entries.push((item.id, item.order_line_item_id, adjustment));
                    active.update(&txn).await?;
                }

                let mut active: entities::fulfillment::ActiveModel = fulfillment.into();
                let metadata = active.metadata.clone().take().unwrap_or_default();
                active.status = Set(STATUS_SHIPPED.to_string());
                active.delivered_note = Set(None);
                active.delivered_at = Set(None);
                active.metadata = Set(append_audit_event(
                    merge_fulfillment_metadata(metadata, input.metadata)?,
                    build_fulfillment_audit_event(
                        FulfillmentItemAction::Reopen,
                        now,
                        &adjusted_entries,
                        None,
                        None,
                        STATUS_SHIPPED,
                    ),
                )?);
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
        self.reship_fulfillment_internal(tenant_id, fulfillment_id, input, None)
            .await
    }

    /// Apply a provider-backed reship result after the provider operation has been journaled.
    pub(crate) async fn reship_fulfillment_with_provider_result(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        input: ReshipFulfillmentInput,
        provider_metadata: Value,
        operation_id: Uuid,
    ) -> FulfillmentResult<FulfillmentResponse> {
        self.reship_fulfillment_internal(
            tenant_id,
            fulfillment_id,
            input,
            Some((provider_metadata, operation_id)),
        )
        .await
    }

    async fn reship_fulfillment_internal(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        mut input: ReshipFulfillmentInput,
        provider_result: Option<(Value, Uuid)>,
    ) -> FulfillmentResult<FulfillmentResponse> {
        validate_tenant_id(tenant_id)?;
        input
            .validate()
            .map_err(|error| FulfillmentError::Validation(error.to_string()))?;
        input.metadata = match provider_result {
            Some((provider_metadata, operation_id)) => prepare_provider_lifecycle_metadata(
                input.metadata,
                provider_metadata,
                operation_id,
                "reship",
            )?,
            None => strip_provider_operation_metadata(input.metadata),
        };

        let txn = self.db.begin().await?;
        let fulfillment = self
            .load_fulfillment_for_update(&txn, tenant_id, fulfillment_id)
            .await?;
        if fulfillment.status != STATUS_DELIVERED {
            return Err(FulfillmentError::InvalidTransition {
                from: fulfillment.status,
                to: STATUS_SHIPPED.to_string(),
            });
        }

        let items = self.load_fulfillment_items(&txn, fulfillment.id).await?;
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
                merge_fulfillment_metadata(metadata, input.metadata)?,
                build_fulfillment_audit_event(
                    FulfillmentItemAction::Reship,
                    now,
                    &[],
                    Some(input.carrier),
                    Some(input.tracking_number),
                    STATUS_SHIPPED,
                ),
            )?);
            active.updated_at = Set(now.into());
            active.update(&txn).await?;
            txn.commit().await?;

            return self.get_fulfillment(tenant_id, fulfillment_id).await;
        }

        validate_item_quantity_adjustments(input.items.as_deref())?;
        let adjustment_plan = resolve_item_adjustments(
            &items,
            input.items.as_deref(),
            FulfillmentItemAction::Reship,
        )?;
        let adjustment_lookup = adjustment_plan.into_iter().collect::<BTreeMap<Uuid, i32>>();
        let mut adjusted_entries = Vec::new();
        for item in items {
            let adjustment = adjustment_lookup.get(&item.id).copied().unwrap_or_default();
            if adjustment == 0 {
                continue;
            }

            let mut active: entities::fulfillment_item::ActiveModel = item.clone().into();
            active.delivered_quantity = Set(item.delivered_quantity - adjustment);
            active.metadata = Set(append_audit_event(
                item.metadata.clone(),
                build_item_audit_event(FulfillmentItemAction::Reship, now, adjustment),
            )?);
            active.updated_at = Set(now.into());
            adjusted_entries.push((item.id, item.order_line_item_id, adjustment));
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
            merge_fulfillment_metadata(metadata, input.metadata)?,
            build_fulfillment_audit_event(
                FulfillmentItemAction::Reship,
                now,
                &adjusted_entries,
                Some(input.carrier),
                Some(input.tracking_number),
                STATUS_SHIPPED,
            ),
        )?);
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
        self.cancel_fulfillment_internal(tenant_id, fulfillment_id, input, None)
            .await
    }

    /// Apply a provider-backed cancellation result after the provider operation has been journaled.
    pub(crate) async fn cancel_fulfillment_with_provider_result(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        input: CancelFulfillmentInput,
        provider_metadata: Value,
        operation_id: Uuid,
    ) -> FulfillmentResult<FulfillmentResponse> {
        self.cancel_fulfillment_internal(
            tenant_id,
            fulfillment_id,
            input,
            Some((provider_metadata, operation_id)),
        )
        .await
    }

    async fn cancel_fulfillment_internal(
        &self,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        mut input: CancelFulfillmentInput,
        provider_result: Option<(Value, Uuid)>,
    ) -> FulfillmentResult<FulfillmentResponse> {
        validate_tenant_id(tenant_id)?;
        input
            .validate()
            .map_err(|error| FulfillmentError::Validation(error.to_string()))?;
        input.metadata = match provider_result {
            Some((provider_metadata, operation_id)) => prepare_provider_lifecycle_metadata(
                input.metadata,
                provider_metadata,
                operation_id,
                "cancel",
            )?,
            None => strip_provider_operation_metadata(input.metadata),
        };
        let txn = self.db.begin().await?;
        let fulfillment = self
            .load_fulfillment_for_update(&txn, tenant_id, fulfillment_id)
            .await?;
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
            merge_fulfillment_metadata(metadata, input.metadata)?,
            build_fulfillment_audit_event(
                FulfillmentItemAction::Cancel,
                now,
                &[],
                None,
                None,
                STATUS_CANCELLED,
            ),
        )?);
        active.cancelled_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());
        active.update(&txn).await?;
        txn.commit().await?;

        self.get_fulfillment(tenant_id, fulfillment_id).await
    }

    async fn begin_read_transaction(&self) -> FulfillmentResult<DatabaseTransaction> {
        // PostgreSQL uses an explicit repeatable-read/read-only transaction. SeaORM's
        // SQLite adapter does not support those two configuration knobs, but the
        // transaction itself still keeps every read on one SQLite snapshot.
        self.db
            .begin_with_config(Some(IsolationLevel::RepeatableRead), Some(AccessMode::ReadOnly))
            .await
            .map_err(Into::into)
    }

    async fn load_fulfillment<C>(
        &self,
        db: &C,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
    ) -> FulfillmentResult<entities::fulfillment::Model>
    where
        C: ConnectionTrait,
    {
        entities::fulfillment::Entity::find_by_id(fulfillment_id)
            .filter(entities::fulfillment::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .ok_or(FulfillmentError::FulfillmentNotFound(fulfillment_id))
    }

    async fn build_fulfillment_responses<C>(
        &self,
        db: &C,
        fulfillments: Vec<entities::fulfillment::Model>,
    ) -> FulfillmentResult<(
        Vec<entities::fulfillment::Model>,
        Vec<FulfillmentResponse>,
    )>
    where
        C: ConnectionTrait,
    {
        if fulfillments.is_empty() {
            return Ok((fulfillments, Vec::new()));
        }

        let fulfillment_ids = fulfillments
            .iter()
            .map(|fulfillment| fulfillment.id)
            .collect::<Vec<_>>();
        let item_rows = entities::fulfillment_item::Entity::find()
            .filter(entities::fulfillment_item::Column::FulfillmentId.is_in(fulfillment_ids))
            .order_by_asc(entities::fulfillment_item::Column::FulfillmentId)
            .order_by_asc(entities::fulfillment_item::Column::CreatedAt)
            .order_by_asc(entities::fulfillment_item::Column::Id)
            .all(db)
            .await?;

        let mut items_by_fulfillment = HashMap::<Uuid, Vec<entities::fulfillment_item::Model>>::new();
        for item in item_rows {
            items_by_fulfillment
                .entry(item.fulfillment_id)
                .or_default()
                .push(item);
        }

        let mut responses = Vec::with_capacity(fulfillments.len());
        for fulfillment in &fulfillments {
            responses.push(map_fulfillment(
                fulfillment.clone(),
                items_by_fulfillment
                    .remove(&fulfillment.id)
                    .unwrap_or_default(),
            ));
        }

        Ok((fulfillments, responses))
    }

    async fn build_fulfillment_response<C>(
        &self,
        db: &C,
        fulfillment: entities::fulfillment::Model,
    ) -> FulfillmentResult<FulfillmentResponse>
    where
        C: ConnectionTrait,
    {
        let items = entities::fulfillment_item::Entity::find()
            .filter(entities::fulfillment_item::Column::FulfillmentId.eq(fulfillment.id))
            .order_by_asc(entities::fulfillment_item::Column::CreatedAt)
            .order_by_asc(entities::fulfillment_item::Column::Id)
            .all(db)
            .await?;

        Ok(map_fulfillment(fulfillment, items))
    }

    /// Lifecycle commands serialize on the parent fulfillment row so their
    /// quantity decisions are made from one current snapshot before any item write.
    async fn load_fulfillment_for_update(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
    ) -> FulfillmentResult<entities::fulfillment::Model> {
        entities::fulfillment::Entity::find_by_id(fulfillment_id)
            .filter(entities::fulfillment::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(txn)
            .await?
            .ok_or(FulfillmentError::FulfillmentNotFound(fulfillment_id))
    }

    async fn load_fulfillment_items<C>(
        &self,
        db: &C,
        fulfillment_id: Uuid,
    ) -> FulfillmentResult<Vec<entities::fulfillment_item::Model>>
    where
        C: ConnectionTrait,
    {
        entities::fulfillment_item::Entity::find()
            .filter(entities::fulfillment_item::Column::FulfillmentId.eq(fulfillment_id))
            .order_by_asc(entities::fulfillment_item::Column::CreatedAt)
            .order_by_asc(entities::fulfillment_item::Column::Id)
            .all(db)
            .await
            .map_err(Into::into)
    }

    async fn set_shipping_option_active(
        &self,
        tenant_id: Uuid,
        shipping_option_id: Uuid,
        active: bool,
    ) -> FulfillmentResult<ShippingOptionResponse> {
        let txn = self.db.begin().await?;
        let shipping_option = entities::shipping_option::Entity::find_by_id(shipping_option_id)
            .filter(entities::shipping_option::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or(FulfillmentError::ShippingOptionNotFound(shipping_option_id))?;

        if shipping_option.active != active {
            let mut option: entities::shipping_option::ActiveModel = shipping_option.into();
            option.active = Set(active);
            option.updated_at = Set(Utc::now().into());
            let option = option.update(&txn).await?;
            let translation_rows =
                load_shipping_option_translation_rows(&txn, shipping_option_id).await?;
            let resource_revision =
                shipping_option_translation_resource_revision(&option, &translation_rows);
            record_shipping_option_translation_change_in_tx(
                &txn,
                tenant_id,
                shipping_option_id,
                generate_id(),
                &resource_revision,
                ShippingOptionTranslationChangeLifecycle::from(active),
            )
            .await
            .map_err(translation_change_error_to_fulfillment_error)?;
        }
        txn.commit().await?;

        self.get_shipping_option(tenant_id, shipping_option_id, None, None)
            .await
    }
}

fn validate_tenant_id(tenant_id: Uuid) -> FulfillmentResult<()> {
    if tenant_id.is_nil() {
        return Err(FulfillmentError::Validation(
            "tenant_id must not be nil".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn normalize_checkout_plan_hash(checkout_plan_hash: &str) -> FulfillmentResult<String> {
    let checkout_plan_hash = checkout_plan_hash.trim().to_ascii_lowercase();
    if checkout_plan_hash.len() != 64
        || !checkout_plan_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(FulfillmentError::Validation(
            "checkout fulfillment plan hash must be a 64-character hexadecimal value".to_string(),
        ));
    }
    Ok(checkout_plan_hash)
}

fn validate_checkout_identity(
    checkout_operation_id: Uuid,
    checkout_fulfillment_index: u32,
    checkout_plan_hash: &str,
) -> FulfillmentResult<String> {
    if checkout_operation_id.is_nil() {
        return Err(FulfillmentError::Validation(
            "checkout operation identity must be non-nil".to_string(),
        ));
    }
    let checkout_plan_hash = normalize_checkout_plan_hash(checkout_plan_hash)?;
    let _ = checkout_fulfillment_index;
    Ok(checkout_plan_hash)
}

fn normalize_provider_id(value: Option<String>) -> FulfillmentResult<String> {
    let provider_id = value
        .map(|provider_id| provider_id.trim().to_string())
        .filter(|provider_id| !provider_id.is_empty())
        .unwrap_or_else(|| MANUAL_PROVIDER_ID.to_string());
    crate::providers::validate_provider_id(&provider_id)?;
    Ok(provider_id)
}

fn normalize_currency_code(value: &str) -> FulfillmentResult<String> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.len() != 3
        || !normalized
            .chars()
            .all(|character| character.is_ascii_alphabetic())
    {
        return Err(FulfillmentError::Validation(
            "currency_code must be a 3-letter code".to_string(),
        ));
    }
    Ok(normalized)
}

fn fulfillment_list_offset(page: u64, per_page: u64) -> u64 {
    page.saturating_sub(1).saturating_mul(per_page)
}

fn merge_fulfillment_metadata(
    current: serde_json::Value,
    patch: serde_json::Value,
) -> FulfillmentResult<serde_json::Value> {
    let current = match current {
        Value::Object(object) => Value::Object(object),
        _ => {
            return Err(FulfillmentError::Validation(
                "fulfillment metadata must be a JSON object".to_string(),
            ));
        }
    };
    let current_audit = current
        .as_object()
        .and_then(|object| object.get("audit"))
        .cloned();
    validate_object_metadata(&patch, "fulfillment metadata patch")?;
    let mut merged = merge_metadata(current, strip_fulfillment_audit_metadata(patch));

    if let Some(audit) = current_audit {
        match &mut merged {
            Value::Object(object) => {
                object.insert("audit".to_string(), audit);
            }
            _ => {
                let mut object = Map::new();
                object.insert("audit".to_string(), audit);
                merged = Value::Object(object);
            }
        }
    }

    strip_fulfillment_identity_metadata(merged)
}

fn strip_fulfillment_metadata(value: serde_json::Value) -> FulfillmentResult<serde_json::Value> {
    let value = match value {
        Value::Object(_) => value,
        _ => {
            return Err(FulfillmentError::Validation(
                "fulfillment metadata must be a JSON object".to_string(),
            ));
        }
    };
    let value = strip_fulfillment_audit_metadata(value);
    let value = strip_provider_operation_metadata(value);
    strip_fulfillment_identity_metadata(value)
}

fn strip_fulfillment_audit_metadata(value: serde_json::Value) -> serde_json::Value {
    match value {
        Value::Object(mut object) => {
            object.remove("audit");
            Value::Object(object)
        }
        other => other,
    }
}

fn strip_provider_operation_metadata(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(mut object) => {
            object.remove("provider_operation");
            serde_json::Value::Object(object)
        }
        other => other,
    }
}

fn prepare_provider_lifecycle_metadata(
    input_metadata: Value,
    provider_metadata: Value,
    operation_id: Uuid,
    operation: &'static str,
) -> FulfillmentResult<Value> {
    if operation_id.is_nil() {
        return Err(FulfillmentError::Validation(
            "fulfillment provider operation id must not be nil".to_string(),
        ));
    }
    let input_metadata =
        strip_provider_operation_metadata(strip_fulfillment_audit_metadata(input_metadata));
    let merged = merge_fulfillment_metadata(input_metadata, provider_metadata)?;
    let mut object = match merged {
        Value::Object(object) => object,
        _ => unreachable!("fulfillment metadata merge returns an object"),
    };
    object.insert(
        "provider_operation".to_string(),
        serde_json::json!({
            "id": operation_id,
            "operation": operation,
        }),
    );
    Ok(Value::Object(object))
}

pub(crate) fn strip_fulfillment_identity_metadata(
    value: serde_json::Value,
) -> FulfillmentResult<serde_json::Value> {
    let mut root = match value {
        serde_json::Value::Object(object) => object,
        _ => return Ok(value),
    };
    if let Some(checkout) = root.remove("checkout") {
        let mut checkout = match checkout {
            serde_json::Value::Object(object) => object,
            _ => {
                return Err(FulfillmentError::Validation(
                    "fulfillment checkout metadata namespace must be a JSON object".to_string(),
                ));
            }
        };
        for key in [
            "operation_id",
            "order_id",
            "order_plan_hash",
            "fulfillment_index",
            "fulfillment_key",
        ] {
            checkout.remove(key);
        }
        if checkout.is_empty() {
            root.remove("checkout");
        } else {
            root.insert("checkout".to_string(), serde_json::Value::Object(checkout));
        }
    }
    Ok(serde_json::Value::Object(root))
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

fn normalize_allowed_shipping_profile_slugs(
    values: Option<Vec<String>>,
) -> FulfillmentResult<Option<Vec<String>>> {
    values
        .map(|values| {
            let mut normalized = BTreeSet::new();
            for value in values {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err(FulfillmentError::Validation(
                        "shipping profile slug must not be empty".to_string(),
                    ));
                }
                if trimmed.chars().count() > 64 {
                    return Err(FulfillmentError::Validation(
                        "shipping profile slug must be at most 64 characters".to_string(),
                    ));
                }
                let value = normalize_shipping_profile_slug(trimmed).ok_or_else(|| {
                    FulfillmentError::Validation(
                        "shipping profile slug must not be empty".to_string(),
                    )
                })?;
                normalized.insert(value);
            }
            Ok(normalized.into_iter().collect())
        })
        .transpose()
}

fn extract_allowed_shipping_profile_slugs(metadata: &Value) -> Option<Vec<String>> {
    let profiles = metadata.get("shipping_profiles")?;
    let Some(profiles) = profiles.as_object() else {
        return Some(Vec::new());
    };
    let Some(values) = profiles.get("allowed_slugs") else {
        return Some(Vec::new());
    };
    let Some(values) = values.as_array() else {
        return Some(Vec::new());
    };
    if values.is_empty() {
        return None;
    }

    let mut normalized = BTreeSet::new();
    for value in values {
        let Some(value) = value.as_str() else {
            return Some(Vec::new());
        };
        let Some(value) = normalize_shipping_profile_slug(value) else {
            return Some(Vec::new());
        };
        normalized.insert(value);
    }

    Some(normalized.into_iter().collect())
}

fn apply_allowed_shipping_profiles_to_metadata(
    metadata: Value,
    allowed_shipping_profile_slugs: Option<Vec<String>>,
) -> FulfillmentResult<Value> {
    let Some(allowed_shipping_profile_slugs) = allowed_shipping_profile_slugs else {
        if let Value::Object(object) = &metadata
            && let Some(shipping_profiles) = object.get("shipping_profiles")
            && !shipping_profiles.is_object()
        {
            return Err(FulfillmentError::Validation(
                "shipping option shipping_profiles metadata namespace must be a JSON object"
                    .to_string(),
            ));
        }
        return Ok(metadata);
    };

    let mut metadata_object = match metadata {
        Value::Object(object) => object,
        _ => {
            return Err(FulfillmentError::Validation(
                "shipping option metadata must be a JSON object when allowed shipping profiles are specified"
                    .to_string(),
            ));
        }
    };
    let mut shipping_profiles = match metadata_object.remove("shipping_profiles") {
        Some(Value::Object(object)) => object,
        Some(_) => {
            return Err(FulfillmentError::Validation(
                "shipping option shipping_profiles metadata namespace must be a JSON object"
                    .to_string(),
            ));
        }
        None => Map::new(),
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
    Ok(Value::Object(metadata_object))
}

pub(crate) fn strip_fulfillment_item_checkout_metadata(value: Value) -> FulfillmentResult<Value> {
    let mut root = match value {
        Value::Object(object) => object,
        _ => {
            return Err(FulfillmentError::Validation(
                "fulfillment item metadata must be a JSON object".to_string(),
            ));
        }
    };

    let Some(checkout) = root.remove("checkout") else {
        return Ok(Value::Object(root));
    };

    let mut checkout = match checkout {
        Value::Object(object) => object,
        _ => {
            return Err(FulfillmentError::Validation(
                "fulfillment item checkout metadata namespace must be a JSON object".to_string(),
            ));
        }
    };

    for key in [
        "operation_id",
        "order_id",
        "order_plan_hash",
        "fulfillment_index",
        "fulfillment_key",
    ] {
        checkout.remove(key);
    }

    if let Some(cart_line_item_id) = checkout.get("cart_line_item_id").cloned() {
        let canonical = cart_line_item_id
            .as_str()
            .and_then(|value| Uuid::parse_str(value).ok())
            .filter(|value| !value.is_nil())
            .map(|value| Value::String(value.to_string()))
            .ok_or_else(|| {
                FulfillmentError::Validation(
                    "fulfillment item checkout cart_line_item_id must be a non-nil UUID string"
                        .to_string(),
                )
            })?;
        checkout.insert("cart_line_item_id".to_string(), canonical);
    }

    if checkout.is_empty() {
        root.remove("checkout");
    } else {
        root.insert("checkout".to_string(), Value::Object(checkout));
    }

    Ok(Value::Object(root))
}

pub(crate) fn strip_fulfillment_item_metadata(value: Value) -> FulfillmentResult<Value> {
    // Item audit history is owner-generated lifecycle evidence and cannot be seeded by
    // create callers. Checkout identity sanitization remains separate and strict.
    let value = strip_fulfillment_audit_metadata(value);
    strip_fulfillment_item_checkout_metadata(value)
}

fn validate_object_metadata(metadata: &Value, resource: &str) -> FulfillmentResult<()> {
    if !metadata.is_object() {
        return Err(FulfillmentError::Validation(format!(
            "{resource} metadata must be a JSON object",
        )));
    }
    Ok(())
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

fn validate_item_progress_snapshot(
    item: &entities::fulfillment_item::Model,
) -> FulfillmentResult<()> {
    if item.quantity <= 0
        || item.shipped_quantity < 0
        || item.delivered_quantity < 0
        || item.delivered_quantity > item.shipped_quantity
        || item.shipped_quantity > item.quantity
    {
        return Err(FulfillmentError::Validation(
            "fulfillment item progress counters are inconsistent".to_string(),
        ));
    }
    Ok(())
}

fn resolve_item_adjustments(
    items: &[entities::fulfillment_item::Model],
    requested: Option<&[FulfillmentItemQuantityInput]>,
    action: FulfillmentItemAction,
) -> FulfillmentResult<Vec<(Uuid, i32)>> {
    for item in items {
        validate_item_progress_snapshot(item)?;
    }

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

fn append_audit_event(metadata: Value, event: Value) -> FulfillmentResult<Value> {
    let mut metadata_object = match metadata {
        Value::Object(object) => object,
        _ => {
            return Err(FulfillmentError::Validation(
                "fulfillment metadata must be a JSON object".to_string(),
            ));
        }
    };
    let mut audit = match metadata_object.remove("audit") {
        Some(Value::Object(object)) => object,
        Some(_) => {
            return Err(FulfillmentError::Validation(
                "fulfillment audit metadata namespace must be a JSON object".to_string(),
            ));
        }
        None => Map::new(),
    };
    let mut events = match audit.remove("events") {
        Some(Value::Array(items)) => items,
        Some(_) => {
            return Err(FulfillmentError::Validation(
                "fulfillment audit events must be a JSON array".to_string(),
            ));
        }
        None => Vec::new(),
    };
    events.push(event);
    audit.insert("events".to_string(), Value::Array(events));
    metadata_object.insert("audit".to_string(), Value::Object(audit));
    Ok(Value::Object(metadata_object))
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
        .order_by_asc(entities::shipping_option_translation::Column::ShippingOptionId)
        .order_by_asc(entities::shipping_option_translation::Column::Locale)
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

    rows.into_iter()
        .map(|row| {
            let translations = translations_by_option.remove(&row.id).unwrap_or_default();
            map_shipping_option(row, translations, requested_locale, tenant_default_locale)
        })
        .collect()
}

fn map_shipping_option(
    option: entities::shipping_option::Model,
    translations: Vec<entities::shipping_option_translation::Model>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> FulfillmentResult<ShippingOptionResponse> {
    validate_persisted_shipping_option_locales(&translations)?;
    let translation_revision =
        shipping_option_translation_resource_revision(&option, &translations);
    let available_locales = translations
        .iter()
        .filter(|translation| translation.locale != UNKNOWN_PROVENANCE_LOCALE)
        .map(|translation| translation.locale.clone())
        .collect::<Vec<_>>();
    let runtime_translations = translations
        .iter()
        .filter(|translation| translation.locale != UNKNOWN_PROVENANCE_LOCALE)
        .collect::<Vec<_>>();
    let requested_locale = requested_locale
        .and_then(normalize_locale_tag)
        .filter(|value| !value.is_empty());
    let (resolved, effective_locale) = resolve_translation(
        &runtime_translations,
        requested_locale.as_deref(),
        tenant_default_locale,
    );
    let name = resolved
        .map(|translation| translation.name.clone())
        .unwrap_or_default();

    Ok(ShippingOptionResponse {
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
        translation_revision,
        translations: translations
            .into_iter()
            .map(|translation| ShippingOptionTranslationResponse {
                locale: translation.locale,
                name: translation.name,
            })
            .collect(),
    })
}

fn validate_persisted_shipping_option_locales(
    translations: &[entities::shipping_option_translation::Model],
) -> FulfillmentResult<()> {
    let mut seen = HashSet::new();
    for translation in translations {
        if translation.locale == UNKNOWN_PROVENANCE_LOCALE {
            continue;
        }
        let locale = TenantLocale::new(&translation.locale).map_err(|error| {
            FulfillmentError::Validation(format!(
                "Shipping option contains an invalid persisted locale: {error}"
            ))
        })?;
        if locale.as_str() != translation.locale {
            return Err(FulfillmentError::Validation(
                "Shipping option contains a non-canonical persisted locale".to_string(),
            ));
        }
        if !seen.insert(locale.into_inner()) {
            return Err(FulfillmentError::Validation(
                "Shipping option contains duplicate canonical persisted locales".to_string(),
            ));
        }
    }
    Ok(())
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
        let locale = TenantLocale::new(&translation.locale)
            .map(TenantLocale::into_inner)
            .map_err(|_| FulfillmentError::Validation("Invalid locale".to_string()))?;
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
        if name.chars().count() > 120 {
            return Err(FulfillmentError::Validation(
                "Shipping option name must be at most 120 characters".to_string(),
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
    db: &DatabaseTransaction,
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

async fn synchronize_translations(
    db: &DatabaseTransaction,
    shipping_option_id: Uuid,
    translations: &[ShippingOptionTranslationInput],
) -> FulfillmentResult<bool> {
    let existing = load_shipping_option_translation_rows(db, shipping_option_id).await?;
    let mut desired = translations
        .iter()
        .map(|translation| (translation.locale.clone(), translation.name.clone()))
        .collect::<HashMap<_, _>>();
    let mut changed = false;

    for current in existing {
        match desired.remove(&current.locale) {
            Some(name) if name == current.name => {}
            Some(name) => {
                let mut active: entities::shipping_option_translation::ActiveModel = current.into();
                active.name = Set(name);
                active.update(db).await?;
                changed = true;
            }
            None => {
                entities::shipping_option_translation::Entity::delete_by_id(current.id)
                    .exec(db)
                    .await?;
                changed = true;
            }
        }
    }

    for (locale, name) in desired {
        entities::shipping_option_translation::ActiveModel {
            id: Set(generate_id()),
            shipping_option_id: Set(shipping_option_id),
            locale: Set(locale),
            name: Set(name),
        }
        .insert(db)
        .await?;
        changed = true;
    }

    Ok(changed)
}

async fn load_shipping_option_translation_rows<C>(
    db: &C,
    shipping_option_id: Uuid,
) -> FulfillmentResult<Vec<entities::shipping_option_translation::Model>>
where
    C: ConnectionTrait,
{
    Ok(entities::shipping_option_translation::Entity::find()
        .filter(
            entities::shipping_option_translation::Column::ShippingOptionId.eq(shipping_option_id),
        )
        .order_by_asc(entities::shipping_option_translation::Column::Locale)
        .all(db)
        .await?)
}

fn translation_change_error_to_fulfillment_error(
    error: ShippingOptionTranslationExactLocaleError,
) -> FulfillmentError {
    match error {
        ShippingOptionTranslationExactLocaleError::Database(error) => {
            FulfillmentError::Database(error)
        }
        other => FulfillmentError::Validation(format!(
            "Fulfillment translation change journal write failed: {other}"
        )),
    }
}

fn resolve_translation<'a>(
    translations: &'a [&'a entities::shipping_option_translation::Model],
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> (
    Option<&'a entities::shipping_option_translation::Model>,
    Option<String>,
) {
    let mut lookup = HashMap::new();
    for translation in translations {
        if let Some(normalized) = normalize_locale_tag(&translation.locale) {
            lookup.insert(normalized, *translation);
        }
    }

    if let Some(locale) = requested_locale.and_then(normalize_locale_tag)
        && let Some(found) = lookup.get(&locale)
    {
        return (Some(*found), Some(found.locale.clone()));
    }
    if let Some(locale) = tenant_default_locale.and_then(normalize_locale_tag)
        && let Some(found) = lookup.get(&locale)
    {
        return (Some(*found), Some(found.locale.clone()));
    }
    translations
        .first()
        .map(|item| (Some(*item), Some(item.locale.clone())))
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

#[cfg(test)]
mod tests {
    use super::{
        fulfillment_list_offset, map_shipping_option, validate_persisted_shipping_option_locales,
        CheckoutFulfillmentIdentity, FulfillmentService,
    };
    use crate::entities::{self, shipping_option, shipping_option_translation};
    use chrono::Utc;
    use rust_decimal::Decimal;
    use serde_json::Value;
    use uuid::Uuid;

    fn translation(
        shipping_option_id: Uuid,
        locale: &str,
        name: &str,
    ) -> shipping_option_translation::Model {
        shipping_option_translation::Model {
            id: Uuid::new_v4(),
            shipping_option_id,
            locale: locale.to_string(),
            name: name.to_string(),
        }
    }

    fn option(id: Uuid) -> shipping_option::Model {
        let now = Utc::now().into();
        shipping_option::Model {
            id,
            tenant_id: Uuid::new_v4(),
            currency_code: "USD".to_string(),
            amount: Decimal::new(1000, 2),
            provider_id: "manual".to_string(),
            active: true,
            metadata: serde_json::json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn checkout_identity_anchor_reuses_same_identity_and_rejects_conflicts() {
        use rustok_test_utils::db::setup_test_db;
        use sea_orm::{ConnectionTrait, Schema, TransactionTrait};

        let db = setup_test_db().await;
        let builder = db.get_database_backend();
        let schema = Schema::new(builder);
        let statement = schema
            .create_table_from_entity(entities::checkout_identity::Entity)
            .if_not_exists()
            .to_owned();
        db.execute_raw(builder.build(&statement))
            .await
            .expect("checkout identity anchor table should be created");

        let service = FulfillmentService::new(db.clone());
        let txn = db.begin().await.expect("transaction should start");
        let tenant_id = Uuid::new_v4();
        let operation_id = Uuid::new_v4();
        let order_id = Uuid::new_v4();
        let customer_id = Some(Uuid::new_v4());
        let identity = CheckoutFulfillmentIdentity {
            operation_id,
            index: 0,
            plan_hash: "a".repeat(64),
        };

        service
            .ensure_checkout_identity_anchor(&txn, tenant_id, order_id, customer_id, &identity)
            .await
            .expect("first checkout identity should bind");

        service
            .ensure_checkout_identity_anchor(&txn, tenant_id, order_id, customer_id, &identity)
            .await
            .expect("identical checkout identity should be idempotent");

        let conflict = service
            .ensure_checkout_identity_anchor(
                &txn,
                tenant_id,
                Uuid::new_v4(),
                customer_id,
                &identity,
            )
            .await;
        assert!(
            conflict.is_err(),
            "same operation must not bind a different order"
        );

        let conflict = service
            .ensure_checkout_identity_anchor(
                &txn,
                tenant_id,
                order_id,
                Some(Uuid::new_v4()),
                &identity,
            )
            .await;
        assert!(
            conflict.is_err(),
            "same operation must not bind a different customer"
        );

        let mut different_plan = identity.clone();
        different_plan.plan_hash = "b".repeat(64);
        let conflict = service
            .ensure_checkout_identity_anchor(
                &txn,
                tenant_id,
                order_id,
                customer_id,
                &different_plan,
            )
            .await;
        assert!(
            conflict.is_err(),
            "same operation must not bind a different plan"
        );

        txn.rollback().await.expect("transaction should roll back");
    }

    #[test]
    fn storage_only_und_is_not_used_as_runtime_shipping_option_locale() {
        let option_id = Uuid::new_v4();
        let result = map_shipping_option(
            option(option_id),
            vec![translation(option_id, "und", "Legacy name")],
            Some("de"),
            Some("en"),
        )
        .expect("storage-only locale should be allowed in persisted data");

        assert_eq!(result.name, "");
        assert_eq!(result.effective_locale, None);
        assert!(result.available_locales.is_empty());
        assert_eq!(result.translations.len(), 1);
        assert_eq!(result.translations[0].locale, "und");
    }

    #[test]
    fn persisted_locale_validation_rejects_invalid_and_noncanonical_rows() {
        let option_id = Uuid::new_v4();

        let invalid = validate_persisted_shipping_option_locales(&[translation(
            option_id,
            "not@a-locale",
            "Broken",
        )]);
        assert!(invalid.is_err());

        let noncanonical =
            validate_persisted_shipping_option_locales(&[translation(option_id, "EN", "Broken")]);
        assert!(noncanonical.is_err());
    }

    #[test]
    fn fulfillment_list_offset_saturates_extreme_page_values() {
        assert_eq!(fulfillment_list_offset(0, 0), 0);
        assert_eq!(fulfillment_list_offset(1, 100), 0);
        assert_eq!(fulfillment_list_offset(u64::MAX, 100), u64::MAX);
    }

    #[test]
    fn checkout_plan_hash_normalization_is_canonical() {
        let hash = "A".repeat(64);
        assert_eq!(
            super::normalize_checkout_plan_hash(&hash).expect("valid hash"),
            "a".repeat(64)
        );
    }

    #[test]
    fn validate_tenant_id_rejects_nil_identity() {
        assert!(super::validate_tenant_id(Uuid::nil()).is_err());
        assert!(super::validate_tenant_id(Uuid::new_v4()).is_ok());
    }

    #[test]
    fn normalize_translation_inputs_rejects_oversized_shipping_option_name() {
        let name = "x".repeat(121);
        let result =
            super::normalize_translation_inputs(vec![crate::dto::ShippingOptionTranslationInput {
                locale: "en".to_string(),
                name,
            }]);

        assert!(result.is_err());
    }

    #[test]
    fn item_progress_validation_rejects_inconsistent_persisted_counters() {
        let now = Utc::now().into();
        let item = entities::fulfillment_item::Model {
            id: Uuid::new_v4(),
            fulfillment_id: Uuid::new_v4(),
            order_line_item_id: Uuid::new_v4(),
            quantity: 1,
            shipped_quantity: 2,
            delivered_quantity: 0,
            metadata: serde_json::json!({}),
            created_at: now,
            updated_at: now,
        };

        assert!(super::validate_item_progress_snapshot(&item).is_err());

        let item = entities::fulfillment_item::Model {
            shipped_quantity: 0,
            delivered_quantity: -1,
            ..item
        };
        assert!(super::validate_item_progress_snapshot(&item).is_err());
    }

    #[test]
    fn item_metadata_drops_caller_supplied_audit_history() {
        let metadata = serde_json::json!({
            "audit": {
                "events": [
                    {
                        "type": "ship",
                        "at": "2000-01-01T00:00:00Z",
                        "quantity": 999
                    }
                ]
            },
            "checkout": {
                "cart_line_item_id": Uuid::new_v4().to_string()
            },
            "note": "keep"
        });

        let sanitized = super::strip_fulfillment_item_metadata(metadata)
            .expect("item metadata should sanitize");
        assert!(sanitized.get("audit").is_none());
        assert_eq!(sanitized.get("note").and_then(Value::as_str), Some("keep"));
        assert!(
            sanitized
                .get("checkout")
                .and_then(|value| value.get("cart_line_item_id"))
                .and_then(Value::as_str)
                .is_some()
        );
    }

    #[test]
    fn item_checkout_metadata_removes_legacy_identity_keys() {
        let cart_line_item_id = Uuid::new_v4();
        let metadata = serde_json::json!({
            "checkout": {
                "operation_id": Uuid::new_v4().to_string(),
                "order_id": Uuid::new_v4().to_string(),
                "order_plan_hash": "a".repeat(64),
                "fulfillment_index": 4,
                "fulfillment_key": "legacy",
                "cart_line_item_id": cart_line_item_id.to_string()
            },
            "note": "keep"
        });

        let sanitized = super::strip_fulfillment_item_checkout_metadata(metadata)
            .expect("valid item checkout metadata should sanitize");
        let canonical_cart_line_item_id = cart_line_item_id.to_string();
        assert_eq!(
            sanitized
                .get("checkout")
                .and_then(|value| value.get("cart_line_item_id"))
                .and_then(Value::as_str),
            Some(canonical_cart_line_item_id.as_str())
        );
        assert!(
            sanitized
                .get("checkout")
                .and_then(|value| value.get("operation_id"))
                .is_none()
        );
        assert_eq!(sanitized.get("note").and_then(Value::as_str), Some("keep"));
    }

    #[test]
    fn item_checkout_metadata_rejects_invalid_cart_line_identity() {
        let metadata = serde_json::json!({
            "checkout": {
                "cart_line_item_id": "not-a-uuid"
            }
        });

        assert!(super::strip_fulfillment_item_checkout_metadata(metadata).is_err());
    }

    #[test]
    fn fulfillment_metadata_rejects_malformed_checkout_namespace() {
        let metadata = serde_json::json!({
            "checkout": "not-an-object",
            "customer_note": "keep"
        });

        assert!(super::strip_fulfillment_metadata(metadata).is_err());
    }

    #[test]
    fn create_fulfillment_metadata_requires_object_shape() {
        assert!(
            super::validate_object_metadata(&serde_json::json!("legacy"), "fulfillment").is_err()
        );
        assert!(
            super::validate_object_metadata(&serde_json::json!([]), "fulfillment item").is_err()
        );
        assert!(super::validate_object_metadata(&serde_json::json!({}), "fulfillment").is_ok());
    }

    #[test]
    fn merge_fulfillment_metadata_rejects_non_object_persisted_metadata() {
        assert!(
            super::merge_fulfillment_metadata(
                serde_json::json!("legacy scalar"),
                serde_json::json!({"customer_note": "replacement"}),
            )
            .is_err()
        );
    }

    #[test]
    fn merge_fulfillment_metadata_rejects_non_object_patch() {
        let current = serde_json::json!({
            "customer_note": "keep",
            "shipping_profile": "express",
            "audit": {
                "events": [{"type": "ship"}]
            }
        });

        assert!(
            super::merge_fulfillment_metadata(current, serde_json::json!("legacy scalar")).is_err()
        );
        assert!(
            super::merge_fulfillment_metadata(
                serde_json::json!({
                    "customer_note": "keep",
                    "shipping_profile": "express",
                    "audit": {
                        "events": [{"type": "ship"}]
                    }
                }),
                serde_json::json!(["legacy", "array"]),
            )
            .is_err()
        );
    }

    #[test]
    fn append_audit_event_rejects_malformed_audit_namespace() {
        assert!(
            super::append_audit_event(
                serde_json::json!({"audit": "legacy scalar"}),
                serde_json::json!({"type": "ship"}),
            )
            .is_err()
        );
    }

    #[test]
    fn append_audit_event_rejects_malformed_audit_events() {
        assert!(
            super::append_audit_event(
                serde_json::json!({"audit": {"events": "legacy scalar"}}),
                serde_json::json!({"type": "ship"}),
            )
            .is_err()
        );
    }

    #[test]
    fn append_audit_event_rejects_non_object_metadata() {
        assert!(
            super::append_audit_event(
                serde_json::json!("legacy scalar"),
                serde_json::json!({"type": "ship"}),
            )
            .is_err()
        );
    }

    #[test]
    fn apply_shipping_profile_projection_rejects_non_object_metadata() {
        assert!(
            super::apply_allowed_shipping_profiles_to_metadata(
                serde_json::json!("legacy scalar"),
                Some(vec!["bulky".to_string()]),
            )
            .is_err()
        );
        assert_eq!(
            super::apply_allowed_shipping_profiles_to_metadata(
                serde_json::json!({"customer_note": "keep"}),
                Some(vec!["bulky".to_string()]),
            )
            .expect("object metadata is valid")
            .get("shipping_profiles")
            .and_then(|value| value.get("allowed_slugs"))
            .and_then(Value::as_array)
            .map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn shipping_profile_namespace_is_validated_without_typed_restriction() {
        for malformed in [
            serde_json::json!({"shipping_profiles": "legacy scalar"}),
            serde_json::json!({"shipping_profiles": ["legacy", "array"]}),
            serde_json::json!({"shipping_profiles": null}),
        ] {
            assert!(super::apply_allowed_shipping_profiles_to_metadata(malformed, None).is_err());
        }

        let untouched = serde_json::json!({
            "customer_note": "keep",
            "shipping_profiles": {
                "legacy_flag": true
            }
        });
        assert_eq!(
            super::apply_allowed_shipping_profiles_to_metadata(untouched.clone(), None)
                .expect("valid namespace should be preserved"),
            untouched
        );

        let scalar_root = serde_json::json!("legacy scalar");
        assert_eq!(
            super::apply_allowed_shipping_profiles_to_metadata(scalar_root.clone(), None)
                .expect("unrelated scalar metadata remains supported"),
            scalar_root
        );
    }

    #[test]
    fn apply_shipping_profile_projection_rejects_malformed_existing_namespace() {
        for malformed in [
            serde_json::json!({"shipping_profiles": "legacy scalar"}),
            serde_json::json!({"shipping_profiles": ["legacy", "array"]}),
            serde_json::json!({"shipping_profiles": null}),
        ] {
            assert!(
                super::apply_allowed_shipping_profiles_to_metadata(
                    malformed,
                    Some(vec!["bulky".to_string()]),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn apply_shipping_profile_projection_preserves_existing_namespace_fields() {
        let projected = super::apply_allowed_shipping_profiles_to_metadata(
            serde_json::json!({
                "customer_note": "keep",
                "shipping_profiles": {
                    "source": "legacy",
                    "allowed_slugs": ["old"],
                }
            }),
            Some(vec!["bulky".to_string(), "standard".to_string()]),
        )
        .expect("valid shipping-profile namespace should project");

        assert_eq!(
            projected.get("customer_note").and_then(Value::as_str),
            Some("keep")
        );
        assert_eq!(
            projected
                .get("shipping_profiles")
                .and_then(|value| value.get("source"))
                .and_then(Value::as_str),
            Some("legacy")
        );
        assert_eq!(
            projected
                .get("shipping_profiles")
                .and_then(|value| value.get("allowed_slugs"))
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn empty_allowed_shipping_profiles_mean_unrestricted() {
        assert_eq!(
            super::extract_allowed_shipping_profile_slugs(&serde_json::json!({
                "shipping_profiles": {
                    "allowed_slugs": []
                }
            })),
            None
        );
    }

    #[test]
    fn malformed_allowed_shipping_profile_entries_fail_closed() {
        assert_eq!(
            super::extract_allowed_shipping_profile_slugs(&serde_json::json!({
                "shipping_profiles": {
                    "allowed_slugs": [""]
                }
            })),
            Some(Vec::new())
        );
        assert_eq!(
            super::extract_allowed_shipping_profile_slugs(&serde_json::json!({
                "shipping_profiles": {
                    "allowed_slugs": [123]
                }
            })),
            Some(Vec::new())
        );
    }

    #[test]
    fn malformed_shipping_profile_metadata_fails_closed() {
        assert_eq!(
            super::extract_allowed_shipping_profile_slugs(&serde_json::json!({
                "shipping_profiles": "not-an-object"
            })),
            Some(Vec::new())
        );
        assert_eq!(
            super::extract_allowed_shipping_profile_slugs(&serde_json::json!({
                "shipping_profiles": {}
            })),
            Some(Vec::new())
        );
        assert_eq!(
            super::extract_allowed_shipping_profile_slugs(&serde_json::json!({
                "shipping_profiles": {
                    "allowed_slugs": "not-an-array"
                }
            })),
            Some(Vec::new())
        );
        assert_eq!(
            super::extract_allowed_shipping_profile_slugs(&serde_json::json!({})),
            None
        );
    }

    #[test]
    fn typed_shipping_profile_allow_list_rejects_blank_entries() {
        assert!(
            super::normalize_allowed_shipping_profile_slugs(Some(vec![
                "bulky".to_string(),
                "   ".to_string(),
            ]))
            .is_err()
        );
        assert!(
            super::normalize_allowed_shipping_profile_slugs(Some(vec!["   ".to_string()])).is_err()
        );
    }

    #[test]
    fn typed_shipping_profile_allow_list_reserves_empty_list_for_unrestricted() {
        assert_eq!(
            super::normalize_allowed_shipping_profile_slugs(Some(Vec::new()))
                .expect("explicit empty allow-list is valid"),
            Some(Vec::new())
        );
    }

    #[test]
    fn typed_shipping_profile_allow_list_matches_profile_slug_limit() {
        let valid = "x".repeat(64);
        assert_eq!(
            super::normalize_allowed_shipping_profile_slugs(Some(vec![valid.clone()]))
                .expect("64-character profile slug is valid"),
            Some(vec![valid])
        );

        assert!(
            super::normalize_allowed_shipping_profile_slugs(Some(vec!["x".repeat(65)])).is_err()
        );
    }

    #[test]
    fn normalize_provider_id_uses_registry_identifier_rules() {
        assert!(super::normalize_provider_id(Some("PayPal".to_string())).is_err());
        assert!(super::normalize_provider_id(Some("foo.bar".to_string())).is_err());
        assert_eq!(
            super::normalize_provider_id(Some(" carrier-1 ".to_string()))
                .expect("valid provider id"),
            "carrier-1"
        );
        assert_eq!(
            super::normalize_provider_id(Some("   ".to_string()))
                .expect("blank provider id uses manual"),
            "manual"
        );
    }

    #[test]
    fn normalize_currency_code_rejects_non_letters() {
        assert!(super::normalize_currency_code("$$$").is_err());
        assert!(super::normalize_currency_code("123").is_err());
    }

    #[test]
    fn normalize_currency_code_canonicalizes_valid_codes() {
        assert_eq!(
            super::normalize_currency_code(" usd ").expect("valid currency"),
            "USD"
        );
    }

    #[test]
    fn normalize_translation_inputs_rejects_storage_only_unknown_provenance_locale() {
        let result =
            super::normalize_translation_inputs(vec![crate::dto::ShippingOptionTranslationInput {
                locale: "und".to_string(),
                name: "Express".to_string(),
            }]);

        assert!(result.is_err());
    }

    #[test]
    fn fulfillment_metadata_merge_preserves_owner_audit_history() {
        let current = serde_json::json!({
            "customer_note": "keep",
            "audit": {
                "events": [
                    {"type": "ship"},
                    {"type": "deliver"}
                ]
            }
        });
        let patch = serde_json::json!({
            "customer_note": "updated",
            "audit": {
                "events": [
                    {"type": "fabricated"},
                    {"type": "fabricated"}
                ]
            }
        });

        let merged =
            super::merge_fulfillment_metadata(current, patch).expect("valid metadata should merge");

        assert_eq!(
            merged.get("customer_note").and_then(Value::as_str),
            Some("updated")
        );
        assert_eq!(
            merged
                .get("audit")
                .and_then(|audit| audit.get("events"))
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(2)
        );
        assert_eq!(
            merged
                .get("audit")
                .and_then(|audit| audit.get("events"))
                .and_then(Value::as_array)
                .and_then(|events| events.first())
                .and_then(|event| event.get("type"))
                .and_then(Value::as_str),
            Some("ship")
        );
    }

    #[test]
    fn provider_backed_lifecycle_metadata_replaces_caller_receipt_with_journal_receipt() {
        let caller_operation_id = Uuid::new_v4();
        let journal_operation_id = Uuid::new_v4();
        let journal_operation_id_string = journal_operation_id.to_string();
        let metadata = super::prepare_provider_lifecycle_metadata(
            serde_json::json!({
                "provider_operation": {
                    "id": caller_operation_id,
                    "operation": "cancel"
                },
                "customer_note": "keep",
                "audit": {
                    "events": [{"type": "fabricated"}]
                }
            }),
            serde_json::json!({
                "provider_field": "keep",
                "provider_operation": {
                    "id": caller_operation_id,
                    "operation": "ship"
                }
            }),
            journal_operation_id,
            "ship",
        )
        .expect("provider-backed metadata should be normalized");

        assert_eq!(
            metadata.get("customer_note").and_then(Value::as_str),
            Some("keep")
        );
        assert_eq!(
            metadata.get("provider_field").and_then(Value::as_str),
            Some("keep")
        );
        assert_eq!(
            metadata
                .get("provider_operation")
                .and_then(|value| value.get("id"))
                .and_then(Value::as_str),
            Some(journal_operation_id_string.as_str())
        );
        assert_eq!(
            metadata
                .get("provider_operation")
                .and_then(|value| value.get("operation"))
                .and_then(Value::as_str),
            Some("ship")
        );
        assert!(metadata.get("audit").is_none());
    }

    #[test]
    fn provider_backed_lifecycle_metadata_rejects_nil_journal_identity() {
        let result = super::prepare_provider_lifecycle_metadata(
            serde_json::json!({}),
            serde_json::json!({}),
            Uuid::nil(),
            "ship",
        );

        assert!(result.is_err());
    }

    #[test]
    fn fulfillment_metadata_sanitization_removes_user_audit_data() {
        let value = serde_json::json!({
            "audit": {
                "events": [{"type": "fabricated"}]
            },
            "provider_operation": {
                "id": Uuid::new_v4().to_string()
            },
            "customer_note": "keep"
        });

        let sanitized =
            super::strip_fulfillment_metadata(value).expect("valid metadata should sanitize");

        assert_eq!(
            sanitized.get("customer_note").and_then(Value::as_str),
            Some("keep")
        );
        assert!(sanitized.get("audit").is_none());
        assert!(sanitized.get("provider_operation").is_none());
    }
}
