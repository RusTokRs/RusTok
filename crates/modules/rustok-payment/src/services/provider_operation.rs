use std::sync::Arc;

use chrono::{DateTime, FixedOffset, Utc};
use rustok_api::PortError;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, sea_query::Expr,
};
use serde_json::Value;
use uuid::Uuid;

use rustok_core::generate_id;

use crate::entities::{payment_collection, provider_operation, refund};
use crate::error::{PaymentError, PaymentResult};
use crate::providers::{
    MAX_EXTERNAL_REFERENCE_LENGTH, PaymentProviderOperationResult,
    validate_provider_operation_payload,
};
use crate::services::checkout_admission::{
    CheckoutAdmissionDecision, CheckoutAdmissionLinkState, CheckoutAdmissionRefusal,
    CheckoutExecutionAdmissionPort, ProviderExecutionEffect, decide_checkout_admission_claim,
    refusal_metric_operation_label, resolve_checkout_operation_id,
};

pub const PROVIDER_OPERATION_PENDING: &str = "pending";
pub const PROVIDER_OPERATION_EXECUTING: &str = "executing";
pub const PROVIDER_OPERATION_SUCCEEDED: &str = "provider_succeeded";
pub const PROVIDER_OPERATION_ERROR: &str = "provider_error";
pub const PROVIDER_OPERATION_RECONCILIATION_REQUIRED: &str = "reconciliation_required";
pub const PROVIDER_OPERATION_COMMITTED: &str = "committed";

#[derive(Clone, Debug)]
pub struct BeginProviderOperation {
    pub tenant_id: Uuid,
    pub payment_collection_id: Uuid,
    pub refund_id: Option<Uuid>,
    pub operation: String,
    pub provider_id: String,
    pub idempotency_key: String,
    pub request_payload: Value,
}

#[derive(Clone)]
pub struct PaymentProviderOperationJournal {
    db: DatabaseConnection,
    admission_port: Option<Arc<dyn CheckoutExecutionAdmissionPort>>,
}

