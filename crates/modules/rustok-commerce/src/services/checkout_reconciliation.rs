use std::sync::Arc;

use rust_decimal::Decimal;
use rustok_api::{Permission, PortContext, PortError};
use rustok_payment::{
    CancelAdminPaymentCollectionRequest, CancelPaymentInput, CreateAdminRefundRequest,
    CreateRefundInput, PaymentAdminCollectionCommandPort, PaymentAdminReadPort,
    PaymentAdminRefundCommandPort, ReadPaymentCollectionProjectionRequest, RefundResponse,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use rustok_outbox::TransactionalEventBus;

use crate::entities::{checkout_operation, checkout_reconciliation_action};

use super::{
    CheckoutOperationError, CheckoutOperationJournal, CheckoutOperationStatus,
    CheckoutReconciliationDecision, CheckoutReconciliationOutcome,
};

/// Upper bound for the operator-supplied idempotency key. Bounded so the key can
/// be embedded into the payment owner's refund creation key without exceeding
/// its own 191 character limit.
pub const MAX_RECONCILIATION_IDEMPOTENCY_KEY_LENGTH: usize = 128;
pub const MAX_RECONCILIATION_REASON_LENGTH: usize = 1500;
pub const MAX_RECONCILIATION_EVIDENCE_LENGTH: usize = 500;
pub const MAX_RECONCILIATION_ACTION_LIST_LIMIT: u64 = 200;

/// Bounded `result` labels for `rustok_checkout_reconciliation_actions_total`.
const ACTION_RESULT_SUCCEEDED: &str = "succeeded";
const ACTION_RESULT_REPLAYED: &str = "replayed";

/// What an operator may do with a parked checkout operation.
///
/// The registry is data, not control flow: the permission requirements, the
/// money involvement and the preconditions of every action live next to the
/// action itself, so adding a case cannot silently skip a guard.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CheckoutReconciliationAction {
    /// Send the operation back to `compensation_required` so the compensation
    /// sweep can retry it. Moves no money.
    RetryCompensation,
    /// Operator reconciled the charge outside the platform (bank statement,
    /// provider dashboard, ticket in another system). Requires `evidence_ref`
    /// and an explicit terminal `outcome`.
    AttestExternal,
    /// Books the checkout as failed without returning funds. Always requires a
    /// second approver different from the operator.
    WriteOff,
    /// Refunds every refundable unit of the bound payment collection through the
    /// payment owner, then closes the operation as compensated.
    RefundFull,
    /// Refunds `amount` of the bound payment collection, then closes the
    /// operation as compensated.
    RefundPartial,
    /// Cancels an uncaptured authorization, then closes the operation as failed.
    VoidAuthorization,
}

