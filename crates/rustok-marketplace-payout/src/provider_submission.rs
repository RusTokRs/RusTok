use std::sync::Arc;

use chrono::Utc;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    dto::MarketplacePayoutStatus,
    entities::{
        payout,
        provider_operation::{
            MarketplacePayoutProviderOperationKind, MarketplacePayoutProviderOperationStatus,
        },
    },
    provider_operation_journal::{
        BeginMarketplacePayoutProviderOperation, MarketplacePayoutProviderOperationJournal,
    },
    providers::{
        PayoutProviderRegistry, PayoutProviderResult, PayoutProviderTransferStatus,
        SubmitPayoutProviderRequest,
    },
    MarketplacePayoutError, MarketplacePayoutResult,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct JournaledPayoutProviderResult {
    pub operation_id: Uuid,
    pub payout_id: Uuid,
    pub result: PayoutProviderResult,
}

#[derive(Clone)]
pub struct MarketplacePayoutProviderSubmissionService {
    journal: MarketplacePayoutProviderOperationJournal,
    providers: Arc<PayoutProviderRegistry>,
}

impl MarketplacePayoutProviderSubmissionService {
    pub fn new(db: DatabaseConnection, providers: Arc<PayoutProviderRegistry>) -> Self {
        Self {
            journal: MarketplacePayoutProviderOperationJournal::new(db),
            providers,
        }
    }

    pub fn journal(&self) -> &MarketplacePayoutProviderOperationJournal {
        &self.journal
    }

    pub async fn submit(
        &self,
        tenant_id: Uuid,
        payout_id: Uuid,
        provider_id: impl Into<String>,
        idempotency_key: impl Into<String>,
    ) -> MarketplacePayoutResult<JournaledPayoutProviderResult> {
        let provider_id = normalize_identity(provider_id.into(), "provider_id")?;
        let idempotency_key = normalize_identity(idempotency_key.into(), "idempotency_key")?;

        if let Some(existing) = self
            .journal
            .find_by_key(tenant_id, provider_id.as_str(), idempotency_key.as_str())
            .await?
        {
            if existing.payout_id != payout_id
                || existing.operation != MarketplacePayoutProviderOperationKind::Submit
            {
                return Err(MarketplacePayoutError::IdempotencyConflict);
            }
            return self.resume(existing).await;
        }

        let payout = load_schedulable_payout(self.journal.database(), tenant_id, payout_id).await?;
        let request = SubmitPayoutProviderRequest {
            tenant_id,
            payout_id,
            seller_id: payout.seller_id,
            amount: payout.total_amount,
            currency_code: payout.currency_code,
            destination_reference: payout.destination_reference,
            idempotency_key: idempotency_key.clone(),
            metadata: payout.metadata,
        };
        let request_json = serde_json::to_value(&request).map_err(|_| {
            MarketplacePayoutError::Validation(
                "payout provider submit request could not be serialized".to_string(),
            )
        })?;
        let operation = self
            .journal
            .begin(BeginMarketplacePayoutProviderOperation {
                tenant_id,
                payout_id,
                operation: MarketplacePayoutProviderOperationKind::Submit,
                provider_id,
                idempotency_key,
                request_hash: json_hash(&request)?,
                request_json,
            })
            .await?;
        self.resume(operation).await
    }

    async fn resume(
        &self,
        operation: crate::entities::provider_operation::Model,
    ) -> MarketplacePayoutResult<JournaledPayoutProviderResult> {
        match operation.status {
            MarketplacePayoutProviderOperationStatus::ProviderSucceeded
            | MarketplacePayoutProviderOperationStatus::Committed => {
                return persisted_result(&operation);
            }
            MarketplacePayoutProviderOperationStatus::ProviderFailed => {
                return Err(MarketplacePayoutError::OperationFailed {
                    operation_id: operation.id,
                    code: operation.last_error_code,
                });
            }
            MarketplacePayoutProviderOperationStatus::ReconciliationRequired => {
                return Err(MarketplacePayoutError::ReconciliationRequired(operation.id));
            }
            MarketplacePayoutProviderOperationStatus::Executing => {
                let now = Utc::now().fixed_offset();
                if operation
                    .lease_expires_at
                    .as_ref()
                    .is_some_and(|expires_at| expires_at > &now)
                {
                    return Err(MarketplacePayoutError::OperationInProgress(operation.id));
                }
                let reconciled = self
                    .journal
                    .mark_expired_execution_reconciliation(operation)
                    .await?;
                return Err(MarketplacePayoutError::ReconciliationRequired(
                    reconciled.id,
                ));
            }
            MarketplacePayoutProviderOperationStatus::Pending
            | MarketplacePayoutProviderOperationStatus::RetryableError => {}
        }

        let claimed = self.journal.claim_execution(operation).await?;
        let request = parse_submit_request(&claimed)?;
        let provider_id = claimed.provider_id.clone();
        let provider_result = self
            .providers
            .execute_submit(provider_id.as_str(), request)
            .await;

        match provider_result {
            Ok(result) => self.checkpoint_result(claimed, result).await,
            Err(error) => self.checkpoint_failure(claimed, error).await,
        }
    }