impl PaymentProviderOperationJournal {
    /// Journal without a checkout admission reader.
    ///
    /// Provider operations whose payment collection is linked to a checkout
    /// operation then fail closed on extending claims
    /// (`checkout_admission_unavailable`): a missing wire is a refusal, never a
    /// silent bypass. Collections that carry no checkout link are unaffected.
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            admission_port: None,
        }
    }

    /// Wires the checkout journal's admission reader into the claim gate.
    pub fn with_checkout_execution_admission_port(
        mut self,
        port: Arc<dyn CheckoutExecutionAdmissionPort>,
    ) -> Self {
        self.admission_port = Some(port);
        self
    }

    /// Create an operation journal row or return the existing row for the same
    /// provider idempotency key. A key collision with a different immutable
    /// request is rejected instead of silently reusing the wrong operation.
    pub async fn begin(
        &self,
        input: BeginProviderOperation,
    ) -> PaymentResult<provider_operation::Model> {
        let input = normalize_begin_input(input)?;
        validate_begin_identity(&self.db, &input).await?;
        if let Some(existing) = self
            .find_by_key(input.tenant_id, &input.provider_id, &input.idempotency_key)
            .await?
        {
            ensure_same_request(&existing, &input)?;
            return Ok(existing);
        }

        let id = generate_id();
        let now = Utc::now();
        // The generation this operation is created under. `0` means the
        // collection is not linked to a checkout operation (or the checkout has
        // not projected its admission yet): the claim gate admits such a row
        // only while the linked checkout is `open`, which mirrors the database
        // guard this contract replaces.
        let admission_epoch = self
            .checkout_admission_epoch(input.tenant_id, input.payment_collection_id)
            .await;
        let insert = provider_operation::ActiveModel {
            id: Set(id),
            tenant_id: Set(input.tenant_id),
            payment_collection_id: Set(input.payment_collection_id),
            refund_id: Set(input.refund_id),
            operation: Set(input.operation.clone()),
            provider_id: Set(input.provider_id.clone()),
            idempotency_key: Set(input.idempotency_key.clone()),
            status: Set(PROVIDER_OPERATION_PENDING.to_string()),
            request_payload: Set(input.request_payload.clone()),
            provider_reference: Set(None),
            provider_result: Set(None),
            error_message: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            provider_completed_at: Set(None),
            committed_at: Set(None),
            admission_epoch: Set(admission_epoch),
            admission_refusal_code: Set(None),
            admission_refused_at: Set(None),
        }
        .insert(&self.db)
        .await;

        match insert {
            Ok(model) => Ok(model),
            Err(insert_error) => {
                if let Some(existing) = self
                    .find_by_key(input.tenant_id, &input.provider_id, &input.idempotency_key)
                    .await?
                {
                    ensure_same_request(&existing, &input)?;
                    Ok(existing)
                } else {
                    Err(insert_error.into())
                }
            }
        }
    }

    pub async fn get(&self, tenant_id: Uuid, id: Uuid) -> PaymentResult<provider_operation::Model> {
        validate_operation_identity(tenant_id, id)?;
        provider_operation::Entity::find_by_id(id)
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                PaymentError::Validation(format!(
                    "payment provider operation {id} not found for tenant {tenant_id}"
                ))
            })
    }

    pub async fn find_by_key(
        &self,
        tenant_id: Uuid,
        provider_id: &str,
        idempotency_key: &str,
    ) -> PaymentResult<Option<provider_operation::Model>> {
        provider_operation::Entity::find()
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::ProviderId.eq(provider_id))
            .filter(provider_operation::Column::IdempotencyKey.eq(idempotency_key))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn list_by_collection(
        &self,
        tenant_id: Uuid,
        payment_collection_id: Uuid,
    ) -> PaymentResult<Vec<provider_operation::Model>> {
        provider_operation::Entity::find()
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::PaymentCollectionId.eq(payment_collection_id))
            .order_by_asc(provider_operation::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(Into::into)
    }

    /// Claims a provider operation for execution.
    ///
    /// This is the single enforcement point of the checkout execution admission
    /// contract (`DECISIONS/2026-10-07-checkout-operation-invariants-owned-by-rust.md`),
    /// the rule the database trigger `payment_provider_operations_checkout_guard`
    /// used to own:
    ///
    /// * An operation whose payment collection is linked to a checkout operation
    ///   is admitted for an **extending** effect (`authorize`, `capture`) only
    ///   while the checkout owner reports the level `open` and the generation the
    ///   operation carries still matches. Level and generation are read through
    ///   the checkout owner port and confirmed by the conditional write itself;
    ///   the park that changes them stamps the new generation on the collection's
    ///   non-terminal operations first, so a claim decided under the previous
    ///   generation fails its write instead of executing.
    /// * An **unwinding** effect (`cancel`, `refund`) is always admitted: the
    ///   compensation itself needs the provider, which is exactly what the
    ///   trigger got wrong (ECOM-ADM-02).
    /// * A collection with no checkout link is not fenced at all, matching the
    ///   trigger's scope.
    /// * A refusal is fail-closed and observable: the bounded reason is written
    ///   to `admission_refusal_code`, counted in
    ///   `rustok_payment_provider_execution_admission_refused_total`, and the
    ///   caller sees `Ok(None)` while
    ///   [`execution_admission_refusal_error`] turns the recorded reason into a
    ///   bounded owner error.
    pub async fn claim_execution(
        &self,
        tenant_id: Uuid,
        id: Uuid,
    ) -> PaymentResult<Option<provider_operation::Model>> {
        validate_operation_identity(tenant_id, id)?;
        let Some(operation) = self.find_optional(tenant_id, id).await? else {
            return Ok(None);
        };
        if !matches!(
            operation.status.as_str(),
            PROVIDER_OPERATION_PENDING | PROVIDER_OPERATION_ERROR
        ) {
            return Ok(None);
        }
        let effect = ProviderExecutionEffect::for_operation(operation.operation.as_str());
        let link = if matches!(effect, Some(ProviderExecutionEffect::Unwinding)) {
            // Unwinding effects are never fenced, so they do not read the
            // checkout at all: compensation must not be blocked by a checkout
            // that cannot be read.
            CheckoutAdmissionLinkState::Unlinked
        } else {
            self.checkout_admission_link(tenant_id, operation.payment_collection_id)
                .await
        };
        match decide_checkout_admission_claim(link, effect, operation.admission_epoch) {
            CheckoutAdmissionDecision::Unfenced => self.claim_without_fence(tenant_id, id).await,
            CheckoutAdmissionDecision::Admitted {
                epoch,
                adopt_legacy,
            } => {
                self.claim_extending(
                    tenant_id,
                    id,
                    operation.operation.as_str(),
                    epoch,
                    adopt_legacy,
                )
                .await
            }
            CheckoutAdmissionDecision::Refused(refusal) => {
                self.record_admission_refusal(tenant_id, id, operation.operation.as_str(), refusal)
                    .await?;
                Ok(None)
            }
        }
    }

    /// Claim without a checkout fence: the operation is unlinked or unwinding.
    async fn claim_without_fence(
        &self,
        tenant_id: Uuid,
        id: Uuid,
    ) -> PaymentResult<Option<provider_operation::Model>> {
        let update = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_EXECUTING),
            )
            .col_expr(
                provider_operation::Column::AdmissionRefusalCode,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                provider_operation::Column::AdmissionRefusedAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::Id.eq(id))
            .filter(
                provider_operation::Column::Status
                    .is_in([PROVIDER_OPERATION_PENDING, PROVIDER_OPERATION_ERROR]),
            )
            .exec(&self.db)
            .await?;

        if update.rows_affected == 0 {
            return Ok(None);
        }
        self.get(tenant_id, id).await.map(Some)
    }

    /// Claim an extending effect under the generation the checkout owner reports.
    ///
    /// One conditional write: the operation moves to `executing` only while its
    /// stored generation still matches the observed one and its status is still
    /// claimable. A park that moves the checkout into the settling set stamps the
    /// new generation on the cart's non-terminal operations
    /// ([`Self::stamp_admission_epoch_for_cart`]) before it publishes the new
    /// admission, so a claim that loses that race fails this filter instead of
    /// executing under a generation the checkout has already left.
    async fn claim_extending(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        operation: &str,
        epoch: i64,
        adopt_legacy: bool,
    ) -> PaymentResult<Option<provider_operation::Model>> {
        let update = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_EXECUTING),
            )
            .col_expr(
                provider_operation::Column::AdmissionEpoch,
                Expr::value(epoch),
            )
            .col_expr(
                provider_operation::Column::AdmissionRefusalCode,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                provider_operation::Column::AdmissionRefusedAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::Id.eq(id))
            .filter(
                provider_operation::Column::Status
                    .is_in([PROVIDER_OPERATION_PENDING, PROVIDER_OPERATION_ERROR]),
            );
        let update = if adopt_legacy {
            // `0` is the pre-contract generation: it is adopted into the
            // observed one, but only while the level is `open` (decided above).
            update.filter(provider_operation::Column::AdmissionEpoch.is_in([0, epoch]))
        } else {
            update.filter(provider_operation::Column::AdmissionEpoch.eq(epoch))
        };
        let update = update.exec(&self.db).await?;
        if update.rows_affected == 1 {
            return self.get(tenant_id, id).await.map(Some);
        }
        let current = self.find_optional(tenant_id, id).await?;
        let still_claimable = current.as_ref().is_some_and(|current| {
            matches!(
                current.status.as_str(),
                PROVIDER_OPERATION_PENDING | PROVIDER_OPERATION_ERROR
            )
        });
        if still_claimable {
            // The row was claimable, so the generation filter is what refused:
            // the checkout moved on between the read and the write.
            self.record_admission_refusal(
                tenant_id,
                id,
                operation,
                CheckoutAdmissionRefusal::EpochMismatch,
            )
            .await?;
        }
        Ok(None)
    }

    /// Stamps an admission generation on the non-terminal operations of every
    /// payment collection a cart owns.
    ///
    /// The checkout journal calls this inside its own transaction whenever an
    /// admission level change is published: bumping the generation on operations
    /// that have not started executing invalidates the claims that were decided
    /// under the previous generation instead of letting them race the park.
    /// Operations already `executing` keep their generation — their invocation
    /// was admitted before the park — and the conditional claim write of a racing
    /// caller fails on the generation filter.
    ///
    /// The scope is the cart, not the single collection the journal happens to
    /// have bound: at most one collection per cart is active
    /// (`ux_payment_collections_active_cart`) and the checkout journal admits one
    /// live checkout per cart, so the cart is the smallest scope that is
    /// guaranteed to hold every provider operation this checkout may still claim
    /// through — independently of the binding write the commerce payment stage
    /// performs only once it reaches `payment_authorized`.
    pub async fn stamp_admission_epoch_for_cart<C>(
        &self,
        db: &C,
        tenant_id: Uuid,
        cart_id: Uuid,
        epoch: i64,
    ) -> PaymentResult<u64>
    where
        C: ConnectionTrait,
    {
        let collection_ids: Vec<Uuid> = payment_collection::Entity::find()
            .select_only()
            .column(payment_collection::Column::Id)
            .filter(payment_collection::Column::TenantId.eq(tenant_id))
            .filter(payment_collection::Column::CartId.eq(cart_id))
            .into_tuple::<Uuid>()
            .all(db)
            .await?;
        if collection_ids.is_empty() {
            return Ok(0);
        }
        let update = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::AdmissionEpoch,
                Expr::value(epoch),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::PaymentCollectionId.is_in(collection_ids))
            .filter(
                provider_operation::Column::Status
                    .is_in([PROVIDER_OPERATION_PENDING, PROVIDER_OPERATION_ERROR]),
            )
            .exec(db)
            .await?;
        Ok(update.rows_affected)
    }

    /// Admission of the checkout operation a payment collection is linked to.
    ///
    /// Unwinding claims never consult the admission, so the caller skips this
    /// read for them: a checkout that cannot be read must never be able to trap
    /// money that has to move back.
    async fn checkout_admission_link(
        &self,
        tenant_id: Uuid,
        payment_collection_id: Uuid,
    ) -> CheckoutAdmissionLinkState {
        let checkout_operation_id =
            match resolve_checkout_operation_id(&self.db, tenant_id, payment_collection_id).await {
                Ok(checkout_operation_id) => checkout_operation_id,
                Err(error) => {
                    tracing::warn!(
                        tenant_id_length = tenant_id.to_string().chars().count(),
                        failure = "checkout_link_lookup_failed",
                        error_is_database = matches!(error, PaymentError::Database(_)),
                        owner = "rustok_payment",
                        "checkout link lookup failed; treating the admission as unavailable"
                    );
                    return CheckoutAdmissionLinkState::Unavailable;
                }
            };
        let Some(checkout_operation_id) = checkout_operation_id else {
            return CheckoutAdmissionLinkState::Unlinked;
        };
        let Some(port) = self.admission_port.as_ref() else {
            return CheckoutAdmissionLinkState::Unavailable;
        };
        match port
            .read_checkout_execution_admission(tenant_id, checkout_operation_id)
            .await
        {
            Ok(None) => CheckoutAdmissionLinkState::Unavailable,
            Ok(Some(record)) => CheckoutAdmissionLinkState::Resolved(record),
            Err(error) => {
                tracing::warn!(
                    tenant_id_length = tenant_id.to_string().chars().count(),
                    port_error_kind = ?error.kind,
                    port_error_code = error.code.as_str(),
                    owner = "rustok_payment",
                    "checkout admission read failed; treating the admission as unavailable"
                );
                CheckoutAdmissionLinkState::Unavailable
            }
        }
    }

    /// Generation an operation created for this collection is admitted under.
    ///
    /// `0` means "not observed": the claim gate adopts such a row only while the
    /// checkout is `open` (see [`decide_checkout_admission_claim`]).
    async fn checkout_admission_epoch(&self, tenant_id: Uuid, payment_collection_id: Uuid) -> i64 {
        match self
            .checkout_admission_link(tenant_id, payment_collection_id)
            .await
        {
            CheckoutAdmissionLinkState::Resolved(record) => record.admission_epoch,
            CheckoutAdmissionLinkState::Unlinked | CheckoutAdmissionLinkState::Unavailable => 0,
        }
    }

    /// Records the bounded refusal reason on the row and in the metric.
    async fn record_admission_refusal(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        operation: &str,
        refusal: CheckoutAdmissionRefusal,
    ) -> PaymentResult<()> {
        provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::AdmissionRefusalCode,
                Expr::value(refusal.as_str()),
            )
            .col_expr(
                provider_operation::Column::AdmissionRefusedAt,
                Expr::current_timestamp(),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::Id.eq(id))
            .exec(&self.db)
            .await?;
        rustok_telemetry::metrics::record_provider_execution_admission_refused(
            refusal_metric_operation_label(operation),
            refusal.as_str(),
        );
        tracing::warn!(
            tenant_id_length = tenant_id.to_string().chars().count(),
            operation,
            reason = refusal.as_str(),
            owner = "rustok_payment",
            "provider execution refused by the checkout admission contract"
        );
        Ok(())
    }

    /// Tenant-scoped optional read used by the claim gate.
    async fn find_optional(
        &self,
        tenant_id: Uuid,
        id: Uuid,
    ) -> PaymentResult<Option<provider_operation::Model>> {
        provider_operation::Entity::find_by_id(id)
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn mark_provider_succeeded(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        provider_reference: Option<String>,
        provider_result: Value,
    ) -> PaymentResult<provider_operation::Model> {
        validate_operation_identity(tenant_id, id)?;
        let model = self.get(tenant_id, id).await?;
        let provider_reference =
            validate_provider_result_for_operation(&model, provider_reference, &provider_result)?;
        if matches!(
            model.status.as_str(),
            PROVIDER_OPERATION_SUCCEEDED
                | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
                | PROVIDER_OPERATION_COMMITTED
        ) {
            return Ok(model);
        }
        ensure_transition(&model.status, PROVIDER_OPERATION_SUCCEEDED)?;

        let now = Utc::now();
        let update = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_SUCCEEDED),
            )
            .col_expr(
                provider_operation::Column::ProviderReference,
                Expr::value(provider_reference),
            )
            .col_expr(
                provider_operation::Column::ProviderResult,
                Expr::value(Some(provider_result)),
            )
            .col_expr(
                provider_operation::Column::ErrorMessage,
                Expr::value(Option::<String>::None),
            )
            .col_expr(provider_operation::Column::UpdatedAt, Expr::value(now))
            .col_expr(
                provider_operation::Column::ProviderCompletedAt,
                Expr::value(Some(now)),
            )
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::Id.eq(id))
            .filter(provider_operation::Column::Status.eq(PROVIDER_OPERATION_EXECUTING))
            .exec(&self.db)
            .await?;

        if update.rows_affected == 0 {
            let current = self.get(tenant_id, id).await?;
            if matches!(
                current.status.as_str(),
                PROVIDER_OPERATION_SUCCEEDED
                    | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
                    | PROVIDER_OPERATION_COMMITTED
            ) {
                return Ok(current);
            }
            return Err(PaymentError::InvalidTransition {
                from: current.status,
                to: PROVIDER_OPERATION_SUCCEEDED.to_string(),
            });
        }

        self.get(tenant_id, id).await
    }

    pub async fn mark_provider_error(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        error_message: impl Into<String>,
    ) -> PaymentResult<provider_operation::Model> {
        validate_operation_identity(tenant_id, id)?;
        let model = self.get(tenant_id, id).await?;
        ensure_transition(&model.status, PROVIDER_OPERATION_ERROR)?;

        let now = Utc::now();
        let error_message = normalize_error(error_message.into());
        let update = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_ERROR),
            )
            .col_expr(
                provider_operation::Column::ErrorMessage,
                Expr::value(Some(error_message)),
            )
            .col_expr(provider_operation::Column::UpdatedAt, Expr::value(now))
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::Id.eq(id))
            .filter(provider_operation::Column::Status.eq(PROVIDER_OPERATION_EXECUTING))
            .exec(&self.db)
            .await?;

        if update.rows_affected == 0 {
            let current = self.get(tenant_id, id).await?;
            if current.status == PROVIDER_OPERATION_ERROR {
                return Ok(current);
            }
            return Err(PaymentError::InvalidTransition {
                from: current.status,
                to: PROVIDER_OPERATION_ERROR.to_string(),
            });
        }

        self.get(tenant_id, id).await
    }

    /// Record an operation whose external outcome cannot be safely retried.
    /// This transition is valid both after a persisted provider success and
    /// directly from `executing` when the provider may have accepted the request
    /// but the response or local success checkpoint is uncertain.
    pub async fn mark_reconciliation_required(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        error_message: impl Into<String>,
    ) -> PaymentResult<provider_operation::Model> {
        validate_operation_identity(tenant_id, id)?;
        let model = self.get(tenant_id, id).await?;
        if model.status == PROVIDER_OPERATION_RECONCILIATION_REQUIRED {
            return Ok(model);
        }
        ensure_transition(&model.status, PROVIDER_OPERATION_RECONCILIATION_REQUIRED)?;

        let error_message = normalize_error(error_message.into());
        let update = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_RECONCILIATION_REQUIRED),
            )
            .col_expr(
                provider_operation::Column::ErrorMessage,
                Expr::value(Some(error_message)),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::Id.eq(id))
            .filter(
                provider_operation::Column::Status
                    .is_in([PROVIDER_OPERATION_EXECUTING, PROVIDER_OPERATION_SUCCEEDED]),
            )
            .exec(&self.db)
            .await?;

        if update.rows_affected == 0 {
            let current = self.get(tenant_id, id).await?;
            if current.status == PROVIDER_OPERATION_RECONCILIATION_REQUIRED {
                return Ok(current);
            }
            return Err(PaymentError::InvalidTransition {
                from: current.status,
                to: PROVIDER_OPERATION_RECONCILIATION_REQUIRED.to_string(),
            });
        }

        self.get(tenant_id, id).await
    }

    pub async fn mark_committed(
        &self,
        tenant_id: Uuid,
        id: Uuid,
    ) -> PaymentResult<provider_operation::Model> {
        validate_operation_identity(tenant_id, id)?;
        let model = self.get(tenant_id, id).await?;
        if model.status == PROVIDER_OPERATION_COMMITTED {
            return Ok(model);
        }
        ensure_transition(&model.status, PROVIDER_OPERATION_COMMITTED)?;

        let provider_completion_missing = model.provider_completed_at.is_none();
        let now = Utc::now();
        let update = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(PROVIDER_OPERATION_COMMITTED),
            )
            .col_expr(
                provider_operation::Column::ErrorMessage,
                Expr::value(Option::<String>::None),
            )
            .col_expr(provider_operation::Column::UpdatedAt, Expr::value(now))
            .col_expr(
                provider_operation::Column::ProviderCompletedAt,
                if provider_completion_missing {
                    Expr::value(Some(now))
                } else {
                    Expr::col(provider_operation::Column::ProviderCompletedAt)
                },
            )
            .col_expr(
                provider_operation::Column::CommittedAt,
                Expr::value(Some(now)),
            )
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::Id.eq(id))
            .filter(provider_operation::Column::Status.is_in([
                PROVIDER_OPERATION_SUCCEEDED,
                PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
            ]))
            .exec(&self.db)
            .await?;

        if update.rows_affected == 0 {
            let current = self.get(tenant_id, id).await?;
            if current.status == PROVIDER_OPERATION_COMMITTED {
                return Ok(current);
            }
            return Err(PaymentError::InvalidTransition {
                from: current.status,
                to: PROVIDER_OPERATION_COMMITTED.to_string(),
            });
        }

        self.get(tenant_id, id).await
    }
}

