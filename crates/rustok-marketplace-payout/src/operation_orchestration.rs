use std::collections::{BTreeMap, BTreeSet};

use chrono::{Duration as ChronoDuration, Utc};
use rustok_api::PortContext;
use rustok_core::generate_id;
use rustok_marketplace_ledger::{
    MarketplaceSellerBalanceTransferKind, MarketplaceSellerBalanceTransferLineInput,
    MarketplaceSellerBalanceTransferResponse, PostMarketplaceSellerBalanceTransferInput,
};
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, Condition, EntityTrait, IntoActiveModel,
    QueryFilter, QueryOrder, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    dto::{MarketplacePayoutResponse, ScheduleMarketplacePayoutInput},
    entities::{
        item, operation,
        operation::{MarketplacePayoutOperationStage, MarketplacePayoutOperationStatus},
        operation_transfer,
        operation_transfer::{
            MarketplacePayoutOperationTransferKind, MarketplacePayoutOperationTransferStatus,
        },
    },
    error::{MarketplacePayoutError, MarketplacePayoutResult},
    receipts::{normalize_idempotency_key, replay_existing, schedule_request_hash},
    service::{normalize_schedule_input, MarketplacePayoutService},
};

const OPERATION_LEASE_SECONDS: i64 = 300;
const RELEASE_SEQUENCE_OFFSET: i32 = 10_000;
const OPERATION_REQUEST_VERSION: u16 = 1;
const TRANSFER_PAYLOAD_VERSION: u16 = 1;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PayoutOperationRequestSnapshot {
    version: u16,
    input: ScheduleMarketplacePayoutInput,
    entries: Vec<PayoutEntrySnapshot>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PayoutEntrySnapshot {
    ledger_entry_id: Uuid,
    order_id: Uuid,
    amount: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PersistedTransferPayload {
    version: u16,
    request: PostMarketplaceSellerBalanceTransferInput,
    response: Option<MarketplaceSellerBalanceTransferResponse>,
}

struct HoldPlan {
    child_id: Uuid,
    order_id: Uuid,
    sequence_no: i32,
    total_amount: i64,
    request: PostMarketplaceSellerBalanceTransferInput,
}

impl MarketplacePayoutService {
    pub async fn schedule_with_operation(
        &self,
        context: PortContext,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        input: ScheduleMarketplacePayoutInput,
    ) -> MarketplacePayoutResult<MarketplacePayoutResponse> {
        let input = normalize_schedule_input(input)?;
        let key = normalize_idempotency_key(idempotency_key)?;
        let request_hash = schedule_request_hash(actor_id, &input)?;

        if let Some(existing) = find_operation(self, tenant_id, key.as_str()).await? {
            ensure_operation_identity(&existing, request_hash.as_str())?;
            return self.resume_operation(context, existing).await;
        }

        // Preserve replay compatibility for payouts scheduled before the durable operation
        // journal was introduced.
        if let Some(response) = replay_existing(
            self.database(),
            tenant_id,
            key.as_str(),
            request_hash.as_str(),
        )
        .await?
        {
            return Ok(response);
        }

        let _ = self.ledger_writer()?;
        let entries = self
            .load_selected_entries(context.clone(), tenant_id, &input)
            .await?;
        ensure_entries_unassigned(self, tenant_id, &entries).await?;
        let operation = admit_operation(
            self,
            tenant_id,
            actor_id,
            key,
            request_hash.as_str(),
            input,
            entries,
        )
        .await?;
        self.resume_operation(context, operation).await
    }

    async fn resume_operation(
        &self,
        context: PortContext,
        operation: operation::Model,
    ) -> MarketplacePayoutResult<MarketplacePayoutResponse> {
        let snapshot = parse_operation_snapshot(&operation)?;

        match operation.status {
            MarketplacePayoutOperationStatus::Completed => {
                let payout_id = operation
                    .payout_id
                    .ok_or(MarketplacePayoutError::OperationCorrupt(operation.id))?;
                return self.read_payout(operation.tenant_id, payout_id).await;
            }
            MarketplacePayoutOperationStatus::Failed
            | MarketplacePayoutOperationStatus::Cancelled => {
                return Err(MarketplacePayoutError::OperationFailed {
                    operation_id: operation.id,
                    code: operation.last_error_code,
                });
            }
            MarketplacePayoutOperationStatus::ReconciliationRequired => {
                return Err(MarketplacePayoutError::ReconciliationRequired(operation.id));
            }
            MarketplacePayoutOperationStatus::CompensationRequired
            | MarketplacePayoutOperationStatus::Compensating => {
                let _ = self.ledger_writer()?;
                let operation_id = operation.id;
                let claimed = claim_operation(
                    self,
                    operation,
                    MarketplacePayoutOperationStatus::Compensating,
                    MarketplacePayoutOperationStage::Releasing,
                )
                .await?;
                let cause_code = claimed
                    .last_error_code
                    .clone()
                    .unwrap_or_else(|| "marketplace_payout.compensation_resume".to_string());
                compensate_claimed_operation(self, context, claimed, cause_code.clone()).await?;
                return Err(MarketplacePayoutError::OperationFailed {
                    operation_id,
                    code: Some(cause_code),
                });
            }
            MarketplacePayoutOperationStatus::Pending
            | MarketplacePayoutOperationStatus::Executing
            | MarketplacePayoutOperationStatus::RetryableError => {}
        }

        // Do not mutate a resumable operation when the host omitted the write-side
        // ledger composition.
        let _ = self.ledger_writer()?;

        let mut claimed = claim_operation(
            self,
            operation,
            MarketplacePayoutOperationStatus::Executing,
            MarketplacePayoutOperationStage::Reserving,
        )
        .await?;

        if let Err(error) = execute_hold_children(self, context.clone(), &claimed).await {
            let cause_code = payout_error_code(&error);
            return match compensate_claimed_operation(self, context, claimed, cause_code).await {
                Ok(()) => Err(error),
                Err(compensation_error) => Err(compensation_error),
            };
        }

        claimed = update_running_operation_stage(
            self,
            claimed,
            MarketplacePayoutOperationStage::Reserved,
        )
        .await?;

        let schedule_result = self
            .schedule_with_receipt(
                context.clone(),
                claimed.tenant_id,
                claimed.actor_id,
                claimed.idempotency_key.clone(),
                snapshot.input,
            )
            .await;

        match schedule_result {
            Ok(response) => {
                complete_operation(self, claimed, response.id).await?;
                Ok(response)
            }
            Err(error) if payout_schedule_outcome_is_ambiguous(&error) => {
                let operation_id = claimed.id;
                let error_code = payout_error_code(&error);
                mark_operation_reconciliation(self, claimed, error_code.as_str()).await?;
                Err(MarketplacePayoutError::ReconciliationRequired(operation_id))
            }
            Err(error) => {
                let cause_code = payout_error_code(&error);
                match compensate_claimed_operation(self, context, claimed, cause_code).await {
                    Ok(()) => Err(error),
                    Err(compensation_error) => Err(compensation_error),
                }
            }
        }
    }
}

async fn admit_operation(
    service: &MarketplacePayoutService,
    tenant_id: Uuid,
    actor_id: Uuid,
    idempotency_key: String,
    request_hash: &str,
    input: ScheduleMarketplacePayoutInput,
    mut entries: Vec<rustok_marketplace_ledger::MarketplaceLedgerEntryResponse>,
) -> MarketplacePayoutResult<operation::Model> {
    entries.sort_by_key(|entry| (entry.order_id, entry.id));
    let operation_id = generate_id();
    let snapshot = PayoutOperationRequestSnapshot {
        version: OPERATION_REQUEST_VERSION,
        input: input.clone(),
        entries: entries
            .iter()
            .map(|entry| PayoutEntrySnapshot {
                ledger_entry_id: entry.id,
                order_id: entry.order_id,
                amount: entry.amount,
            })
            .collect(),
    };
    let request_json = serde_json::to_value(&snapshot)
        .map_err(|_| MarketplacePayoutError::OperationCorrupt(operation_id))?;
    let hold_plans = build_hold_plans(operation_id, &input, &entries)?;
    let transaction = service.database().begin().await?;
    let now = Utc::now().fixed_offset();

    let inserted = operation::ActiveModel {
        id: Set(operation_id),
        tenant_id: Set(tenant_id),
        actor_id: Set(actor_id),
        seller_id: Set(input.seller_id),
        currency_code: Set(input.currency_code.clone()),
        idempotency_key: Set(idempotency_key.clone()),
        request_hash: Set(request_hash.to_string()),
        request_json: Set(request_json),
        status: Set(MarketplacePayoutOperationStatus::Pending),
        stage: Set(MarketplacePayoutOperationStage::Created),
        payout_id: Set(None),
        attempt_count: Set(0),
        revision: Set(0),
        lease_owner: Set(None),
        lease_expires_at: Set(None),
        last_error_code: Set(None),
        created_at: Set(now.clone()),
        updated_at: Set(now.clone()),
        completed_at: Set(None),
    }
    .insert(&transaction)
    .await;

    let inserted = match inserted {
        Ok(model) => model,
        Err(error) if is_unique_constraint(&error) => {
            transaction.rollback().await?;
            let existing = find_operation(service, tenant_id, idempotency_key.as_str())
                .await?
                .ok_or(error)?;
            ensure_operation_identity(&existing, request_hash)?;
            return Ok(existing);
        }
        Err(error) => {
            transaction.rollback().await?;
            return Err(error.into());
        }
    };

    for plan in hold_plans {
        let transfer_id = plan.child_id;
        let idempotency_key = transfer_idempotency_key(
            operation_id,
            MarketplacePayoutOperationTransferKind::ReserveHold,
            transfer_id,
        );
        let request_hash = json_hash(&plan.request)?;
        let request_json = serde_json::to_value(PersistedTransferPayload {
            version: TRANSFER_PAYLOAD_VERSION,
            request: plan.request,
            response: None,
        })
        .map_err(|_| MarketplacePayoutError::OperationCorrupt(operation_id))?;

        operation_transfer::ActiveModel {
            id: Set(transfer_id),
            tenant_id: Set(tenant_id),
            operation_id: Set(operation_id),
            sequence_no: Set(plan.sequence_no),
            order_id: Set(plan.order_id),
            transfer_kind: Set(MarketplacePayoutOperationTransferKind::ReserveHold),
            status: Set(MarketplacePayoutOperationTransferStatus::Pending),
            idempotency_key: Set(idempotency_key),
            request_hash: Set(request_hash),
            request_json: Set(request_json),
            total_amount: Set(plan.total_amount),
            ledger_transfer_id: Set(None),
            ledger_transaction_id: Set(None),
            attempt_count: Set(0),
            revision: Set(0),
            last_error_code: Set(None),
            created_at: Set(now.clone()),
            updated_at: Set(now.clone()),
            completed_at: Set(None),
        }
        .insert(&transaction)
        .await?;
    }

    transaction.commit().await?;
    Ok(inserted)
}

fn build_hold_plans(
    operation_id: Uuid,
    input: &ScheduleMarketplacePayoutInput,
    entries: &[rustok_marketplace_ledger::MarketplaceLedgerEntryResponse],
) -> MarketplacePayoutResult<Vec<HoldPlan>> {
    let mut by_order =
        BTreeMap::<Uuid, Vec<&rustok_marketplace_ledger::MarketplaceLedgerEntryResponse>>::new();
    for entry in entries {
        by_order.entry(entry.order_id).or_default().push(entry);
    }

    let mut plans = Vec::with_capacity(by_order.len());
    for (index, (order_id, mut order_entries)) in by_order.into_iter().enumerate() {
        order_entries.sort_by_key(|entry| entry.id);
        let sequence_no = i32::try_from(index).map_err(|_| {
            MarketplacePayoutError::Validation(
                "payout order group count exceeds supported range".to_string(),
            )
        })?;
        let total_amount = order_entries.iter().try_fold(0_i64, |total, entry| {
            total.checked_add(entry.amount).ok_or_else(|| {
                MarketplacePayoutError::Validation("payout reserve hold total overflow".to_string())
            })
        })?;
        if total_amount <= 0 {
            return Err(MarketplacePayoutError::Validation(
                "payout reserve hold total must be greater than zero".to_string(),
            ));
        }

        let now = Utc::now().fixed_offset();
        let transferred_at = order_entries
            .iter()
            .map(|entry| entry.created_at.clone())
            .max()
            .map(|created_at| created_at.max(now.clone()))
            .unwrap_or(now);
        let child_id = generate_id();
        let lines = order_entries
            .iter()
            .map(|entry| MarketplaceSellerBalanceTransferLineInput {
                reference_entry_id: entry.id,
                amount: entry.amount,
            })
            .collect();

        plans.push(HoldPlan {
            child_id,
            order_id,
            sequence_no,
            total_amount,
            request: PostMarketplaceSellerBalanceTransferInput {
                kind: MarketplaceSellerBalanceTransferKind::ReserveHold,
                source_id: child_id,
                seller_id: input.seller_id,
                currency_code: input.currency_code.clone(),
                transferred_at,
                lines,
                metadata: serde_json::json!({
                    "payout_operation_id": operation_id,
                    "order_id": order_id,
                    "sequence_no": sequence_no,
                    "transfer_kind": "reserve_hold",
                }),
            },
        });
    }
    Ok(plans)
}

async fn execute_hold_children(
    service: &MarketplacePayoutService,
    context: PortContext,
    operation: &operation::Model,
) -> MarketplacePayoutResult<()> {
    let holds = operation_transfer::Entity::find()
        .filter(operation_transfer::Column::TenantId.eq(operation.tenant_id))
        .filter(operation_transfer::Column::OperationId.eq(operation.id))
        .filter(
            operation_transfer::Column::TransferKind
                .eq(MarketplacePayoutOperationTransferKind::ReserveHold),
        )
        .order_by_asc(operation_transfer::Column::SequenceNo)
        .all(service.database())
        .await?;
    if holds.is_empty() {
        return Err(MarketplacePayoutError::OperationCorrupt(operation.id));
    }

    for hold in holds {
        match hold.status {
            MarketplacePayoutOperationTransferStatus::Posted => continue,
            MarketplacePayoutOperationTransferStatus::Pending
            | MarketplacePayoutOperationTransferStatus::Executing
            | MarketplacePayoutOperationTransferStatus::RetryableError => {
                execute_transfer(service, context.clone(), hold).await?;
            }
            MarketplacePayoutOperationTransferStatus::ReconciliationRequired => {
                return Err(MarketplacePayoutError::ReconciliationRequired(operation.id));
            }
            MarketplacePayoutOperationTransferStatus::Compensated
            | MarketplacePayoutOperationTransferStatus::Failed => {
                return Err(MarketplacePayoutError::OperationFailed {
                    operation_id: operation.id,
                    code: hold.last_error_code,
                });
            }
        }
    }
    Ok(())
}

async fn execute_transfer(
    service: &MarketplacePayoutService,
    context: PortContext,
    model: operation_transfer::Model,
) -> MarketplacePayoutResult<operation_transfer::Model> {
    if matches!(
        model.status,
        MarketplacePayoutOperationTransferStatus::Posted
            | MarketplacePayoutOperationTransferStatus::Compensated
    ) {
        return Ok(model);
    }
    if model.status == MarketplacePayoutOperationTransferStatus::ReconciliationRequired {
        return Err(MarketplacePayoutError::ReconciliationRequired(
            model.operation_id,
        ));
    }
    if model.status == MarketplacePayoutOperationTransferStatus::Failed {
        return Err(MarketplacePayoutError::OperationFailed {
            operation_id: model.operation_id,
            code: model.last_error_code,
        });
    }

    let mut payload = match parse_transfer_payload(&model) {
        Ok(payload) => payload,
        Err(_) => {
            let operation_id = model.operation_id;
            mark_transfer_reconciliation(
                service,
                model,
                "marketplace_payout.transfer_payload_corrupt",
            )
            .await?;
            return Err(MarketplacePayoutError::ReconciliationRequired(operation_id));
        }
    };
    if json_hash(&payload.request)? != model.request_hash {
        let operation_id = model.operation_id;
        mark_transfer_reconciliation(
            service,
            model,
            "marketplace_payout.transfer_request_hash_mismatch",
        )
        .await?;
        return Err(MarketplacePayoutError::OperationCorrupt(operation_id));
    }

    let executing = mark_transfer_executing(service, model).await?;
    let writer = service.ledger_writer()?;
    let call_context = context
        .clone()
        .with_idempotency_key(executing.idempotency_key.clone());
    let response = writer
        .post_seller_balance_transfer(call_context, payload.request.clone())
        .await;

    match response {
        Ok(response) => {
            if !transfer_response_matches(&executing, &payload.request, &response) {
                mark_transfer_reconciliation(
                    service,
                    executing.clone(),
                    "marketplace_payout.ledger_transfer_response_mismatch",
                )
                .await?;
                return Err(MarketplacePayoutError::ReconciliationRequired(
                    executing.operation_id,
                ));
            }
            payload.response = Some(response.clone());
            mark_transfer_posted(service, executing, payload, &response).await
        }
        Err(error) => {
            let mapped = map_ledger_port_error(error);
            mark_transfer_error(service, executing, &mapped).await?;
            Err(mapped)
        }
    }
}

async fn compensate_claimed_operation(
    service: &MarketplacePayoutService,
    context: PortContext,
    operation: operation::Model,
    cause_code: String,
) -> MarketplacePayoutResult<()> {
    let operation = if operation.status == MarketplacePayoutOperationStatus::Compensating {
        operation
    } else {
        update_running_operation(
            service,
            operation,
            MarketplacePayoutOperationStatus::Compensating,
            MarketplacePayoutOperationStage::Releasing,
            Some(cause_code.clone()),
        )
        .await?
    };

    let holds = operation_transfer::Entity::find()
        .filter(operation_transfer::Column::TenantId.eq(operation.tenant_id))
        .filter(operation_transfer::Column::OperationId.eq(operation.id))
        .filter(
            operation_transfer::Column::TransferKind
                .eq(MarketplacePayoutOperationTransferKind::ReserveHold),
        )
        .order_by_desc(operation_transfer::Column::SequenceNo)
        .all(service.database())
        .await?;

    for mut hold in holds {
        match hold.status {
            MarketplacePayoutOperationTransferStatus::Pending
            | MarketplacePayoutOperationTransferStatus::Failed => continue,
            MarketplacePayoutOperationTransferStatus::Compensated => continue,
            MarketplacePayoutOperationTransferStatus::ReconciliationRequired => {
                mark_operation_reconciliation(
                    service,
                    operation,
                    "marketplace_payout.hold_reconciliation_required",
                )
                .await?;
                return Err(MarketplacePayoutError::ReconciliationRequired(
                    hold.operation_id,
                ));
            }
            MarketplacePayoutOperationTransferStatus::Executing
            | MarketplacePayoutOperationTransferStatus::RetryableError => {
                match execute_transfer(service, context.clone(), hold).await {
                    Ok(recovered) => hold = recovered,
                    Err(error) => {
                        return persist_compensation_failure(service, operation, error).await;
                    }
                }
            }
            MarketplacePayoutOperationTransferStatus::Posted => {}
        }

        if hold.status != MarketplacePayoutOperationTransferStatus::Posted {
            continue;
        }
        let release = match ensure_release_child(service, &operation, &hold).await {
            Ok(release) => release,
            Err(error) => {
                return persist_compensation_failure(service, operation, error).await;
            }
        };
        if release.status != MarketplacePayoutOperationTransferStatus::Posted {
            if let Err(error) = execute_transfer(service, context.clone(), release).await {
                return persist_compensation_failure(service, operation, error).await;
            }
        }
        mark_hold_compensated(service, hold).await?;
    }

    finish_failed_operation(service, operation, cause_code).await?;
    Ok(())
}

async fn ensure_release_child(
    service: &MarketplacePayoutService,
    operation: &operation::Model,
    hold: &operation_transfer::Model,
) -> MarketplacePayoutResult<operation_transfer::Model> {
    if let Some(existing) = operation_transfer::Entity::find()
        .filter(operation_transfer::Column::TenantId.eq(operation.tenant_id))
        .filter(operation_transfer::Column::OperationId.eq(operation.id))
        .filter(operation_transfer::Column::OrderId.eq(hold.order_id))
        .filter(
            operation_transfer::Column::TransferKind
                .eq(MarketplacePayoutOperationTransferKind::ReserveRelease),
        )
        .one(service.database())
        .await?
    {
        return Ok(existing);
    }

    let hold_payload = parse_transfer_payload(hold)?;
    let hold_response = hold_payload
        .response
        .ok_or(MarketplacePayoutError::OperationCorrupt(operation.id))?;
    let release_sequence = RELEASE_SEQUENCE_OFFSET
        .checked_add(hold.sequence_no)
        .ok_or_else(|| {
            MarketplacePayoutError::Validation("payout release sequence overflow".to_string())
        })?;
    let release_id = generate_id();
    let mut lines = hold_response
        .lines
        .iter()
        .map(|line| MarketplaceSellerBalanceTransferLineInput {
            reference_entry_id: line.credit_entry.id,
            amount: line.amount,
        })
        .collect::<Vec<_>>();
    lines.sort_by_key(|line| line.reference_entry_id);
    let request = PostMarketplaceSellerBalanceTransferInput {
        kind: MarketplaceSellerBalanceTransferKind::ReserveRelease,
        source_id: release_id,
        seller_id: operation.seller_id,
        currency_code: operation.currency_code.clone(),
        transferred_at: Utc::now().fixed_offset(),
        lines,
        metadata: serde_json::json!({
            "payout_operation_id": operation.id,
            "order_id": hold.order_id,
            "hold_transfer_id": hold.ledger_transfer_id,
            "hold_child_id": hold.id,
            "sequence_no": release_sequence,
            "transfer_kind": "reserve_release",
        }),
    };
    let payload = PersistedTransferPayload {
        version: TRANSFER_PAYLOAD_VERSION,
        request: request.clone(),
        response: None,
    };
    let now = Utc::now().fixed_offset();
    let inserted = operation_transfer::ActiveModel {
        id: Set(release_id),
        tenant_id: Set(operation.tenant_id),
        operation_id: Set(operation.id),
        sequence_no: Set(release_sequence),
        order_id: Set(hold.order_id),
        transfer_kind: Set(MarketplacePayoutOperationTransferKind::ReserveRelease),
        status: Set(MarketplacePayoutOperationTransferStatus::Pending),
        idempotency_key: Set(transfer_idempotency_key(
            operation.id,
            MarketplacePayoutOperationTransferKind::ReserveRelease,
            release_id,
        )),
        request_hash: Set(json_hash(&request)?),
        request_json: Set(serde_json::to_value(payload)
            .map_err(|_| MarketplacePayoutError::OperationCorrupt(operation.id))?),
        total_amount: Set(hold.total_amount),
        ledger_transfer_id: Set(None),
        ledger_transaction_id: Set(None),
        attempt_count: Set(0),
        revision: Set(0),
        last_error_code: Set(None),
        created_at: Set(now.clone()),
        updated_at: Set(now),
        completed_at: Set(None),
    }
    .insert(service.database())
    .await;

    match inserted {
        Ok(model) => Ok(model),
        Err(error) if is_unique_constraint(&error) => operation_transfer::Entity::find()
            .filter(operation_transfer::Column::TenantId.eq(operation.tenant_id))
            .filter(operation_transfer::Column::OperationId.eq(operation.id))
            .filter(operation_transfer::Column::OrderId.eq(hold.order_id))
            .filter(
                operation_transfer::Column::TransferKind
                    .eq(MarketplacePayoutOperationTransferKind::ReserveRelease),
            )
            .one(service.database())
            .await?
            .ok_or(error.into()),
        Err(error) => Err(error.into()),
    }
}

async fn find_operation(
    service: &MarketplacePayoutService,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> MarketplacePayoutResult<Option<operation::Model>> {
    operation::Entity::find()
        .filter(operation::Column::TenantId.eq(tenant_id))
        .filter(operation::Column::IdempotencyKey.eq(idempotency_key))
        .one(service.database())
        .await
        .map_err(Into::into)
}

async fn ensure_entries_unassigned(
    service: &MarketplacePayoutService,
    tenant_id: Uuid,
    entries: &[rustok_marketplace_ledger::MarketplaceLedgerEntryResponse],
) -> MarketplacePayoutResult<()> {
    let entry_ids = entries.iter().map(|entry| entry.id).collect::<Vec<_>>();
    if let Some(existing) = item::Entity::find()
        .filter(item::Column::TenantId.eq(tenant_id))
        .filter(item::Column::LedgerEntryId.is_in(entry_ids))
        .one(service.database())
        .await?
    {
        return Err(MarketplacePayoutError::LedgerEntryAlreadyAssigned(
            existing.ledger_entry_id,
        ));
    }
    Ok(())
}

async fn claim_operation(
    service: &MarketplacePayoutService,
    model: operation::Model,
    target_status: MarketplacePayoutOperationStatus,
    target_stage: MarketplacePayoutOperationStage,
) -> MarketplacePayoutResult<operation::Model> {
    let now = Utc::now().fixed_offset();
    if matches!(
        model.status,
        MarketplacePayoutOperationStatus::Executing
            | MarketplacePayoutOperationStatus::Compensating
    ) && model
        .lease_expires_at
        .as_ref()
        .is_some_and(|expires_at| expires_at > &now)
    {
        return Err(MarketplacePayoutError::OperationInProgress(model.id));
    }

    let next_attempt = model.attempt_count.checked_add(1).ok_or_else(|| {
        MarketplacePayoutError::Validation("payout operation attempt count overflow".to_string())
    })?;
    let next_revision = model.revision.checked_add(1).ok_or_else(|| {
        MarketplacePayoutError::Validation("payout operation revision overflow".to_string())
    })?;
    let lease_owner = format!("marketplace-payout:{}", Uuid::new_v4());
    let lease_expires_at = now.clone() + ChronoDuration::seconds(OPERATION_LEASE_SECONDS);
    let result = operation::Entity::update_many()
        .col_expr(
            operation::Column::Status,
            Expr::value(target_status.as_str()),
        )
        .col_expr(operation::Column::Stage, Expr::value(target_stage.as_str()))
        .col_expr(operation::Column::AttemptCount, Expr::value(next_attempt))
        .col_expr(operation::Column::Revision, Expr::value(next_revision))
        .col_expr(operation::Column::LeaseOwner, Expr::value(lease_owner))
        .col_expr(
            operation::Column::LeaseExpiresAt,
            Expr::value(lease_expires_at),
        )
        .col_expr(operation::Column::UpdatedAt, Expr::value(now.clone()))
        .filter(operation::Column::Id.eq(model.id))
        .filter(operation::Column::TenantId.eq(model.tenant_id))
        .filter(operation::Column::Status.eq(model.status))
        .filter(operation::Column::Revision.eq(model.revision))
        .filter(
            Condition::any()
                .add(operation::Column::LeaseExpiresAt.is_null())
                .add(operation::Column::LeaseExpiresAt.lte(now)),
        )
        .exec(service.database())
        .await?;
    if result.rows_affected != 1 {
        return Err(MarketplacePayoutError::OperationInProgress(model.id));
    }
    load_operation(service, model.tenant_id, model.id).await
}

async fn load_operation(
    service: &MarketplacePayoutService,
    tenant_id: Uuid,
    operation_id: Uuid,
) -> MarketplacePayoutResult<operation::Model> {
    operation::Entity::find()
        .filter(operation::Column::TenantId.eq(tenant_id))
        .filter(operation::Column::Id.eq(operation_id))
        .one(service.database())
        .await?
        .ok_or(MarketplacePayoutError::OperationCorrupt(operation_id))
}

struct ClaimedOperationUpdate {
    status: MarketplacePayoutOperationStatus,
    stage: MarketplacePayoutOperationStage,
    payout_id: Option<Option<Uuid>>,
    last_error_code: Option<String>,
    completed_at: Option<chrono::DateTime<chrono::FixedOffset>>,
    clear_lease: bool,
}

async fn update_running_operation_stage(
    service: &MarketplacePayoutService,
    model: operation::Model,
    stage: MarketplacePayoutOperationStage,
) -> MarketplacePayoutResult<operation::Model> {
    update_claimed_operation(
        service,
        model,
        ClaimedOperationUpdate {
            status: MarketplacePayoutOperationStatus::Executing,
            stage,
            payout_id: None,
            last_error_code: None,
            completed_at: None,
            clear_lease: false,
        },
    )
    .await
}

async fn update_running_operation(
    service: &MarketplacePayoutService,
    model: operation::Model,
    status: MarketplacePayoutOperationStatus,
    stage: MarketplacePayoutOperationStage,
    last_error_code: Option<String>,
) -> MarketplacePayoutResult<operation::Model> {
    update_claimed_operation(
        service,
        model,
        ClaimedOperationUpdate {
            status,
            stage,
            payout_id: None,
            last_error_code,
            completed_at: None,
            clear_lease: false,
        },
    )
    .await
}

async fn complete_operation(
    service: &MarketplacePayoutService,
    model: operation::Model,
    payout_id: Uuid,
) -> MarketplacePayoutResult<operation::Model> {
    update_claimed_operation(
        service,
        model,
        ClaimedOperationUpdate {
            status: MarketplacePayoutOperationStatus::Completed,
            stage: MarketplacePayoutOperationStage::Completed,
            payout_id: Some(Some(payout_id)),
            last_error_code: None,
            completed_at: Some(Utc::now().fixed_offset()),
            clear_lease: true,
        },
    )
    .await
}

async fn finish_failed_operation(
    service: &MarketplacePayoutService,
    model: operation::Model,
    cause_code: String,
) -> MarketplacePayoutResult<operation::Model> {
    update_claimed_operation(
        service,
        model,
        ClaimedOperationUpdate {
            status: MarketplacePayoutOperationStatus::Failed,
            stage: MarketplacePayoutOperationStage::Released,
            payout_id: None,
            last_error_code: Some(cause_code),
            completed_at: Some(Utc::now().fixed_offset()),
            clear_lease: true,
        },
    )
    .await
}

async fn mark_operation_reconciliation(
    service: &MarketplacePayoutService,
    model: operation::Model,
    error_code: &str,
) -> MarketplacePayoutResult<operation::Model> {
    update_claimed_operation(
        service,
        model,
        ClaimedOperationUpdate {
            status: MarketplacePayoutOperationStatus::ReconciliationRequired,
            stage: MarketplacePayoutOperationStage::Releasing,
            payout_id: None,
            last_error_code: Some(error_code.to_string()),
            completed_at: None,
            clear_lease: true,
        },
    )
    .await
}

async fn persist_compensation_failure(
    service: &MarketplacePayoutService,
    model: operation::Model,
    error: MarketplacePayoutError,
) -> MarketplacePayoutResult<()> {
    let retryable = payout_error_retryable(&error);
    let operation_id = model.id;
    update_claimed_operation(
        service,
        model,
        ClaimedOperationUpdate {
            status: if retryable {
                MarketplacePayoutOperationStatus::CompensationRequired
            } else {
                MarketplacePayoutOperationStatus::ReconciliationRequired
            },
            stage: MarketplacePayoutOperationStage::Releasing,
            payout_id: None,
            last_error_code: Some(payout_error_code(&error)),
            completed_at: None,
            clear_lease: true,
        },
    )
    .await?;
    if retryable {
        Err(MarketplacePayoutError::CompensationRequired(operation_id))
    } else {
        Err(MarketplacePayoutError::ReconciliationRequired(operation_id))
    }
}

async fn update_claimed_operation(
    service: &MarketplacePayoutService,
    model: operation::Model,
    transition: ClaimedOperationUpdate,
) -> MarketplacePayoutResult<operation::Model> {
    let lease_owner = model
        .lease_owner
        .clone()
        .ok_or(MarketplacePayoutError::OperationCorrupt(model.id))?;
    let next_revision = model.revision.checked_add(1).ok_or_else(|| {
        MarketplacePayoutError::Validation("payout operation revision overflow".to_string())
    })?;
    let now = Utc::now().fixed_offset();
    let mut update = operation::Entity::update_many()
        .col_expr(
            operation::Column::Status,
            Expr::value(transition.status.as_str()),
        )
        .col_expr(
            operation::Column::Stage,
            Expr::value(transition.stage.as_str()),
        )
        .col_expr(operation::Column::Revision, Expr::value(next_revision))
        .col_expr(
            operation::Column::LastErrorCode,
            Expr::value(transition.last_error_code),
        )
        .col_expr(operation::Column::UpdatedAt, Expr::value(now))
        .filter(operation::Column::Id.eq(model.id))
        .filter(operation::Column::TenantId.eq(model.tenant_id))
        .filter(operation::Column::Revision.eq(model.revision))
        .filter(operation::Column::LeaseOwner.eq(lease_owner));

    if let Some(payout_id) = transition.payout_id {
        update = update.col_expr(operation::Column::PayoutId, Expr::value(payout_id));
    }
    if let Some(completed_at) = transition.completed_at {
        update = update.col_expr(
            operation::Column::CompletedAt,
            Expr::value(Some(completed_at)),
        );
    }
    if transition.clear_lease {
        update = update
            .col_expr(
                operation::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                operation::Column::LeaseExpiresAt,
                Expr::value(Option::<chrono::DateTime<chrono::FixedOffset>>::None),
            );
    }

    let result = update.exec(service.database()).await?;
    if result.rows_affected != 1 {
        return Err(MarketplacePayoutError::OperationInProgress(model.id));
    }
    load_operation(service, model.tenant_id, model.id).await
}

async fn mark_transfer_executing(
    service: &MarketplacePayoutService,
    model: operation_transfer::Model,
) -> MarketplacePayoutResult<operation_transfer::Model> {
    let next_attempt = model.attempt_count.checked_add(1).ok_or_else(|| {
        MarketplacePayoutError::Validation("payout transfer attempt count overflow".to_string())
    })?;
    let next_revision = model.revision.checked_add(1).ok_or_else(|| {
        MarketplacePayoutError::Validation("payout transfer revision overflow".to_string())
    })?;
    let mut active = model.into_active_model();
    active.status = Set(MarketplacePayoutOperationTransferStatus::Executing);
    active.attempt_count = Set(next_attempt);
    active.revision = Set(next_revision);
    active.last_error_code = Set(None);
    active.completed_at = Set(None);
    active.updated_at = Set(Utc::now().fixed_offset());
    active.update(service.database()).await.map_err(Into::into)
}

async fn mark_transfer_posted(
    service: &MarketplacePayoutService,
    model: operation_transfer::Model,
    payload: PersistedTransferPayload,
    response: &MarketplaceSellerBalanceTransferResponse,
) -> MarketplacePayoutResult<operation_transfer::Model> {
    let next_revision = model.revision.checked_add(1).ok_or_else(|| {
        MarketplacePayoutError::Validation("payout transfer revision overflow".to_string())
    })?;
    let operation_id = model.operation_id;
    let now = Utc::now().fixed_offset();
    let mut active = model.into_active_model();
    active.status = Set(MarketplacePayoutOperationTransferStatus::Posted);
    active.request_json = Set(serde_json::to_value(payload)
        .map_err(|_| MarketplacePayoutError::OperationCorrupt(operation_id))?);
    active.ledger_transfer_id = Set(Some(response.id));
    active.ledger_transaction_id = Set(Some(response.transaction_id));
    active.revision = Set(next_revision);
    active.last_error_code = Set(None);
    active.updated_at = Set(now.clone());
    active.completed_at = Set(Some(now));
    active.update(service.database()).await.map_err(Into::into)
}

async fn mark_transfer_error(
    service: &MarketplacePayoutService,
    model: operation_transfer::Model,
    error: &MarketplacePayoutError,
) -> MarketplacePayoutResult<operation_transfer::Model> {
    let next_revision = model.revision.checked_add(1).ok_or_else(|| {
        MarketplacePayoutError::Validation("payout transfer revision overflow".to_string())
    })?;
    let retryable = payout_error_retryable(error);
    let now = Utc::now().fixed_offset();
    let mut active = model.into_active_model();
    active.status = Set(if retryable {
        MarketplacePayoutOperationTransferStatus::RetryableError
    } else {
        MarketplacePayoutOperationTransferStatus::Failed
    });
    active.revision = Set(next_revision);
    active.last_error_code = Set(Some(payout_error_code(error)));
    active.updated_at = Set(now.clone());
    active.completed_at = Set((!retryable).then_some(now));
    active.update(service.database()).await.map_err(Into::into)
}

async fn mark_transfer_reconciliation(
    service: &MarketplacePayoutService,
    model: operation_transfer::Model,
    error_code: &str,
) -> MarketplacePayoutResult<operation_transfer::Model> {
    let next_revision = model.revision.checked_add(1).ok_or_else(|| {
        MarketplacePayoutError::Validation("payout transfer revision overflow".to_string())
    })?;
    let mut active = model.into_active_model();
    active.status = Set(MarketplacePayoutOperationTransferStatus::ReconciliationRequired);
    active.revision = Set(next_revision);
    active.last_error_code = Set(Some(error_code.to_string()));
    active.completed_at = Set(None);
    active.updated_at = Set(Utc::now().fixed_offset());
    active.update(service.database()).await.map_err(Into::into)
}

async fn mark_hold_compensated(
    service: &MarketplacePayoutService,
    model: operation_transfer::Model,
) -> MarketplacePayoutResult<operation_transfer::Model> {
    let next_revision = model.revision.checked_add(1).ok_or_else(|| {
        MarketplacePayoutError::Validation("payout transfer revision overflow".to_string())
    })?;
    let mut active = model.into_active_model();
    active.status = Set(MarketplacePayoutOperationTransferStatus::Compensated);
    active.revision = Set(next_revision);
    active.last_error_code = Set(None);
    active.updated_at = Set(Utc::now().fixed_offset());
    active.update(service.database()).await.map_err(Into::into)
}

fn parse_operation_snapshot(
    model: &operation::Model,
) -> MarketplacePayoutResult<PayoutOperationRequestSnapshot> {
    let snapshot =
        serde_json::from_value::<PayoutOperationRequestSnapshot>(model.request_json.clone())
            .map_err(|_| MarketplacePayoutError::OperationCorrupt(model.id))?;
    let input_entry_ids = snapshot
        .input
        .ledger_entry_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let snapshot_entry_ids = snapshot
        .entries
        .iter()
        .map(|entry| entry.ledger_entry_id)
        .collect::<BTreeSet<_>>();
    if snapshot.version != OPERATION_REQUEST_VERSION
        || snapshot.input.seller_id != model.seller_id
        || snapshot.input.currency_code != model.currency_code
        || snapshot.entries.is_empty()
        || snapshot.entries.iter().any(|entry| {
            entry.ledger_entry_id.is_nil() || entry.order_id.is_nil() || entry.amount <= 0
        })
        || input_entry_ids != snapshot_entry_ids
    {
        return Err(MarketplacePayoutError::OperationCorrupt(model.id));
    }
    Ok(snapshot)
}

fn parse_transfer_payload(
    model: &operation_transfer::Model,
) -> MarketplacePayoutResult<PersistedTransferPayload> {
    let payload = serde_json::from_value::<PersistedTransferPayload>(model.request_json.clone())
        .map_err(|_| MarketplacePayoutError::OperationCorrupt(model.operation_id))?;
    if payload.version != TRANSFER_PAYLOAD_VERSION
        || payload.request.source_id != model.id
        || payload.request.kind.as_str() != model.transfer_kind.as_str()
        || payload.request.currency_code.trim().to_ascii_uppercase()
            != payload.request.currency_code
        || payload.request.lines.is_empty()
    {
        return Err(MarketplacePayoutError::OperationCorrupt(model.operation_id));
    }
    Ok(payload)
}

fn transfer_response_matches(
    model: &operation_transfer::Model,
    request: &PostMarketplaceSellerBalanceTransferInput,
    response: &MarketplaceSellerBalanceTransferResponse,
) -> bool {
    if response.kind != request.kind
        || response.source_id != request.source_id
        || response.seller_id != request.seller_id
        || response.currency_code != request.currency_code
        || response.total_amount != model.total_amount
        || response.transaction.order_id != model.order_id
        || response.lines.len() != request.lines.len()
    {
        return false;
    }
    let request_lines = request
        .lines
        .iter()
        .map(|line| (line.reference_entry_id, line.amount))
        .collect::<BTreeMap<_, _>>();
    let response_lines = response
        .lines
        .iter()
        .map(|line| (line.reference_entry_id, line.amount))
        .collect::<BTreeMap<_, _>>();
    request_lines == response_lines
}

fn ensure_operation_identity(
    model: &operation::Model,
    expected_request_hash: &str,
) -> MarketplacePayoutResult<()> {
    if model.request_hash != expected_request_hash {
        return Err(MarketplacePayoutError::IdempotencyConflict);
    }
    Ok(())
}

fn transfer_idempotency_key(
    operation_id: Uuid,
    kind: MarketplacePayoutOperationTransferKind,
    child_id: Uuid,
) -> String {
    format!(
        "marketplace-payout:{operation_id}:{}:{child_id}:v1",
        kind.as_str()
    )
}

fn json_hash<T: Serialize>(value: &T) -> MarketplacePayoutResult<String> {
    let encoded = serde_json::to_vec(value).map_err(|_| {
        MarketplacePayoutError::Validation(
            "payout transfer request could not be hashed".to_string(),
        )
    })?;
    Ok(hex::encode(Sha256::digest(encoded)))
}

fn map_ledger_port_error(error: rustok_api::PortError) -> MarketplacePayoutError {
    MarketplacePayoutError::LedgerBoundary {
        code: error.code,
        message: error.message,
        retryable: error.retryable,
    }
}

fn payout_error_code(error: &MarketplacePayoutError) -> String {
    match error {
        MarketplacePayoutError::LedgerBoundary { code, .. } => code.clone(),
        MarketplacePayoutError::LedgerWriterNotConfigured => {
            "marketplace_payout.ledger_writer_not_configured".to_string()
        }
        MarketplacePayoutError::LedgerEntryNotFound(_) => {
            "marketplace_payout.ledger_entry_not_found".to_string()
        }
        MarketplacePayoutError::LedgerEntryAlreadyAssigned(_) => {
            "marketplace_payout.ledger_entry_already_assigned".to_string()
        }
        MarketplacePayoutError::IdempotencyConflict => {
            "marketplace_payout.idempotency_conflict".to_string()
        }
        MarketplacePayoutError::OperationInProgress(_) => {
            "marketplace_payout.operation_in_progress".to_string()
        }
        MarketplacePayoutError::OperationFailed { code, .. } => code
            .clone()
            .unwrap_or_else(|| "marketplace_payout.operation_failed".to_string()),
        MarketplacePayoutError::CompensationRequired(_) => {
            "marketplace_payout.compensation_required".to_string()
        }
        MarketplacePayoutError::ReconciliationRequired(_) => {
            "marketplace_payout.reconciliation_required".to_string()
        }
        MarketplacePayoutError::OperationCorrupt(_) => {
            "marketplace_payout.operation_corrupt".to_string()
        }
        MarketplacePayoutError::ProviderConfiguration { .. } => {
            "marketplace_payout.provider_configuration".to_string()
        }
        MarketplacePayoutError::ProviderUnavailable { .. } => {
            "marketplace_payout.provider_unavailable".to_string()
        }
        MarketplacePayoutError::ProviderRejected { .. } => {
            "marketplace_payout.provider_rejected".to_string()
        }
        MarketplacePayoutError::ProviderInvalidResponse { .. } => {
            "marketplace_payout.provider_invalid_response".to_string()
        }
        MarketplacePayoutError::ProviderOutcomeUnknown { .. } => {
            "marketplace_payout.provider_outcome_unknown".to_string()
        }
        MarketplacePayoutError::PayoutNotFound(_) => "marketplace_payout.not_found".to_string(),
        MarketplacePayoutError::ReceiptCorrupt => "marketplace_payout.receipt_corrupt".to_string(),
        MarketplacePayoutError::Validation(_) => "marketplace_payout.validation".to_string(),
        MarketplacePayoutError::Database(_) => "marketplace_payout.storage_unavailable".to_string(),
    }
}

fn payout_schedule_outcome_is_ambiguous(error: &MarketplacePayoutError) -> bool {
    matches!(
        error,
        MarketplacePayoutError::Database(_)
            | MarketplacePayoutError::ReceiptCorrupt
            | MarketplacePayoutError::IdempotencyConflict
    )
}

fn payout_error_retryable(error: &MarketplacePayoutError) -> bool {
    match error {
        MarketplacePayoutError::LedgerBoundary { retryable, .. } => *retryable,
        MarketplacePayoutError::OperationInProgress(_)
        | MarketplacePayoutError::CompensationRequired(_)
        | MarketplacePayoutError::Database(_) => true,
        _ => false,
    }
}

fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    ) || error
        .to_string()
        .to_ascii_lowercase()
        .contains("unique constraint")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_idempotency_keys_are_stable_and_kind_scoped() {
        let operation_id = Uuid::new_v4();
        let child_id = Uuid::new_v4();
        let first = transfer_idempotency_key(
            operation_id,
            MarketplacePayoutOperationTransferKind::ReserveHold,
            child_id,
        );
        let replay = transfer_idempotency_key(
            operation_id,
            MarketplacePayoutOperationTransferKind::ReserveHold,
            child_id,
        );
        let release = transfer_idempotency_key(
            operation_id,
            MarketplacePayoutOperationTransferKind::ReserveRelease,
            child_id,
        );
        assert_eq!(first, replay);
        assert_ne!(first, release);
    }
}