impl CheckoutReconciliationAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RetryCompensation => "retry_compensation",
            Self::AttestExternal => "attest_external",
            Self::WriteOff => "write_off",
            Self::RefundFull => "refund_full",
            Self::RefundPartial => "refund_partial",
            Self::VoidAuthorization => "void_authorization",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [
            Self::RetryCompensation,
            Self::AttestExternal,
            Self::WriteOff,
            Self::RefundFull,
            Self::RefundPartial,
            Self::VoidAuthorization,
        ]
        .into_iter()
        .find(|action| action.as_str() == value)
    }

    /// Actions that call the payment owner and move money. They additionally
    /// require the payment write permission and are never automatic.
    pub const fn moves_money(self) -> bool {
        matches!(self, Self::RefundFull | Self::RefundPartial)
    }

    pub const fn requires_second_approver(self) -> bool {
        matches!(self, Self::WriteOff)
    }

    /// Permissions required to execute this action. Single source of truth for
    /// both the HTTP layer and any future worker that drives the registry.
    pub fn required_permissions(self) -> &'static [Permission] {
        match self {
            Self::RetryCompensation | Self::AttestExternal | Self::WriteOff => {
                &[Permission::ORDERS_MANAGE]
            }
            // Money never moves without the payment owner's write permission.
            Self::RefundFull | Self::RefundPartial | Self::VoidAuthorization => {
                &[Permission::ORDERS_MANAGE, Permission::PAYMENTS_UPDATE]
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct CheckoutReconciliationActionRequest {
    pub tenant_id: Uuid,
    pub operation_id: Uuid,
    pub action: CheckoutReconciliationAction,
    pub operator_id: Uuid,
    pub idempotency_key: String,
    pub reason: String,
    /// Terminal outcome; mandatory for `attest_external`, rejected by actions
    /// that derive their own outcome.
    pub outcome: Option<CheckoutReconciliationOutcome>,
    /// Reference to the out-of-band artefact; mandatory for `attest_external`.
    pub evidence_ref: Option<String>,
    /// Second approver; mandatory (and different from the operator) for
    /// `write_off`.
    pub second_approver_id: Option<Uuid>,
    /// Amount for `refund_partial`; rejected by every other action.
    pub amount: Option<Decimal>,
}

#[derive(Debug, Error)]
pub enum CheckoutReconciliationError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("checkout operation {0} not found")]
    NotFound(Uuid),
    #[error("checkout reconciliation conflict: {0}")]
    Conflict(String),
    #[error("checkout reconciliation action journal conflict: {0}")]
    JournalConflict(String),
    #[error("payment owner rejected `{operation}`: [{code}] {message} (retryable: {retryable})")]
    PaymentOwner {
        operation: &'static str,
        code: String,
        message: String,
        retryable: bool,
    },
    /// The payment owner already executed the money step, but the transaction
    /// that closes the operation and records the decision did not commit, so the
    /// operation is still parked and no decision row exists. The refund id (when
    /// the action created one) is reported so the operator can either replay the
    /// same request — the payment owner deduplicates the refund by creation key —
    /// or attest the outcome by hand.
    #[error(
        "the payment provider already executed the money step but the checkout operation could not be closed: {source}; replay the same request for a refund (the payment owner deduplicates it) or attest the outcome manually"
    )]
    CloseAfterMoneyMoved {
        /// Refund created by the failed action, when the action created one.
        refund_id: Option<Uuid>,
        #[source]
        source: Box<CheckoutReconciliationError>,
    },
    #[error(transparent)]
    Operation(#[from] CheckoutOperationError),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub type CheckoutReconciliationResult<T> = Result<T, CheckoutReconciliationError>;

/// Operator-facing registry of reconciliation actions for parked checkout
/// operations.
///
/// Every action is a small, ordered sequence: check the parked state, do the
/// money step through the payment owner's ports, and then close the operation
/// through [`CheckoutOperationJournal`] with the decision that explains it. The
/// close and the append to `checkout_reconciliation_actions` are one database
/// transaction owned by the journal, so an operation can never leave
/// `reconciliation_required` without its decision row, and a decision row can
/// never describe an operation that is still parked. Money steps are idempotent
/// because the action's idempotency key is handed to the payment owner as the
/// refund creation key, so a replay returns the existing refund instead of
/// creating a second one.
#[derive(Clone)]
pub struct CheckoutReconciliationService {
    db: DatabaseConnection,
    operation_journal: CheckoutOperationJournal,
    payment_read_port: Arc<dyn PaymentAdminReadPort>,
    refund_command_port: Arc<dyn PaymentAdminRefundCommandPort>,
    collection_command_port: Arc<dyn PaymentAdminCollectionCommandPort>,
}

impl CheckoutReconciliationService {
    pub fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        payment_read_port: Arc<dyn PaymentAdminReadPort>,
        refund_command_port: Arc<dyn PaymentAdminRefundCommandPort>,
        collection_command_port: Arc<dyn PaymentAdminCollectionCommandPort>,
    ) -> Self {
        Self {
            operation_journal: CheckoutOperationJournal::new(db.clone(), event_bus),
            db,
            payment_read_port,
            refund_command_port,
            collection_command_port,
        }
    }

    /// Executes one operator action on a parked operation.
    ///
    /// The public entry point owns the metrics: every call records exactly one
    /// `rustok_checkout_reconciliation_actions_total` sample with a bounded
    /// `result` label, whether the request was rejected, replayed or executed.
    pub async fn execute(
        &self,
        context: PortContext,
        request: CheckoutReconciliationActionRequest,
    ) -> CheckoutReconciliationResult<checkout_reconciliation_action::Model> {
        let action = request.action;
        let outcome = self.execute_action(context, request).await;
        match &outcome {
            Ok(outcome) => rustok_telemetry::metrics::record_checkout_reconciliation_action(
                action.as_str(),
                if outcome.replayed {
                    ACTION_RESULT_REPLAYED
                } else {
                    ACTION_RESULT_SUCCEEDED
                },
            ),
            Err(error) => rustok_telemetry::metrics::record_checkout_reconciliation_action(
                action.as_str(),
                action_result_label(error),
            ),
        }
        outcome.map(|outcome| outcome.record)
    }

    async fn execute_action(
        &self,
        context: PortContext,
        mut request: CheckoutReconciliationActionRequest,
    ) -> CheckoutReconciliationResult<ActionOutcome> {
        validate_request(&mut request)?;
        let request_hash = fingerprint(&request);

        if let Some(existing) = self
            .find_action(
                request.tenant_id,
                request.operation_id,
                request.idempotency_key.as_str(),
            )
            .await?
        {
            return Ok(ActionOutcome {
                record: replay(existing, request_hash.as_str())?,
                replayed: true,
            });
        }

        let operation = self
            .operation_journal
            .get(request.tenant_id, request.operation_id)
            .await?;
        if operation.status != CheckoutOperationStatus::ReconciliationRequired.as_str() {
            return Err(CheckoutReconciliationError::Conflict(format!(
                "checkout operation {} is `{}`; only `{}` operations accept reconciliation actions",
                operation.id,
                operation.status,
                CheckoutOperationStatus::ReconciliationRequired.as_str()
            )));
        }

        match self
            .perform(context, &request, &operation, request_hash.as_str())
            .await
        {
            Ok(record) => Ok(ActionOutcome {
                record,
                replayed: false,
            }),
            Err(error) => {
                // A concurrent request holding the same idempotency key can win
                // the race after this one already moved money: the payment owner
                // deduplicates refunds by creation key, so both requests share
                // one refund, while this request cannot close an operation the
                // winner already closed. When the winner's row is visible this is
                // a replay, not a failure; otherwise the error is real and
                // reaches the operator with the refund id, which is what makes
                // the leftover money attestable.
                if matches!(
                    &error,
                    CheckoutReconciliationError::CloseAfterMoneyMoved { .. }
                ) {
                    if let Some(existing) = self
                        .find_action(
                            request.tenant_id,
                            request.operation_id,
                            request.idempotency_key.as_str(),
                        )
                        .await?
                    {
                        return Ok(ActionOutcome {
                            record: replay(existing, request_hash.as_str())?,
                            replayed: true,
                        });
                    }
                }
                Err(error)
            }
        }
    }

    /// Runs one action and closes the parked operation with its decision.
    async fn perform(
        &self,
        context: PortContext,
        request: &CheckoutReconciliationActionRequest,
        operation: &checkout_operation::Model,
        request_hash: &str,
    ) -> CheckoutReconciliationResult<checkout_reconciliation_action::Model> {
        let effect = self.action_effect(&context, request, operation).await?;
        let decision = CheckoutReconciliationDecision {
            action: request.action.as_str().to_string(),
            operator_id: request.operator_id,
            idempotency_key: request.idempotency_key.clone(),
            request_hash: request_hash.to_string(),
            reason: request.reason.clone(),
            evidence_ref: request.evidence_ref.clone(),
            approver_id: request.second_approver_id,
            amount: effect.amount,
            currency_code: effect.currency_code.clone(),
            refund_id: effect.refund_id,
            refund_status: effect.refund_status.clone(),
        };
        self.close(request, effect, decision).await
    }

    /// Closes the parked operation and records the operator decision.
    ///
    /// Both writes belong to one transaction inside [`CheckoutOperationJournal`],
    /// so a failure leaves the operation exactly as it was: parked, with no
    /// decision row. The operator then either replays the request (the money step
    /// is deduplicated by the payment owner) or attests the outcome by hand.
    async fn close(
        &self,
        request: &CheckoutReconciliationActionRequest,
        effect: ActionEffect,
        decision: CheckoutReconciliationDecision,
    ) -> CheckoutReconciliationResult<checkout_reconciliation_action::Model> {
        let close = match effect.settlement {
            Settlement::Resolve { outcome, reason } => {
                self.operation_journal
                    .resolve_reconciliation_required(
                        request.tenant_id,
                        request.operation_id,
                        outcome,
                        reason,
                        decision,
                    )
                    .await
            }
            Settlement::Retry => {
                self.operation_journal
                    .request_compensation_retry(
                        request.tenant_id,
                        request.operation_id,
                        request.reason.clone(),
                        decision,
                    )
                    .await
            }
        };
        match close {
            Ok(resolution) => Ok(resolution.decision),
            // The action already called the payment owner, so a bare conflict
            // would hide the provider-side effect that exists; the caller gets the
            // refund id and the alertable `close_after_money_moved` reason.
            Err(source) if effect.money_moved => {
                Err(CheckoutReconciliationError::CloseAfterMoneyMoved {
                    refund_id: effect.refund_id,
                    source: Box::new(CheckoutReconciliationError::Operation(source)),
                })
            }
            Err(source) => Err(source.into()),
        }
    }

    /// Runs the registered action and returns what it did to the money.
    ///
    /// The registry is the only dispatcher: every branch is an action the
    /// operator can name, the request-level preconditions were checked by
    /// [`validate_request`], and the per-action guards live next to the action
    /// implementation.
    async fn action_effect(
        &self,
        context: &PortContext,
        request: &CheckoutReconciliationActionRequest,
        operation: &checkout_operation::Model,
    ) -> CheckoutReconciliationResult<ActionEffect> {
        match request.action {
            // The retry re-arms the operation and records the decision; it has no
            // money step and no precondition beyond the parked status the caller
            // already checked.
            CheckoutReconciliationAction::RetryCompensation => {
                Ok(ActionEffect::without_money(Settlement::Retry))
            }
            CheckoutReconciliationAction::AttestExternal => self.attest_external(request).await,
            CheckoutReconciliationAction::WriteOff => self.write_off(request).await,
            CheckoutReconciliationAction::RefundFull => self.refund(request, operation, None).await,
            CheckoutReconciliationAction::RefundPartial => {
                self.refund(request, operation, request.amount).await
            }
            CheckoutReconciliationAction::VoidAuthorization => {
                self.void_authorization(context, request, operation).await
            }
        }
    }

    /// Actions recorded for one operation, newest first.
    pub async fn list_actions(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        limit: u64,
    ) -> CheckoutReconciliationResult<Vec<checkout_reconciliation_action::Model>> {
        if tenant_id.is_nil() || operation_id.is_nil() {
            return Err(CheckoutReconciliationError::Validation(
                "action listing requires non-nil tenant and operation identifiers".to_string(),
            ));
        }
        let limit = limit.clamp(1, MAX_RECONCILIATION_ACTION_LIST_LIMIT);
        checkout_reconciliation_action::Entity::find()
            .filter(checkout_reconciliation_action::Column::TenantId.eq(tenant_id))
            .filter(checkout_reconciliation_action::Column::CheckoutOperationId.eq(operation_id))
            .order_by_desc(checkout_reconciliation_action::Column::CreatedAt)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(Into::into)
    }

    async fn find_action(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        idempotency_key: &str,
    ) -> CheckoutReconciliationResult<Option<checkout_reconciliation_action::Model>> {
        checkout_reconciliation_action::Entity::find()
            .filter(checkout_reconciliation_action::Column::TenantId.eq(tenant_id))
            .filter(checkout_reconciliation_action::Column::CheckoutOperationId.eq(operation_id))
            .filter(checkout_reconciliation_action::Column::IdempotencyKey.eq(idempotency_key))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    async fn attest_external(
        &self,
        request: &CheckoutReconciliationActionRequest,
    ) -> CheckoutReconciliationResult<ActionEffect> {
        let outcome = request.outcome.ok_or_else(|| {
            CheckoutReconciliationError::Validation(
                "`attest_external` requires an explicit `compensated` or `failed` outcome"
                    .to_string(),
            )
        })?;
        let evidence_ref = request.evidence_ref.as_deref().ok_or_else(|| {
            CheckoutReconciliationError::Validation(
                "`attest_external` requires an `evidence_ref` pointing at the out-of-band reconciliation"
                    .to_string(),
            )
        })?;
        Ok(ActionEffect::without_money(Settlement::Resolve {
            outcome,
            reason: format!(
                "attested externally: {} (evidence: {evidence_ref})",
                request.reason
            ),
        }))
    }

    async fn write_off(
        &self,
        request: &CheckoutReconciliationActionRequest,
    ) -> CheckoutReconciliationResult<ActionEffect> {
        let approver_id = request.second_approver_id.ok_or_else(|| {
            CheckoutReconciliationError::Validation(
                "`write_off` requires a second approver".to_string(),
            )
        })?;
        if approver_id == request.operator_id {
            return Err(CheckoutReconciliationError::Validation(
                "the second approver of a `write_off` must differ from the operator".to_string(),
            ));
        }
        Ok(ActionEffect::without_money(Settlement::Resolve {
            outcome: CheckoutReconciliationOutcome::Failed,
            reason: format!(
                "written off (approved by {approver_id}): {}",
                request.reason
            ),
        }))
    }

    async fn refund(
        &self,
        request: &CheckoutReconciliationActionRequest,
        operation: &checkout_operation::Model,
        requested_amount: Option<Decimal>,
    ) -> CheckoutReconciliationResult<ActionEffect> {
        let collection = self.read_collection(request, operation).await?;
        let refundable = collection.captured_amount - collection.refunded_amount;
        if refundable <= Decimal::ZERO {
            return Err(CheckoutReconciliationError::Conflict(format!(
                "payment collection {} has nothing refundable (captured {}, refunded {})",
                collection.id, collection.captured_amount, collection.refunded_amount
            )));
        }
        let amount = match requested_amount {
            Some(amount) => amount,
            None => refundable,
        };
        if amount <= Decimal::ZERO || amount > refundable {
            return Err(CheckoutReconciliationError::Validation(format!(
                "refund amount {amount} must be greater than zero and at most the refundable {refundable}"
            )));
        }

        let refund = self
            .call_refund_port(
                request,
                collection.id,
                amount,
                collection.currency_code.as_str(),
            )
            .await?;

        Ok(ActionEffect {
            settlement: Settlement::Resolve {
                outcome: CheckoutReconciliationOutcome::Compensated,
                reason: format!(
                    "refunded {amount} {} through the payment owner: {}",
                    collection.currency_code, request.reason
                ),
            },
            money_moved: true,
            amount: Some(amount),
            currency_code: Some(collection.currency_code.clone()),
            refund_id: Some(refund.id),
            refund_status: Some(refund.status.clone()),
        })
    }

    async fn void_authorization(
        &self,
        context: &PortContext,
        request: &CheckoutReconciliationActionRequest,
        operation: &checkout_operation::Model,
    ) -> CheckoutReconciliationResult<ActionEffect> {
        let collection = self.read_collection(request, operation).await?;
        if collection.captured_amount > Decimal::ZERO {
            return Err(CheckoutReconciliationError::Conflict(format!(
                "payment collection {} already captured {}; captured funds must be refunded, not voided",
                collection.id, collection.captured_amount
            )));
        }
        let cancelled = self
            .collection_command_port
            .cancel_payment_collection(
                context.clone(),
                CancelAdminPaymentCollectionRequest {
                    collection_id: collection.id,
                    input: CancelPaymentInput {
                        reason: Some(request.reason.clone()),
                        metadata: action_metadata(request),
                    },
                },
            )
            .await
            .map_err(|error| payment_owner_error("cancel_payment_collection", error))?;

        // The cancelled collection carries no refund, so the journal records the
        // collection status as the money evidence for this action. The provider
        // side already changed (the authorization is released), which is why the
        // action is reported as one that moved money: a failed close must be
        // visible as such instead of looking like a plain conflict.
        Ok(ActionEffect {
            settlement: Settlement::Resolve {
                outcome: CheckoutReconciliationOutcome::Failed,
                reason: format!(
                    "authorization voided for collection {}: {}",
                    collection.id, request.reason
                ),
            },
            money_moved: true,
            amount: None,
            currency_code: Some(cancelled.currency_code.clone()),
            refund_id: None,
            refund_status: Some(cancelled.status.clone()),
        })
    }

    async fn read_collection(
        &self,
        request: &CheckoutReconciliationActionRequest,
        operation: &checkout_operation::Model,
    ) -> CheckoutReconciliationResult<rustok_payment::PaymentCollectionResponse> {
        let collection_id = operation.payment_collection_id.ok_or_else(|| {
            CheckoutReconciliationError::Conflict(format!(
                "checkout operation {} is not bound to a payment collection",
                request.operation_id
            ))
        })?;
        self.payment_read_port
            .read_payment_collection_projection(
                PortContext::new(
                    request.tenant_id.to_string(),
                    rustok_api::PortActor::user(request.operator_id.to_string()),
                    "en",
                    format!("commerce-checkout-reconciliation:read:{collection_id}"),
                ),
                ReadPaymentCollectionProjectionRequest { collection_id },
            )
            .await
            .map_err(|error| payment_owner_error("read_payment_collection_projection", error))
    }

    async fn call_refund_port(
        &self,
        request: &CheckoutReconciliationActionRequest,
        collection_id: Uuid,
        amount: Decimal,
        currency_code: &str,
    ) -> CheckoutReconciliationResult<RefundResponse> {
        self.refund_command_port
            .create_refund(
                PortContext::new(
                    request.tenant_id.to_string(),
                    rustok_api::PortActor::user(request.operator_id.to_string()),
                    "en",
                    format!("commerce-checkout-reconciliation:refund:{collection_id}"),
                ),
                CreateAdminRefundRequest {
                    collection_id,
                    creation_key: refund_creation_key(request.idempotency_key.as_str()),
                    input: CreateRefundInput {
                        amount,
                        reason: Some(format!(
                            "checkout reconciliation ({}): {}",
                            request.action.as_str(),
                            request.reason
                        )),
                        metadata: json!({
                            "checkout_operation_id": request.operation_id,
                            "checkout_reconciliation_action": request.action.as_str(),
                            "operator_id": request.operator_id,
                            "currency_code": currency_code,
                        }),
                    },
                },
            )
            .await
            .map_err(|error| payment_owner_error("create_refund", error))
    }
}

/// Result of one action attempt: the journal row plus whether it was replayed.
struct ActionOutcome {
    record: checkout_reconciliation_action::Model,
    replayed: bool,
}

/// How one action leaves the parked state.
///
/// The close itself is performed by [`CheckoutOperationJournal`], which writes
/// the transition and the decision row in one transaction; the action only
/// describes which transition it decided on and with which operator-facing text.
enum Settlement {
    /// Terminal close of the parked operation.
    Resolve {
        outcome: CheckoutReconciliationOutcome,
        /// Text recorded on the operation row, e.g. the refund that was issued.
        reason: String,
    },
    /// Hand the operation back to the compensation sweep.
    Retry,
}

impl ActionEffect {
    /// An action that called no payment owner command.
    fn without_money(settlement: Settlement) -> Self {
        Self {
            settlement,
            money_moved: false,
            amount: None,
            currency_code: None,
            refund_id: None,
            refund_status: None,
        }
    }
}

/// What one action did before the operation is closed.
struct ActionEffect {
    settlement: Settlement,
    /// The action already moved money through the payment owner. A failed close
    /// is then reported as `CloseAfterMoneyMoved` instead of a plain conflict, so
    /// the operator learns that the refund exists and that replaying the same
    /// request is safe (the payment owner deduplicates by creation key).
    money_moved: bool,
    amount: Option<Decimal>,
    currency_code: Option<String>,
    refund_id: Option<Uuid>,
    refund_status: Option<String>,
}

fn validate_request(
    request: &mut CheckoutReconciliationActionRequest,
) -> CheckoutReconciliationResult<()> {
    if request.tenant_id.is_nil() || request.operation_id.is_nil() || request.operator_id.is_nil() {
        return Err(CheckoutReconciliationError::Validation(
            "reconciliation actions require non-nil tenant, operation and operator identifiers"
                .to_string(),
        ));
    }
    request.idempotency_key = normalize_bounded(
        "idempotency_key",
        request.idempotency_key.clone(),
        MAX_RECONCILIATION_IDEMPOTENCY_KEY_LENGTH,
    )?;
    request.reason = normalize_bounded(
        "reason",
        request.reason.clone(),
        MAX_RECONCILIATION_REASON_LENGTH,
    )?;
    request.evidence_ref = request
        .evidence_ref
        .clone()
        .map(|value| normalize_bounded("evidence_ref", value, MAX_RECONCILIATION_EVIDENCE_LENGTH))
        .transpose()?;

    match request.action {
        CheckoutReconciliationAction::AttestExternal => {
            if request.outcome.is_none() {
                return Err(CheckoutReconciliationError::Validation(
                    "`attest_external` requires an explicit `compensated` or `failed` outcome"
                        .to_string(),
                ));
            }
            if request.evidence_ref.is_none() {
                return Err(CheckoutReconciliationError::Validation(
                    "`attest_external` requires an `evidence_ref`".to_string(),
                ));
            }
            reject_unexpected_fields(
                request,
                UnexpectedFields {
                    outcome: true,
                    amount: false,
                    second_approver: false,
                },
            )?;
        }
        CheckoutReconciliationAction::WriteOff => {
            reject_unexpected_fields(
                request,
                UnexpectedFields {
                    outcome: false,
                    amount: false,
                    second_approver: true,
                },
            )?;
        }
        CheckoutReconciliationAction::RefundFull => {
            if request.amount.is_some() {
                return Err(CheckoutReconciliationError::Validation(
                    "`refund_full` refunds the whole refundable amount; use `refund_partial` for a specific amount"
                        .to_string(),
                ));
            }
            reject_unexpected_fields(
                request,
                UnexpectedFields {
                    outcome: false,
                    amount: false,
                    second_approver: false,
                },
            )?;
        }
        CheckoutReconciliationAction::RefundPartial => {
            if request.amount.is_none() {
                return Err(CheckoutReconciliationError::Validation(
                    "`refund_partial` requires an amount".to_string(),
                ));
            }
            reject_unexpected_fields(
                request,
                UnexpectedFields {
                    outcome: false,
                    amount: true,
                    second_approver: false,
                },
            )?;
        }
        CheckoutReconciliationAction::RetryCompensation
        | CheckoutReconciliationAction::VoidAuthorization => {
            reject_unexpected_fields(
                request,
                UnexpectedFields {
                    outcome: false,
                    amount: false,
                    second_approver: false,
                },
            )?;
        }
    }
    Ok(())
}

/// Which of the action-specific request fields this action accepts.
///
/// `evidence_ref` is deliberately not part of the matrix: it is free-form
/// operator evidence that any action may carry (a ticket id for a written-off
/// charge is as meaningful as one for an external attestation), so it is only
/// required by `attest_external` and never rejected elsewhere.
struct UnexpectedFields {
    outcome: bool,
    amount: bool,
    second_approver: bool,
}

/// Rejects fields that do not belong to the action instead of ignoring them.
///
/// An operator who sends `outcome` with `refund_full`, or a second approver with
/// an action that has one signature, is describing an action this endpoint does
/// not implement; silently dropping the field would record a decision nobody
/// made.
fn reject_unexpected_fields(
    request: &CheckoutReconciliationActionRequest,
    accepted: UnexpectedFields,
) -> CheckoutReconciliationResult<()> {
    if request.outcome.is_some() && !accepted.outcome {
        return Err(CheckoutReconciliationError::Validation(format!(
            "`{}` does not take an `outcome`; the action decides it",
            request.action.as_str()
        )));
    }
    if request.amount.is_some() && !accepted.amount {
        return Err(CheckoutReconciliationError::Validation(format!(
            "`{}` never carries an amount",
            request.action.as_str()
        )));
    }
    if request.second_approver_id.is_some() && !accepted.second_approver {
        return Err(CheckoutReconciliationError::Validation(format!(
            "`{}` does not take a second approver",
            request.action.as_str()
        )));
    }
    Ok(())
}

/// Deterministic fingerprint of an action request. Replaying the same
/// idempotency key with a different fingerprint is a conflict, never a second
/// journal row.
fn fingerprint(request: &CheckoutReconciliationActionRequest) -> String {
    let mut digest = Sha256::new();
    digest.update(request.tenant_id.as_bytes());
    digest.update(request.operation_id.as_bytes());
    digest.update(request.action.as_str().as_bytes());
    digest.update(request.operator_id.as_bytes());
    digest.update(request.reason.as_bytes());
    digest.update(request.evidence_ref.as_deref().unwrap_or("none").as_bytes());
    digest.update(
        request
            .second_approver_id
            .map(|approver| approver.to_string())
            .unwrap_or_else(|| "none".to_string())
            .as_bytes(),
    );
    digest.update(
        request
            .outcome
            .map(|outcome| outcome.as_str())
            .unwrap_or("none")
            .as_bytes(),
    );
    digest.update(
        request
            .amount
            .map(|amount| amount.to_string())
            .unwrap_or_else(|| "none".to_string())
            .as_bytes(),
    );
    let digest = digest.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

fn replay(
    existing: checkout_reconciliation_action::Model,
    request_hash: &str,
) -> CheckoutReconciliationResult<checkout_reconciliation_action::Model> {
    if existing.request_hash != request_hash {
        return Err(CheckoutReconciliationError::JournalConflict(format!(
            "idempotency key `{}` was already used for a different reconciliation action",
            existing.idempotency_key
        )));
    }
    Ok(existing)
}

fn refund_creation_key(idempotency_key: &str) -> String {
    format!("checkout-reconciliation:{idempotency_key}")
}

fn action_metadata(request: &CheckoutReconciliationActionRequest) -> serde_json::Value {
    json!({
        "checkout_operation_id": request.operation_id,
        "checkout_reconciliation_action": request.action.as_str(),
        "operator_id": request.operator_id,
    })
}

/// Bounded `result` label for a failed action.
fn action_result_label(error: &CheckoutReconciliationError) -> &'static str {
    match error {
        CheckoutReconciliationError::Validation(_) => "rejected",
        CheckoutReconciliationError::NotFound(_) => "not_found",
        CheckoutReconciliationError::Conflict(_) => "conflict",
        CheckoutReconciliationError::JournalConflict(_) => "idempotency_conflict",
        CheckoutReconciliationError::PaymentOwner { .. } => "payment_owner",
        CheckoutReconciliationError::CloseAfterMoneyMoved { .. } => "close_after_money_moved",
        CheckoutReconciliationError::Operation(_) => "operation_conflict",
        CheckoutReconciliationError::Database(_) => "storage_error",
    }
}

fn payment_owner_error(operation: &'static str, error: PortError) -> CheckoutReconciliationError {
    CheckoutReconciliationError::PaymentOwner {
        operation,
        code: error.code,
        message: error.message,
        retryable: error.retryable,
    }
}

fn normalize_bounded(
    field: &str,
    value: String,
    maximum_length: usize,
) -> CheckoutReconciliationResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() || value.chars().count() > maximum_length {
        return Err(CheckoutReconciliationError::Validation(format!(
            "{field} must contain 1 to {maximum_length} characters"
        )));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(action: CheckoutReconciliationAction) -> CheckoutReconciliationActionRequest {
        CheckoutReconciliationActionRequest {
            tenant_id: generate_id(),
            operation_id: generate_id(),
            action,
            operator_id: generate_id(),
            idempotency_key: "key-1".to_string(),
            reason: "provider confirmed the capture".to_string(),
            outcome: None,
            evidence_ref: None,
            second_approver_id: None,
            amount: None,
        }
    }

    #[test]
    fn reconciliation_actions_round_trip_and_declare_their_guards() {
        for action in [
            CheckoutReconciliationAction::RetryCompensation,
            CheckoutReconciliationAction::AttestExternal,
            CheckoutReconciliationAction::WriteOff,
            CheckoutReconciliationAction::RefundFull,
            CheckoutReconciliationAction::RefundPartial,
            CheckoutReconciliationAction::VoidAuthorization,
        ] {
            assert_eq!(
                CheckoutReconciliationAction::parse(action.as_str()),
                Some(action)
            );
            assert!(!action.required_permissions().is_empty());
        }
        assert!(CheckoutReconciliationAction::RefundFull.moves_money());
        assert!(CheckoutReconciliationAction::RefundPartial.moves_money());
        assert!(!CheckoutReconciliationAction::WriteOff.moves_money());
        assert!(CheckoutReconciliationAction::WriteOff.requires_second_approver());
        assert!(!CheckoutReconciliationAction::AttestExternal.requires_second_approver());
        assert_eq!(
            CheckoutReconciliationAction::RefundPartial.required_permissions(),
            &[Permission::ORDERS_MANAGE, Permission::PAYMENTS_UPDATE]
        );
    }

    #[test]
    fn attest_external_requires_outcome_and_evidence() {
        let mut missing_outcome = request(CheckoutReconciliationAction::AttestExternal);
        missing_outcome.evidence_ref = Some("ticket-42".to_string());
        assert!(validate_request(&mut missing_outcome).is_err());

        let mut valid = request(CheckoutReconciliationAction::AttestExternal);
        valid.evidence_ref = Some("ticket-42".to_string());
        valid.outcome = Some(CheckoutReconciliationOutcome::Compensated);
        assert!(validate_request(&mut valid).is_ok());
    }

    #[test]
    fn write_off_rejects_amount_and_conflicting_outcome() {
        let mut with_amount = request(CheckoutReconciliationAction::WriteOff);
        with_amount.amount = Some(Decimal::new(1000, 2));
        assert!(validate_request(&mut with_amount).is_err());

        let mut with_outcome = request(CheckoutReconciliationAction::WriteOff);
        with_outcome.outcome = Some(CheckoutReconciliationOutcome::Failed);
        assert!(validate_request(&mut with_outcome).is_err());
    }

    #[test]
    fn fields_that_do_not_belong_to_the_action_are_rejected_not_ignored() {
        let mut outcome_on_refund = request(CheckoutReconciliationAction::RefundFull);
        outcome_on_refund.outcome = Some(CheckoutReconciliationOutcome::Compensated);
        assert!(validate_request(&mut outcome_on_refund).is_err());

        let mut approver_on_void = request(CheckoutReconciliationAction::VoidAuthorization);
        approver_on_void.second_approver_id = Some(generate_id());
        assert!(validate_request(&mut approver_on_void).is_err());

        let mut approver_on_attest = request(CheckoutReconciliationAction::AttestExternal);
        approver_on_attest.outcome = Some(CheckoutReconciliationOutcome::Failed);
        approver_on_attest.evidence_ref = Some("ticket-7".to_string());
        approver_on_attest.second_approver_id = Some(generate_id());
        assert!(validate_request(&mut approver_on_attest).is_err());

        // The evidence reference is the one free-form field every action may
        // carry: it is required by `attest_external` and recorded elsewhere.
        let mut evidence_on_void = request(CheckoutReconciliationAction::VoidAuthorization);
        evidence_on_void.evidence_ref = Some("bank-statement-2026-10-07".to_string());
        assert!(validate_request(&mut evidence_on_void).is_ok());
    }

    #[test]
    fn fingerprints_differ_when_the_decision_changes() {
        let base = request(CheckoutReconciliationAction::RefundFull);
        let mut other = base.clone();
        other.amount = Some(Decimal::new(2500, 2));
        assert_ne!(fingerprint(&base), fingerprint(&other));
        assert_eq!(fingerprint(&base), fingerprint(&base.clone()));
    }

    #[test]
    fn refund_creation_key_stays_within_the_payment_owner_limit() {
        let key = "k".repeat(MAX_RECONCILIATION_IDEMPOTENCY_KEY_LENGTH);
        assert!(refund_creation_key(key.as_str()).chars().count() <= 191);
    }
}