async fn validate_begin_identity(
    db: &DatabaseConnection,
    input: &BeginProviderOperation,
) -> PaymentResult<()> {
    validate_operation_identity(input.tenant_id, input.payment_collection_id)?;

    let collection = payment_collection::Entity::find_by_id(input.payment_collection_id)
        .one(db)
        .await?
        .ok_or(PaymentError::PaymentCollectionNotFound(
            input.payment_collection_id,
        ))?;
    if collection.tenant_id != input.tenant_id {
        return Err(PaymentError::Validation(
            "payment provider operation tenant does not match payment collection tenant"
                .to_string(),
        ));
    }

    if let Some(refund_id) = input.refund_id {
        if refund_id.is_nil() {
            return Err(PaymentError::Validation(
                "payment provider operation refund_id must not be nil".to_string(),
            ));
        }
        let refund = refund::Entity::find_by_id(refund_id)
            .one(db)
            .await?
            .ok_or(PaymentError::RefundNotFound(refund_id))?;
        if refund.tenant_id != input.tenant_id
            || refund.payment_collection_id != input.payment_collection_id
        {
            return Err(PaymentError::Validation(
                "payment provider operation refund does not belong to the payment collection and tenant"
                    .to_string(),
            ));
        }
    }

    Ok(())
}

/// Bounded owner error for a provider operation whose last claim was refused by
/// the checkout execution admission contract.
///
/// The claim gate records the refusal on the row instead of returning a typed
/// error, so the callers that observe `Ok(None)` from
/// [`PaymentProviderOperationJournal::claim_execution`] can turn the recorded
/// reason into one bounded owner error. `None` means the claim was simply not
/// claimable (another worker holds it, or the provider result is already
/// persisted).
pub fn execution_admission_refusal_error(
    operation: &provider_operation::Model,
) -> Option<PortError> {
    let refusal = operation
        .admission_refusal_code
        .as_deref()
        .and_then(CheckoutAdmissionRefusal::parse)?;
    Some(match refusal {
        // A projection that drifted away can be repaired, so the caller may
        // retry once the checkout journal has projected its admission again.
        CheckoutAdmissionRefusal::Unavailable => PortError::unavailable(
            refusal.error_code(),
            "provider execution is fenced by an unresolved checkout admission",
        ),
        _ => PortError::conflict(
            refusal.error_code(),
            "provider execution is refused by the checkout admission contract",
        ),
    })
}