    async fn checkpoint_result(
        &self,
        claimed: crate::entities::provider_operation::Model,
        result: PayoutProviderResult,
    ) -> MarketplacePayoutResult<JournaledPayoutProviderResult> {
        match result.status {
            PayoutProviderTransferStatus::Failed | PayoutProviderTransferStatus::Cancelled => {
                return self.checkpoint_confirmed_failure(claimed, result).await;
            }
            PayoutProviderTransferStatus::Unknown => {
                let operation_id = claimed.id;
                let result_json = match serde_json::to_value(&result) {
                    Ok(value) => value,
                    Err(_) => {
                        let _ = self
                            .journal
                            .mark_reconciliation_required(
                                claimed,
                                "marketplace_payout.provider_result_serialization_failed",
                            )
                            .await;
                        return Err(MarketplacePayoutError::ReconciliationRequired(operation_id));
                    }
                };
                self.journal
                    .mark_reconciliation_result(
                        claimed,
                        result.external_reference,
                        result_json,
                        "marketplace_payout.provider_status_unknown",
                    )
                    .await?;
                return Err(MarketplacePayoutError::ReconciliationRequired(operation_id));
            }
            PayoutProviderTransferStatus::Submitted
            | PayoutProviderTransferStatus::Processing
            | PayoutProviderTransferStatus::Paid => {}
        }
        let result_json = match serde_json::to_value(&result) {
            Ok(value) => value,
            Err(_) => {
                let operation_id = claimed.id;
                let _ = self
                    .journal
                    .mark_reconciliation_required(
                        claimed,
                        "marketplace_payout.provider_result_serialization_failed",
                    )
                    .await;
                return Err(MarketplacePayoutError::ReconciliationRequired(operation_id));
            }
        };
        let checkpoint = match self
            .journal
            .mark_provider_succeeded(
                claimed.clone(),
                result.external_reference.clone(),
                result_json,
            )
            .await
        {
            Ok(checkpoint) => checkpoint,
            Err(_) => {
                return Err(MarketplacePayoutError::ProviderOutcomeUnknown {
                    provider_id: claimed.provider_id,
                    operation: claimed.operation.as_str().to_string(),
                });
            }
        };
        let persisted = persisted_result(&checkpoint)?;
        if persisted.result != result {
            let _ = self
                .journal
                .mark_reconciliation_required(
                    checkpoint.clone(),
                    "marketplace_payout.provider_result_checkpoint_mismatch",
                )
                .await;
            return Err(MarketplacePayoutError::ReconciliationRequired(
                checkpoint.id,
            ));
        }
        Ok(persisted)
    }

    async fn checkpoint_confirmed_failure(
        &self,
        claimed: crate::entities::provider_operation::Model,
        result: PayoutProviderResult,
    ) -> MarketplacePayoutResult<JournaledPayoutProviderResult> {
        let operation_id = claimed.id;
        let code = match result.status {
            PayoutProviderTransferStatus::Failed => "marketplace_payout.provider_failed",
            PayoutProviderTransferStatus::Cancelled => "marketplace_payout.provider_cancelled",
            _ => "marketplace_payout.provider_result_invalid",
        };
        let result_json = match serde_json::to_value(&result) {
            Ok(value) => value,
            Err(_) => {
                let _ = self
                    .journal
                    .mark_reconciliation_required(
                        claimed,
                        "marketplace_payout.provider_result_serialization_failed",
                    )
                    .await;
                return Err(MarketplacePayoutError::ReconciliationRequired(operation_id));
            }
        };
        self.journal
            .mark_provider_failed_result(claimed, result.external_reference, result_json, code)
            .await?;
        Err(MarketplacePayoutError::OperationFailed {
            operation_id,
            code: Some(code.to_string()),
        })
    }

    async fn checkpoint_failure(
        &self,
        claimed: crate::entities::provider_operation::Model,
        error: MarketplacePayoutError,
    ) -> MarketplacePayoutResult<JournaledPayoutProviderResult> {
        let code = provider_error_code(&error);
        if provider_error_requires_reconciliation(&error) {
            let operation_id = claimed.id;
            self.journal
                .mark_reconciliation_required(claimed, code.as_str())
                .await
                .map_err(|_| MarketplacePayoutError::ProviderOutcomeUnknown {
                    provider_id: "payout-provider".to_string(),
                    operation: "submit".to_string(),
                })?;
            return Err(MarketplacePayoutError::ReconciliationRequired(operation_id));
        }
        if provider_error_is_retryable_without_effect(&error) {
            self.journal
                .mark_retryable_error(claimed, code.as_str())
                .await?;
            return Err(error);
        }

        self.journal
            .mark_provider_failed(claimed, code.as_str())
            .await?;
        Err(error)
    }
}

