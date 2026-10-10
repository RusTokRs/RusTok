use chrono::{DateTime, Duration, FixedOffset, Utc};
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait, sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use rustok_core::generate_id;
use rustok_events::{
    CHECKOUT_OPERATION_ADMISSION_CLOSED, CHECKOUT_OPERATION_ADMISSION_OPEN,
    CHECKOUT_OPERATION_ADMISSION_SETTLING,
    CHECKOUT_OPERATION_OUTCOME_COMPENSATION_REQUIRED as RECONCILED_OUTCOME_COMPENSATION_REQUIRED,
    CHECKOUT_OPERATION_PARK_REASON_ATTEMPTS_EXHAUSTED as PARK_REASON_ATTEMPTS_EXHAUSTED,
    CHECKOUT_OPERATION_PARK_REASON_MANUAL_RECONCILIATION as PARK_REASON_MANUAL_RECONCILIATION,
    CheckoutOperationEvent,
};
use rustok_outbox::TransactionalEventBus;
use rustok_payment::{PaymentError, PaymentProviderOperationJournal};

use crate::entities::{checkout_operation, checkout_reconciliation_action};

pub const DEFAULT_CHECKOUT_LEASE_SECONDS: i64 = 60;
pub const MAX_CHECKOUT_LEASE_SECONDS: i64 = 900;
pub const MAX_CHECKOUT_OPERATION_LIST_LIMIT: u64 = 200;

/// Upper bound on compensation claims for one operation before the journal
/// parks it for an operator instead of letting the sweep retry forever.
///
/// The counter is the row's `attempt_count`, which every claim increments
/// (`claim_execution`, `claim_compensation`). Eight attempts cover a provider
/// outage or a scripted retry, and the park makes the stuck cart visible in the
/// admin list and in `rustok_checkout_reconciliation_parked_total`.
pub const MAX_CHECKOUT_COMPENSATION_ATTEMPTS: i32 = 8;

/// Error code written by the compensation pipeline when the operation reached a
/// stage whose funds cannot be rolled back automatically. The journal parks the
/// operation in [`CheckoutOperationStatus::ReconciliationRequired`] the moment
/// this code is written, so the parking decision is owned by typed Rust code.
pub const CHECKOUT_COMPENSATION_MANUAL_RECONCILIATION_CODE: &str =
    "checkout.compensation_manual_reconciliation";

/// Error code recorded when an operator sends a parked operation back to the
/// compensation queue.
pub const CHECKOUT_RECONCILIATION_RETRY_REQUESTED_CODE: &str =
    "checkout.reconciliation_retry_requested";

/// Error code recorded when an operator closes a parked operation.
pub const CHECKOUT_RECONCILIATION_RESOLVED_CODE: &str = "checkout.reconciliation_resolved";

/// Error code recorded when the compensation sweep gives up on an operation
/// whose [`MAX_CHECKOUT_COMPENSATION_ATTEMPTS`] are exhausted. Like the manual
/// reconciliation code, it parks the operation so the cart is no longer retried
/// in the dark.
pub const CHECKOUT_COMPENSATION_ATTEMPTS_EXHAUSTED_CODE: &str =
    "checkout.compensation_attempts_exhausted";

// The bounded park labels (`PARK_REASON_*`) are imported from
// `rustok-events`: the metric label and the published `checkout.operation.parked`
// payload share one vocabulary, so a new reason cannot reach one without the other.

/// Status of a checkout operation journal row.
///
/// The state machine below is the single source of truth for legal status
/// transitions. It lives in typed Rust code (see [`CheckedTransition`]), not in
/// PL/pgSQL functions or SQLite triggers, because the repository contract
/// (`AGENTS.md`: "Multi-row domain rules, state transition validations,
/// workflow guards ... MUST be owned and validated by typed Rust domain
/// entities and services inside a transaction boundary") forbids business rules
/// inside constraint triggers. The database keeps only column-level `CHECK`
/// constraints and the `ux_checkout_operations_active_cart` uniqueness index.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CheckoutOperationStatus {
    Pending,
    Executing,
    RetryableError,
    CompensationRequired,
    Compensating,
    /// Terminal parking state written by the journal when a compensation ends
    /// in `ManualReconciliation`: the operation must be reconciled by an
    /// operator before the cart can start a new checkout.
    ReconciliationRequired,
    Completed,
    Compensated,
    Failed,
}

impl CheckoutOperationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Executing => "executing",
            Self::RetryableError => "retryable_error",
            Self::CompensationRequired => "compensation_required",
            Self::Compensating => "compensating",
            Self::ReconciliationRequired => "reconciliation_required",
            Self::Completed => "completed",
            Self::Compensated => "compensated",
            Self::Failed => "failed",
        }
    }

    /// Statuses this one may move to. A write that changes the status of a
    /// journal row without listing the target here is a bug and is rejected by
    /// [`CheckedTransition::new`] before the row is touched.
    pub const fn allowed_transitions(self) -> &'static [Self] {
        match self {
            Self::Pending => &[Self::Executing],
            Self::Executing => &[
                Self::RetryableError,
                Self::CompensationRequired,
                Self::ReconciliationRequired,
                Self::Completed,
                Self::Failed,
            ],
            Self::RetryableError => &[Self::Executing],
            Self::CompensationRequired => &[Self::Compensating, Self::ReconciliationRequired],
            Self::Compensating => &[
                Self::CompensationRequired,
                Self::ReconciliationRequired,
                Self::Compensated,
                Self::Failed,
            ],
            // Operators may close a parked operation, or send it back to the
            // compensation queue after the blocking condition was removed.
            Self::ReconciliationRequired => {
                &[Self::Compensated, Self::Failed, Self::CompensationRequired]
            }
            Self::Completed | Self::Compensated | Self::Failed => &[],
        }
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        self.allowed_transitions().contains(&next)
    }

    /// Statuses that must carry `completed_at` (mirrors the
    /// `ck_checkout_operations_completion` column-level constraint).
    pub const fn requires_completed_at(self) -> bool {
        matches!(
            self,
            Self::ReconciliationRequired | Self::Completed | Self::Compensated | Self::Failed
        )
    }

    /// Parked operations block the cart's active-checkout slot and wait for an
    /// operator decision instead of automation.
    pub const fn is_parked(self) -> bool {
        matches!(self, Self::ReconciliationRequired)
    }
}

/// Provider execution admission level owned by the checkout journal.
///
/// The level is the checkout-owned half of the provider execution admission
/// contract: `rustok-payment` projects it and refuses to start an *extending*
/// provider operation (`authorize`, `capture`) whose generation is not the
/// admitted one, while *unwinding* effects (`cancel`, `refund`) stay admitted
/// because the compensation itself needs the provider.
///
/// The level only moves forward (`open` -> `settling` -> `closed`) and every
/// move increments the operation's `admission_epoch`, so a provider operation
/// created under an older generation can never be claimed against a newer one.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CheckoutExecutionAdmission {
    Open,
    Settling,
    Closed,
}

impl CheckoutExecutionAdmission {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => CHECKOUT_OPERATION_ADMISSION_OPEN,
            Self::Settling => CHECKOUT_OPERATION_ADMISSION_SETTLING,
            Self::Closed => CHECKOUT_OPERATION_ADMISSION_CLOSED,
        }
    }

    /// Parses the bounded vocabulary back into the typed level. The journal
    /// writes the level itself, so a value outside the vocabulary is a
    /// programming error, not user input.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            CHECKOUT_OPERATION_ADMISSION_OPEN => Some(Self::Open),
            CHECKOUT_OPERATION_ADMISSION_SETTLING => Some(Self::Settling),
            CHECKOUT_OPERATION_ADMISSION_CLOSED => Some(Self::Closed),
            _ => None,
        }
    }

    /// Admission level a journal status belongs to. Every writer derives the
    /// level from this function, so the status machine and the admission
    /// contract cannot drift apart.
    pub const fn for_status(status: CheckoutOperationStatus) -> Self {
        match status {
            CheckoutOperationStatus::Pending
            | CheckoutOperationStatus::Executing
            | CheckoutOperationStatus::RetryableError => Self::Open,
            CheckoutOperationStatus::CompensationRequired
            | CheckoutOperationStatus::Compensating
            | CheckoutOperationStatus::ReconciliationRequired => Self::Settling,
            CheckoutOperationStatus::Completed
            | CheckoutOperationStatus::Compensated
            | CheckoutOperationStatus::Failed => Self::Closed,
        }
    }
}

/// Operator-selected outcome for closing an operation parked in
/// `reconciliation_required`.
///
/// The parked state means automation gave up: the checkout failed after funds may
/// have been captured and the compensation path refuses to continue. The operator
/// reconciles the money out of band (refund issued, or the charge written off) and
/// closes the journal row with one of these outcomes, which also releases the
/// cart's active-checkout slot (`ux_checkout_operations_active_cart`).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CheckoutReconciliationOutcome {
    /// Funds were returned to the customer; the checkout closes as compensated.
    Compensated,
    /// The checkout is written off without returning funds.
    Failed,
}