fn validate_operation_identity(tenant_id: Uuid, operation_id: Uuid) -> PaymentResult<()> {
    if tenant_id.is_nil() || operation_id.is_nil() {
        return Err(PaymentError::Validation(
            "payment provider operation requires non-nil tenant_id and operation_id".to_string(),
        ));
    }
    Ok(())
}

fn normalize_begin_input(
    mut input: BeginProviderOperation,
) -> PaymentResult<BeginProviderOperation> {
    input.operation = input.operation.trim().to_ascii_lowercase();
    if !matches!(
        input.operation.as_str(),
        "authorize" | "capture" | "cancel" | "refund"
    ) {
        return Err(PaymentError::Validation(format!(
            "unsupported payment provider operation `{}`",
            input.operation
        )));
    }

    input.provider_id = input.provider_id.trim().to_string();
    input.idempotency_key = input.idempotency_key.trim().to_string();
    if input.provider_id.is_empty() || input.provider_id.len() > 100 {
        return Err(PaymentError::Validation(
            "provider_id must contain 1 to 100 characters".to_string(),
        ));
    }
    if input.idempotency_key.is_empty() || input.idempotency_key.len() > 191 {
        return Err(PaymentError::Validation(
            "idempotency_key must contain 1 to 191 characters".to_string(),
        ));
    }

    Ok(input)
}

