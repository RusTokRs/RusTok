use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{
    AccessMode, ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection,
    DatabaseTransaction, EntityTrait, IsolationLevel, JoinType, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, RelationTrait, Set, TransactionTrait, sea_query::{Expr, OnConflict, Query},
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

    pub(crate) fn database(&self) -> &DatabaseConnection {
        &self.db
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id))]
    pub async fn create_shipping_option(
        &self,
        tenant_id: Uuid,
        input: CreateShippingOptionInput,
    ) -> FulfillmentResult<ShippingOptionResponse> {
        let txn = self.db.begin().await?;
        let response = self
            .create_shipping_option_in_txn(&txn, tenant_id, input, generate_id())
            .await?;
        txn.commit().await?;
        Ok(response)
    }

    /// Creates a Shipping Option inside a caller-owned transaction.
    ///
    /// The caller supplies the durable operation identity that is written to the
    /// translation-change journal. This lets admin idempotency receipts and the
    /// localized-copy change evidence commit atomically with the resource itself.
    pub(crate) async fn create_shipping_option_in_txn(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        input: CreateShippingOptionInput,
        operation_id: Uuid,
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
        .insert(txn)
        .await?;

        insert_translations(txn, shipping_option_id, &translations).await?;
        let translation_rows =
            load_shipping_option_translation_rows(txn, tenant_id, shipping_option_id).await?;
        let resource_revision =
            shipping_option_translation_resource_revision(&option, &translation_rows);
        record_shipping_option_translation_change_in_tx(
            txn,
            tenant_id,
            shipping_option_id,
            operation_id,
            &resource_revision,
            ShippingOptionTranslationChangeLifecycle::Active,
        )
        .await
        .map_err(translation_change_error_to_fulfillment_error)?;

        map_shipping_option(option, translation_rows, None, None)
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
            tenant_id,
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
            tenant_id,
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
        let txn = self.db.begin().await?;
        let response = self
            .update_shipping_option_in_txn(
                &txn,
                tenant_id,
                shipping_option_id,
                input,
                generate_id(),
            )
            .await?;
        txn.commit().await?;
        Ok(response)
    }

    pub(crate) async fn update_shipping_option_in_txn(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        shipping_option_id: Uuid,
        input: UpdateShippingOptionInput,
        operation_id: Uuid,
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

        let shipping_option = entities::shipping_option::Entity::find_by_id(shipping_option_id)
            .filter(entities::shipping_option::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(txn)
            .await?
            .ok_or(FulfillmentError::ShippingOptionNotFound(shipping_option_id))?;

        if let Some(expected_revision) = expected_translation_revision.as_deref() {
            let current_translations =
                load_shipping_option_translation_rows(txn, tenant_id, shipping_option_id).await?;
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
        let option = active.update(txn).await?;

        let localized_copy_changed = match translations.as_deref() {
            Some(translations) => {
                synchronize_translations(txn, tenant_id, shipping_option_id, translations).await?
            }
            None => false,
        };
        if localized_copy_changed {
            let translation_rows =
                load_shipping_option_translation_rows(txn, tenant_id, shipping_option_id).await?;
            let resource_revision =
                shipping_option_translation_resource_revision(&option, &translation_rows);
            record_shipping_option_translation_change_in_tx(
                txn,
                tenant_id,
                shipping_option_id,
                operation_id,
                &resource_revision,
                ShippingOptionTranslationChangeLifecycle::from(option.active),
            )
            .await
            .map_err(translation_change_error_to_fulfillment_error)?;
        }

        let translation_rows =
            load_shipping_option_translation_rows(txn, tenant_id, shipping_option_id).await?;
        map_shipping_option(option, translation_rows, None, None)
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
            tenant_id,
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
    pub(crate) async fn create_fulfillment_in_txn(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        fulfillment_id: Uuid,
        input: CreateFulfillmentInput,
    ) -> FulfillmentResult<()> {
        validate_tenant_id(tenant_id)?;
        self.validate_create_fulfillment_input(tenant_id, &input).await?;
        self.insert_fulfillment_in_txn(
            txn,
            tenant_id,
            fulfillment_id,
            input,
            None,
            Utc::now(),
        )
        .await
    }

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
        let response = self
            .build_fulfillment_response(&txn, tenant_id, fulfillment)
            .await?;
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
        let (rows, fulfillments) = self
            .build_fulfillment_responses(&txn, tenant_id, rows)
            .await?;

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
            Some(fulfillment) => Some(
                self.build_fulfillment_response(&txn, tenant_id, fulfillment)
                    .await?,
            ),
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

        let (_, items) = self
            .build_fulfillment_responses(&txn, tenant_id, rows)
            .await?;
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

        let (_, items) = self
            .build_fulfillment_responses(&txn, tenant_id, rows)
            .await?;
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
        let items = self.load_fulfillment_items(&txn, tenant_id, fulfillment.id).await?;
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
        let items = self.load_fulfillment_items(&txn, tenant_id, fulfillment.id).await?;
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
                let items = self.load_fulfillment_items(&txn, tenant_id, fulfillment.id).await?;
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
                let items = self.load_fulfillment_items(&txn, tenant_id, fulfillment.id).await?;
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

        let items = self.load_fulfillment_items(&txn, tenant_id, fulfillment.id).await?;
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
        tenant_id: Uuid,
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
            .join(
                JoinType::InnerJoin,
                entities::fulfillment_item::Relation::Fulfillment.def(),
            )
            .filter(entities::fulfillment::Column::TenantId.eq(tenant_id))
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
        tenant_id: Uuid,
        fulfillment: entities::fulfillment::Model,
    ) -> FulfillmentResult<FulfillmentResponse>
    where
        C: ConnectionTrait,
    {
        let items = entities::fulfillment_item::Entity::find()
            .join(
                JoinType::InnerJoin,
                entities::fulfillment_item::Relation::Fulfillment.def(),
            )
            .filter(entities::fulfillment::Column::TenantId.eq(tenant_id))
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
        tenant_id: Uuid,
        fulfillment_id: Uuid,
    ) -> FulfillmentResult<Vec<entities::fulfillment_item::Model>>
    where
        C: ConnectionTrait,
    {
        entities::fulfillment_item::Entity::find()
            .join(
                JoinType::InnerJoin,
                entities::fulfillment_item::Relation::Fulfillment.def(),
            )
            .filter(entities::fulfillment::Column::TenantId.eq(tenant_id))
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
        let response = self
            .set_shipping_option_active_in_txn(
                &txn,
                tenant_id,
                shipping_option_id,
                active,
                generate_id(),
            )
            .await?;
        txn.commit().await?;
        Ok(response)
    }

    pub(crate) async fn set_shipping_option_active_in_txn(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        shipping_option_id: Uuid,
        active: bool,
        operation_id: Uuid,
    ) -> FulfillmentResult<ShippingOptionResponse> {
        validate_tenant_id(tenant_id)?;
        let shipping_option = entities::shipping_option::Entity::find_by_id(shipping_option_id)
            .filter(entities::shipping_option::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(txn)
            .await?
            .ok_or(FulfillmentError::ShippingOptionNotFound(shipping_option_id))?;

        let option = if shipping_option.active != active {
            let mut option: entities::shipping_option::ActiveModel = shipping_option.into();
            option.active = Set(active);
            option.updated_at = Set(Utc::now().into());
            let option = option.update(txn).await?;
            let translation_rows =
                load_shipping_option_translation_rows(txn, tenant_id, shipping_option_id).await?;
            let resource_revision =
                shipping_option_translation_resource_revision(&option, &translation_rows);
            record_shipping_option_translation_change_in_tx(
                txn,
                tenant_id,
                shipping_option_id,
                operation_id,
                &resource_revision,
                ShippingOptionTranslationChangeLifecycle::from(active),
            )
            .await
            .map_err(translation_change_error_to_fulfillment_error)?;
            option
        } else {
            shipping_option
        };

        let translation_rows =
            load_shipping_option_translation_rows(txn, tenant_id, shipping_option_id).await?;
        map_shipping_option(option, translation_rows, None, None)
    }
}