impl CheckoutReconciliationOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Compensated => "compensated",
            Self::Failed => "failed",
        }
    }
}

/// The operator decision that closes a parked operation.
///
/// It is passed into [`CheckoutOperationJournal::resolve_reconciliation_required`]
/// and [`CheckoutOperationJournal::request_compensation_retry`] so the transition
/// and the append-only `checkout_reconciliation_actions` row are written by one
/// transaction: an operation can never leave `reconciliation_required` without
/// the row that says who decided what on which evidence, and a row can never
/// describe a decision for an operation that is still parked. A crash between
/// the two writes is therefore impossible, and a retry of the same operator
/// request finds the operation exactly as the failed attempt left it.
///
/// Every column is bounded by the owning service before it gets here (action
/// name, idempotency key, reason, evidence reference); the journal only adds the
/// fields that come from the row it is closing (`tenant_id`, operation id,
/// `cart_id`, resulting status) and from the payment owner's refund response.
#[derive(Clone, Debug)]
pub struct CheckoutReconciliationDecision {
    /// Operator action from the bounded registry vocabulary
    /// (`CheckoutReconciliationAction::as_str`).
    pub action: String,
    /// Operator who took the decision; also the actor of the event.
    pub operator_id: Uuid,
    /// Operator-supplied idempotency key of the request.
    pub idempotency_key: String,
    /// Fingerprint of the whole request, so a replayed key with a different
    /// payload is a conflict instead of a second decision.
    pub request_hash: String,
    /// The operator's own justification, recorded verbatim.
    pub reason: String,
    /// Evidence of an out-of-band reconciliation, when the action had one.
    pub evidence_ref: Option<String>,
    /// Second approver, for decisions that require one.
    pub approver_id: Option<Uuid>,
    /// Refunded amount for the money actions.
    pub amount: Option<Decimal>,
    /// Currency of `amount`.
    pub currency_code: Option<String>,
    /// Refund created through the payment owner, when the action issued one.
    pub refund_id: Option<Uuid>,
    /// Status the payment owner reported for the refund or the cancelled
    /// collection.
    pub refund_status: Option<String>,
}

/// Result of a reconciliation command: the closed operation and the decision row
/// the same transaction appended for it.
#[derive(Clone, Debug)]
pub struct ReconciliationResolution {
    /// The operation as it was committed by the command.
    pub operation: checkout_operation::Model,
    /// The append-only decision row written by the same transaction.
    pub decision: checkout_reconciliation_action::Model,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CheckoutOperationStage {
    Created,
    CartLocked,
    OrderCreated,
    InventoryReserved,
    PaymentReady,
    PaymentAuthorized,
    PaymentCaptured,
    FulfillmentCreated,
    CartCompleted,
    Completed,
}

impl CheckoutOperationStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::CartLocked => "cart_locked",
            Self::OrderCreated => "order_created",
            Self::InventoryReserved => "inventory_reserved",
            Self::PaymentReady => "payment_ready",
            Self::PaymentAuthorized => "payment_authorized",
            Self::PaymentCaptured => "payment_captured",
            Self::FulfillmentCreated => "fulfillment_created",
            Self::CartCompleted => "cart_completed",
            Self::Completed => "completed",
        }
    }
}

#[derive(Debug, Error)]
pub enum CheckoutOperationError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("checkout operation {0} not found")]
    NotFound(Uuid),
    #[error("checkout operation conflict: {0}")]
    Conflict(String),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
    /// The transition and the event that describes it are one unit of work: when
    /// the outbox write fails the status write is rolled back and the caller sees
    /// why. A published state that has no event, and an event for a state that
    /// was rolled back, are both impossible.
    #[error("checkout operation event publication failed: {0}")]
    Event(#[from] rustok_core::Error),
}

pub type CheckoutOperationResult<T> = Result<T, CheckoutOperationError>;

#[derive(Clone, Debug)]
pub struct BeginCheckoutOperation {
    pub tenant_id: Uuid,
    pub cart_id: Uuid,
    pub idempotency_key: String,
    pub request_hash: String,
    pub snapshot_hash: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CheckoutOperationCheckpoint {
    pub tenant_id: Uuid,
    pub operation_id: Uuid,
    pub lease_owner: String,
    pub expected_stage: CheckoutOperationStage,
    pub next_stage: CheckoutOperationStage,
    pub snapshot_hash: Option<String>,
    pub order_id: Option<Uuid>,
    pub payment_collection_id: Option<Uuid>,
    pub lease_seconds: i64,
}

/// Journal of checkout operations.
///
/// The journal owns the checkout state machine in typed Rust code:
/// [`CheckoutOperationStatus::allowed_transitions`] is enforced by
/// [`CheckedTransition`] on every status write, the parking decision lives in
/// [`compensation_next_status`], the lease/completion shape follows from those
/// writers, identity columns are never part of an update, and the
/// cross-aggregate tenant checks run in the same transaction as the write. The
/// database keeps the column-level `CHECK` constraints and the
/// `ux_checkout_operations_active_cart` uniqueness index only.
///
/// The journal is also the single publisher of the checkout operation money-path
/// events ([`CheckoutOperationEvent`]): the transition that parks or releases an
/// operation is written together with its event in one transaction, so a
/// consumer can never observe a parked operation without a `parked` event.
#[derive(Clone)]
pub struct CheckoutOperationJournal {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    payment_operations: PaymentProviderOperationJournal,
}

struct LeaseErrorTransition {
    lease_owner: String,
    expected_status: CheckoutOperationStatus,
    next_status: CheckoutOperationStatus,
    error_code: String,
    error_message: String,
}

struct TerminalTransition {
    lease_owner: String,
    expected_status: CheckoutOperationStatus,
    next_status: CheckoutOperationStatus,
    next_stage: Option<CheckoutOperationStage>,
    error_code: Option<String>,
    error_message: Option<String>,
}

/// Validated status transition. Every journal write that changes
/// `checkout_operations.status` builds one of these first, so the state machine
/// in [`CheckoutOperationStatus::allowed_transitions`] is enforced by the
/// service inside the transaction.
struct CheckedTransition {
    expected: CheckoutOperationStatus,
    next: CheckoutOperationStatus,
}

impl CheckedTransition {
    fn new(
        expected: CheckoutOperationStatus,
        next: CheckoutOperationStatus,
    ) -> CheckoutOperationResult<Self> {
        if !expected.can_transition_to(next) {
            return Err(CheckoutOperationError::Conflict(format!(
                "checkout operation cannot move from `{}` to `{}`; allowed targets: {}",
                expected.as_str(),
                next.as_str(),
                allowed_transition_targets(expected)
            )));
        }
        Ok(Self { expected, next })
    }
}

/// Admission columns of one status write.
///
/// `previous` is derived from the CAS-expected status: the conditional update
/// only lands while the row still carries that status, so the journal knows the
/// level it moves away from without a second read. The epoch moves exactly when
/// the level changes, so a write that stays inside one level keeps the
/// generation that in-flight provider operations were admitted under.
#[derive(Clone, Copy)]
struct AdmissionWrite {
    previous: CheckoutExecutionAdmission,
    next: CheckoutExecutionAdmission,
}

impl AdmissionWrite {
    fn between(expected: CheckoutOperationStatus, next: CheckoutOperationStatus) -> Self {
        Self {
            previous: CheckoutExecutionAdmission::for_status(expected),
            next: CheckoutExecutionAdmission::for_status(next),
        }
    }

    /// A write that re-asserts one level without moving the generation: the
    /// lease/claim writers keep the operation inside the level it already had.
    fn unchanged(level: CheckoutExecutionAdmission) -> Self {
        Self {
            previous: level,
            next: level,
        }
    }

    fn changes_the_level(self) -> bool {
        self.previous != self.next
    }