fn ensure_same_request(
    existing: &provider_operation::Model,
    input: &BeginProviderOperation,
) -> PaymentResult<()> {
    if existing.tenant_id != input.tenant_id
        || existing.payment_collection_id != input.payment_collection_id
        || existing.refund_id != input.refund_id
        || existing.operation != input.operation
        || existing.provider_id != input.provider_id
        || existing.request_payload != input.request_payload
    {
        return Err(PaymentError::Validation(format!(
            "provider idempotency key `{}` is already bound to another request",
            input.idempotency_key
        )));
    }
    Ok(())
}

fn ensure_transition(from: &str, to: &str) -> PaymentResult<()> {
    let allowed = matches!(
        (from, to),
        (PROVIDER_OPERATION_PENDING, PROVIDER_OPERATION_EXECUTING)
            | (PROVIDER_OPERATION_ERROR, PROVIDER_OPERATION_EXECUTING)
            | (PROVIDER_OPERATION_EXECUTING, PROVIDER_OPERATION_SUCCEEDED)
            | (PROVIDER_OPERATION_EXECUTING, PROVIDER_OPERATION_ERROR)
            | (
                PROVIDER_OPERATION_EXECUTING,
                PROVIDER_OPERATION_RECONCILIATION_REQUIRED
            )
            | (
                PROVIDER_OPERATION_SUCCEEDED,
                PROVIDER_OPERATION_RECONCILIATION_REQUIRED
            )
            | (PROVIDER_OPERATION_SUCCEEDED, PROVIDER_OPERATION_COMMITTED)
            | (
                PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
                PROVIDER_OPERATION_COMMITTED
            )
    );
    if allowed {
        Ok(())
    } else {
        Err(PaymentError::InvalidTransition {
            from: from.to_string(),
            to: to.to_string(),
        })
    }
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn validate_provider_result_for_operation(
    current: &provider_operation::Model,
    provider_reference: Option<String>,
    provider_result: &Value,
) -> PaymentResult<Option<String>> {
    if !provider_result.is_object() {
        return Err(PaymentError::ProviderInvalidResponse {
            provider_id: current.provider_id.clone(),
            operation: current.operation.clone(),
        });
    }
    validate_provider_operation_payload(provider_result, "result").map_err(|_| {
        PaymentError::ProviderInvalidResponse {
            provider_id: current.provider_id.clone(),
            operation: current.operation.clone(),
        }
    })?;

    let typed_result: PaymentProviderOperationResult =
        serde_json::from_value(provider_result.clone()).map_err(|_| {
            PaymentError::ProviderInvalidResponse {
                provider_id: current.provider_id.clone(),
                operation: current.operation.clone(),
            }
        })?;

    if typed_result.provider_id != current.provider_id
        || typed_result
            .external_reference
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| value.is_empty() || value.len() > MAX_EXTERNAL_REFERENCE_LENGTH)
        || !typed_result.metadata.is_object()
    {
        return Err(PaymentError::ProviderInvalidResponse {
            provider_id: current.provider_id.clone(),
            operation: current.operation.clone(),
        });
    }

    validate_provider_operation_payload(
        &serde_json::to_value(&typed_result).map_err(|_| {
            PaymentError::ProviderInvalidResponse {
                provider_id: current.provider_id.clone(),
                operation: current.operation.clone(),
            }
        })?,
        "canonical result",
    )
    .map_err(|_| PaymentError::ProviderInvalidResponse {
        provider_id: current.provider_id.clone(),
        operation: current.operation.clone(),
    })?;

    validate_provider_reference(current, provider_reference.as_deref())?;

    let result_reference = normalize_optional(typed_result.external_reference);
    let supplied_reference = normalize_optional(provider_reference);
    if let (Some(supplied), Some(result)) = (&supplied_reference, &result_reference)
        && supplied != result
    {
        return Err(PaymentError::ProviderInvalidResponse {
            provider_id: current.provider_id.clone(),
            operation: current.operation.clone(),
        });
    }

    Ok(supplied_reference.or(result_reference))
}

