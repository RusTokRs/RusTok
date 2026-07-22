use chrono::{Duration as ChronoDuration, Utc};
use rustok_core::generate_id;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    entities::provider_operation::{
        self, MarketplacePayoutProviderOperationKind, MarketplacePayoutProviderOperationStatus,
    },
    MarketplacePayoutError, MarketplacePayoutResult,
};

const PROVIDER_OPERATION_LEASE_SECONDS: i64 = 300;
const MAX_PROVIDER_IDENTITY_LENGTH: usize = 191;
const MAX_ERROR_CODE_LENGTH: usize = 120;

#[derive(Clone, Debug)]
pub struct BeginMarketplacePayoutProviderOperation {
    pub tenant_id: Uuid,
    pub payout_id: Uuid,
    pub operation: MarketplacePayoutProviderOperationKind,
    pub provider_id: String,
    pub idempotency_key: String,
    pub request_hash: String,
    pub request_json: Value,
}

#[derive(Clone)]
pub struct MarketplacePayoutProviderOperationJournal {
    db: DatabaseConnection,
}

impl MarketplacePayoutProviderOperationJournal {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn database(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn begin(
        &self,
        input: BeginMarketplacePayoutProviderOperation,
    ) -> MarketplacePayoutResult<provider_operation::Model> {
        let input = normalize_begin_input(input)?;

        if let Some(existing) = self
            .find_by_key(
                input.tenant_id,
                input.provider_id.as_str(),
                input.idempotency_key.as_str(),
            )
            .await?
        {
            ensure_same_request(&existing, &input)?;
            return Ok(existing);
        }
        if let Some(existing) = self
            .find_by_payout_kind(input.tenant_id, input.payout_id, input.operation)
            .await?
        {
            ensure_same_request(&existing, &input)?;
            return Ok(existing);
        }

        let now = Utc::now().fixed_offset();
        let insert = provider_operation::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(input.tenant_id),
            payout_id: Set(input.payout_id),
            operation: Set(input.operation),
            provider_id: Set(input.provider_id.clone()),
            idempotency_key: Set(input.idempotency_key.clone()),
            request_hash: Set(input.request_hash.clone()),
            request_json: Set(input.request_json.clone()),
            status: Set(MarketplacePayoutProviderOperationStatus::Pending),
            provider_reference: Set(None),
            provider_result_json: Set(None),
            attempt_count: Set(0),
            revision: Set(0),
            lease_owner: Set(None),
            lease_expires_at: Set(None),
            last_error_code: Set(None),
            created_at: Set(now.clone()),
            updated_at: Set(now),
            provider_completed_at: Set(None),
            committed_at: Set(None),
        }
        .insert(&self.db)
        .await;

        match insert {
            Ok(model) => Ok(model),
            Err(error) if is_unique_constraint(&error) => {
                let existing = if let Some(existing) = self
                    .find_by_key(
                        input.tenant_id,
                        input.provider_id.as_str(),
                        input.idempotency_key.as_str(),
                    )
                    .await?
                {
                    Some(existing)
                } else {
                    self.find_by_payout_kind(input.tenant_id, input.payout_id, input.operation)
                        .await?
                };
                let existing = existing.ok_or(error)?;
                ensure_same_request(&existing, &input)?;
                Ok(existing)
            }
            Err(error) => Err(error.into()),
        }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
    ) -> MarketplacePayoutResult<provider_operation::Model> {
        provider_operation::Entity::find()
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::Id.eq(operation_id))
            .one(&self.db)
            .await?
            .ok_or(MarketplacePayoutError::OperationCorrupt(operation_id))
    }

    pub async fn find_by_key(
        &self,
        tenant_id: Uuid,
        provider_id: &str,
        idempotency_key: &str,
    ) -> MarketplacePayoutResult<Option<provider_operation::Model>> {
        provider_operation::Entity::find()
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::ProviderId.eq(provider_id))
            .filter(provider_operation::Column::IdempotencyKey.eq(idempotency_key))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn find_by_payout_kind(
        &self,
        tenant_id: Uuid,
        payout_id: Uuid,
        operation: MarketplacePayoutProviderOperationKind,
    ) -> MarketplacePayoutResult<Option<provider_operation::Model>> {
        provider_operation::Entity::find()
            .filter(provider_operation::Column::TenantId.eq(tenant_id))
            .filter(provider_operation::Column::PayoutId.eq(payout_id))
            .filter(provider_operation::Column::Operation.eq(operation))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn claim_execution(
        &self,
        model: provider_operation::Model,
    ) -> MarketplacePayoutResult<provider_operation::Model> {
        if !matches!(
            model.status,
            MarketplacePayoutProviderOperationStatus::Pending
                | MarketplacePayoutProviderOperationStatus::RetryableError
        ) {
            return Err(MarketplacePayoutError::OperationInProgress(model.id));
        }

        let next_attempt = model.attempt_count.checked_add(1).ok_or_else(|| {
            MarketplacePayoutError::Validation(
                "payout provider operation attempt count overflow".to_string(),
            )
        })?;
        let next_revision = model.revision.checked_add(1).ok_or_else(|| {
            MarketplacePayoutError::Validation(
                "payout provider operation revision overflow".to_string(),
            )
        })?;
        let now = Utc::now().fixed_offset();
        let lease_owner = format!("marketplace-payout-provider:{}", Uuid::new_v4());
        let lease_expires_at =
            now.clone() + ChronoDuration::seconds(PROVIDER_OPERATION_LEASE_SECONDS);
        let result = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(MarketplacePayoutProviderOperationStatus::Executing.as_str()),
            )
            .col_expr(
                provider_operation::Column::AttemptCount,
                Expr::value(next_attempt),
            )
            .col_expr(
                provider_operation::Column::Revision,
                Expr::value(next_revision),
            )
            .col_expr(
                provider_operation::Column::LeaseOwner,
                Expr::value(lease_owner),
            )
            .col_expr(
                provider_operation::Column::LeaseExpiresAt,
                Expr::value(lease_expires_at),
            )
            .col_expr(
                provider_operation::Column::LastErrorCode,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::value(now.clone()),
            )
            .filter(provider_operation::Column::TenantId.eq(model.tenant_id))
            .filter(provider_operation::Column::Id.eq(model.id))
            .filter(provider_operation::Column::Status.eq(model.status))
            .filter(provider_operation::Column::Revision.eq(model.revision))
            .filter(provider_operation::Column::LeaseOwner.is_null())
            .filter(provider_operation::Column::LeaseExpiresAt.is_null())
            .exec(&self.db)
            .await?;
        if result.rows_affected != 1 {
            return Err(MarketplacePayoutError::OperationInProgress(model.id));
        }
        self.get(model.tenant_id, model.id).await
    }

    pub async fn mark_expired_execution_reconciliation(
        &self,
        model: provider_operation::Model,
    ) -> MarketplacePayoutResult<provider_operation::Model> {
        let now = Utc::now().fixed_offset();
        if model.status != MarketplacePayoutProviderOperationStatus::Executing
            || model
                .lease_expires_at
                .as_ref()
                .is_some_and(|expires_at| expires_at > &now)
        {
            return Err(MarketplacePayoutError::OperationInProgress(model.id));
        }

        let result = self
            .transition(
                &model,
                MarketplacePayoutProviderOperationStatus::ReconciliationRequired,
                Some("marketplace_payout.provider_outcome_unknown"),
                false,
            )
            .await?;
        Ok(result)
    }

    pub async fn mark_provider_succeeded(
        &self,
        model: provider_operation::Model,
        provider_reference: Option<String>,
        provider_result: Value,
    ) -> MarketplacePayoutResult<provider_operation::Model> {
        ensure_transition(
            model.status,
            MarketplacePayoutProviderOperationStatus::ProviderSucceeded,
        )?;
        let next_revision = next_revision(&model)?;
        let now = Utc::now().fixed_offset();
        let result = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(MarketplacePayoutProviderOperationStatus::ProviderSucceeded.as_str()),
            )
            .col_expr(
                provider_operation::Column::ProviderReference,
                Expr::value(provider_reference),
            )
            .col_expr(
                provider_operation::Column::ProviderResultJson,
                Expr::value(Some(provider_result)),
            )
            .col_expr(
                provider_operation::Column::Revision,
                Expr::value(next_revision),
            )
            .col_expr(
                provider_operation::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                provider_operation::Column::LeaseExpiresAt,
                Expr::value(Option::<chrono::DateTime<chrono::FixedOffset>>::None),
            )
            .col_expr(
                provider_operation::Column::LastErrorCode,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                provider_operation::Column::ProviderCompletedAt,
                Expr::value(Some(now.clone())),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::value(now.clone()),
            )
            .filter(provider_operation::Column::TenantId.eq(model.tenant_id))
            .filter(provider_operation::Column::Id.eq(model.id))
            .filter(
                provider_operation::Column::Status
                    .eq(MarketplacePayoutProviderOperationStatus::Executing),
            )
            .filter(provider_operation::Column::Revision.eq(model.revision))
            .filter(provider_operation::Column::LeaseOwner.eq(model.lease_owner.clone()))
            .exec(&self.db)
            .await?;
        if result.rows_affected != 1 {
            let current = self.get(model.tenant_id, model.id).await?;
            if matches!(
                current.status,
                MarketplacePayoutProviderOperationStatus::ProviderSucceeded
                    | MarketplacePayoutProviderOperationStatus::Committed
            ) {
                return Ok(current);
            }
            let _ = self
                .mark_current_reconciliation(
                    current,
                    "marketplace_payout.provider_success_checkpoint_conflict",
                )
                .await;
            return Err(MarketplacePayoutError::ReconciliationRequired(model.id));
        }
        self.get(model.tenant_id, model.id).await
    }

    pub async fn mark_provider_failed(
        &self,
        model: provider_operation::Model,
        error_code: &str,
    ) -> MarketplacePayoutResult<provider_operation::Model> {
        self.transition(
            &model,
            MarketplacePayoutProviderOperationStatus::ProviderFailed,
            Some(error_code),
            true,
        )
        .await
    }

    pub async fn mark_provider_failed_result(
        &self,
        model: provider_operation::Model,
        provider_reference: Option<String>,
        provider_result: Value,
        error_code: &str,
    ) -> MarketplacePayoutResult<provider_operation::Model> {
        ensure_transition(
            model.status,
            MarketplacePayoutProviderOperationStatus::ProviderFailed,
        )?;
        let error_code = normalize_error_code(error_code)?;
        let next_revision = next_revision(&model)?;
        let now = Utc::now().fixed_offset();
        let result = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(MarketplacePayoutProviderOperationStatus::ProviderFailed.as_str()),
            )
            .col_expr(
                provider_operation::Column::ProviderReference,
                Expr::value(provider_reference),
            )
            .col_expr(
                provider_operation::Column::ProviderResultJson,
                Expr::value(Some(provider_result)),
            )
            .col_expr(
                provider_operation::Column::Revision,
                Expr::value(next_revision),
            )
            .col_expr(
                provider_operation::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                provider_operation::Column::LeaseExpiresAt,
                Expr::value(Option::<chrono::DateTime<chrono::FixedOffset>>::None),
            )
            .col_expr(
                provider_operation::Column::LastErrorCode,
                Expr::value(Some(error_code)),
            )
            .col_expr(
                provider_operation::Column::ProviderCompletedAt,
                Expr::value(Some(now.clone())),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::value(now.clone()),
            )
            .filter(provider_operation::Column::TenantId.eq(model.tenant_id))
            .filter(provider_operation::Column::Id.eq(model.id))
            .filter(
                provider_operation::Column::Status
                    .eq(MarketplacePayoutProviderOperationStatus::Executing),
            )
            .filter(provider_operation::Column::Revision.eq(model.revision))
            .filter(provider_operation::Column::LeaseOwner.eq(model.lease_owner.clone()))
            .exec(&self.db)
            .await?;
        if result.rows_affected != 1 {
            let current = self.get(model.tenant_id, model.id).await?;
            if current.status == MarketplacePayoutProviderOperationStatus::ProviderFailed {
                return Ok(current);
            }
            let _ = self
                .mark_current_reconciliation(
                    current,
                    "marketplace_payout.provider_failure_checkpoint_conflict",
                )
                .await;
            return Err(MarketplacePayoutError::ReconciliationRequired(model.id));
        }
        self.get(model.tenant_id, model.id).await
    }

    pub async fn mark_reconciliation_result(
        &self,
        model: provider_operation::Model,
        provider_reference: Option<String>,
        provider_result: Value,
        error_code: &str,
    ) -> MarketplacePayoutResult<provider_operation::Model> {
        ensure_transition(
            model.status,
            MarketplacePayoutProviderOperationStatus::ReconciliationRequired,
        )?;
        let error_code = normalize_error_code(error_code)?;
        let next_revision = next_revision(&model)?;
        let now = Utc::now().fixed_offset();
        let result = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(
                    MarketplacePayoutProviderOperationStatus::ReconciliationRequired.as_str(),
                ),
            )
            .col_expr(
                provider_operation::Column::ProviderReference,
                Expr::value(provider_reference),
            )
            .col_expr(
                provider_operation::Column::ProviderResultJson,
                Expr::value(Some(provider_result)),
            )
            .col_expr(
                provider_operation::Column::Revision,
                Expr::value(next_revision),
            )
            .col_expr(
                provider_operation::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                provider_operation::Column::LeaseExpiresAt,
                Expr::value(Option::<chrono::DateTime<chrono::FixedOffset>>::None),
            )
            .col_expr(
                provider_operation::Column::LastErrorCode,
                Expr::value(Some(error_code)),
            )
            .col_expr(
                provider_operation::Column::ProviderCompletedAt,
                Expr::value(Some(now.clone())),
            )
            .col_expr(provider_operation::Column::UpdatedAt, Expr::value(now))
            .filter(provider_operation::Column::TenantId.eq(model.tenant_id))
            .filter(provider_operation::Column::Id.eq(model.id))
            .filter(
                provider_operation::Column::Status
                    .eq(MarketplacePayoutProviderOperationStatus::Executing),
            )
            .filter(provider_operation::Column::Revision.eq(model.revision))
            .filter(provider_operation::Column::LeaseOwner.eq(model.lease_owner.clone()))
            .exec(&self.db)
            .await?;
        if result.rows_affected != 1 {
            let current = self.get(model.tenant_id, model.id).await?;
            if current.status == MarketplacePayoutProviderOperationStatus::ReconciliationRequired {
                return Ok(current);
            }
            return Err(MarketplacePayoutError::ReconciliationRequired(model.id));
        }
        self.get(model.tenant_id, model.id).await
    }

    pub async fn mark_retryable_error(
        &self,
        model: provider_operation::Model,
        error_code: &str,
    ) -> MarketplacePayoutResult<provider_operation::Model> {
        self.transition(
            &model,
            MarketplacePayoutProviderOperationStatus::RetryableError,
            Some(error_code),
            false,
        )
        .await
    }

    pub async fn mark_reconciliation_required(
        &self,
        model: provider_operation::Model,
        error_code: &str,
    ) -> MarketplacePayoutResult<provider_operation::Model> {
        self.transition(
            &model,
            MarketplacePayoutProviderOperationStatus::ReconciliationRequired,
            Some(error_code),
            false,
        )
        .await
    }

    async fn mark_current_reconciliation(
        &self,
        model: provider_operation::Model,
        error_code: &str,
    ) -> MarketplacePayoutResult<provider_operation::Model> {
        if model.status == MarketplacePayoutProviderOperationStatus::ReconciliationRequired {
            return Ok(model);
        }
        self.transition(
            &model,
            MarketplacePayoutProviderOperationStatus::ReconciliationRequired,
            Some(error_code),
            false,
        )
        .await
    }

    async fn transition(
        &self,
        model: &provider_operation::Model,
        target: MarketplacePayoutProviderOperationStatus,
        error_code: Option<&str>,
        provider_completed: bool,
    ) -> MarketplacePayoutResult<provider_operation::Model> {
        ensure_transition(model.status, target)?;
        let next_revision = next_revision(model)?;
        let now = Utc::now().fixed_offset();
        let error_code = error_code.map(normalize_error_code).transpose()?;
        let mut update = provider_operation::Entity::update_many()
            .col_expr(
                provider_operation::Column::Status,
                Expr::value(target.as_str()),
            )
            .col_expr(
                provider_operation::Column::Revision,
                Expr::value(next_revision),
            )
            .col_expr(
                provider_operation::Column::LeaseOwner,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                provider_operation::Column::LeaseExpiresAt,
                Expr::value(Option::<chrono::DateTime<chrono::FixedOffset>>::None),
            )
            .col_expr(
                provider_operation::Column::LastErrorCode,
                Expr::value(error_code),
            )
            .col_expr(
                provider_operation::Column::UpdatedAt,
                Expr::value(now.clone()),
            )
            .filter(provider_operation::Column::TenantId.eq(model.tenant_id))
            .filter(provider_operation::Column::Id.eq(model.id))
            .filter(provider_operation::Column::Status.eq(model.status))
            .filter(provider_operation::Column::Revision.eq(model.revision));
        if model.status == MarketplacePayoutProviderOperationStatus::Executing {
            update =
                update.filter(provider_operation::Column::LeaseOwner.eq(model.lease_owner.clone()));
        }
        if provider_completed {
            update = update.col_expr(
                provider_operation::Column::ProviderCompletedAt,
                Expr::value(Some(now)),
            );
        }
        let result = update.exec(&self.db).await?;
        if result.rows_affected != 1 {
            let current = self.get(model.tenant_id, model.id).await?;
            if current.status == target {
                return Ok(current);
            }
            return Err(MarketplacePayoutError::OperationInProgress(model.id));
        }
        self.get(model.tenant_id, model.id).await
    }
}

