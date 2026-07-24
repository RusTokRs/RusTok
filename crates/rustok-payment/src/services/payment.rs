use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

use rustok_core::generate_id;

use crate::dto::{
    AuthorizePaymentInput, CancelPaymentInput, CancelRefundInput, CapturePaymentInput,
    CompleteRefundInput, CreatePaymentCollectionInput, ListPaymentCollectionsInput,
    ListRefundsInput, PaymentCollectionResponse, PaymentCollectionStatusKind, PaymentResponse,
    PaymentStatusKind, RefundResponse, RefundStatusKind,
};
use crate::entities;
use crate::error::{PaymentError, PaymentResult};

const STATUS_PENDING: &str = "pending";
const STATUS_AUTHORIZED: &str = "authorized";
const STATUS_CAPTURED: &str = "captured";
const STATUS_CANCELLED: &str = "cancelled";
const STATUS_REFUNDED: &str = "refunded";
const STATUS_REFUND_CANCELLED: &str = "cancelled";
const MANUAL_PROVIDER_ID: &str = "manual";

pub struct PaymentService {
    db: DatabaseConnection,
}

struct CartCollectionRecovery {
    tenant_id: Uuid,
    cart_id: Uuid,
    order_id: Option<Uuid>,
    customer_id: Option<Uuid>,
    currency_code: String,
    amount: Decimal,
    metadata: serde_json::Value,
    conflict: sea_orm::DbErr,
}