fn validate_provider_reference(
    current: &provider_operation::Model,
    value: Option<&str>,
) -> PaymentResult<()> {
    if let Some(value) = value {
        let value = value.trim();
        if value.is_empty() || value.len() > MAX_EXTERNAL_REFERENCE_LENGTH {
            return Err(PaymentError::ProviderInvalidResponse {
                provider_id: current.provider_id.clone(),
                operation: current.operation.clone(),
            });
        }
    }
    Ok(())
}

fn normalize_error(value: String) -> String {
    let value = value.trim();
    let value = if value.is_empty() {
        "provider operation failed"
    } else {
        value
    };
    value.chars().take(2000).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_operation() -> provider_operation::Model {
        let now = chrono::Utc::now().fixed_offset();
        provider_operation::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            payment_collection_id: Uuid::new_v4(),
            refund_id: None,
            operation: "authorize".to_string(),
            provider_id: "gateway".to_string(),
            idempotency_key: "test-operation".to_string(),
            status: PROVIDER_OPERATION_EXECUTING.to_string(),
            request_payload: serde_json::json!({}),
            provider_reference: None,
            provider_result: None,
            error_message: None,
            created_at: now,
            updated_at: now,
            provider_completed_at: None,
            committed_at: None,
            admission_epoch: 0,
            admission_refusal_code: None,
            admission_refused_at: None,
        }
    }

    #[test]
    fn provider_reference_boundary_is_enforced() {
        let current = test_operation();
        assert!(validate_provider_reference(&current, None).is_ok());
        assert!(validate_provider_reference(&current, Some("reference-1")).is_ok());
        assert!(validate_provider_reference(&current, Some("   ")).is_err());
        assert!(validate_provider_reference(&current, Some(&"r".repeat(192))).is_err());
    }

    #[test]
    fn journal_provider_result_identity_is_enforced() {
        let current = test_operation();
        let valid = serde_json::to_value(PaymentProviderOperationResult {
            provider_id: "gateway".to_string(),
            external_reference: Some("reference-1".to_string()),
            authorized_amount: rust_decimal::Decimal::new(100, 0),
            captured_amount: rust_decimal::Decimal::ZERO,
            metadata: serde_json::json!({}),
        })
        .expect("provider result should serialize");

        assert_eq!(
            validate_provider_result_for_operation(
                &current,
                Some("reference-1".to_string()),
                &valid,
            )
            .expect("valid provider result should pass"),
            Some("reference-1".to_string())
        );
        assert!(
            validate_provider_result_for_operation(
                &current,
                Some("other-reference".to_string()),
                &valid,
            )
            .is_err(),
            "supplied provider reference must match the external reference"
        );
        assert!(
            validate_provider_result_for_operation(&current, None, &serde_json::Value::Null)
                .is_err(),
            "non-object provider result must be rejected"
        );
    }

    #[test]
    fn uncertain_executing_outcome_requires_reconciliation() {
        assert!(
            ensure_transition(
                PROVIDER_OPERATION_EXECUTING,
                PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
            )
            .is_ok()
        );
        assert!(
            ensure_transition(
                PROVIDER_OPERATION_PENDING,
                PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
            )
            .is_err()
        );
    }
}