fn ensure_transition(
    from: MarketplacePayoutProviderOperationStatus,
    to: MarketplacePayoutProviderOperationStatus,
) -> MarketplacePayoutResult<()> {
    let allowed = matches!(
        (from, to),
        (
            MarketplacePayoutProviderOperationStatus::Executing,
            MarketplacePayoutProviderOperationStatus::ProviderSucceeded
        ) | (
            MarketplacePayoutProviderOperationStatus::Executing,
            MarketplacePayoutProviderOperationStatus::ProviderFailed
        ) | (
            MarketplacePayoutProviderOperationStatus::Executing,
            MarketplacePayoutProviderOperationStatus::RetryableError
        ) | (
            MarketplacePayoutProviderOperationStatus::Executing,
            MarketplacePayoutProviderOperationStatus::ReconciliationRequired
        ) | (
            MarketplacePayoutProviderOperationStatus::ProviderSucceeded,
            MarketplacePayoutProviderOperationStatus::ReconciliationRequired
        )
    );
    if allowed {
        Ok(())
    } else {
        Err(MarketplacePayoutError::Validation(format!(
            "invalid payout provider operation transition {} -> {}",
            from.as_str(),
            to.as_str()
        )))
    }
}

fn normalize_begin_input(
    mut input: BeginMarketplacePayoutProviderOperation,
) -> MarketplacePayoutResult<BeginMarketplacePayoutProviderOperation> {
    if input.tenant_id.is_nil() || input.payout_id.is_nil() {
        return Err(MarketplacePayoutError::Validation(
            "payout provider operation requires tenant_id and payout_id".to_string(),
        ));
    }
    input.provider_id = normalize_identity(input.provider_id, "provider_id")?;
    input.idempotency_key = normalize_identity(input.idempotency_key, "idempotency_key")?;
    input.request_hash = input.request_hash.trim().to_ascii_lowercase();
    if input.request_hash.len() != 64
        || !input
            .request_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(MarketplacePayoutError::Validation(
            "payout provider operation request_hash must be 64 hexadecimal characters".to_string(),
        ));
    }
    if !input.request_json.is_object() {
        return Err(MarketplacePayoutError::Validation(
            "payout provider operation request_json must be an object".to_string(),
        ));
    }
    Ok(input)
}