impl PaymentService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id))]
    pub async fn create_collection(
        &self,
        tenant_id: Uuid,
        input: CreatePaymentCollectionInput,
    ) -> PaymentResult<PaymentCollectionResponse> {
        input
            .validate()
            .map_err(|error| PaymentError::Validation(error.to_string()))?;

        let currency_code = normalize_currency_code(&input.currency_code)?;
        if input.amount <= Decimal::ZERO {
            return Err(PaymentError::Validation(
                "amount must be greater than zero".to_string(),
            ));
        }

        let cart_id = input.cart_id;
        let order_id = input.order_id;
        let customer_id = input.customer_id;
        let amount = input.amount;
        let metadata = input.metadata;
        let collection_id = generate_id();
        let now = Utc::now();

        let insert = entities::payment_collection::ActiveModel {
            id: Set(collection_id),
            tenant_id: Set(tenant_id),
            cart_id: Set(cart_id),
            order_id: Set(order_id),
            customer_id: Set(customer_id),
            status: Set(STATUS_PENDING.to_string()),
            currency_code: Set(currency_code.clone()),
            amount: Set(amount),
            authorized_amount: Set(Decimal::ZERO),
            captured_amount: Set(Decimal::ZERO),
            provider_id: Set(None),
            cancellation_reason: Set(None),
            metadata: Set(metadata.clone()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            authorized_at: Set(None),
            captured_at: Set(None),
            cancelled_at: Set(None),
        }
        .insert(&self.db)
        .await;

        match insert {
            Ok(_) => self.get_collection(tenant_id, collection_id).await,
            Err(error) if cart_id.is_some() && is_unique_constraint(&error) => {
                self.recover_active_cart_collection(CartCollectionRecovery {
                    tenant_id,
                    cart_id: cart_id.expect("cart_id was checked before race recovery"),
                    order_id,
                    customer_id,
                    currency_code,
                    amount,
                    metadata,
                    conflict: error,
                })
                .await
            }
            Err(error) => Err(error.into()),
        }
    }

    async fn recover_active_cart_collection(
        &self,
        recovery: CartCollectionRecovery,
    ) -> PaymentResult<PaymentCollectionResponse> {
        let CartCollectionRecovery {
            tenant_id,
            cart_id,
            order_id,
            customer_id,
            currency_code,
            amount,
            metadata,
            conflict,
        } = recovery;
        let Some(existing) = self
            .find_reusable_collection_by_cart(tenant_id, cart_id)
            .await?
        else {
            return Err(conflict.into());
        };

        validate_reusable_collection(&existing, cart_id, customer_id, &currency_code, amount)?;

        match order_id {
            Some(order_id) => {
                self.attach_order_to_collection(tenant_id, existing.id, order_id, metadata)
                    .await
            }
            None => Ok(existing),
        }
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, collection_id = %collection_id))]
    pub async fn get_collection(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
    ) -> PaymentResult<PaymentCollectionResponse> {
        let collection = self.load_collection(tenant_id, collection_id).await?;
        self.build_response(collection).await
    }

    pub async fn find_latest_collection_by_cart(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
    ) -> PaymentResult<Option<PaymentCollectionResponse>> {
        let collection = entities::payment_collection::Entity::find()
            .filter(entities::payment_collection::Column::TenantId.eq(tenant_id))
            .filter(entities::payment_collection::Column::CartId.eq(cart_id))
            .order_by_desc(entities::payment_collection::Column::CreatedAt)
            .one(&self.db)
            .await?;

        match collection {
            Some(collection) => self.build_response(collection).await.map(Some),
            None => Ok(None),
        }
    }

    pub async fn find_reusable_collection_by_cart(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
    ) -> PaymentResult<Option<PaymentCollectionResponse>> {
        let collection = entities::payment_collection::Entity::find()
            .filter(entities::payment_collection::Column::TenantId.eq(tenant_id))
            .filter(entities::payment_collection::Column::CartId.eq(cart_id))
            .filter(entities::payment_collection::Column::Status.is_in([
                STATUS_PENDING,
                STATUS_AUTHORIZED,
                STATUS_CAPTURED,
            ]))
            .order_by_desc(entities::payment_collection::Column::CreatedAt)
            .one(&self.db)
            .await?;

        match collection {
            Some(collection) => self.build_response(collection).await.map(Some),
            None => Ok(None),
        }
    }

    pub async fn find_latest_collection_by_order(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
    ) -> PaymentResult<Option<PaymentCollectionResponse>> {
        let collection = entities::payment_collection::Entity::find()
            .filter(entities::payment_collection::Column::TenantId.eq(tenant_id))
            .filter(entities::payment_collection::Column::OrderId.eq(order_id))
            .order_by_desc(entities::payment_collection::Column::CreatedAt)
            .one(&self.db)
            .await?;

        match collection {
            Some(collection) => self.build_response(collection).await.map(Some),
            None => Ok(None),
        }
    }

    pub async fn list_collections(
        &self,
        tenant_id: Uuid,
        input: ListPaymentCollectionsInput,
    ) -> PaymentResult<(Vec<PaymentCollectionResponse>, u64)> {
        let page = input.page.max(1);
        let per_page = input.per_page.clamp(1, 100);
        let offset = (page.saturating_sub(1)) * per_page;

        let mut query = entities::payment_collection::Entity::find()
            .filter(entities::payment_collection::Column::TenantId.eq(tenant_id));

        if let Some(status) = input.status {
            let normalized_status = Self::normalize_collection_status_filter(&status)?;
            query =
                query.filter(entities::payment_collection::Column::Status.eq(normalized_status));
        }
        if let Some(order_id) = input.order_id {
            query = query.filter(entities::payment_collection::Column::OrderId.eq(order_id));
        }
        if let Some(cart_id) = input.cart_id {
            query = query.filter(entities::payment_collection::Column::CartId.eq(cart_id));
        }
        if let Some(customer_id) = input.customer_id {
            query = query.filter(entities::payment_collection::Column::CustomerId.eq(customer_id));
        }

        let total = query.clone().count(&self.db).await?;
        let rows = query
            .order_by_desc(entities::payment_collection::Column::CreatedAt)
            .offset(offset)
            .limit(per_page)
            .all(&self.db)
            .await?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            items.push(self.build_response(row).await?);
        }

        Ok((items, total))
    }

    pub async fn attach_order_to_collection(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
        order_id: Uuid,
        metadata: serde_json::Value,
    ) -> PaymentResult<PaymentCollectionResponse> {
        let collection = self.load_collection(tenant_id, collection_id).await?;
        if let Some(existing_order_id) = collection.order_id {
            if existing_order_id != order_id {
                return Err(PaymentError::Validation(format!(
                    "payment collection {collection_id} is already attached to order {existing_order_id}"
                )));
            }
        }

        let mut active: entities::payment_collection::ActiveModel = collection.into();
        let collection_metadata = active.metadata.clone().take().unwrap_or_default();
        active.order_id = Set(Some(order_id));
        active.metadata = Set(merge_metadata(collection_metadata, metadata));
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        self.get_collection(tenant_id, collection_id).await
    }

    pub async fn get_refund(
        &self,
        tenant_id: Uuid,
        refund_id: Uuid,
    ) -> PaymentResult<RefundResponse> {
        let refund = self
            .load_refund_in_tx(&self.db, tenant_id, refund_id)
            .await?;
        Ok(self.build_refund_response(refund))
    }

    pub async fn list_refunds(
        &self,
        tenant_id: Uuid,
        input: ListRefundsInput,
    ) -> PaymentResult<(Vec<RefundResponse>, u64)> {
        let page = input.page.max(1);
        let per_page = input.per_page.clamp(1, 100);
        let offset = (page.saturating_sub(1)) * per_page;

        let mut query = entities::refund::Entity::find()
            .filter(entities::refund::Column::TenantId.eq(tenant_id));

        if let Some(collection_id) = input.payment_collection_id {
            query = query.filter(entities::refund::Column::PaymentCollectionId.eq(collection_id));
        }
        if let Some(order_id) = input.order_id {
            if let Some(collection_id) = input.payment_collection_id {
                let matches_order = self
                    .payment_collection_matches_order(tenant_id, collection_id, order_id)
                    .await?;
                if !matches_order {
                    return Ok((Vec::new(), 0));
                }
            }

            let collection_ids = entities::payment_collection::Entity::find()
                .select_only()
                .column(entities::payment_collection::Column::Id)
                .filter(entities::payment_collection::Column::TenantId.eq(tenant_id))
                .filter(entities::payment_collection::Column::OrderId.eq(order_id))
                .into_tuple::<Uuid>()
                .all(&self.db)
                .await?;

            if collection_ids.is_empty() {
                return Ok((Vec::new(), 0));
            }

            query =
                query.filter(entities::refund::Column::PaymentCollectionId.is_in(collection_ids));
        }
        if let Some(status) = input.status {
            let normalized_status = Self::normalize_refund_status_filter(&status)?;
            query = query.filter(entities::refund::Column::Status.eq(normalized_status));
        }

        let total = query.clone().count(&self.db).await?;
        let rows = query
            .order_by_desc(entities::refund::Column::CreatedAt)
            .offset(offset)
            .limit(per_page)
            .all(&self.db)
            .await?;

        Ok((
            rows.into_iter()
                .map(|row| self.build_refund_response(row))
                .collect(),
            total,
        ))
    }

    async fn payment_collection_matches_order(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
        order_id: Uuid,
    ) -> PaymentResult<bool> {
        let count = entities::payment_collection::Entity::find()
            .filter(entities::payment_collection::Column::TenantId.eq(tenant_id))
            .filter(entities::payment_collection::Column::Id.eq(collection_id))
            .filter(entities::payment_collection::Column::OrderId.eq(order_id))
            .count(&self.db)
            .await?;
        Ok(count > 0)
    }

    fn normalize_collection_status_filter(status: &str) -> PaymentResult<String> {
        let normalized = status.trim().to_ascii_lowercase();
        PaymentCollectionStatusKind::from_raw(normalized.as_str())
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| {
                PaymentError::Validation(
                    "invalid payment collection status filter: expected one of pending, authorized, captured, cancelled"
                        .to_string(),
                )
            })
    }

    fn normalize_refund_status_filter(status: &str) -> PaymentResult<String> {
        let normalized = status.trim().to_ascii_lowercase();
        RefundStatusKind::from_raw(normalized.as_str())
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| {
                PaymentError::Validation(
                    "invalid refund status filter: expected one of pending, refunded, cancelled"
                        .to_string(),
                )
            })
    }

    pub async fn complete_refund(
        &self,
        tenant_id: Uuid,
        refund_id: Uuid,
        input: CompleteRefundInput,
    ) -> PaymentResult<RefundResponse> {
        let txn = self.db.begin().await?;
        let refund = self.load_refund_in_tx(&txn, tenant_id, refund_id).await?;
        if !RefundStatusKind::from_raw(refund.status.as_str()).can_complete() {
            return Err(PaymentError::InvalidTransition {
                from: refund.status,
                to: STATUS_REFUNDED.to_string(),
            });
        }

        let now = Utc::now();
        let mut active: entities::refund::ActiveModel = refund.into();
        let current_metadata = active.metadata.clone().take().unwrap_or_default();
        active.status = Set(STATUS_REFUNDED.to_string());
        active.metadata = Set(merge_metadata(current_metadata, input.metadata));
        active.updated_at = Set(now.into());
        active.refunded_at = Set(Some(now.into()));
        active.update(&txn).await?;

        txn.commit().await?;
        self.get_refund(tenant_id, refund_id).await
    }

    pub async fn cancel_refund(
        &self,
        tenant_id: Uuid,
        refund_id: Uuid,
        input: CancelRefundInput,
    ) -> PaymentResult<RefundResponse> {
        let txn = self.db.begin().await?;
        let refund = self.load_refund_in_tx(&txn, tenant_id, refund_id).await?;
        if !RefundStatusKind::from_raw(refund.status.as_str()).can_cancel() {
            return Err(PaymentError::InvalidTransition {
                from: refund.status,
                to: STATUS_REFUND_CANCELLED.to_string(),
            });
        }

        let now = Utc::now();
        let fallback_reason = refund.reason.clone();
        let mut active: entities::refund::ActiveModel = refund.into();
        let current_metadata = active.metadata.clone().take().unwrap_or_default();
        active.status = Set(STATUS_REFUND_CANCELLED.to_string());
        active.reason = Set(normalize_optional_reason(input.reason).or(fallback_reason));
        active.metadata = Set(merge_metadata(current_metadata, input.metadata));
        active.updated_at = Set(now.into());
        active.cancelled_at = Set(Some(now.into()));
        active.update(&txn).await?;

        txn.commit().await?;
        self.get_refund(tenant_id, refund_id).await
    }

    pub async fn authorize_collection(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
        input: AuthorizePaymentInput,
    ) -> PaymentResult<PaymentCollectionResponse> {
        input
            .validate()
            .map_err(|error| PaymentError::Validation(error.to_string()))?;

        let txn = self.db.begin().await?;
        let collection = self
            .load_collection_in_tx(&txn, tenant_id, collection_id)
            .await?;
        if !PaymentCollectionStatusKind::from_raw(collection.status.as_str()).can_authorize() {
            return Err(PaymentError::InvalidTransition {
                from: collection.status,
                to: STATUS_AUTHORIZED.to_string(),
            });
        }

        let authorize_amount = input.amount.unwrap_or(collection.amount);
        if authorize_amount <= Decimal::ZERO || authorize_amount > collection.amount {
            return Err(PaymentError::Validation(
                "authorize amount must be positive and not exceed collection amount".to_string(),
            ));
        }
        let provider_id = normalize_provider_id(input.provider_id)?;
        let provider_payment_id = normalize_provider_payment_id(input.provider_payment_id);

        let now = Utc::now();
        entities::payment::ActiveModel {
            id: Set(generate_id()),
            payment_collection_id: Set(collection_id),
            provider_id: Set(provider_id.clone()),
            provider_payment_id: Set(provider_payment_id),
            status: Set(STATUS_AUTHORIZED.to_string()),
            currency_code: Set(collection.currency_code.clone()),
            amount: Set(authorize_amount),
            captured_amount: Set(Decimal::ZERO),
            error_message: Set(None),
            metadata: Set(input.metadata),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            authorized_at: Set(Some(now.into())),
            captured_at: Set(None),
            cancelled_at: Set(None),
        }
        .insert(&txn)
        .await?;

        let mut active: entities::payment_collection::ActiveModel = collection.into();
        active.status = Set(STATUS_AUTHORIZED.to_string());
        active.authorized_amount = Set(authorize_amount);
        active.provider_id = Set(Some(provider_id));
        active.authorized_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());
        active.update(&txn).await?;

        txn.commit().await?;
        self.get_collection(tenant_id, collection_id).await
    }

    pub async fn capture_collection(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
        input: CapturePaymentInput,
    ) -> PaymentResult<PaymentCollectionResponse> {
        let txn = self.db.begin().await?;
        let collection = self
            .load_collection_in_tx(&txn, tenant_id, collection_id)
            .await?;
        if !PaymentCollectionStatusKind::from_raw(collection.status.as_str()).can_capture() {
            return Err(PaymentError::InvalidTransition {
                from: collection.status,
                to: STATUS_CAPTURED.to_string(),
            });
        }

        let capture_amount = input.amount.unwrap_or(collection.authorized_amount);
        if capture_amount <= Decimal::ZERO || capture_amount > collection.authorized_amount {
            return Err(PaymentError::Validation(
                "capture amount must be positive and not exceed authorized amount".to_string(),
            ));
        }

        let payment = self
            .latest_payment_in_tx(&txn, collection_id, STATUS_AUTHORIZED)
            .await?;
        let now = Utc::now();

        let mut payment_active: entities::payment::ActiveModel = payment.into();
        let payment_metadata = payment_active.metadata.clone().take().unwrap_or_default();
        payment_active.status = Set(STATUS_CAPTURED.to_string());
        payment_active.captured_amount = Set(capture_amount);
        payment_active.metadata = Set(merge_metadata(payment_metadata, input.metadata.clone()));
        payment_active.updated_at = Set(now.into());
        payment_active.captured_at = Set(Some(now.into()));
        payment_active.update(&txn).await?;

        let mut active: entities::payment_collection::ActiveModel = collection.into();
        let collection_metadata = active.metadata.clone().take().unwrap_or_default();
        active.status = Set(STATUS_CAPTURED.to_string());
        active.captured_amount = Set(capture_amount);
        active.metadata = Set(merge_metadata(collection_metadata, input.metadata));
        active.captured_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());
        active.update(&txn).await?;

        txn.commit().await?;
        self.get_collection(tenant_id, collection_id).await
    }

    pub async fn cancel_collection(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
        input: CancelPaymentInput,
    ) -> PaymentResult<PaymentCollectionResponse> {
        let txn = self.db.begin().await?;
        let collection = self
            .load_collection_in_tx(&txn, tenant_id, collection_id)
            .await?;
        let collection_status = PaymentCollectionStatusKind::from_raw(collection.status.as_str());
        if !collection_status.can_cancel() {
            return Err(PaymentError::InvalidTransition {
                from: collection.status,
                to: STATUS_CANCELLED.to_string(),
            });
        }

        let payment = match self
            .latest_payment_any_status_in_tx(&txn, collection_id)
            .await
        {
            Ok(payment) => Some(payment),
            Err(PaymentError::PaymentNotFound(_))
                if collection_status == PaymentCollectionStatusKind::Pending =>
            {
                None
            }
            Err(error) => return Err(error),
        };

        let now = Utc::now();
        if let Some(payment) = payment {
            let payment_status = PaymentStatusKind::from_raw(payment.status.as_str());
            if matches!(
                payment_status,
                PaymentStatusKind::Captured | PaymentStatusKind::Unknown
            ) {
                return Err(PaymentError::InvalidTransition {
                    from: payment.status,
                    to: STATUS_CANCELLED.to_string(),
                });
            }
            let mut payment_active: entities::payment::ActiveModel = payment.into();
            let reason = input
                .reason
                .clone()
                .unwrap_or_else(|| "cancelled".to_string());
            let payment_metadata = payment_active.metadata.clone().take().unwrap_or_default();
            payment_active.status = Set(STATUS_CANCELLED.to_string());
            payment_active.error_message = Set(Some(reason));
            payment_active.metadata = Set(merge_metadata(payment_metadata, input.metadata.clone()));
            payment_active.updated_at = Set(now.into());
            payment_active.cancelled_at = Set(Some(now.into()));
            payment_active.update(&txn).await?;
        }

        let mut active: entities::payment_collection::ActiveModel = collection.into();
        let collection_metadata = active.metadata.clone().take().unwrap_or_default();
        active.status = Set(STATUS_CANCELLED.to_string());
        active.cancellation_reason = Set(input.reason);
        active.metadata = Set(merge_metadata(collection_metadata, input.metadata));
        active.cancelled_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());
        active.update(&txn).await?;

        txn.commit().await?;
        self.get_collection(tenant_id, collection_id).await
    }

    async fn load_collection(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
    ) -> PaymentResult<entities::payment_collection::Model> {
        self.load_collection_in_tx(&self.db, tenant_id, collection_id)
            .await
    }

    async fn load_collection_in_tx<C>(
        &self,
        conn: &C,
        tenant_id: Uuid,
        collection_id: Uuid,
    ) -> PaymentResult<entities::payment_collection::Model>
    where
        C: sea_orm::ConnectionTrait,
    {
        entities::payment_collection::Entity::find_by_id(collection_id)
            .filter(entities::payment_collection::Column::TenantId.eq(tenant_id))
            .one(conn)
            .await?
            .ok_or(PaymentError::PaymentCollectionNotFound(collection_id))
    }

    async fn latest_payment_in_tx<C>(
        &self,
        conn: &C,
        collection_id: Uuid,
        status: &str,
    ) -> PaymentResult<entities::payment::Model>
    where
        C: sea_orm::ConnectionTrait,
    {
        entities::payment::Entity::find()
            .filter(entities::payment::Column::PaymentCollectionId.eq(collection_id))
            .filter(entities::payment::Column::Status.eq(status))
            .order_by_desc(entities::payment::Column::CreatedAt)
            .one(conn)
            .await?
            .ok_or(PaymentError::PaymentNotFound(collection_id))
    }

    async fn latest_payment_any_status_in_tx<C>(
        &self,
        conn: &C,
        collection_id: Uuid,
    ) -> PaymentResult<entities::payment::Model>
    where
        C: sea_orm::ConnectionTrait,
    {
        entities::payment::Entity::find()
            .filter(entities::payment::Column::PaymentCollectionId.eq(collection_id))
            .order_by_desc(entities::payment::Column::CreatedAt)
            .one(conn)
            .await?
            .ok_or(PaymentError::PaymentNotFound(collection_id))
    }

    async fn load_refund_in_tx<C>(
        &self,
        conn: &C,
        tenant_id: Uuid,
        refund_id: Uuid,
    ) -> PaymentResult<entities::refund::Model>
    where
        C: sea_orm::ConnectionTrait,
    {
        entities::refund::Entity::find_by_id(refund_id)
            .filter(entities::refund::Column::TenantId.eq(tenant_id))
            .one(conn)
            .await?
            .ok_or(PaymentError::RefundNotFound(refund_id))
    }

    async fn build_response(
        &self,
        collection: entities::payment_collection::Model,
    ) -> PaymentResult<PaymentCollectionResponse> {
        let payments = entities::payment::Entity::find()
            .filter(entities::payment::Column::PaymentCollectionId.eq(collection.id))
            .order_by_asc(entities::payment::Column::CreatedAt)
            .all(&self.db)
            .await?;
        let refunds = entities::refund::Entity::find()
            .filter(entities::refund::Column::PaymentCollectionId.eq(collection.id))
            .order_by_asc(entities::refund::Column::CreatedAt)
            .all(&self.db)
            .await?;
        let refunded_amount = refunds
            .iter()
            .filter(|refund| {
                RefundStatusKind::from_raw(refund.status.as_str()) == RefundStatusKind::Refunded
            })
            .fold(Decimal::ZERO, |sum, refund| sum + refund.amount);

        Ok(PaymentCollectionResponse {
            id: collection.id,
            tenant_id: collection.tenant_id,
            cart_id: collection.cart_id,
            order_id: collection.order_id,
            customer_id: collection.customer_id,
            status: collection.status,
            currency_code: collection.currency_code,
            amount: collection.amount,
            authorized_amount: collection.authorized_amount,
            captured_amount: collection.captured_amount,
            refunded_amount,
            provider_id: collection.provider_id,
            cancellation_reason: collection.cancellation_reason,
            metadata: collection.metadata,
            created_at: collection.created_at.with_timezone(&Utc),
            updated_at: collection.updated_at.with_timezone(&Utc),
            authorized_at: collection
                .authorized_at
                .map(|value| value.with_timezone(&Utc)),
            captured_at: collection
                .captured_at
                .map(|value| value.with_timezone(&Utc)),
            cancelled_at: collection
                .cancelled_at
                .map(|value| value.with_timezone(&Utc)),
            payments: payments
                .into_iter()
                .map(|payment| PaymentResponse {
                    id: payment.id,
                    payment_collection_id: payment.payment_collection_id,
                    provider_id: payment.provider_id,
                    provider_payment_id: payment.provider_payment_id,
                    status: payment.status,
                    currency_code: payment.currency_code,
                    amount: payment.amount,
                    captured_amount: payment.captured_amount,
                    error_message: payment.error_message,
                    metadata: payment.metadata,
                    created_at: payment.created_at.with_timezone(&Utc),
                    updated_at: payment.updated_at.with_timezone(&Utc),
                    authorized_at: payment.authorized_at.map(|value| value.with_timezone(&Utc)),
                    captured_at: payment.captured_at.map(|value| value.with_timezone(&Utc)),
                    cancelled_at: payment.cancelled_at.map(|value| value.with_timezone(&Utc)),
                })
                .collect(),
            refunds: refunds
                .into_iter()
                .map(|refund| self.build_refund_response(refund))
                .collect(),
        })
    }

    fn build_refund_response(&self, refund: entities::refund::Model) -> RefundResponse {
        RefundResponse {
            id: refund.id,
            tenant_id: refund.tenant_id,
            payment_collection_id: refund.payment_collection_id,
            status: refund.status,
            currency_code: refund.currency_code,
            amount: refund.amount,
            reason: refund.reason,
            metadata: refund.metadata,
            created_at: refund.created_at.with_timezone(&Utc),
            updated_at: refund.updated_at.with_timezone(&Utc),
            refunded_at: refund.refunded_at.map(|value| value.with_timezone(&Utc)),
            cancelled_at: refund.cancelled_at.map(|value| value.with_timezone(&Utc)),
        }
    }
}