async fn load_schedulable_payout(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    payout_id: Uuid,
) -> MarketplacePayoutResult<payout::Model> {
    let payout = payout::Entity::find()
        .filter(payout::Column::TenantId.eq(tenant_id))
        .filter(payout::Column::Id.eq(payout_id))
        .one(db)
        .await?
        .ok_or(MarketplacePayoutError::PayoutNotFound(payout_id))?;
    let status = MarketplacePayoutStatus::parse(payout.status.as_str())
        .ok_or(MarketplacePayoutError::OperationCorrupt(payout_id))?;
    if status != MarketplacePayoutStatus::Scheduled {
        return Err(MarketplacePayoutError::Validation(format!(
            "marketplace payout {payout_id} cannot be submitted from status {}",
            status.as_str()
        )));
    }
    if payout.scheduled_for > Utc::now().fixed_offset() {
        return Err(MarketplacePayoutError::Validation(format!(
            "marketplace payout {payout_id} is not due for submission"
        )));
    }
    if payout.total_amount <= 0 {
        return Err(MarketplacePayoutError::OperationCorrupt(payout_id));
    }
    Ok(payout)
}

fn parse_submit_request(
    operation: &crate::entities::provider_operation::Model,
) -> MarketplacePayoutResult<SubmitPayoutProviderRequest> {
    if operation.operation != MarketplacePayoutProviderOperationKind::Submit {
        return Err(MarketplacePayoutError::OperationCorrupt(operation.id));
    }
    let request =
        serde_json::from_value::<SubmitPayoutProviderRequest>(operation.request_json.clone())
            .map_err(|_| MarketplacePayoutError::OperationCorrupt(operation.id))?;
    if request.tenant_id != operation.tenant_id
        || request.payout_id != operation.payout_id
        || request.idempotency_key != operation.idempotency_key
        || json_hash(&request)? != operation.request_hash
    {
        return Err(MarketplacePayoutError::OperationCorrupt(operation.id));
    }
    Ok(request)
}

fn persisted_result(
    operation: &crate::entities::provider_operation::Model,
) -> MarketplacePayoutResult<JournaledPayoutProviderResult> {
    let value = operation
        .provider_result_json
        .clone()
        .ok_or(MarketplacePayoutError::OperationCorrupt(operation.id))?;
    let result = serde_json::from_value::<PayoutProviderResult>(value)
        .map_err(|_| MarketplacePayoutError::OperationCorrupt(operation.id))?;
    if result.provider_id != operation.provider_id
        || result.external_reference != operation.provider_reference
        || matches!(
            result.status,
            PayoutProviderTransferStatus::Failed
                | PayoutProviderTransferStatus::Cancelled
                | PayoutProviderTransferStatus::Unknown
        )
    {
        return Err(MarketplacePayoutError::OperationCorrupt(operation.id));
    }
    Ok(JournaledPayoutProviderResult {
        operation_id: operation.id,
        payout_id: operation.payout_id,
        result,
    })
}

fn json_hash<T: Serialize>(value: &T) -> MarketplacePayoutResult<String> {
    let encoded = serde_json::to_vec(value).map_err(|_| {
        MarketplacePayoutError::Validation(
            "payout provider request could not be hashed".to_string(),
        )
    })?;
    Ok(hex::encode(Sha256::digest(encoded)))
}

fn normalize_identity(value: String, label: &str) -> MarketplacePayoutResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() || value.len() > 191 {
        return Err(MarketplacePayoutError::Validation(format!(
            "payout provider {label} must contain 1 to 191 bytes"
        )));
    }
    Ok(value)
}

fn provider_error_code(error: &MarketplacePayoutError) -> String {
    match error {
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
        MarketplacePayoutError::Validation(_) => "marketplace_payout.validation".to_string(),
        MarketplacePayoutError::Database(_) => "marketplace_payout.storage_unavailable".to_string(),
        _ => "marketplace_payout.provider_submit_unclassified".to_string(),
    }
}

fn provider_error_requires_reconciliation(error: &MarketplacePayoutError) -> bool {
    !matches!(
        error,
        MarketplacePayoutError::ProviderConfiguration { .. }
            | MarketplacePayoutError::ProviderUnavailable { .. }
            | MarketplacePayoutError::ProviderRejected { .. }
            | MarketplacePayoutError::Validation(_)
    )
}

fn provider_error_is_retryable_without_effect(error: &MarketplacePayoutError) -> bool {
    matches!(
        error,
        MarketplacePayoutError::ProviderConfiguration { .. }
            | MarketplacePayoutError::ProviderUnavailable { .. }
    )
}