fn ensure_same_request(
    existing: &provider_operation::Model,
    input: &BeginMarketplacePayoutProviderOperation,
) -> MarketplacePayoutResult<()> {
    if existing.tenant_id != input.tenant_id
        || existing.payout_id != input.payout_id
        || existing.operation != input.operation
        || existing.provider_id != input.provider_id
        || existing.idempotency_key != input.idempotency_key
        || existing.request_hash != input.request_hash
        || existing.request_json != input.request_json
    {
        return Err(MarketplacePayoutError::IdempotencyConflict);
    }
    Ok(())
}

fn normalize_identity(value: String, label: &str) -> MarketplacePayoutResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() || value.len() > MAX_PROVIDER_IDENTITY_LENGTH {
        return Err(MarketplacePayoutError::Validation(format!(
            "payout provider operation {label} must contain 1 to {MAX_PROVIDER_IDENTITY_LENGTH} bytes"
        )));
    }
    Ok(value)
}

fn normalize_error_code(value: &str) -> MarketplacePayoutResult<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > MAX_ERROR_CODE_LENGTH {
        return Err(MarketplacePayoutError::Validation(format!(
            "payout provider operation error code must contain 1 to {MAX_ERROR_CODE_LENGTH} bytes"
        )));
    }
    Ok(value.to_string())
}

fn next_revision(model: &provider_operation::Model) -> MarketplacePayoutResult<i64> {
    model.revision.checked_add(1).ok_or_else(|| {
        MarketplacePayoutError::Validation(
            "payout provider operation revision overflow".to_string(),
        )
    })
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