fn validate_reusable_collection(
    existing: &PaymentCollectionResponse,
    cart_id: Uuid,
    customer_id: Option<Uuid>,
    currency_code: &str,
    amount: Decimal,
) -> PaymentResult<()> {
    if existing.cart_id != Some(cart_id) {
        return Err(PaymentError::Validation(format!(
            "payment collection {} is not bound to cart {cart_id}",
            existing.id
        )));
    }
    if existing.customer_id != customer_id {
        return Err(PaymentError::Validation(format!(
            "active payment collection {} belongs to a different customer",
            existing.id
        )));
    }
    if !existing.currency_code.eq_ignore_ascii_case(currency_code) {
        return Err(PaymentError::Validation(format!(
            "active payment collection {} uses currency {}, expected {}",
            existing.id, existing.currency_code, currency_code
        )));
    }
    if existing.amount != amount {
        return Err(PaymentError::Validation(format!(
            "active payment collection {} has amount {}, expected {}",
            existing.id, existing.amount, amount
        )));
    }
    if matches!(
        existing.status_kind(),
        PaymentCollectionStatusKind::Cancelled | PaymentCollectionStatusKind::Unknown
    ) {
        return Err(PaymentError::InvalidTransition {
            from: existing.status.clone(),
            to: STATUS_PENDING.to_string(),
        });
    }
    Ok(())
}

fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}

fn normalize_currency_code(value: &str) -> PaymentResult<String> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.len() != 3 {
        return Err(PaymentError::Validation(
            "currency_code must be a 3-letter code".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_provider_id(value: Option<String>) -> PaymentResult<String> {
    let normalized = value
        .map(|provider| provider.trim().to_string())
        .filter(|provider| !provider.is_empty())
        .unwrap_or_else(|| MANUAL_PROVIDER_ID.to_string());
    if normalized.len() > 100 {
        return Err(PaymentError::Validation(
            "provider_id must be at most 100 characters".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_provider_payment_id(value: Option<String>) -> String {
    value
        .map(|provider_payment_id| provider_payment_id.trim().to_string())
        .filter(|provider_payment_id| !provider_payment_id.is_empty())
        .unwrap_or_else(|| format!("manual_{}", generate_id()))
}

fn normalize_optional_reason(value: Option<String>) -> Option<String> {
    value
        .map(|reason| reason.trim().to_string())
        .filter(|reason| !reason.is_empty())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_collection_status_filter_accepts_supported_values() {
        assert_eq!(
            PaymentService::normalize_collection_status_filter("pending").expect("pending status"),
            "pending"
        );
        assert_eq!(
            PaymentService::normalize_collection_status_filter("  AuThOrIzEd  ")
                .expect("authorized status"),
            "authorized"
        );
        assert_eq!(
            PaymentService::normalize_collection_status_filter("CAPTURED")
                .expect("captured status"),
            "captured"
        );
    }

    #[test]
    fn normalize_collection_status_filter_rejects_unknown_values() {
        let error = PaymentService::normalize_collection_status_filter("processing")
            .expect_err("unknown status must fail validation");
        assert!(
            matches!(error, PaymentError::Validation(ref message) if message.contains("invalid payment collection status filter")),
            "expected validation error for unknown collection status, got: {error:?}"
        );
    }

    #[test]
    fn normalize_refund_status_filter_accepts_supported_values() {
        assert_eq!(
            PaymentService::normalize_refund_status_filter("pending").expect("pending status"),
            "pending"
        );
        assert_eq!(
            PaymentService::normalize_refund_status_filter("  ReFuNdEd  ")
                .expect("refunded status"),
            "refunded"
        );
        assert_eq!(
            PaymentService::normalize_refund_status_filter("CANCELLED").expect("cancelled status"),
            "cancelled"
        );
    }

    #[test]
    fn normalize_refund_status_filter_rejects_unknown_values() {
        let error = PaymentService::normalize_refund_status_filter("processing")
            .expect_err("unknown status must fail validation");
        assert!(
            matches!(error, PaymentError::Validation(ref message) if message.contains("invalid refund status filter")),
            "expected validation error for unknown refund status, got: {error:?}"
        );
    }
}