    fn epoch_delta(self) -> i64 {
        if self.changes_the_level() { 1 } else { 0 }
    }
}

/// Status written by the compensation journal for a failed compensation step.
///
/// `ManualReconciliation` failures mean the funds cannot be rolled back by the
/// pipeline (captured money, or a payment/order owner that refuses the
/// compensation). Those park the operation so an operator sees it in the
/// reconciliation queue, instead of leaving it to be swept forever.
fn compensation_next_status(error_code: &str) -> CheckoutOperationStatus {
    if error_code == CHECKOUT_COMPENSATION_MANUAL_RECONCILIATION_CODE {
        CheckoutOperationStatus::ReconciliationRequired
    } else {
        CheckoutOperationStatus::CompensationRequired
    }
}

fn allowed_transition_targets(status: CheckoutOperationStatus) -> String {
    let targets: Vec<&'static str> = status
        .allowed_transitions()
        .iter()
        .map(|status| status.as_str())
        .collect();
    if targets.is_empty() {
        "none (terminal status)".to_string()
    } else {
        targets.join(", ")
    }
}

impl CheckoutOperationJournal {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            payment_operations: PaymentProviderOperationJournal::new(db.clone()),
            db,
            event_bus,
        }
    }

    pub async fn begin(
        &self,
        input: BeginCheckoutOperation,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        let input = normalize_begin_input(input)?;
        if let Some(existing) = self
            .find_by_key(
                input.tenant_id,
                input.cart_id,
                input.idempotency_key.as_str(),
            )
            .await?
        {
            ensure_same_request(&existing, &input)?;
            return Ok(existing);
        }
        if let Some(active) = self
            .find_active_by_cart(input.tenant_id, input.cart_id)
            .await?
        {
            return Err(active_cart_conflict(input.cart_id, active.id));
        }

        ensure_cart_tenant(&self.db, input.tenant_id, input.cart_id).await?;

        let id = generate_id();
        let now = Utc::now();
        let admission_epoch = self
            .next_admission_epoch(&self.db, input.tenant_id, input.cart_id)
            .await?;
        let txn = self.db.begin().await?;
        let insert = checkout_operation::ActiveModel {
            id: Set(id),
            tenant_id: Set(input.tenant_id),
            cart_id: Set(input.cart_id),
            idempotency_key: Set(input.idempotency_key.clone()),
            request_hash: Set(input.request_hash.clone()),
            snapshot_hash: Set(input.snapshot_hash.clone()),
            status: Set(CheckoutOperationStatus::Pending.as_str().to_string()),
            execution_admission: Set(CheckoutExecutionAdmission::Open.as_str().to_string()),
            admission_epoch: Set(admission_epoch),
            stage: Set(CheckoutOperationStage::Created.as_str().to_string()),
            order_id: Set(None),
            payment_collection_id: Set(None),
            attempt_count: Set(0),
            lease_owner: Set(None),
            lease_expires_at: Set(None),
            last_error_code: Set(None),
            last_error_message: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            completed_at: Set(None),
        }
        .insert(&txn)
        .await;

        match insert {
            Ok(model) => {
                // The record of the fresh generation, its event and the
                // invalidation of the previous generation are one unit of work,
                // so no consumer can see the new checkout attempt while an
                // operation of the old one is still claimable.
                self.publish_admission_changed(&txn, &model, None).await?;
                txn.commit().await?;
                Ok(model)
            }
            Err(insert_error) => {
                txn.rollback().await?;
                if let Some(existing) = self
                    .find_by_key(
                        input.tenant_id,
                        input.cart_id,
                        input.idempotency_key.as_str(),
                    )
                    .await?
                {
                    ensure_same_request(&existing, &input)?;
                    return Ok(existing);
                }
                if let Some(active) = self
                    .find_active_by_cart(input.tenant_id, input.cart_id)
                    .await?
                {
                    return Err(active_cart_conflict(input.cart_id, active.id));
                }
                Err(insert_error.into())
            }
        }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        id: Uuid,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        self.get_in(&self.db, tenant_id, id).await
    }

    /// Reads the row through an explicit connection, so a writer that owns a
    /// transaction reads back exactly the state it wrote in that transaction.
    async fn get_in<C>(
        &self,
        db: &C,
        tenant_id: Uuid,
        id: Uuid,
    ) -> CheckoutOperationResult<checkout_operation::Model>
    where
        C: ConnectionTrait,
    {
        checkout_operation::Entity::find_by_id(id)
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .ok_or(CheckoutOperationError::NotFound(id))
    }

    /// Publishes `checkout.operation.parked` in the writer's transaction.
    ///
    /// The park decision is made by the state machine, not by a human, so the
    /// envelope carries no actor; the operation row records who held the lease
    /// and why the compensation stopped.
    async fn publish_parked<C>(
        &self,
        txn: &C,
        operation: &checkout_operation::Model,
        reason: &'static str,
    ) -> CheckoutOperationResult<()>
    where
        C: ConnectionTrait,
    {
        self.event_bus
            .publish_contract_in_tx(
                txn,
                operation.tenant_id,
                None,
                CheckoutOperationEvent::Parked {
                    operation_id: operation.id,
                    cart_id: operation.cart_id,
                    reason: reason.to_string(),
                },
            )
            .await?;
        Ok(())
    }

    /// Publishes `checkout.operation.reconciled` in the writer's transaction.
    async fn publish_reconciled<C>(
        &self,
        txn: &C,
        operation: &checkout_operation::Model,
        outcome: &'static str,
        operator_id: Uuid,
    ) -> CheckoutOperationResult<()>
    where
        C: ConnectionTrait,
    {
        self.event_bus
            .publish_contract_in_tx(
                txn,
                operation.tenant_id,
                Some(operator_id),
                CheckoutOperationEvent::Reconciled {
                    operation_id: operation.id,
                    cart_id: operation.cart_id,
                    outcome: outcome.to_string(),
                    operator_id,
                },
            )
            .await?;
        Ok(())
    }

    /// Publishes `checkout.operation.admission_changed` in the writer's transaction.
    ///
    /// The level, its generation, the event and the invalidation of the provider
    /// operations the previous generation admitted are one unit of work. The
    /// payment claim gate fences on the generation stored on the provider
    /// operation (`payment_provider_operations.admission_epoch`), so this write
    /// stamps the cart's non-terminal operations with the new generation before
    /// the new level becomes visible: a claim that was decided under the
    /// previous generation fails its own conditional write instead of racing the
    /// park, and a claim that already committed is ordered before it.
    async fn publish_admission_changed<C>(
        &self,
        txn: &C,
        operation: &checkout_operation::Model,
        previous_admission: Option<CheckoutExecutionAdmission>,
    ) -> CheckoutOperationResult<()>
    where
        C: ConnectionTrait,
    {
        let admission = CheckoutExecutionAdmission::parse(operation.execution_admission.as_str())
            .ok_or_else(|| {
            CheckoutOperationError::Validation(format!(
                "checkout operation {} carries unknown admission level `{}`",
                operation.id, operation.execution_admission
            ))
        })?;
        self.invalidate_provider_execution_admitted_by(txn, operation)
            .await?;
        self.event_bus
            .publish_contract_in_tx(
                txn,
                operation.tenant_id,
                None,
                CheckoutOperationEvent::AdmissionChanged {
                    operation_id: operation.id,
                    cart_id: operation.cart_id,
                    previous_admission: previous_admission.map(|level| level.as_str().to_string()),
                    admission: admission.as_str().to_string(),
                    admission_epoch: operation.admission_epoch,
                    status: operation.status.clone(),
                },
            )
            .await?;
        Ok(())
    }

    /// Stamps the new generation on the provider operations that were admitted
    /// under the previous one, inside the writer's transaction.
    ///
    /// The scope is the cart, not `operation.payment_collection_id`: the binding
    /// column is a convenience the payment stage writes once it reaches
    /// `payment_authorized`, so a park that lands between the payment collection
    /// being created and that write would have nothing to stamp if this fence read
    /// the binding. `ux_checkout_operations_active_cart` allows one live operation
    /// per cart and every provider operation of this checkout belongs to a
    /// collection of that cart, so this scope stamps every claim the park must
    /// invalidate — and it also fences the previous generation's claims at
    /// `begin`, when the new row has no binding yet.
    ///
    /// A failure fails the whole admission transition: the journal must never
    /// publish a level that in-flight claims can still execute through.
    async fn invalidate_provider_execution_admitted_by<C>(
        &self,
        txn: &C,
        operation: &checkout_operation::Model,
    ) -> CheckoutOperationResult<()>
    where
        C: ConnectionTrait,
    {
        let stamped = self
            .payment_operations
            .stamp_admission_epoch_for_cart(
                txn,
                operation.tenant_id,
                operation.cart_id,
                operation.admission_epoch,
            )
            .await
            .map_err(|error| match error {
                PaymentError::Database(error) => CheckoutOperationError::Database(error),
                _ => CheckoutOperationError::Conflict(
                    "payment provider operations could not be stamped with the new admission generation"
                        .to_string(),
                ),
            })?;
        if stamped > 0 {
            tracing::info!(
                operation_id = %operation.id,
                tenant_id_length = operation.tenant_id.to_string().chars().count(),
                admission = operation.execution_admission.as_str(),
                admission_epoch = operation.admission_epoch,
                stamped_provider_operations = stamped,
                "checkout admission transition invalidated in-flight provider claims"
            );
        }
        Ok(())
    }

    /// Next admission generation for a cart.
    ///
    /// Every journal row starts a new generation, so a provider operation created
    /// under an earlier checkout attempt can never be claimed against a later one
    /// even when the payment collection is reused for the new attempt. Two
    /// concurrent `begin` calls for one cart cannot both insert: the partial
    /// unique index `ux_checkout_operations_active_cart` admits a single active
    /// row, so the read-then-insert cannot publish two live generations.
    async fn next_admission_epoch<C>(
        &self,
        db: &C,
        tenant_id: Uuid,
        cart_id: Uuid,
    ) -> CheckoutOperationResult<i64>
    where
        C: ConnectionTrait,
    {
        let latest = checkout_operation::Entity::find()
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .filter(checkout_operation::Column::CartId.eq(cart_id))
            .order_by_desc(checkout_operation::Column::AdmissionEpoch)
            .one(db)
            .await?;
        Ok(latest.map(|row| row.admission_epoch).unwrap_or(0) + 1)
    }

    pub async fn find_by_key(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        idempotency_key: &str,
    ) -> CheckoutOperationResult<Option<checkout_operation::Model>> {
        checkout_operation::Entity::find()
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .filter(checkout_operation::Column::CartId.eq(cart_id))
            .filter(checkout_operation::Column::IdempotencyKey.eq(idempotency_key))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn find_latest_by_cart(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
    ) -> CheckoutOperationResult<Option<checkout_operation::Model>> {
        checkout_operation::Entity::find()
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .filter(checkout_operation::Column::CartId.eq(cart_id))
            .order_by_desc(checkout_operation::Column::CreatedAt)
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn find_active_by_cart(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
    ) -> CheckoutOperationResult<Option<checkout_operation::Model>> {
        checkout_operation::Entity::find()
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .filter(checkout_operation::Column::CartId.eq(cart_id))
            .filter(checkout_operation::Column::Status.is_in(active_statuses()))
            .order_by_desc(checkout_operation::Column::CreatedAt)
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    /// Lists checkout operations for the admin reconciliation surfaces, newest
    /// update first, optionally filtered by exact status.
    pub async fn list_by_status(
        &self,
        tenant_id: Uuid,
        status: Option<&str>,
        limit: u64,
    ) -> CheckoutOperationResult<Vec<checkout_operation::Model>> {
        if tenant_id.is_nil() {
            return Err(CheckoutOperationError::Validation(
                "checkout operation listing requires a non-nil tenant identifier".to_string(),
            ));
        }
        if let Some(status) = status
            && !is_known_status(status)
        {
            return Err(CheckoutOperationError::Validation(format!(
                "unknown checkout operation status `{status}`"
            )));
        }
        let limit = limit.clamp(1, MAX_CHECKOUT_OPERATION_LIST_LIMIT);
        let mut query = checkout_operation::Entity::find()
            .filter(checkout_operation::Column::TenantId.eq(tenant_id));
        if let Some(status) = status {
            query = query.filter(checkout_operation::Column::Status.eq(status));
        }
        query
            .order_by_desc(checkout_operation::Column::UpdatedAt)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn claim_execution(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        lease_owner: impl Into<String>,
        lease_seconds: i64,
    ) -> CheckoutOperationResult<Option<checkout_operation::Model>> {
        let lease_owner = normalize_lease_owner(lease_owner.into())?;
        let lease_seconds = normalize_lease_seconds(lease_seconds)?;
        let admission = AdmissionWrite::unchanged(CheckoutExecutionAdmission::Open);
        let now = Utc::now().fixed_offset();
        let lease_expires_at = now + Duration::seconds(lease_seconds);
        let claimable = Condition::any()
            .add(checkout_operation::Column::Status.is_in([
                CheckoutOperationStatus::Pending.as_str(),
                CheckoutOperationStatus::RetryableError.as_str(),
            ]))
            .add(
                Condition::all()
                    .add(
                        checkout_operation::Column::Status
                            .eq(CheckoutOperationStatus::Executing.as_str()),
                    )
                    .add(checkout_operation::Column::LeaseExpiresAt.lte(now)),
            );

        let update = checkout_operation::Entity::update_many()
            .col_expr(
                checkout_operation::Column::Status,
                Expr::value(CheckoutOperationStatus::Executing.as_str()),
            )
            .col_expr(
                checkout_operation::Column::ExecutionAdmission,
                Expr::value(admission.next.as_str()),
            )
            .col_expr(
                checkout_operation::Column::AdmissionEpoch,
                sea_orm::sea_query::ExprTrait::add(
                    Expr::col(checkout_operation::Column::AdmissionEpoch),
                    admission.epoch_delta(),
                ),
            )
            .col_expr(
                checkout_operation::Column::LeaseOwner,
                Expr::value(Some(lease_owner)),
            )
            .col_expr(
                checkout_operation::Column::LeaseExpiresAt,
                Expr::value(Some(lease_expires_at)),
            )
            .col_expr(
                checkout_operation::Column::AttemptCount,
                sea_orm::sea_query::ExprTrait::add(
                    Expr::col(checkout_operation::Column::AttemptCount),
                    1,
                ),
            )
            .col_expr(
                checkout_operation::Column::LastErrorCode,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                checkout_operation::Column::LastErrorMessage,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                checkout_operation::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .filter(checkout_operation::Column::Id.eq(id))
            .filter(claimable)
            .exec(&self.db)
            .await?;

        if update.rows_affected == 0 {
            return Ok(None);
        }
        self.get(tenant_id, id).await.map(Some)
    }

    pub async fn renew_lease(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        lease_owner: impl Into<String>,
        lease_seconds: i64,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        let lease_owner = normalize_lease_owner(lease_owner.into())?;
        let lease_seconds = normalize_lease_seconds(lease_seconds)?;
        let now = Utc::now().fixed_offset();
        let lease_expires_at = now + Duration::seconds(lease_seconds);

        let update = checkout_operation::Entity::update_many()
            .col_expr(
                checkout_operation::Column::LeaseExpiresAt,
                Expr::value(Some(lease_expires_at)),
            )
            .col_expr(
                checkout_operation::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .filter(checkout_operation::Column::Id.eq(id))
            .filter(
                checkout_operation::Column::Status.eq(CheckoutOperationStatus::Executing.as_str()),
            )
            .filter(checkout_operation::Column::LeaseOwner.eq(lease_owner.clone()))
            .filter(checkout_operation::Column::LeaseExpiresAt.gt(now))
            .exec(&self.db)
            .await?;

        if update.rows_affected == 0 {
            return Err(CheckoutOperationError::Conflict(format!(
                "checkout operation {id} lease is no longer held by `{lease_owner}` or has expired"
            )));
        }
        self.get(tenant_id, id).await
    }

    pub async fn checkpoint(
        &self,
        input: CheckoutOperationCheckpoint,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        let lease_owner = normalize_lease_owner(input.lease_owner)?;
        let lease_seconds = normalize_lease_seconds(input.lease_seconds)?;
        let snapshot_hash = input.snapshot_hash.map(normalize_hash).transpose()?;
        let now = Utc::now().fixed_offset();
        let lease_expires_at = now + Duration::seconds(lease_seconds);

        let mut update = checkout_operation::Entity::update_many()
            .col_expr(
                checkout_operation::Column::Stage,
                Expr::value(input.next_stage.as_str()),
            )
            .col_expr(
                checkout_operation::Column::LeaseExpiresAt,
                Expr::value(Some(lease_expires_at)),
            )
            .col_expr(
                checkout_operation::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .filter(checkout_operation::Column::TenantId.eq(input.tenant_id))
            .filter(checkout_operation::Column::Id.eq(input.operation_id))
            .filter(
                checkout_operation::Column::Status.eq(CheckoutOperationStatus::Executing.as_str()),
            )
            .filter(checkout_operation::Column::Stage.eq(input.expected_stage.as_str()))
            .filter(checkout_operation::Column::LeaseOwner.eq(lease_owner))
            .filter(checkout_operation::Column::LeaseExpiresAt.gt(now));

        if let Some(snapshot_hash) = snapshot_hash {
            update = update.col_expr(
                checkout_operation::Column::SnapshotHash,
                Expr::value(Some(snapshot_hash)),
            );
        }
        if let Some(order_id) = input.order_id {
            ensure_order_tenant(&self.db, input.tenant_id, order_id).await?;
            update = update.col_expr(
                checkout_operation::Column::OrderId,
                Expr::value(Some(order_id)),
            );
        }
        if let Some(payment_collection_id) = input.payment_collection_id {
            ensure_payment_collection_tenant(&self.db, input.tenant_id, payment_collection_id)
                .await?;
            // The binding is write-once, the rule the removed payment-collection
            // guard enforced from the collection side: the operation may accept a
            // collection while it carries none, or re-assert the one it already
            // carries, but it may never be re-pointed at a different collection.
            // Provider operations, marketplace financial rows and refunds are all
            // keyed by the collection this column names, so a silent rebind would
            // detach the money evidence from the checkout it belongs to. The
            // predicate is part of the same conditional write, so there is no
            // read-then-write window.
            let binding = checkout_operation::Column::PaymentCollectionId;
            update = update
                .filter(
                    Condition::any()
                        .add(binding.is_null())
                        .add(binding.eq(payment_collection_id)),
                )
                .col_expr(binding, Expr::value(Some(payment_collection_id)));
        }

        let result = update.exec(&self.db).await?;
        if result.rows_affected == 0 {
            let current = self.get(input.tenant_id, input.operation_id).await?;
            match (current.payment_collection_id, input.payment_collection_id) {
                (Some(bound), Some(requested)) if bound != requested => {
                    return Err(CheckoutOperationError::Conflict(format!(
                        "checkout operation {} is already bound to payment collection {bound}; \
                         refusing to rebind it to {requested}",
                        input.operation_id
                    )));
                }
                _ => {}
            }
            return Err(self
                .cas_conflict(input.tenant_id, input.operation_id, "checkpoint")
                .await?);
        }
        self.get(input.tenant_id, input.operation_id).await
    }

    pub async fn mark_retryable_error(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        lease_owner: impl Into<String>,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        self.release_lease_with_error(
            tenant_id,
            id,
            LeaseErrorTransition {
                lease_owner: lease_owner.into(),
                expected_status: CheckoutOperationStatus::Executing,
                next_status: CheckoutOperationStatus::RetryableError,
                error_code: error_code.into(),
                error_message: error_message.into(),
            },
        )
        .await
    }

    /// Records that a failed checkout step needs the compensation pipeline.
    ///
    /// When the failure code is
    /// [`CHECKOUT_COMPENSATION_MANUAL_RECONCILIATION_CODE`] the operation is
    /// parked in `reconciliation_required` immediately: the code means the
    /// pipeline cannot roll the money back on its own.
    pub async fn mark_compensation_required(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        lease_owner: impl Into<String>,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        let error_code = error_code.into();
        let next_status = compensation_next_status(error_code.as_str());
        self.release_lease_with_error(
            tenant_id,
            id,
            LeaseErrorTransition {
                lease_owner: lease_owner.into(),
                expected_status: CheckoutOperationStatus::Executing,
                next_status,
                error_code,
                error_message: error_message.into(),
            },
        )
        .await
    }

    pub async fn claim_compensation(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        lease_owner: impl Into<String>,
        lease_seconds: i64,
    ) -> CheckoutOperationResult<Option<checkout_operation::Model>> {
        let lease_owner = normalize_lease_owner(lease_owner.into())?;
        let lease_seconds = normalize_lease_seconds(lease_seconds)?;
        let admission = AdmissionWrite::unchanged(CheckoutExecutionAdmission::Settling);
        let now = Utc::now().fixed_offset();
        let lease_expires_at = now + Duration::seconds(lease_seconds);
        let claimable = Condition::any()
            .add(
                checkout_operation::Column::Status
                    .eq(CheckoutOperationStatus::CompensationRequired.as_str()),
            )
            .add(
                Condition::all()
                    .add(
                        checkout_operation::Column::Status
                            .eq(CheckoutOperationStatus::Compensating.as_str()),
                    )
                    .add(checkout_operation::Column::LeaseExpiresAt.lte(now)),
            );
        let update = checkout_operation::Entity::update_many()
            .col_expr(
                checkout_operation::Column::Status,
                Expr::value(CheckoutOperationStatus::Compensating.as_str()),
            )
            .col_expr(
                checkout_operation::Column::ExecutionAdmission,
                Expr::value(admission.next.as_str()),
            )
            .col_expr(
                checkout_operation::Column::AdmissionEpoch,
                sea_orm::sea_query::ExprTrait::add(
                    Expr::col(checkout_operation::Column::AdmissionEpoch),
                    admission.epoch_delta(),
                ),
            )
            .col_expr(
                checkout_operation::Column::LeaseOwner,
                Expr::value(Some(lease_owner)),
            )
            .col_expr(
                checkout_operation::Column::LeaseExpiresAt,
                Expr::value(Some(lease_expires_at)),
            )
            .col_expr(
                checkout_operation::Column::AttemptCount,
                sea_orm::sea_query::ExprTrait::add(
                    Expr::col(checkout_operation::Column::AttemptCount),
                    1,
                ),
            )
            .col_expr(
                checkout_operation::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .filter(checkout_operation::Column::Id.eq(id))
            .filter(claimable)
            .exec(&self.db)
            .await?;
        if update.rows_affected == 0 {
            return Ok(None);
        }
        self.get(tenant_id, id).await.map(Some)
    }

    /// Releases the compensation lease after a failed compensation step.
    ///
    /// A `ManualReconciliation` failure parks the operation in
    /// `reconciliation_required` (with `completed_at` set) instead of returning
    /// it to `compensation_required`, so the sweep stops retrying a step that
    /// automation cannot finish.
    pub async fn mark_compensation_retryable(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        lease_owner: impl Into<String>,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        let error_code = error_code.into();
        let next_status = compensation_next_status(error_code.as_str());
        self.release_lease_with_error(
            tenant_id,
            id,
            LeaseErrorTransition {
                lease_owner: lease_owner.into(),
                expected_status: CheckoutOperationStatus::Compensating,
                next_status,
                error_code,
                error_message: error_message.into(),
            },
        )
        .await
    }

    /// Parks a `compensation_required` operation whose compensation attempts are
    /// exhausted.
    ///
    /// The sweep calls this instead of claiming the operation again once
    /// `attempt_count` reached [`MAX_CHECKOUT_COMPENSATION_ATTEMPTS`]: a cart must
    /// not stay in an endless retry loop when a provider or an order keeps
    /// rejecting the same compensation step. The park is a legal
    /// `compensation_required -> reconciliation_required` transition, sets the
    /// completion timestamp, and hands the case to the operator action registry.
    pub async fn park_exhausted_compensation(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        reason: impl Into<String>,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        if tenant_id.is_nil() || id.is_nil() {
            return Err(CheckoutOperationError::Validation(
                "parking an exhausted compensation requires non-nil tenant and operation identifiers"
                    .to_string(),
            ));
        }
        // Both compensation states are parkable, exactly like the sweep's
        // claim predicate: an unclaimed `compensation_required` row and a
        // `compensating` row whose worker died and whose lease expired.
        let now = Utc::now().fixed_offset();
        let from_required = CheckedTransition::new(
            CheckoutOperationStatus::CompensationRequired,
            CheckoutOperationStatus::ReconciliationRequired,
        )?;
        let from_compensating = CheckedTransition::new(
            CheckoutOperationStatus::Compensating,
            CheckoutOperationStatus::ReconciliationRequired,
        )?;
        let parkable = Condition::any()
            .add(checkout_operation::Column::Status.eq(from_required.expected.as_str()))
            .add(
                Condition::all()
                    .add(checkout_operation::Column::Status.eq(from_compensating.expected.as_str()))
                    .add(checkout_operation::Column::LeaseExpiresAt.lte(now)),
            );
        // Both compensation states already carry the `settling` admission level,
        // so the park re-asserts it without moving the generation.
        let admission = AdmissionWrite::between(
            CheckoutOperationStatus::CompensationRequired,
            CheckoutOperationStatus::ReconciliationRequired,
        );
        let reason = normalize_bounded("compensation_exhaustion_reason", reason.into(), 1500)?;
        let error_code =
            normalize_error_code(CHECKOUT_COMPENSATION_ATTEMPTS_EXHAUSTED_CODE.to_string())?;
        let error_message = normalize_error_message(format!(
            "checkout compensation attempts exhausted after {MAX_CHECKOUT_COMPENSATION_ATTEMPTS} attempts: {reason}"
        ))?;
        let txn = self.db.begin().await?;
        let result = checkout_operation::Entity::update_many()
            .col_expr(
                checkout_operation::Column::Status,
                Expr::value(from_required.next.as_str()),
            )
            .col_expr(
                checkout_operation::Column::ExecutionAdmission,
                Expr::value(admission.next.as_str()),
            )
            .col_expr(
                checkout_operation::Column::AdmissionEpoch,
                sea_orm::sea_query::ExprTrait::add(
                    Expr::col(checkout_operation::Column::AdmissionEpoch),
                    admission.epoch_delta(),
                ),
            )
            .col_expr(
                checkout_operation::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                checkout_operation::Column::LeaseExpiresAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(
                checkout_operation::Column::LastErrorCode,
                Expr::value(Some(error_code)),
            )
            .col_expr(
                checkout_operation::Column::LastErrorMessage,
                Expr::value(Some(error_message)),
            )
            .col_expr(
                checkout_operation::Column::CompletedAt,
                Expr::value(Some(now)),
            )
            .col_expr(
                checkout_operation::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .filter(checkout_operation::Column::Id.eq(id))
            .filter(parkable)
            .exec(&txn)
            .await?;
        if result.rows_affected == 0 {
            txn.rollback().await?;
            return Err(self
                .cas_conflict(tenant_id, id, "park exhausted compensation")
                .await?);
        }
        let parked_operation = self.get_in(&txn, tenant_id, id).await?;
        self.publish_parked(&txn, &parked_operation, PARK_REASON_ATTEMPTS_EXHAUSTED)
            .await?;
        txn.commit().await?;
        rustok_telemetry::metrics::record_checkout_reconciliation_parked(
            PARK_REASON_ATTEMPTS_EXHAUSTED,
        );
        self.get(tenant_id, id).await
    }

    pub async fn mark_completed(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        lease_owner: impl Into<String>,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        self.mark_terminal(
            tenant_id,
            id,
            TerminalTransition {
                lease_owner: lease_owner.into(),
                expected_status: CheckoutOperationStatus::Executing,
                next_status: CheckoutOperationStatus::Completed,
                next_stage: Some(CheckoutOperationStage::Completed),
                error_code: None,
                error_message: None,
            },
        )
        .await
    }

    pub async fn mark_compensated(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        lease_owner: impl Into<String>,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        self.mark_terminal(
            tenant_id,
            id,
            TerminalTransition {
                lease_owner: lease_owner.into(),
                expected_status: CheckoutOperationStatus::Compensating,
                next_status: CheckoutOperationStatus::Compensated,
                next_stage: None,
                error_code: None,
                error_message: None,
            },
        )
        .await
    }

    pub async fn mark_failed(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        lease_owner: impl Into<String>,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        let current = self.get(tenant_id, id).await?;
        let expected_status = match current.status.as_str() {
            "executing" => CheckoutOperationStatus::Executing,
            "compensating" => CheckoutOperationStatus::Compensating,
            other => {
                return Err(CheckoutOperationError::Conflict(format!(
                    "checkout operation {id} cannot fail from status `{other}`"
                )));
            }
        };
        self.mark_terminal(
            tenant_id,
            id,
            TerminalTransition {
                lease_owner: lease_owner.into(),
                expected_status,
                next_status: CheckoutOperationStatus::Failed,
                next_stage: None,
                error_code: Some(error_code.into()),
                error_message: Some(error_message.into()),
            },
        )
        .await
    }

    /// Closes an operation parked in `reconciliation_required` with an explicit
    /// operator outcome and justification.
    ///
    /// This is the terminal writer behind the `attest_external` and `write_off`
    /// reconciliation actions
    /// ([`crate::CheckoutReconciliationService`]). It moves **no money**: the
    /// operator attests that the payment was reconciled out of band (refund
    /// issued or the charge written off), and the call records who closed the
    /// operation, how, and why, and frees the cart's active-checkout slot. The
    /// operation row keeps `completed_at` from the parking transition; the
    /// resolution moment is `updated_at`.
    /// Resolves a parked operation into a terminal settlement and records the
    /// operator decision that closed it.
    ///
    /// The status write, the events, the admission change, the stamping of the
    /// provider operations the previous generation admitted and the
    /// `checkout_reconciliation_actions` row are one transaction: either the
    /// operation is closed with the decision that explains it, or nothing
    /// happened and the operator retries.
    pub async fn resolve_reconciliation_required(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        outcome: CheckoutReconciliationOutcome,
        reason: impl Into<String>,
        decision: CheckoutReconciliationDecision,
    ) -> CheckoutOperationResult<ReconciliationResolution> {
        let operator_id = decision.operator_id;
        if tenant_id.is_nil() || id.is_nil() || operator_id.is_nil() {
            return Err(CheckoutOperationError::Validation(
                "checkout reconciliation resolution requires non-nil tenant, operation and operator identifiers"
                    .to_string(),
            ));
        }
        let next_status = match outcome {
            CheckoutReconciliationOutcome::Compensated => CheckoutOperationStatus::Compensated,
            CheckoutReconciliationOutcome::Failed => CheckoutOperationStatus::Failed,
        };
        let checked =
            CheckedTransition::new(CheckoutOperationStatus::ReconciliationRequired, next_status)?;
        let admission = AdmissionWrite::between(checked.expected, checked.next);
        let reason = normalize_bounded("reconciliation_reason", reason.into(), 1500)?;
        let error_code = normalize_error_code(CHECKOUT_RECONCILIATION_RESOLVED_CODE.to_string())?;
        let error_message = normalize_error_message(format!(
            "checkout reconciliation resolved as `{}` by operator {operator_id}: {reason}",
            outcome.as_str()
        ))?;

        let txn = self.db.begin().await?;
        let result = checkout_operation::Entity::update_many()
            .col_expr(
                checkout_operation::Column::Status,
                Expr::value(checked.next.as_str()),
            )
            .col_expr(
                checkout_operation::Column::ExecutionAdmission,
                Expr::value(admission.next.as_str()),
            )
            .col_expr(
                checkout_operation::Column::AdmissionEpoch,
                sea_orm::sea_query::ExprTrait::add(
                    Expr::col(checkout_operation::Column::AdmissionEpoch),
                    admission.epoch_delta(),
                ),
            )
            .col_expr(
                checkout_operation::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                checkout_operation::Column::LeaseExpiresAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(
                checkout_operation::Column::LastErrorCode,
                Expr::value(Some(error_code)),
            )
            .col_expr(
                checkout_operation::Column::LastErrorMessage,
                Expr::value(Some(error_message)),
            )
            .col_expr(
                checkout_operation::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .filter(checkout_operation::Column::Id.eq(id))
            .filter(checkout_operation::Column::Status.eq(checked.expected.as_str()))
            .exec(&txn)
            .await?;
        if result.rows_affected == 0 {
            txn.rollback().await?;
            return Err(self.cas_conflict(tenant_id, id, "resolve").await?);
        }
        let reconciled = self.get_in(&txn, tenant_id, id).await?;
        let recorded = append_reconciliation_decision(&txn, &reconciled, decision).await?;
        self.publish_reconciled(&txn, &reconciled, outcome.as_str(), operator_id)
            .await?;
        if admission.changes_the_level() {
            self.publish_admission_changed(&txn, &reconciled, Some(admission.previous))
                .await?;
        }
        txn.commit().await?;
        Ok(ReconciliationResolution {
            operation: reconciled,
            decision: recorded,
        })
    }

    /// Sends a parked operation back to `compensation_required` so the
    /// compensation sweep may try again.
    ///
    /// This is the non-money escape hatch behind the `retry_compensation`
    /// reconciliation action: an operator who removed the blocking condition
    /// (provider outage over, supporting document attached, wrong operation
    /// parked) can re-arm automation without touching the money. The operation
    /// loses `completed_at` again because it is no longer in a settled state.
    pub async fn request_compensation_retry(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        reason: impl Into<String>,
        decision: CheckoutReconciliationDecision,
    ) -> CheckoutOperationResult<ReconciliationResolution> {
        let operator_id = decision.operator_id;
        if tenant_id.is_nil() || id.is_nil() || operator_id.is_nil() {
            return Err(CheckoutOperationError::Validation(
                "checkout reconciliation retry requires non-nil tenant, operation and operator identifiers"
                    .to_string(),
            ));
        }
        let checked = CheckedTransition::new(
            CheckoutOperationStatus::ReconciliationRequired,
            CheckoutOperationStatus::CompensationRequired,
        )?;
        let admission = AdmissionWrite::between(checked.expected, checked.next);
        let reason = normalize_bounded("reconciliation_retry_reason", reason.into(), 1500)?;
        let error_code =
            normalize_error_code(CHECKOUT_RECONCILIATION_RETRY_REQUESTED_CODE.to_string())?;
        let error_message = normalize_error_message(format!(
            "checkout reconciliation retry requested by operator {operator_id}: {reason}"
        ))?;

        let txn = self.db.begin().await?;
        let result = checkout_operation::Entity::update_many()
            .col_expr(
                checkout_operation::Column::Status,
                Expr::value(checked.next.as_str()),
            )
            .col_expr(
                checkout_operation::Column::ExecutionAdmission,
                Expr::value(admission.next.as_str()),
            )
            .col_expr(
                checkout_operation::Column::AdmissionEpoch,
                sea_orm::sea_query::ExprTrait::add(
                    Expr::col(checkout_operation::Column::AdmissionEpoch),
                    admission.epoch_delta(),
                ),
            )
            .col_expr(
                checkout_operation::Column::CompletedAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(
                checkout_operation::Column::LastErrorCode,
                Expr::value(Some(error_code)),
            )
            .col_expr(
                checkout_operation::Column::LastErrorMessage,
                Expr::value(Some(error_message)),
            )
            .col_expr(
                checkout_operation::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .filter(checkout_operation::Column::Id.eq(id))
            .filter(checkout_operation::Column::Status.eq(checked.expected.as_str()))
            .exec(&txn)
            .await?;
        if result.rows_affected == 0 {
            txn.rollback().await?;
            return Err(self
                .cas_conflict(tenant_id, id, "request_compensation_retry")
                .await?);
        }
        let rearmed = self.get_in(&txn, tenant_id, id).await?;
        let recorded = append_reconciliation_decision(&txn, &rearmed, decision).await?;
        self.publish_reconciled(
            &txn,
            &rearmed,
            RECONCILED_OUTCOME_COMPENSATION_REQUIRED,
            operator_id,
        )
        .await?;
        txn.commit().await?;
        Ok(ReconciliationResolution {
            operation: rearmed,
            decision: recorded,
        })
    }

    async fn release_lease_with_error(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        transition: LeaseErrorTransition,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        let checked = CheckedTransition::new(transition.expected_status, transition.next_status)?;
        let admission = AdmissionWrite::between(checked.expected, checked.next);
        let lease_owner = normalize_lease_owner(transition.lease_owner)?;
        let error_code = normalize_error_code(transition.error_code)?;
        let error_message = normalize_error_message(transition.error_message)?;
        let now = Utc::now().fixed_offset();
        let completed_at = if checked.next.requires_completed_at() {
            Some(now)
        } else {
            None
        };
        let txn = self.db.begin().await?;
        let update = checkout_operation::Entity::update_many()
            .col_expr(
                checkout_operation::Column::Status,
                Expr::value(checked.next.as_str()),
            )
            .col_expr(
                checkout_operation::Column::ExecutionAdmission,
                Expr::value(admission.next.as_str()),
            )
            .col_expr(
                checkout_operation::Column::AdmissionEpoch,
                sea_orm::sea_query::ExprTrait::add(
                    Expr::col(checkout_operation::Column::AdmissionEpoch),
                    admission.epoch_delta(),
                ),
            )
            .col_expr(
                checkout_operation::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                checkout_operation::Column::LeaseExpiresAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(
                checkout_operation::Column::LastErrorCode,
                Expr::value(Some(error_code)),
            )
            .col_expr(
                checkout_operation::Column::LastErrorMessage,
                Expr::value(Some(error_message)),
            )
            .col_expr(
                checkout_operation::Column::CompletedAt,
                Expr::value(completed_at),
            )
            .col_expr(
                checkout_operation::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .filter(checkout_operation::Column::Id.eq(id))
            .filter(checkout_operation::Column::Status.eq(checked.expected.as_str()))
            .filter(checkout_operation::Column::LeaseOwner.eq(lease_owner))
            .filter(checkout_operation::Column::LeaseExpiresAt.gt(now))
            .exec(&txn)
            .await?;
        if update.rows_affected == 0 {
            txn.rollback().await?;
            return Err(self
                .cas_conflict(tenant_id, id, checked.next.as_str())
                .await?);
        }
        // The parking decision, the admission change and their events are one
        // unit of work; the metric is recorded only after the transaction that
        // carries them is durable.
        let parked = checked.next == CheckoutOperationStatus::ReconciliationRequired;
        if parked || admission.changes_the_level() {
            let operation = self.get_in(&txn, tenant_id, id).await?;
            if parked {
                self.publish_parked(&txn, &operation, PARK_REASON_MANUAL_RECONCILIATION)
                    .await?;
            }
            if admission.changes_the_level() {
                self.publish_admission_changed(&txn, &operation, Some(admission.previous))
                    .await?;
            }
        }
        txn.commit().await?;
        if parked {
            rustok_telemetry::metrics::record_checkout_reconciliation_parked(
                PARK_REASON_MANUAL_RECONCILIATION,
            );
        }
        self.get(tenant_id, id).await
    }

    async fn mark_terminal(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        transition: TerminalTransition,
    ) -> CheckoutOperationResult<checkout_operation::Model> {
        let checked = CheckedTransition::new(transition.expected_status, transition.next_status)?;
        let admission = AdmissionWrite::between(checked.expected, checked.next);
        let lease_owner = normalize_lease_owner(transition.lease_owner)?;
        let error_code = transition
            .error_code
            .map(normalize_error_code)
            .transpose()?;
        let error_message = transition
            .error_message
            .map(normalize_error_message)
            .transpose()?;
        let now = Utc::now().fixed_offset();
        let mut update = checkout_operation::Entity::update_many()
            .col_expr(
                checkout_operation::Column::Status,
                Expr::value(checked.next.as_str()),
            )
            .col_expr(
                checkout_operation::Column::ExecutionAdmission,
                Expr::value(admission.next.as_str()),
            )
            .col_expr(
                checkout_operation::Column::AdmissionEpoch,
                sea_orm::sea_query::ExprTrait::add(
                    Expr::col(checkout_operation::Column::AdmissionEpoch),
                    admission.epoch_delta(),
                ),
            )
            .col_expr(
                checkout_operation::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                checkout_operation::Column::LeaseExpiresAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(
                checkout_operation::Column::LastErrorCode,
                Expr::value(error_code),
            )
            .col_expr(
                checkout_operation::Column::LastErrorMessage,
                Expr::value(error_message),
            )
            .col_expr(
                checkout_operation::Column::CompletedAt,
                Expr::value(Some(now)),
            )
            .col_expr(
                checkout_operation::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .filter(checkout_operation::Column::Id.eq(id))
            .filter(checkout_operation::Column::Status.eq(checked.expected.as_str()))
            .filter(checkout_operation::Column::LeaseOwner.eq(lease_owner))
            .filter(checkout_operation::Column::LeaseExpiresAt.gt(now));
        if let Some(next_stage) = transition.next_stage {
            update = update.col_expr(
                checkout_operation::Column::Stage,
                Expr::value(next_stage.as_str()),
            );
        }
        // The terminal status, the closed admission level and their events are
        // one unit of work: a consumer can never see a completed checkout whose
        // admission still admits extending provider execution.
        let txn = self.db.begin().await?;
        let result = update.exec(&txn).await?;
        if result.rows_affected == 0 {
            txn.rollback().await?;
            return Err(self
                .cas_conflict(tenant_id, id, checked.next.as_str())
                .await?);
        }
        let operation = self.get_in(&txn, tenant_id, id).await?;
        if admission.changes_the_level() {
            self.publish_admission_changed(&txn, &operation, Some(admission.previous))
                .await?;
        }
        txn.commit().await?;
        Ok(operation)
    }

    async fn cas_conflict(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        action: &str,
    ) -> CheckoutOperationResult<CheckoutOperationError> {
        let current = self.get(tenant_id, id).await?;
        Ok(CheckoutOperationError::Conflict(format!(
            "cannot {action} checkout operation {id}; current status={}, stage={}, lease_owner={}",
            current.status,
            current.stage,
            current.lease_owner.as_deref().unwrap_or("none")
        )))
    }
}

/// Appends the operator decision that closed a parked operation.
///
/// Runs inside the writer's transaction, after the compare-and-set that moved the
/// operation out of `reconciliation_required` and before the commit, so the row
/// records the status the same transaction wrote (`operation.status`). The row
/// carries the tenant and cart of the operation it closes instead of a copy of
/// the request, which is what makes the decision attributable to exactly one
/// operation of exactly one tenant.
async fn append_reconciliation_decision<C>(
    txn: &C,
    operation: &checkout_operation::Model,
    decision: CheckoutReconciliationDecision,
) -> CheckoutOperationResult<checkout_reconciliation_action::Model>
where
    C: ConnectionTrait,
{
    checkout_reconciliation_action::ActiveModel {
        id: Set(generate_id()),
        tenant_id: Set(operation.tenant_id),
        checkout_operation_id: Set(operation.id),
        cart_id: Set(operation.cart_id),
        action: Set(decision.action),
        result_status: Set(operation.status.clone()),
        amount: Set(decision.amount),
        currency_code: Set(decision.currency_code),
        reason: Set(decision.reason),
        evidence_ref: Set(decision.evidence_ref),
        operator_id: Set(decision.operator_id),
        approver_id: Set(decision.approver_id),
        idempotency_key: Set(decision.idempotency_key),
        request_hash: Set(decision.request_hash),
        refund_id: Set(decision.refund_id),
        refund_status: Set(decision.refund_status),
        created_at: Set(Utc::now().fixed_offset()),
    }
    .insert(txn)
    .await
    .map_err(Into::into)
}

/// Cross-aggregate tenant checks for the rows a checkout operation binds.
///
/// `m20260713_000009`/`m20260713_000017` enforced these inside the
/// `enforce_checkout_operation_integrity()` PL/pgSQL function, which is exactly
/// the "cross-row business validation inside a constraint trigger" the
/// repository contract forbids. The journal owns them now: they run in the same
/// transaction as the write, use the owning crate's entity, and fail closed.
async fn ensure_cart_tenant(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    cart_id: Uuid,
) -> CheckoutOperationResult<()> {
    let found = rustok_cart::entities::cart::Entity::find_by_id(cart_id)
        .filter(rustok_cart::entities::cart::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?;
    if found.is_none() {
        return Err(CheckoutOperationError::Validation(format!(
            "checkout operation cannot bind cart {cart_id}: it does not belong to tenant {tenant_id}"
        )));
    }
    Ok(())
}

async fn ensure_order_tenant(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    order_id: Uuid,
) -> CheckoutOperationResult<()> {
    let found = rustok_order::entities::order::Entity::find_by_id(order_id)
        .filter(rustok_order::entities::order::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?;
    if found.is_none() {
        return Err(CheckoutOperationError::Validation(format!(
            "checkout operation cannot bind order {order_id}: it does not belong to tenant {tenant_id}"
        )));
    }
    Ok(())
}

async fn ensure_payment_collection_tenant(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    payment_collection_id: Uuid,
) -> CheckoutOperationResult<()> {
    let found =
        rustok_payment::entities::payment_collection::Entity::find_by_id(payment_collection_id)
            .filter(rustok_payment::entities::payment_collection::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?;
    if found.is_none() {
        return Err(CheckoutOperationError::Validation(format!(
            "checkout operation cannot bind payment collection {payment_collection_id}: it does not belong to tenant {tenant_id}"
        )));
    }
    Ok(())
}

/// Statuses that keep the cart's partial unique index
/// (`ux_checkout_operations_active_cart`) occupied. Must stay in sync with the
/// `WHERE status IN (...)` predicate of that index in
/// `m20260713_000009`/`m20260713_000017`.
fn active_statuses() -> [&'static str; 6] {
    [
        CheckoutOperationStatus::Pending.as_str(),
        CheckoutOperationStatus::Executing.as_str(),
        CheckoutOperationStatus::RetryableError.as_str(),
        CheckoutOperationStatus::CompensationRequired.as_str(),
        CheckoutOperationStatus::Compensating.as_str(),
        CheckoutOperationStatus::ReconciliationRequired.as_str(),
    ]
}

/// Every status accepted by the `checkout_operations.status` column-level check
/// constraint. Keep in sync with `CheckoutOperationStatus` and with the
/// constraint in the checkout migrations.
fn known_statuses() -> [&'static str; 9] {
    [
        CheckoutOperationStatus::Pending.as_str(),
        CheckoutOperationStatus::Executing.as_str(),
        CheckoutOperationStatus::RetryableError.as_str(),
        CheckoutOperationStatus::CompensationRequired.as_str(),
        CheckoutOperationStatus::Compensating.as_str(),
        CheckoutOperationStatus::ReconciliationRequired.as_str(),
        CheckoutOperationStatus::Completed.as_str(),
        CheckoutOperationStatus::Compensated.as_str(),
        CheckoutOperationStatus::Failed.as_str(),
    ]
}

fn is_known_status(value: &str) -> bool {
    known_statuses().contains(&value)
}

fn active_cart_conflict(cart_id: Uuid, operation_id: Uuid) -> CheckoutOperationError {
    CheckoutOperationError::Conflict(format!(
        "cart {cart_id} already has active checkout operation {operation_id}"
    ))
}

fn normalize_begin_input(
    mut input: BeginCheckoutOperation,
) -> CheckoutOperationResult<BeginCheckoutOperation> {
    input.idempotency_key = normalize_bounded("idempotency_key", input.idempotency_key, 191)?;
    input.request_hash = normalize_hash(input.request_hash)?;
    input.snapshot_hash = input.snapshot_hash.map(normalize_hash).transpose()?;
    Ok(input)
}

fn ensure_same_request(
    existing: &checkout_operation::Model,
    input: &BeginCheckoutOperation,
) -> CheckoutOperationResult<()> {
    let snapshot_hash_mismatch = match (&existing.snapshot_hash, &input.snapshot_hash) {
        (Some(existing_hash), Some(input_hash)) => existing_hash != input_hash,
        (None, Some(_)) => true,
        (_, None) => false,
    };
    if existing.request_hash != input.request_hash || snapshot_hash_mismatch {
        return Err(CheckoutOperationError::Conflict(format!(
            "idempotency key `{}` is already bound to a different checkout request",
            input.idempotency_key
        )));
    }
    Ok(())
}

fn normalize_hash(value: String) -> CheckoutOperationResult<String> {
    let value = normalize_bounded("hash", value, 128)?;
    if !value.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(CheckoutOperationError::Validation(
            "checkout hashes must contain only hexadecimal characters".to_string(),
        ));
    }
    Ok(value.to_ascii_lowercase())
}

fn normalize_lease_owner(value: String) -> CheckoutOperationResult<String> {
    normalize_bounded("lease_owner", value, 191)
}

fn normalize_lease_seconds(value: i64) -> CheckoutOperationResult<i64> {
    if (1..=MAX_CHECKOUT_LEASE_SECONDS).contains(&value) {
        Ok(value)
    } else {
        Err(CheckoutOperationError::Validation(format!(
            "lease_seconds must be between 1 and {MAX_CHECKOUT_LEASE_SECONDS}"
        )))
    }
}

fn normalize_error_code(value: String) -> CheckoutOperationResult<String> {
    normalize_bounded("error_code", value, 100)
}

fn normalize_error_message(value: String) -> CheckoutOperationResult<String> {
    normalize_bounded("error_message", value, 2000)
}

fn normalize_bounded(
    field: &str,
    value: String,
    maximum_length: usize,
) -> CheckoutOperationResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() || value.chars().count() > maximum_length {
        return Err(CheckoutOperationError::Validation(format!(
            "{field} must contain 1 to {maximum_length} characters"
        )));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkout_hashes_are_normalized_and_fail_closed() {
        assert_eq!(
            normalize_hash("A0ff".to_string()).expect("hex hash"),
            "a0ff"
        );
        assert!(normalize_hash("not-a-hash".to_string()).is_err());
        assert!(normalize_hash(String::new()).is_err());
    }

    #[test]
    fn checkout_lease_duration_is_bounded() {
        assert!(normalize_lease_seconds(1).is_ok());
        assert!(normalize_lease_seconds(MAX_CHECKOUT_LEASE_SECONDS).is_ok());
        assert!(normalize_lease_seconds(0).is_err());
        assert!(normalize_lease_seconds(MAX_CHECKOUT_LEASE_SECONDS + 1).is_err());
    }

    #[test]
    fn checkout_state_machine_rejects_unlisted_transitions() {
        use CheckoutOperationStatus::*;

        assert!(Pending.can_transition_to(Executing));
        assert!(Compensating.can_transition_to(Compensated));
        assert!(ReconciliationRequired.can_transition_to(Compensated));
        assert!(ReconciliationRequired.can_transition_to(CompensationRequired));
        assert!(!ReconciliationRequired.can_transition_to(Executing));
        assert!(!Compensated.can_transition_to(CompensationRequired));
        assert!(!Executing.can_transition_to(Pending));

        assert!(CheckedTransition::new(Pending, Executing).is_ok());
        assert!(CheckedTransition::new(Executing, Compensated).is_err());
        assert!(CheckedTransition::new(Completed, Failed).is_err());
    }

    #[test]
    fn admission_level_follows_the_status_and_never_moves_backwards() {
        use CheckoutOperationStatus::*;

        let levels = [
            (Pending, CheckoutExecutionAdmission::Open),
            (Executing, CheckoutExecutionAdmission::Open),
            (RetryableError, CheckoutExecutionAdmission::Open),
            (CompensationRequired, CheckoutExecutionAdmission::Settling),
            (Compensating, CheckoutExecutionAdmission::Settling),
            (ReconciliationRequired, CheckoutExecutionAdmission::Settling),
            (Completed, CheckoutExecutionAdmission::Closed),
            (Compensated, CheckoutExecutionAdmission::Closed),
            (Failed, CheckoutExecutionAdmission::Closed),
        ];
        let rank = |level: CheckoutExecutionAdmission| match level {
            CheckoutExecutionAdmission::Open => 0,
            CheckoutExecutionAdmission::Settling => 1,
            CheckoutExecutionAdmission::Closed => 2,
        };

        for (status, level) in levels {
            assert_eq!(CheckoutExecutionAdmission::for_status(status), level);
            assert_eq!(
                CheckoutExecutionAdmission::parse(level.as_str()),
                Some(level)
            );
            for next in status.allowed_transitions() {
                let next_level = CheckoutExecutionAdmission::for_status(*next);
                assert!(
                    rank(next_level) >= rank(level),
                    "{status:?} -> {next:?} must not lower the admission level"
                );
                // The epoch moves exactly when the level moves, so a write that
                // stays inside one level keeps the generation in-flight provider
                // operations were admitted under.
                let write = AdmissionWrite::between(status, *next);
                assert_eq!(
                    write.changes_the_level(),
                    rank(next_level) > rank(level),
                    "{status:?} -> {next:?} admission epoch delta"
                );
                let expected_delta = i64::from(write.changes_the_level());
                assert_eq!(write.epoch_delta(), expected_delta);
                assert_eq!(write.next, next_level);
            }
        }

        assert_eq!(
            AdmissionWrite::unchanged(CheckoutExecutionAdmission::Open).epoch_delta(),
            0
        );
        assert_eq!(
            AdmissionWrite::between(Executing, Completed).previous,
            CheckoutExecutionAdmission::Open
        );
        assert_eq!(
            AdmissionWrite::between(Executing, Completed).next,
            CheckoutExecutionAdmission::Closed
        );
    }

    #[test]
    fn manual_reconciliation_codes_park_the_operation() {
        assert_eq!(
            compensation_next_status(CHECKOUT_COMPENSATION_MANUAL_RECONCILIATION_CODE),
            CheckoutOperationStatus::ReconciliationRequired
        );
        assert_eq!(
            compensation_next_status("checkout.compensation_boundary_failed"),
            CheckoutOperationStatus::CompensationRequired
        );
        assert!(CheckoutOperationStatus::ReconciliationRequired.requires_completed_at());
        assert!(CheckoutOperationStatus::ReconciliationRequired.is_parked());
        assert!(!CheckoutOperationStatus::Compensating.is_parked());
        assert!(!CheckoutOperationStatus::CompensationRequired.requires_completed_at());
    }
}
