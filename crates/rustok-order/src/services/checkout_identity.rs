use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter,
    Set, TransactionTrait,
};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::{order, order_checkout_identity};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordOrderCheckoutIdentity {
    pub tenant_id: Uuid,
    pub checkout_operation_id: Uuid,
    pub order_id: Uuid,
    pub source_cart_id: Uuid,
    pub payment_collection_id: Option<Uuid>,
    pub shipping_option_id: Option<Uuid>,
    pub snapshot_hash: String,
    pub request_hash: String,
}

#[derive(Debug, Error)]
pub enum OrderCheckoutIdentityError {
    #[error("order checkout identity validation failed: {0}")]
    Validation(String),
    #[error("order checkout identity conflict: {0}")]
    Conflict(String),
    #[error("order {0} was not found for checkout identity")]
    OrderNotFound(Uuid),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub type OrderCheckoutIdentityResult<T> = Result<T, OrderCheckoutIdentityError>;

#[derive(Clone)]
pub struct OrderCheckoutIdentityJournal {
    db: DatabaseConnection,
}

impl OrderCheckoutIdentityJournal {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn get_by_operation(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
    ) -> OrderCheckoutIdentityResult<Option<order_checkout_identity::Model>> {
        order_checkout_identity::Entity::find_by_id(checkout_operation_id)
            .filter(order_checkout_identity::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn get_by_order(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
    ) -> OrderCheckoutIdentityResult<Option<order_checkout_identity::Model>> {
        order_checkout_identity::Entity::find()
            .filter(order_checkout_identity::Column::TenantId.eq(tenant_id))
            .filter(order_checkout_identity::Column::OrderId.eq(order_id))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn get_by_cart(
        &self,
        tenant_id: Uuid,
        source_cart_id: Uuid,
    ) -> OrderCheckoutIdentityResult<Option<order_checkout_identity::Model>> {
        order_checkout_identity::Entity::find()
            .filter(order_checkout_identity::Column::TenantId.eq(tenant_id))
            .filter(order_checkout_identity::Column::SourceCartId.eq(source_cart_id))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn record(
        &self,
        input: RecordOrderCheckoutIdentity,
    ) -> OrderCheckoutIdentityResult<order_checkout_identity::Model> {
        let input = normalize_input(input)?;
        let transaction = self.db.begin().await?;
        let order_exists = order::Entity::find_by_id(input.order_id)
            .filter(order::Column::TenantId.eq(input.tenant_id))
            .one(&transaction)
            .await?
            .is_some();
        if !order_exists {
            transaction.rollback().await?;
            return Err(OrderCheckoutIdentityError::OrderNotFound(input.order_id));
        }

        if let Some(existing) = order_checkout_identity::Entity::find_by_id(
            input.checkout_operation_id,
        )
        .filter(order_checkout_identity::Column::TenantId.eq(input.tenant_id))
        .one(&transaction)
        .await?
        {
            return enrich_existing_identity(transaction, existing, &input).await;
        }

        let insert = order_checkout_identity::ActiveModel {
            checkout_operation_id: Set(input.checkout_operation_id),
            tenant_id: Set(input.tenant_id),
            order_id: Set(input.order_id),
            source_cart_id: Set(Some(input.source_cart_id)),
            payment_collection_id: Set(input.payment_collection_id),
            shipping_option_id: Set(input.shipping_option_id),
            snapshot_hash: Set(Some(input.snapshot_hash.clone())),
            request_hash: Set(Some(input.request_hash.clone())),
            created_at: Set(Utc::now().fixed_offset()),
        }
        .insert(&transaction)
        .await;

        match insert {
            Ok(model) => {
                transaction.commit().await?;
                Ok(model)
            }
            Err(insert_error) => {
                transaction.rollback().await?;
                let existing = order_checkout_identity::Entity::find_by_id(
                    input.checkout_operation_id,
                )
                .one(&self.db)
                .await?;
                if let Some(existing) = existing {
                    if existing.tenant_id != input.tenant_id {
                        return Err(OrderCheckoutIdentityError::Conflict(format!(
                            "checkout operation {} is already bound outside the requested tenant",
                            input.checkout_operation_id
                        )));
                    }
                    let transaction = self.db.begin().await?;
                    return enrich_existing_identity(transaction, existing, &input).await;
                }
                if let Some(existing) = self
                    .get_by_order(input.tenant_id, input.order_id)
                    .await?
                {
                    return Err(OrderCheckoutIdentityError::Conflict(format!(
                        "order {} is already bound to checkout operation {}",
                        input.order_id, existing.checkout_operation_id
                    )));
                }
                if let Some(existing) = self
                    .get_by_cart(input.tenant_id, input.source_cart_id)
                    .await?
                {
                    return Err(OrderCheckoutIdentityError::Conflict(format!(
                        "cart {} is already bound to order {}",
                        input.source_cart_id, existing.order_id
                    )));
                }
                Err(OrderCheckoutIdentityError::Database(insert_error))
            }
        }
    }
}

async fn enrich_existing_identity(
    transaction: sea_orm::DatabaseTransaction,
    existing: order_checkout_identity::Model,
    input: &RecordOrderCheckoutIdentity,
) -> OrderCheckoutIdentityResult<order_checkout_identity::Model> {
    ensure_compatible_identity(&existing, input)?;
    let needs_update = existing.source_cart_id.is_none()
        || existing.payment_collection_id.is_none() && input.payment_collection_id.is_some()
        || existing.shipping_option_id.is_none() && input.shipping_option_id.is_some()
        || existing.snapshot_hash.is_none()
        || existing.request_hash.is_none();
    if !needs_update {
        transaction.commit().await?;
        return Ok(existing);
    }

    let mut active = existing.clone().into_active_model();
    if existing.source_cart_id.is_none() {
        active.source_cart_id = Set(Some(input.source_cart_id));
    }
    if existing.payment_collection_id.is_none() && input.payment_collection_id.is_some() {
        active.payment_collection_id = Set(input.payment_collection_id);
    }
    if existing.shipping_option_id.is_none() && input.shipping_option_id.is_some() {
        active.shipping_option_id = Set(input.shipping_option_id);
    }
    if existing.snapshot_hash.is_none() {
        active.snapshot_hash = Set(Some(input.snapshot_hash.clone()));
    }
    if existing.request_hash.is_none() {
        active.request_hash = Set(Some(input.request_hash.clone()));
    }

    match active.update(&transaction).await {
        Ok(model) => {
            ensure_exact_identity(&model, input)?;
            transaction.commit().await?;
            Ok(model)
        }
        Err(update_error) => {
            transaction.rollback().await?;
            let current = order_checkout_identity::Entity::find_by_id(input.checkout_operation_id)
                .one(existing_connection_placeholder())
                .await;
            drop(current);
            Err(OrderCheckoutIdentityError::Database(update_error))
        }
    }
}

fn existing_connection_placeholder() -> &'static DatabaseConnection {
    panic!("identity reload requires journal connection")
}

fn normalize_input(
    mut input: RecordOrderCheckoutIdentity,
) -> OrderCheckoutIdentityResult<RecordOrderCheckoutIdentity> {
    if input.tenant_id.is_nil()
        || input.checkout_operation_id.is_nil()
        || input.order_id.is_nil()
        || input.source_cart_id.is_nil()
        || input.payment_collection_id.is_some_and(|id| id.is_nil())
        || input.shipping_option_id.is_some_and(|id| id.is_nil())
    {
        return Err(OrderCheckoutIdentityError::Validation(
            "tenant, checkout operation, order, cart, payment, and shipping identities must be valid UUIDs"
                .to_string(),
        ));
    }
    input.snapshot_hash = normalize_hash(input.snapshot_hash, "snapshot_hash", 1, 128)?;
    input.request_hash = normalize_hash(input.request_hash, "request_hash", 64, 64)?;
    Ok(input)
}

fn normalize_hash(
    value: String,
    field: &str,
    min_len: usize,
    max_len: usize,
) -> OrderCheckoutIdentityResult<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.len() < min_len
        || value.len() > max_len
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(OrderCheckoutIdentityError::Validation(format!(
            "{field} must be a lowercase hexadecimal value with {min_len} to {max_len} bytes"
        )));
    }
    Ok(value)
}

fn ensure_compatible_identity(
    existing: &order_checkout_identity::Model,
    input: &RecordOrderCheckoutIdentity,
) -> OrderCheckoutIdentityResult<()> {
    let conflicts = existing.tenant_id != input.tenant_id
        || existing.checkout_operation_id != input.checkout_operation_id
        || existing.order_id != input.order_id
        || existing
            .source_cart_id
            .is_some_and(|value| value != input.source_cart_id)
        || existing
            .payment_collection_id
            .is_some_and(|value| Some(value) != input.payment_collection_id)
        || existing
            .shipping_option_id
            .is_some_and(|value| Some(value) != input.shipping_option_id)
        || existing
            .snapshot_hash
            .as_deref()
            .is_some_and(|value| value != input.snapshot_hash)
        || existing
            .request_hash
            .as_deref()
            .is_some_and(|value| value != input.request_hash);
    if conflicts {
        return Err(OrderCheckoutIdentityError::Conflict(format!(
            "checkout operation {} is already bound to different order identity evidence",
            input.checkout_operation_id
        )));
    }
    Ok(())
}

fn ensure_exact_identity(
    existing: &order_checkout_identity::Model,
    input: &RecordOrderCheckoutIdentity,
) -> OrderCheckoutIdentityResult<()> {
    if existing.tenant_id != input.tenant_id
        || existing.checkout_operation_id != input.checkout_operation_id
        || existing.order_id != input.order_id
        || existing.source_cart_id != Some(input.source_cart_id)
        || existing.payment_collection_id != input.payment_collection_id
        || existing.shipping_option_id != input.shipping_option_id
        || existing.snapshot_hash.as_deref() != Some(input.snapshot_hash.as_str())
        || existing.request_hash.as_deref() != Some(input.request_hash.as_str())
    {
        return Err(OrderCheckoutIdentityError::Conflict(format!(
            "checkout operation {} did not retain the requested identity evidence",
            input.checkout_operation_id
        )));
    }
    Ok(())
}
