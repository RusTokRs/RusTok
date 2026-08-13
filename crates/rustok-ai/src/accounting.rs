use chrono::{DateTime, Duration, Utc};
use rustok_api::{PortError, manifest_hash::hash_manifest};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, TransactionTrait,
    sea_query::{Expr, ExprTrait},
};
use serde::Serialize;
use std::collections::BTreeSet;
use uuid::Uuid;

use crate::entities::{
    ai_structured_attempts, ai_structured_budgets, ai_structured_executions,
    ai_structured_provider_policies, ai_structured_reservations, ai_structured_results,
};
use crate::structured_result::SealedStructuredResult;
use crate::{AiStructuredTaskEstimate, AiTaskDataClassification, ProviderUsage};

const TOKENS_PER_PRICE_UNIT: u128 = 1_000_000;
const RECOVERY_ERROR_CODE: &str = "ai.structured.execution_lease_expired";
const RESULT_HANDOFF_ERROR_CODE: &str = "ai.structured.result_handoff_incomplete";
const RECOVERY_LEASE_SECONDS: i64 = 30;
pub(crate) const PROVIDER_EGRESS_CLASSIFICATION_DENIED_CODE: &str =
    "ai.structured.provider_egress_classification_denied";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BudgetPolicy {
    pub tenant_id: Uuid,
    pub currency_code: String,
    pub limit_minor_units: u64,
    pub max_concurrent: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderPolicy {
    pub tenant_id: Uuid,
    pub provider_profile_id: Uuid,
    pub currency_code: String,
    pub input_cost_per_million_minor: u64,
    pub output_cost_per_million_minor: u64,
    pub max_concurrent: u32,
    pub is_active: bool,
    pub allowed_classifications: Vec<AiTaskDataClassification>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reservation {
    pub id: Uuid,
    pub execution_id: Uuid,
    pub currency_code: String,
    pub reserved_minor_units: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct Attempt {
    pub model: ai_structured_attempts::Model,
}

#[derive(Debug, Clone)]
pub(crate) enum AttemptOutcome {
    Failed {
        usage: Option<ProviderUsage>,
        error_code: String,
        retryable: bool,
        retry_after_ms: Option<u64>,
    },
    Cancelled {
        usage: Option<ProviderUsage>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AttemptCost {
    pub cost_minor_units: u64,
    pub currency_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TerminalOutcome {
    Failed {
        error_code: String,
        retryable: bool,
        retry_after_ms: Option<u64>,
    },
    Cancelled,
}

#[derive(Clone)]
pub(crate) struct StructuredAccounting {
    database: DatabaseConnection,
}

#[derive(Serialize)]
struct PriceSnapshot<'a> {
    provider_profile_id: Uuid,
    allowed_classifications: &'a [AiTaskDataClassification],
    is_active: bool,
    currency_code: &'a str,
    input_cost_per_million_minor: i64,
    output_cost_per_million_minor: i64,
    revision: i64,
}

#[derive(Serialize)]
struct EstimatePriceSnapshot {
    provider_profile_id: Uuid,
    price_snapshot_digest: String,
}

impl StructuredAccounting {
    pub(crate) fn new(database: DatabaseConnection) -> Self {
        Self { database }
    }

    pub(crate) async fn put_budget(&self, policy: BudgetPolicy) -> Result<(), PortError> {
        validate_currency(&policy.currency_code)?;
        if policy.max_concurrent == 0 {
            return Err(PortError::validation(
                "ai.structured.budget_concurrency_invalid",
                "structured execution tenant concurrency must be positive",
            ));
        }
        let limit = to_i64(policy.limit_minor_units)?;
        let max_concurrent =
            i32::try_from(policy.max_concurrent).map_err(|_| accounting_limit())?;
        let now = Utc::now();
        let existing = ai_structured_budgets::Entity::find()
            .filter(ai_structured_budgets::Column::TenantId.eq(policy.tenant_id))
            .filter(ai_structured_budgets::Column::CurrencyCode.eq(policy.currency_code.as_str()))
            .one(&self.database)
            .await
            .map_err(|_| accounting_unavailable())?;
        match existing {
            Some(existing) => {
                if existing.reserved_minor_units + existing.committed_minor_units > limit
                    || existing.in_flight > max_concurrent
                {
                    return Err(PortError::conflict(
                        "ai.structured.budget_policy_conflict",
                        "structured execution budget cannot be reduced below current commitments",
                    ));
                }
                let next_revision = existing
                    .revision
                    .checked_add(1)
                    .ok_or_else(accounting_limit)?;
                let mut active: ai_structured_budgets::ActiveModel = existing.into();
                active.limit_minor_units = Set(limit);
                active.max_concurrent = Set(max_concurrent);
                active.revision = Set(next_revision);
                active.updated_at = Set(now.into());
                active
                    .update(&self.database)
                    .await
                    .map_err(|_| accounting_unavailable())?;
            }
            None => {
                ai_structured_budgets::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(policy.tenant_id),
                    currency_code: Set(policy.currency_code),
                    limit_minor_units: Set(limit),
                    reserved_minor_units: Set(0),
                    committed_minor_units: Set(0),
                    max_concurrent: Set(max_concurrent),
                    in_flight: Set(0),
                    revision: Set(1),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&self.database)
                .await
                .map_err(|_| accounting_unavailable())?;
            }
        }
        Ok(())
    }

    pub(crate) async fn put_provider_policy(
        &self,
        policy: ProviderPolicy,
    ) -> Result<(), PortError> {
        validate_currency(&policy.currency_code)?;
        let allowed_classifications =
            canonical_allowed_classifications(&policy.allowed_classifications)?;
        let allowed_classifications_json =
            serde_json::to_value(&allowed_classifications).map_err(|_| accounting_invariant())?;
        let policy_is_active = policy.is_active;
        if policy.max_concurrent == 0 {
            return Err(PortError::validation(
                "ai.structured.provider_concurrency_invalid",
                "structured execution provider concurrency must be positive",
            ));
        }
        let input_price = to_i64(policy.input_cost_per_million_minor)?;
        let output_price = to_i64(policy.output_cost_per_million_minor)?;
        let max_concurrent =
            i32::try_from(policy.max_concurrent).map_err(|_| accounting_limit())?;
        let now = Utc::now();
        let transaction = self
            .database
            .begin()
            .await
            .map_err(|_| accounting_unavailable())?;
        let existing = ai_structured_provider_policies::Entity::find()
            .filter(ai_structured_provider_policies::Column::TenantId.eq(policy.tenant_id))
            .filter(
                ai_structured_provider_policies::Column::ProviderProfileId
                    .eq(policy.provider_profile_id),
            )
            .one(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?;
        match existing {
            Some(existing) => {
                if existing.in_flight > max_concurrent {
                    return Err(PortError::conflict(
                        "ai.structured.provider_policy_conflict",
                        "provider concurrency cannot be reduced below current usage",
                    ));
                }
                let existing_allowed_classifications = provider_allowed_classifications(&existing)?;
                if existing.in_flight > 0
                    && (existing_allowed_classifications != allowed_classifications
                        || existing.is_active != policy.is_active)
                {
                    return Err(PortError::conflict(
                        "ai.structured.provider_egress_policy_in_use",
                        "provider egress classification policy cannot change while structured attempts are in flight",
                    ));
                }
                let next_revision = existing
                    .revision
                    .checked_add(1)
                    .ok_or_else(accounting_limit)?;
                let egress_changed = existing_allowed_classifications != allowed_classifications
                    || existing.is_active != policy.is_active;
                let mut update = ai_structured_provider_policies::Entity::update_many()
                    .col_expr(
                        ai_structured_provider_policies::Column::AllowedClassifications,
                        Expr::value(allowed_classifications_json.clone()),
                    )
                    .col_expr(
                        ai_structured_provider_policies::Column::CurrencyCode,
                        Expr::value(policy.currency_code.clone()),
                    )
                    .col_expr(
                        ai_structured_provider_policies::Column::InputCostPerMillionMinor,
                        Expr::value(input_price),
                    )
                    .col_expr(
                        ai_structured_provider_policies::Column::OutputCostPerMillionMinor,
                        Expr::value(output_price),
                    )
                    .col_expr(
                        ai_structured_provider_policies::Column::MaxConcurrent,
                        Expr::value(max_concurrent),
                    )
                    .col_expr(
                        ai_structured_provider_policies::Column::IsActive,
                        Expr::value(policy_is_active),
                    )
                    .col_expr(
                        ai_structured_provider_policies::Column::Revision,
                        Expr::value(next_revision),
                    )
                    .col_expr(
                        ai_structured_provider_policies::Column::UpdatedAt,
                        Expr::value(now),
                    )
                    .filter(ai_structured_provider_policies::Column::Id.eq(existing.id));
                if egress_changed {
                    update = update.filter(
                        ai_structured_provider_policies::Column::InFlight.eq(existing.in_flight),
                    );
                }
                let updated = update
                    .exec(&transaction)
                    .await
                    .map_err(|_| accounting_unavailable())?;
                if updated.rows_affected != 1 {
                    return Err(PortError::conflict(
                        "ai.structured.provider_policy_conflict",
                        "structured provider policy changed concurrently",
                    ));
                }
            }
            None => {
                ai_structured_provider_policies::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(policy.tenant_id),
                    provider_profile_id: Set(policy.provider_profile_id),
                    allowed_classifications: Set(allowed_classifications_json),
                    currency_code: Set(policy.currency_code),
                    input_cost_per_million_minor: Set(input_price),
                    output_cost_per_million_minor: Set(output_price),
                    max_concurrent: Set(max_concurrent),
                    in_flight: Set(0),
                    is_active: Set(policy.is_active),
                    revision: Set(1),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&transaction)
                .await
                .map_err(|_| accounting_unavailable())?;
            }
        }
        transaction
            .commit()
            .await
            .map_err(|_| accounting_unavailable())?;
        Ok(())
    }

    pub(crate) async fn provider_profile_ids_permitting_classification(
        &self,
        tenant_id: Uuid,
        provider_profile_ids: &[Uuid],
        classification: AiTaskDataClassification,
    ) -> Result<BTreeSet<Uuid>, PortError> {
        Ok(self
            .provider_policies_permitting_classification(
                tenant_id,
                provider_profile_ids,
                classification,
            )
            .await?
            .into_iter()
            .map(|policy| policy.provider_profile_id)
            .collect())
    }

    async fn provider_policies_permitting_classification(
        &self,
        tenant_id: Uuid,
        provider_profile_ids: &[Uuid],
        classification: AiTaskDataClassification,
    ) -> Result<Vec<ai_structured_provider_policies::Model>, PortError> {
        if provider_profile_ids.is_empty() {
            return Err(PortError::unavailable(
                "ai.structured.provider_unavailable",
                "no structured generation provider is available",
            ));
        }
        let policies = ai_structured_provider_policies::Entity::find()
            .filter(ai_structured_provider_policies::Column::TenantId.eq(tenant_id))
            .filter(
                ai_structured_provider_policies::Column::ProviderProfileId
                    .is_in(provider_profile_ids.iter().copied()),
            )
            .filter(ai_structured_provider_policies::Column::IsActive.eq(true))
            .all(&self.database)
            .await
            .map_err(|_| accounting_unavailable())?;
        if policies.len() != provider_profile_ids.len() {
            return Err(PortError::unavailable(
                "ai.structured.provider_accounting_unavailable",
                "one or more structured generation providers have no active accounting policy",
            ));
        }
        let permitted = policies
            .into_iter()
            .map(|policy| {
                let allowed = provider_allowed_classifications(&policy)?;
                Ok((policy, allowed))
            })
            .collect::<Result<Vec<_>, PortError>>()?
            .into_iter()
            .filter_map(|(policy, allowed)| allowed.contains(&classification).then_some(policy))
            .collect::<Vec<_>>();
        if permitted.is_empty() {
            return Err(provider_egress_classification_denied());
        }
        Ok(permitted)
    }

    pub(crate) async fn reserve(
        &self,
        execution_id: Uuid,
        provider_profile_ids: &[Uuid],
    ) -> Result<Reservation, PortError> {
        if provider_profile_ids.is_empty() {
            return Err(PortError::unavailable(
                "ai.structured.provider_unavailable",
                "no structured generation provider is available",
            ));
        }
        let execution = ai_structured_executions::Entity::find_by_id(execution_id)
            .one(&self.database)
            .await
            .map_err(|_| accounting_unavailable())?
            .ok_or_else(|| {
                PortError::not_found(
                    "ai.structured.execution_not_found",
                    "structured task execution was not found",
                )
            })?;
        if execution.status != "queued" {
            return Err(PortError::conflict(
                "ai.structured.reservation_state_conflict",
                "only queued structured executions can reserve budget",
            ));
        }
        if let Some(existing) = ai_structured_reservations::Entity::find()
            .filter(ai_structured_reservations::Column::ExecutionId.eq(execution_id))
            .one(&self.database)
            .await
            .map_err(|_| accounting_unavailable())?
        {
            return map_reservation(existing);
        }

        let input_tokens = u64::try_from(execution.input_bytes).map_err(|_| accounting_limit())?;
        let output_tokens =
            u64::try_from(execution.max_output_bytes).map_err(|_| accounting_limit())?;
        let classification = parse_execution_classification(&execution.classification)?;
        let estimate = self
            .estimate(
                execution.tenant_id,
                classification,
                input_tokens,
                output_tokens,
                u16::try_from(execution.max_attempts).map_err(|_| accounting_limit())?,
                provider_profile_ids,
            )
            .await?;
        let currency = estimate.currency_code;
        let reservation_amount = estimate.cost_minor_units_upper_bound;
        let reservation_amount_i64 = to_i64(reservation_amount)?;

        let transaction = self
            .database
            .begin()
            .await
            .map_err(|_| accounting_unavailable())?;
        let budget = ai_structured_budgets::Entity::find()
            .filter(ai_structured_budgets::Column::TenantId.eq(execution.tenant_id))
            .filter(ai_structured_budgets::Column::CurrencyCode.eq(currency.as_str()))
            .one(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?
            .ok_or_else(|| {
                PortError::unavailable(
                    "ai.structured.budget_unconfigured",
                    "structured execution budget is not configured",
                )
            })?;
        let updated = ai_structured_budgets::Entity::update_many()
            .col_expr(
                ai_structured_budgets::Column::ReservedMinorUnits,
                Expr::col(ai_structured_budgets::Column::ReservedMinorUnits)
                    .add(reservation_amount_i64),
            )
            .col_expr(
                ai_structured_budgets::Column::InFlight,
                Expr::col(ai_structured_budgets::Column::InFlight).add(1),
            )
            .col_expr(
                ai_structured_budgets::Column::Revision,
                Expr::col(ai_structured_budgets::Column::Revision).add(1),
            )
            .col_expr(
                ai_structured_budgets::Column::UpdatedAt,
                Expr::value(Utc::now()),
            )
            .filter(ai_structured_budgets::Column::Id.eq(budget.id))
            .filter(
                Expr::col(ai_structured_budgets::Column::ReservedMinorUnits)
                    .add(Expr::col(
                        ai_structured_budgets::Column::CommittedMinorUnits,
                    ))
                    .add(reservation_amount_i64)
                    .lte(Expr::col(ai_structured_budgets::Column::LimitMinorUnits)),
            )
            .filter(
                Expr::col(ai_structured_budgets::Column::InFlight)
                    .lt(Expr::col(ai_structured_budgets::Column::MaxConcurrent)),
            )
            .exec(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?;
        if updated.rows_affected != 1 {
            transaction.rollback().await.ok();
            return Err(PortError::new(
                rustok_api::PortErrorKind::Unavailable,
                "ai.structured.quota_exhausted",
                "structured execution budget or tenant concurrency is exhausted",
                true,
            ));
        }
        let now = Utc::now();
        let reservation = ai_structured_reservations::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(execution.tenant_id),
            execution_id: Set(execution.id),
            budget_id: Set(budget.id),
            currency_code: Set(currency),
            reserved_minor_units: Set(reservation_amount_i64),
            committed_minor_units: Set(0),
            state: Set("reserved".to_string()),
            revision: Set(1),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&transaction)
        .await
        .map_err(|_| accounting_unavailable())?;
        transaction
            .commit()
            .await
            .map_err(|_| accounting_unavailable())?;
        map_reservation(reservation)
    }

    pub(crate) async fn finalize(
        &self,
        execution_id: Uuid,
        lease_token: Uuid,
        outcome: TerminalOutcome,
    ) -> Result<ai_structured_executions::Model, PortError> {
        let TerminalFields {
            status,
            error_code,
            retryable,
            retry_after_ms,
        } = terminal_fields(&outcome)?;
        let transaction = self
            .database
            .begin()
            .await
            .map_err(|_| accounting_unavailable())?;
        let execution = ai_structured_executions::Entity::find_by_id(execution_id)
            .one(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?
            .ok_or_else(|| {
                PortError::not_found(
                    "ai.structured.execution_not_found",
                    "structured task execution was not found",
                )
            })?;
        if is_terminal(&execution.status) {
            ensure_terminal_replay(
                &execution,
                status,
                error_code.as_deref(),
                retryable,
                retry_after_ms,
            )?;
            transaction
                .commit()
                .await
                .map_err(|_| accounting_unavailable())?;
            return Ok(execution);
        }
        if execution.status != "running" || execution.lease_token != Some(lease_token) {
            return Err(PortError::conflict(
                "ai.structured.finalize_lease_conflict",
                "structured execution lease is not active",
            ));
        }
        if status != "cancelled" && execution.cancel_requested_at.is_some() {
            return Err(PortError::conflict(
                "ai.structured.cancellation_pending",
                "structured execution must observe its durable cancellation request",
            ));
        }
        ensure_no_running_attempt(execution_id, &transaction).await?;
        let committed = attempt_cost_total(execution_id, &transaction).await?;
        settle_reservation(execution_id, committed, &transaction).await?;
        let now = Utc::now();
        let mut update = ai_structured_executions::Entity::update_many()
            .col_expr(
                ai_structured_executions::Column::Status,
                Expr::value(status),
            )
            .col_expr(
                ai_structured_executions::Column::ErrorCode,
                Expr::value(error_code.clone()),
            )
            .col_expr(
                ai_structured_executions::Column::Retryable,
                Expr::value(retryable),
            )
            .col_expr(
                ai_structured_executions::Column::RetryAfterMs,
                Expr::value(retry_after_ms),
            )
            .col_expr(
                ai_structured_executions::Column::LeaseToken,
                Expr::value(Option::<Uuid>::None),
            )
            .col_expr(
                ai_structured_executions::Column::LeaseExpiresAt,
                Expr::value(Option::<DateTime<Utc>>::None),
            )
            .col_expr(
                ai_structured_executions::Column::CompletedAt,
                Expr::value(Some(now)),
            )
            .col_expr(
                ai_structured_executions::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(ai_structured_executions::Column::Id.eq(execution_id))
            .filter(ai_structured_executions::Column::Status.eq("running"))
            .filter(ai_structured_executions::Column::LeaseToken.eq(lease_token));
        if status != "cancelled" {
            update = update.filter(ai_structured_executions::Column::CancelRequestedAt.is_null());
        }
        let changed = update
            .exec(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?;
        if changed.rows_affected != 1 {
            return Err(PortError::conflict(
                "ai.structured.finalize_state_conflict",
                "structured execution changed while it was being finalized",
            ));
        }
        let finalized = ai_structured_executions::Entity::find_by_id(execution_id)
            .one(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?
            .ok_or_else(accounting_invariant)?;
        transaction
            .commit()
            .await
            .map_err(|_| accounting_unavailable())?;
        Ok(finalized)
    }

    pub(crate) async fn complete_attempt(
        &self,
        execution_id: Uuid,
        lease_token: Uuid,
        attempt_id: Uuid,
        usage: ProviderUsage,
        result: SealedStructuredResult,
    ) -> Result<ai_structured_executions::Model, PortError> {
        if usage.total_tokens != usage.input_tokens.saturating_add(usage.output_tokens) {
            return Err(PortError::invariant_violation(
                "ai.structured.provider_usage_invalid",
                "structured generation provider token usage does not reconcile",
            ));
        }
        let transaction = self
            .database
            .begin()
            .await
            .map_err(|_| accounting_unavailable())?;
        let execution = ai_structured_executions::Entity::find_by_id(execution_id)
            .one(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?
            .ok_or_else(|| {
                PortError::not_found(
                    "ai.structured.execution_not_found",
                    "structured task execution was not found",
                )
            })?;
        if execution.status != "running" || execution.lease_token != Some(lease_token) {
            return Err(PortError::conflict(
                "ai.structured.finalize_lease_conflict",
                "structured execution lease is not active",
            ));
        }
        if execution.cancel_requested_at.is_some() {
            return Err(PortError::conflict(
                "ai.structured.cancellation_pending",
                "structured execution must observe its durable cancellation request",
            ));
        }
        if result.tenant_id != execution.tenant_id
            || result.execution_id != execution.id
            || result.request_digest != execution.request_digest
            || result.plaintext_bytes <= 0
            || result.expires_at <= result.created_at
        {
            return Err(accounting_invariant());
        }
        let attempt = ai_structured_attempts::Entity::find_by_id(attempt_id)
            .filter(ai_structured_attempts::Column::TenantId.eq(execution.tenant_id))
            .filter(ai_structured_attempts::Column::ExecutionId.eq(execution_id))
            .one(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?
            .ok_or_else(|| {
                PortError::not_found(
                    "ai.structured.attempt_not_found",
                    "structured execution attempt was not found",
                )
            })?;
        if attempt.status != "running" {
            return Err(PortError::conflict(
                "ai.structured.attempt_state_conflict",
                "structured execution attempt is no longer running",
            ));
        }
        let cost = actual_cost(
            usage.input_tokens,
            usage.output_tokens,
            u64::try_from(attempt.input_cost_per_million_minor)
                .map_err(|_| accounting_invariant())?,
            u64::try_from(attempt.output_cost_per_million_minor)
                .map_err(|_| accounting_invariant())?,
        )?;
        let cost_minor_units = to_i64(cost)?;
        let now = Utc::now();
        let finished = ai_structured_attempts::Entity::update_many()
            .col_expr(
                ai_structured_attempts::Column::Status,
                Expr::value("completed"),
            )
            .col_expr(
                ai_structured_attempts::Column::InputTokens,
                Expr::value(Some(to_i64(usage.input_tokens)?)),
            )
            .col_expr(
                ai_structured_attempts::Column::OutputTokens,
                Expr::value(Some(to_i64(usage.output_tokens)?)),
            )
            .col_expr(
                ai_structured_attempts::Column::TotalTokens,
                Expr::value(Some(to_i64(usage.total_tokens)?)),
            )
            .col_expr(
                ai_structured_attempts::Column::CostMinorUnits,
                Expr::value(Some(cost_minor_units)),
            )
            .col_expr(
                ai_structured_attempts::Column::CompletedAt,
                Expr::value(Some(now)),
            )
            .filter(ai_structured_attempts::Column::Id.eq(attempt.id))
            .filter(ai_structured_attempts::Column::Status.eq("running"))
            .exec(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?;
        if finished.rows_affected != 1 {
            return Err(accounting_invariant());
        }
        release_provider_slot(&attempt, now, &transaction).await?;
        ai_structured_results::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(result.tenant_id),
            execution_id: Set(result.execution_id),
            request_digest: Set(result.request_digest),
            output_digest: Set(result.output_digest),
            key_id: Set(result.key_id),
            nonce: Set(result.nonce),
            ciphertext: Set(result.ciphertext),
            plaintext_bytes: Set(result.plaintext_bytes),
            replay_count: Set(0),
            created_at: Set(result.created_at.into()),
            expires_at: Set(result.expires_at.into()),
            last_replayed_at: Set(None),
        }
        .insert(&transaction)
        .await
        .map_err(|_| accounting_unavailable())?;
        let committed = attempt_cost_total(execution_id, &transaction).await?;
        settle_reservation(execution_id, committed, &transaction).await?;
        let changed = ai_structured_executions::Entity::update_many()
            .col_expr(
                ai_structured_executions::Column::Status,
                Expr::value("completed"),
            )
            .col_expr(
                ai_structured_executions::Column::ErrorCode,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                ai_structured_executions::Column::Retryable,
                Expr::value(false),
            )
            .col_expr(
                ai_structured_executions::Column::RetryAfterMs,
                Expr::value(Option::<i64>::None),
            )
            .col_expr(
                ai_structured_executions::Column::LeaseToken,
                Expr::value(Option::<Uuid>::None),
            )
            .col_expr(
                ai_structured_executions::Column::LeaseExpiresAt,
                Expr::value(Option::<DateTime<Utc>>::None),
            )
            .col_expr(
                ai_structured_executions::Column::CompletedAt,
                Expr::value(Some(now)),
            )
            .col_expr(
                ai_structured_executions::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(ai_structured_executions::Column::Id.eq(execution.id))
            .filter(ai_structured_executions::Column::Status.eq("running"))
            .filter(ai_structured_executions::Column::LeaseToken.eq(lease_token))
            .filter(ai_structured_executions::Column::CancelRequestedAt.is_null())
            .exec(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?;
        if changed.rows_affected != 1 {
            return Err(PortError::conflict(
                "ai.structured.finalize_state_conflict",
                "structured execution changed while it was being finalized",
            ));
        }
        let completed = ai_structured_executions::Entity::find_by_id(execution.id)
            .one(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?
            .ok_or_else(accounting_invariant)?;
        transaction
            .commit()
            .await
            .map_err(|_| accounting_unavailable())?;
        Ok(completed)
    }

    pub(crate) async fn cancel_queued(
        &self,
        execution_id: Uuid,
    ) -> Result<ai_structured_executions::Model, PortError> {
        let transaction = self
            .database
            .begin()
            .await
            .map_err(|_| accounting_unavailable())?;
        let execution = ai_structured_executions::Entity::find_by_id(execution_id)
            .one(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?
            .ok_or_else(|| {
                PortError::not_found(
                    "ai.structured.execution_not_found",
                    "structured task execution was not found",
                )
            })?;
        if execution.status == "cancelled" {
            transaction
                .commit()
                .await
                .map_err(|_| accounting_unavailable())?;
            return Ok(execution);
        }
        if execution.status != "queued" || execution.cancel_requested_at.is_none() {
            return Err(PortError::conflict(
                "ai.structured.queued_cancellation_conflict",
                "structured execution is not a queued cancellation",
            ));
        }
        if ai_structured_reservations::Entity::find()
            .filter(ai_structured_reservations::Column::ExecutionId.eq(execution_id))
            .filter(ai_structured_reservations::Column::State.eq("reserved"))
            .one(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?
            .is_some()
        {
            settle_reservation(execution_id, 0, &transaction).await?;
        }
        let now = Utc::now();
        let changed = ai_structured_executions::Entity::update_many()
            .col_expr(
                ai_structured_executions::Column::Status,
                Expr::value("cancelled"),
            )
            .col_expr(
                ai_structured_executions::Column::CompletedAt,
                Expr::value(Some(now)),
            )
            .col_expr(
                ai_structured_executions::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(ai_structured_executions::Column::Id.eq(execution_id))
            .filter(ai_structured_executions::Column::Status.eq("queued"))
            .filter(ai_structured_executions::Column::CancelRequestedAt.is_not_null())
            .exec(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?;
        if changed.rows_affected != 1 {
            return Err(PortError::conflict(
                "ai.structured.queued_cancellation_conflict",
                "structured execution changed while queued cancellation was finalized",
            ));
        }
        let cancelled = ai_structured_executions::Entity::find_by_id(execution_id)
            .one(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?
            .ok_or_else(accounting_invariant)?;
        transaction
            .commit()
            .await
            .map_err(|_| accounting_unavailable())?;
        Ok(cancelled)
    }

    pub(crate) async fn recover_expired(&self, now: DateTime<Utc>) -> Result<u64, PortError> {
        let expired = ai_structured_executions::Entity::find()
            .filter(ai_structured_executions::Column::Status.eq("running"))
            .filter(ai_structured_executions::Column::LeaseExpiresAt.lt(now))
            .all(&self.database)
            .await
            .map_err(|_| accounting_unavailable())?;
        let mut recovered = 0_u64;
        for candidate in expired {
            let Some(previous_token) = candidate.lease_token else {
                return Err(accounting_invariant());
            };
            let recovery_token = Uuid::new_v4();
            let transaction = self
                .database
                .begin()
                .await
                .map_err(|_| accounting_unavailable())?;
            let acquired = ai_structured_executions::Entity::update_many()
                .col_expr(
                    ai_structured_executions::Column::LeaseToken,
                    Expr::value(Some(recovery_token)),
                )
                .col_expr(
                    ai_structured_executions::Column::LeaseExpiresAt,
                    Expr::value(Some(now + Duration::seconds(RECOVERY_LEASE_SECONDS))),
                )
                .col_expr(
                    ai_structured_executions::Column::UpdatedAt,
                    Expr::value(now),
                )
                .filter(ai_structured_executions::Column::Id.eq(candidate.id))
                .filter(ai_structured_executions::Column::Status.eq("running"))
                .filter(ai_structured_executions::Column::LeaseToken.eq(previous_token))
                .filter(ai_structured_executions::Column::LeaseExpiresAt.lt(now))
                .exec(&transaction)
                .await
                .map_err(|_| accounting_unavailable())?;
            if acquired.rows_affected != 1 {
                transaction.rollback().await.ok();
                continue;
            }
            let execution = ai_structured_executions::Entity::find_by_id(candidate.id)
                .one(&transaction)
                .await
                .map_err(|_| accounting_unavailable())?
                .ok_or_else(accounting_invariant)?;
            let cancelled = execution.cancel_requested_at.is_some();
            finish_recovered_attempt(&execution, cancelled, now, &transaction).await?;
            let attempt_count = ai_structured_attempts::Entity::find()
                .filter(ai_structured_attempts::Column::ExecutionId.eq(execution.id))
                .count(&transaction)
                .await
                .map_err(|_| accounting_unavailable())?;
            let attempts_exhausted = attempt_count
                >= u64::try_from(execution.max_attempts).map_err(|_| accounting_invariant())?;
            let last_attempt = ai_structured_attempts::Entity::find()
                .filter(ai_structured_attempts::Column::ExecutionId.eq(execution.id))
                .order_by_desc(ai_structured_attempts::Column::Attempt)
                .one(&transaction)
                .await
                .map_err(|_| accounting_unavailable())?;
            let completed_without_handoff = last_attempt
                .as_ref()
                .is_some_and(|attempt| attempt.status == "completed");
            let non_retryable_failure = last_attempt
                .as_ref()
                .is_some_and(|attempt| attempt.status == "failed" && !attempt.retryable);
            let terminal = cancelled
                || attempts_exhausted
                || completed_without_handoff
                || non_retryable_failure;
            if terminal {
                let committed = attempt_cost_total(execution.id, &transaction).await?;
                settle_reservation(execution.id, committed, &transaction).await?;
            }
            let status = if cancelled {
                "cancelled"
            } else if terminal {
                "failed"
            } else {
                "queued"
            };
            let error_code = if cancelled {
                None
            } else if completed_without_handoff {
                Some(RESULT_HANDOFF_ERROR_CODE.to_string())
            } else if non_retryable_failure {
                last_attempt.and_then(|attempt| attempt.error_code)
            } else {
                Some(RECOVERY_ERROR_CODE.to_string())
            };
            let changed = ai_structured_executions::Entity::update_many()
                .col_expr(
                    ai_structured_executions::Column::Status,
                    Expr::value(status),
                )
                .col_expr(
                    ai_structured_executions::Column::ErrorCode,
                    Expr::value(error_code),
                )
                .col_expr(
                    ai_structured_executions::Column::Retryable,
                    Expr::value(!terminal),
                )
                .col_expr(
                    ai_structured_executions::Column::RetryAfterMs,
                    Expr::value((!terminal).then_some(0_i64)),
                )
                .col_expr(
                    ai_structured_executions::Column::LeaseToken,
                    Expr::value(Option::<Uuid>::None),
                )
                .col_expr(
                    ai_structured_executions::Column::LeaseExpiresAt,
                    Expr::value(Option::<DateTime<Utc>>::None),
                )
                .col_expr(
                    ai_structured_executions::Column::CompletedAt,
                    Expr::value(terminal.then_some(now)),
                )
                .col_expr(
                    ai_structured_executions::Column::UpdatedAt,
                    Expr::value(now),
                )
                .filter(ai_structured_executions::Column::Id.eq(execution.id))
                .filter(ai_structured_executions::Column::Status.eq("running"))
                .filter(ai_structured_executions::Column::LeaseToken.eq(recovery_token))
                .exec(&transaction)
                .await
                .map_err(|_| accounting_unavailable())?;
            if changed.rows_affected != 1 {
                return Err(accounting_invariant());
            }
            transaction
                .commit()
                .await
                .map_err(|_| accounting_unavailable())?;
            recovered = recovered.saturating_add(1);
        }
        Ok(recovered)
    }

    pub(crate) async fn recover_queued_cancellations(&self) -> Result<u64, PortError> {
        let queued = ai_structured_executions::Entity::find()
            .filter(ai_structured_executions::Column::Status.eq("queued"))
            .filter(ai_structured_executions::Column::CancelRequestedAt.is_not_null())
            .all(&self.database)
            .await
            .map_err(|_| accounting_unavailable())?;
        let mut recovered = 0_u64;
        for execution in queued {
            match self.cancel_queued(execution.id).await {
                Ok(_) => recovered = recovered.saturating_add(1),
                Err(error) if error.kind == rustok_api::PortErrorKind::Conflict => {}
                Err(error) => return Err(error),
            }
        }
        Ok(recovered)
    }

    pub(crate) async fn begin_attempt(
        &self,
        execution_id: Uuid,
        lease_token: Uuid,
        provider_profile_id: Uuid,
        provider_slug: &str,
        model: &str,
    ) -> Result<Attempt, PortError> {
        if provider_slug.trim().is_empty()
            || provider_slug.len() > 128
            || model.trim().is_empty()
            || model.len() > 256
        {
            return Err(PortError::validation(
                "ai.structured.attempt_identity_invalid",
                "structured execution provider and model identities must be bounded",
            ));
        }
        let transaction = self
            .database
            .begin()
            .await
            .map_err(|_| accounting_unavailable())?;
        let execution = ai_structured_executions::Entity::find_by_id(execution_id)
            .one(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?
            .ok_or_else(|| {
                PortError::not_found(
                    "ai.structured.execution_not_found",
                    "structured task execution was not found",
                )
            })?;
        if execution.status != "running"
            || execution.lease_token != Some(lease_token)
            || execution
                .lease_expires_at
                .is_none_or(|expires_at| expires_at <= Utc::now())
            || execution.cancel_requested_at.is_some()
        {
            return Err(PortError::conflict(
                "ai.structured.attempt_lease_conflict",
                "structured execution lease is not active",
            ));
        }
        let reservation = ai_structured_reservations::Entity::find()
            .filter(ai_structured_reservations::Column::ExecutionId.eq(execution_id))
            .filter(ai_structured_reservations::Column::State.eq("reserved"))
            .one(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?
            .ok_or_else(|| {
                PortError::invariant_violation(
                    "ai.structured.reservation_missing",
                    "structured execution has no active budget reservation",
                )
            })?;
        if ai_structured_attempts::Entity::find()
            .filter(ai_structured_attempts::Column::ExecutionId.eq(execution_id))
            .filter(ai_structured_attempts::Column::Status.eq("running"))
            .one(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?
            .is_some()
        {
            return Err(PortError::conflict(
                "ai.structured.attempt_already_running",
                "structured execution already has a running provider attempt",
            ));
        }
        let attempt_count = ai_structured_attempts::Entity::find()
            .filter(ai_structured_attempts::Column::ExecutionId.eq(execution_id))
            .count(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?;
        if attempt_count >= u64::try_from(execution.max_attempts).map_err(|_| accounting_limit())? {
            return Err(PortError::conflict(
                "ai.structured.attempt_limit_reached",
                "structured execution exhausted its provider attempt limit",
            ));
        }
        let policy = ai_structured_provider_policies::Entity::find()
            .filter(ai_structured_provider_policies::Column::TenantId.eq(execution.tenant_id))
            .filter(
                ai_structured_provider_policies::Column::ProviderProfileId.eq(provider_profile_id),
            )
            .filter(ai_structured_provider_policies::Column::IsActive.eq(true))
            .one(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?
            .ok_or_else(|| {
                PortError::unavailable(
                    "ai.structured.provider_accounting_unavailable",
                    "structured generation provider has no active accounting policy",
                )
            })?;
        let classification = parse_execution_classification(&execution.classification)?;
        if !provider_allowed_classifications(&policy)?.contains(&classification) {
            return Err(provider_egress_classification_denied());
        }
        if policy.currency_code != reservation.currency_code {
            return Err(PortError::conflict(
                "ai.structured.provider_currency_changed",
                "structured generation provider currency changed after budget reservation",
            ));
        }
        let acquired = ai_structured_provider_policies::Entity::update_many()
            .col_expr(
                ai_structured_provider_policies::Column::InFlight,
                Expr::col(ai_structured_provider_policies::Column::InFlight).add(1),
            )
            .col_expr(
                ai_structured_provider_policies::Column::Revision,
                Expr::col(ai_structured_provider_policies::Column::Revision).add(1),
            )
            .col_expr(
                ai_structured_provider_policies::Column::UpdatedAt,
                Expr::value(Utc::now()),
            )
            .filter(ai_structured_provider_policies::Column::Id.eq(policy.id))
            .filter(ai_structured_provider_policies::Column::IsActive.eq(true))
            .filter(
                ai_structured_provider_policies::Column::AllowedClassifications
                    .eq(policy.allowed_classifications.clone()),
            )
            .filter(
                Expr::col(ai_structured_provider_policies::Column::InFlight).lt(Expr::col(
                    ai_structured_provider_policies::Column::MaxConcurrent,
                )),
            )
            .exec(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?;
        if acquired.rows_affected != 1 {
            let current = ai_structured_provider_policies::Entity::find_by_id(policy.id)
                .one(&transaction)
                .await
                .map_err(|_| accounting_unavailable())?;
            if current.is_none_or(|current| {
                !current.is_active
                    || provider_allowed_classifications(&current)
                        .map(|allowed| !allowed.contains(&classification))
                        .unwrap_or(true)
            }) {
                return Err(provider_egress_classification_denied());
            }
            return Err(PortError::new(
                rustok_api::PortErrorKind::Unavailable,
                "ai.structured.provider_concurrency_exhausted",
                "structured generation provider concurrency is exhausted",
                true,
            ));
        }
        let attempt_number =
            i32::try_from(attempt_count.saturating_add(1)).map_err(|_| accounting_limit())?;
        let now = Utc::now();
        let attempt = ai_structured_attempts::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(execution.tenant_id),
            execution_id: Set(execution.id),
            attempt: Set(attempt_number),
            provider_profile_id: Set(provider_profile_id),
            provider_slug: Set(provider_slug.to_string()),
            model: Set(model.to_string()),
            fallback: Set(attempt_number > 1),
            status: Set("running".to_string()),
            price_snapshot_digest: Set(Self::price_snapshot_digest(&policy)?),
            currency_code: Set(policy.currency_code),
            input_cost_per_million_minor: Set(policy.input_cost_per_million_minor),
            output_cost_per_million_minor: Set(policy.output_cost_per_million_minor),
            input_tokens: Set(None),
            output_tokens: Set(None),
            total_tokens: Set(None),
            cost_minor_units: Set(None),
            error_code: Set(None),
            retryable: Set(false),
            retry_after_ms: Set(None),
            created_at: Set(now.into()),
            started_at: Set(now.into()),
            completed_at: Set(None),
        }
        .insert(&transaction)
        .await
        .map_err(|_| accounting_unavailable())?;
        transaction
            .commit()
            .await
            .map_err(|_| accounting_unavailable())?;
        Ok(Attempt { model: attempt })
    }

    pub(crate) async fn finish_attempt(
        &self,
        execution_id: Uuid,
        lease_token: Uuid,
        attempt_id: Uuid,
        outcome: AttemptOutcome,
    ) -> Result<AttemptCost, PortError> {
        let transaction = self
            .database
            .begin()
            .await
            .map_err(|_| accounting_unavailable())?;
        let execution = ai_structured_executions::Entity::find_by_id(execution_id)
            .one(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?
            .ok_or_else(|| {
                PortError::not_found(
                    "ai.structured.execution_not_found",
                    "structured task execution was not found",
                )
            })?;
        if execution.status != "running" || execution.lease_token != Some(lease_token) {
            return Err(PortError::conflict(
                "ai.structured.attempt_lease_conflict",
                "structured execution lease is not active",
            ));
        }
        let attempt = ai_structured_attempts::Entity::find_by_id(attempt_id)
            .filter(ai_structured_attempts::Column::TenantId.eq(execution.tenant_id))
            .filter(ai_structured_attempts::Column::ExecutionId.eq(execution_id))
            .one(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?
            .ok_or_else(|| {
                PortError::not_found(
                    "ai.structured.attempt_not_found",
                    "structured execution attempt was not found",
                )
            })?;
        if attempt.status != "running" {
            return Err(PortError::conflict(
                "ai.structured.attempt_state_conflict",
                "structured execution attempt is no longer running",
            ));
        }
        let (status, usage, error_code, retryable, retry_after_ms) = match outcome {
            AttemptOutcome::Failed {
                usage,
                error_code,
                retryable,
                retry_after_ms,
            } => ("failed", usage, Some(error_code), retryable, retry_after_ms),
            AttemptOutcome::Cancelled { usage } => ("cancelled", usage, None, false, None),
        };
        let (input_tokens, output_tokens, total_tokens, cost_minor_units) = match usage {
            Some(usage) => {
                if usage.total_tokens != usage.input_tokens.saturating_add(usage.output_tokens) {
                    return Err(PortError::invariant_violation(
                        "ai.structured.provider_usage_invalid",
                        "structured generation provider token usage does not reconcile",
                    ));
                }
                let cost = actual_cost(
                    usage.input_tokens,
                    usage.output_tokens,
                    u64::try_from(attempt.input_cost_per_million_minor)
                        .map_err(|_| accounting_invariant())?,
                    u64::try_from(attempt.output_cost_per_million_minor)
                        .map_err(|_| accounting_invariant())?,
                )?;
                (
                    Some(to_i64(usage.input_tokens)?),
                    Some(to_i64(usage.output_tokens)?),
                    Some(to_i64(usage.total_tokens)?),
                    Some(to_i64(cost)?),
                )
            }
            None => (None, None, None, None),
        };
        let now = Utc::now();
        let finished = ai_structured_attempts::Entity::update_many()
            .col_expr(ai_structured_attempts::Column::Status, Expr::value(status))
            .col_expr(
                ai_structured_attempts::Column::InputTokens,
                Expr::value(input_tokens),
            )
            .col_expr(
                ai_structured_attempts::Column::OutputTokens,
                Expr::value(output_tokens),
            )
            .col_expr(
                ai_structured_attempts::Column::TotalTokens,
                Expr::value(total_tokens),
            )
            .col_expr(
                ai_structured_attempts::Column::CostMinorUnits,
                Expr::value(cost_minor_units),
            )
            .col_expr(
                ai_structured_attempts::Column::ErrorCode,
                Expr::value(error_code),
            )
            .col_expr(
                ai_structured_attempts::Column::Retryable,
                Expr::value(retryable),
            )
            .col_expr(
                ai_structured_attempts::Column::RetryAfterMs,
                Expr::value(retry_after_ms.map(to_i64).transpose()?),
            )
            .col_expr(
                ai_structured_attempts::Column::CompletedAt,
                Expr::value(Some(now)),
            )
            .filter(ai_structured_attempts::Column::Id.eq(attempt.id))
            .filter(ai_structured_attempts::Column::Status.eq("running"))
            .exec(&transaction)
            .await
            .map_err(|_| accounting_unavailable())?;
        if finished.rows_affected != 1 {
            return Err(PortError::conflict(
                "ai.structured.attempt_state_conflict",
                "structured execution attempt changed while it was being completed",
            ));
        }
        release_provider_slot(&attempt, now, &transaction).await?;
        transaction
            .commit()
            .await
            .map_err(|_| accounting_unavailable())?;
        Ok(AttemptCost {
            cost_minor_units: u64::try_from(cost_minor_units.unwrap_or_default())
                .map_err(|_| accounting_invariant())?,
            currency_code: attempt.currency_code,
        })
    }

    pub(crate) fn price_snapshot_digest(
        policy: &ai_structured_provider_policies::Model,
    ) -> Result<String, PortError> {
        let allowed_classifications = provider_allowed_classifications(policy)?;
        hash_manifest(&PriceSnapshot {
            provider_profile_id: policy.provider_profile_id,
            allowed_classifications: &allowed_classifications,
            is_active: policy.is_active,
            currency_code: &policy.currency_code,
            input_cost_per_million_minor: policy.input_cost_per_million_minor,
            output_cost_per_million_minor: policy.output_cost_per_million_minor,
            revision: policy.revision,
        })
        .map_err(|_| accounting_invariant())
    }

    pub(crate) async fn estimate(
        &self,
        tenant_id: Uuid,
        classification: AiTaskDataClassification,
        input_tokens_upper_bound: u64,
        output_tokens_upper_bound: u64,
        max_attempts: u16,
        provider_profile_ids: &[Uuid],
    ) -> Result<AiStructuredTaskEstimate, PortError> {
        let policies = self
            .provider_policies_permitting_classification(
                tenant_id,
                provider_profile_ids,
                classification,
            )
            .await?;
        let currency_code = policies[0].currency_code.clone();
        if policies
            .iter()
            .any(|policy| policy.currency_code != currency_code)
        {
            return Err(PortError::validation(
                "ai.structured.provider_currency_mismatch",
                "structured execution fallback providers must use one budget currency",
            ));
        }
        let maximum_attempt_cost = policies
            .iter()
            .map(|policy| {
                estimated_cost(
                    input_tokens_upper_bound,
                    output_tokens_upper_bound,
                    u64::try_from(policy.input_cost_per_million_minor)
                        .map_err(|_| accounting_limit())?,
                    u64::try_from(policy.output_cost_per_million_minor)
                        .map_err(|_| accounting_limit())?,
                )
            })
            .collect::<Result<Vec<_>, PortError>>()?
            .into_iter()
            .max()
            .unwrap_or_default();
        let attempts_upper_bound =
            max_attempts.min(u16::try_from(policies.len()).map_err(|_| accounting_limit())?);
        let cost_minor_units_upper_bound = maximum_attempt_cost
            .checked_mul(u64::from(attempts_upper_bound))
            .ok_or_else(accounting_limit)?;
        let mut snapshots = policies
            .iter()
            .map(|policy| {
                Ok(EstimatePriceSnapshot {
                    provider_profile_id: policy.provider_profile_id,
                    price_snapshot_digest: Self::price_snapshot_digest(policy)?,
                })
            })
            .collect::<Result<Vec<_>, PortError>>()?;
        snapshots.sort_by_key(|snapshot| snapshot.provider_profile_id);
        let price_snapshot_digest =
            hash_manifest(&snapshots).map_err(|_| accounting_invariant())?;

        Ok(AiStructuredTaskEstimate {
            input_tokens_upper_bound,
            output_tokens_upper_bound,
            attempts_upper_bound,
            cost_minor_units_upper_bound,
            currency_code,
            price_snapshot_digest,
        })
    }
}

struct TerminalFields {
    status: &'static str,
    error_code: Option<String>,
    retryable: bool,
    retry_after_ms: Option<i64>,
}

fn terminal_fields(outcome: &TerminalOutcome) -> Result<TerminalFields, PortError> {
    match outcome {
        TerminalOutcome::Cancelled => Ok(TerminalFields {
            status: "cancelled",
            error_code: None,
            retryable: false,
            retry_after_ms: None,
        }),
        TerminalOutcome::Failed {
            error_code,
            retryable,
            retry_after_ms,
        } => {
            if error_code.trim().is_empty() || error_code.len() > 128 {
                return Err(PortError::validation(
                    "ai.structured.terminal_error_invalid",
                    "structured execution terminal error code must be bounded",
                ));
            }
            Ok(TerminalFields {
                status: "failed",
                error_code: Some(error_code.clone()),
                retryable: *retryable,
                retry_after_ms: retry_after_ms.map(to_i64).transpose()?,
            })
        }
    }
}

fn ensure_terminal_replay(
    execution: &ai_structured_executions::Model,
    status: &str,
    error_code: Option<&str>,
    retryable: bool,
    retry_after_ms: Option<i64>,
) -> Result<(), PortError> {
    if execution.status == status
        && execution.error_code.as_deref() == error_code
        && execution.retryable == retryable
        && execution.retry_after_ms == retry_after_ms
    {
        return Ok(());
    }
    Err(PortError::conflict(
        "ai.structured.finalize_replay_conflict",
        "structured execution was already finalized with another outcome",
    ))
}

fn is_terminal(status: &str) -> bool {
    matches!(status, "completed" | "failed" | "cancelled")
}

async fn ensure_no_running_attempt(
    execution_id: Uuid,
    transaction: &DatabaseTransaction,
) -> Result<(), PortError> {
    if ai_structured_attempts::Entity::find()
        .filter(ai_structured_attempts::Column::ExecutionId.eq(execution_id))
        .filter(ai_structured_attempts::Column::Status.eq("running"))
        .one(transaction)
        .await
        .map_err(|_| accounting_unavailable())?
        .is_some()
    {
        return Err(PortError::conflict(
            "ai.structured.attempt_still_running",
            "structured execution cannot finalize while a provider attempt is running",
        ));
    }
    Ok(())
}

async fn release_provider_slot(
    attempt: &ai_structured_attempts::Model,
    now: DateTime<Utc>,
    transaction: &DatabaseTransaction,
) -> Result<(), PortError> {
    let released = ai_structured_provider_policies::Entity::update_many()
        .col_expr(
            ai_structured_provider_policies::Column::InFlight,
            Expr::col(ai_structured_provider_policies::Column::InFlight).sub(1),
        )
        .col_expr(
            ai_structured_provider_policies::Column::Revision,
            Expr::col(ai_structured_provider_policies::Column::Revision).add(1),
        )
        .col_expr(
            ai_structured_provider_policies::Column::UpdatedAt,
            Expr::value(now),
        )
        .filter(
            ai_structured_provider_policies::Column::ProviderProfileId
                .eq(attempt.provider_profile_id),
        )
        .filter(ai_structured_provider_policies::Column::TenantId.eq(attempt.tenant_id))
        .filter(ai_structured_provider_policies::Column::InFlight.gt(0))
        .exec(transaction)
        .await
        .map_err(|_| accounting_unavailable())?;
    if released.rows_affected != 1 {
        return Err(accounting_invariant());
    }
    Ok(())
}

async fn attempt_cost_total(
    execution_id: Uuid,
    transaction: &DatabaseTransaction,
) -> Result<i64, PortError> {
    let attempts = ai_structured_attempts::Entity::find()
        .filter(ai_structured_attempts::Column::ExecutionId.eq(execution_id))
        .all(transaction)
        .await
        .map_err(|_| accounting_unavailable())?;
    attempts.into_iter().try_fold(0_i64, |total, attempt| {
        let cost = attempt.cost_minor_units.unwrap_or_default();
        if cost < 0 {
            return Err(accounting_invariant());
        }
        total.checked_add(cost).ok_or_else(accounting_limit)
    })
}

async fn settle_reservation(
    execution_id: Uuid,
    committed: i64,
    transaction: &DatabaseTransaction,
) -> Result<(), PortError> {
    let reservation = ai_structured_reservations::Entity::find()
        .filter(ai_structured_reservations::Column::ExecutionId.eq(execution_id))
        .one(transaction)
        .await
        .map_err(|_| accounting_unavailable())?
        .ok_or_else(|| {
            PortError::invariant_violation(
                "ai.structured.reservation_missing",
                "structured execution has no budget reservation",
            )
        })?;
    if reservation.state != "reserved" {
        return Err(PortError::conflict(
            "ai.structured.reservation_settlement_conflict",
            "structured execution reservation is already settled",
        ));
    }
    if committed < 0 || committed > reservation.reserved_minor_units {
        return Err(PortError::invariant_violation(
            "ai.structured.reservation_exceeded",
            "structured execution cost exceeded its durable reservation",
        ));
    }
    let updated = ai_structured_budgets::Entity::update_many()
        .col_expr(
            ai_structured_budgets::Column::ReservedMinorUnits,
            Expr::col(ai_structured_budgets::Column::ReservedMinorUnits)
                .sub(reservation.reserved_minor_units),
        )
        .col_expr(
            ai_structured_budgets::Column::CommittedMinorUnits,
            Expr::col(ai_structured_budgets::Column::CommittedMinorUnits).add(committed),
        )
        .col_expr(
            ai_structured_budgets::Column::InFlight,
            Expr::col(ai_structured_budgets::Column::InFlight).sub(1),
        )
        .col_expr(
            ai_structured_budgets::Column::Revision,
            Expr::col(ai_structured_budgets::Column::Revision).add(1),
        )
        .col_expr(
            ai_structured_budgets::Column::UpdatedAt,
            Expr::value(Utc::now()),
        )
        .filter(ai_structured_budgets::Column::Id.eq(reservation.budget_id))
        .filter(
            ai_structured_budgets::Column::ReservedMinorUnits.gte(reservation.reserved_minor_units),
        )
        .filter(ai_structured_budgets::Column::InFlight.gt(0))
        .exec(transaction)
        .await
        .map_err(|_| accounting_unavailable())?;
    if updated.rows_affected != 1 {
        return Err(accounting_invariant());
    }
    let next_revision = reservation
        .revision
        .checked_add(1)
        .ok_or_else(accounting_limit)?;
    let settled = ai_structured_reservations::Entity::update_many()
        .col_expr(
            ai_structured_reservations::Column::CommittedMinorUnits,
            Expr::value(committed),
        )
        .col_expr(
            ai_structured_reservations::Column::State,
            Expr::value(if committed > 0 {
                "committed"
            } else {
                "released"
            }),
        )
        .col_expr(
            ai_structured_reservations::Column::Revision,
            Expr::value(next_revision),
        )
        .col_expr(
            ai_structured_reservations::Column::UpdatedAt,
            Expr::value(Utc::now()),
        )
        .filter(ai_structured_reservations::Column::Id.eq(reservation.id))
        .filter(ai_structured_reservations::Column::State.eq("reserved"))
        .filter(ai_structured_reservations::Column::Revision.eq(reservation.revision))
        .exec(transaction)
        .await
        .map_err(|_| accounting_unavailable())?;
    if settled.rows_affected != 1 {
        return Err(accounting_invariant());
    }
    Ok(())
}

async fn finish_recovered_attempt(
    execution: &ai_structured_executions::Model,
    cancelled: bool,
    now: DateTime<Utc>,
    transaction: &DatabaseTransaction,
) -> Result<(), PortError> {
    let running = ai_structured_attempts::Entity::find()
        .filter(ai_structured_attempts::Column::ExecutionId.eq(execution.id))
        .filter(ai_structured_attempts::Column::Status.eq("running"))
        .all(transaction)
        .await
        .map_err(|_| accounting_unavailable())?;
    for attempt in running {
        let changed = ai_structured_attempts::Entity::update_many()
            .col_expr(
                ai_structured_attempts::Column::Status,
                Expr::value(if cancelled { "cancelled" } else { "failed" }),
            )
            .col_expr(
                ai_structured_attempts::Column::ErrorCode,
                Expr::value((!cancelled).then(|| RECOVERY_ERROR_CODE.to_string())),
            )
            .col_expr(
                ai_structured_attempts::Column::Retryable,
                Expr::value(!cancelled),
            )
            .col_expr(
                ai_structured_attempts::Column::RetryAfterMs,
                Expr::value((!cancelled).then_some(0_i64)),
            )
            .col_expr(
                ai_structured_attempts::Column::CompletedAt,
                Expr::value(Some(now)),
            )
            .filter(ai_structured_attempts::Column::Id.eq(attempt.id))
            .filter(ai_structured_attempts::Column::Status.eq("running"))
            .exec(transaction)
            .await
            .map_err(|_| accounting_unavailable())?;
        if changed.rows_affected != 1 {
            return Err(accounting_invariant());
        }
        let released = ai_structured_provider_policies::Entity::update_many()
            .col_expr(
                ai_structured_provider_policies::Column::InFlight,
                Expr::col(ai_structured_provider_policies::Column::InFlight).sub(1),
            )
            .col_expr(
                ai_structured_provider_policies::Column::Revision,
                Expr::col(ai_structured_provider_policies::Column::Revision).add(1),
            )
            .col_expr(
                ai_structured_provider_policies::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(
                ai_structured_provider_policies::Column::ProviderProfileId
                    .eq(attempt.provider_profile_id),
            )
            .filter(ai_structured_provider_policies::Column::TenantId.eq(attempt.tenant_id))
            .filter(ai_structured_provider_policies::Column::InFlight.gt(0))
            .exec(transaction)
            .await
            .map_err(|_| accounting_unavailable())?;
        if released.rows_affected != 1 {
            return Err(accounting_invariant());
        }
    }
    Ok(())
}

pub(crate) fn actual_cost(
    input_tokens: u64,
    output_tokens: u64,
    input_cost_per_million_minor: u64,
    output_cost_per_million_minor: u64,
) -> Result<u64, PortError> {
    estimated_cost(
        input_tokens,
        output_tokens,
        input_cost_per_million_minor,
        output_cost_per_million_minor,
    )
}

fn estimated_cost(
    input_tokens: u64,
    output_tokens: u64,
    input_cost_per_million_minor: u64,
    output_cost_per_million_minor: u64,
) -> Result<u64, PortError> {
    let input = u128::from(input_tokens)
        .checked_mul(u128::from(input_cost_per_million_minor))
        .ok_or_else(accounting_limit)?;
    let output = u128::from(output_tokens)
        .checked_mul(u128::from(output_cost_per_million_minor))
        .ok_or_else(accounting_limit)?;
    let numerator = input.checked_add(output).ok_or_else(accounting_limit)?;
    let rounded = numerator
        .checked_add(TOKENS_PER_PRICE_UNIT - 1)
        .ok_or_else(accounting_limit)?
        / TOKENS_PER_PRICE_UNIT;
    u64::try_from(rounded).map_err(|_| accounting_limit())
}

fn map_reservation(model: ai_structured_reservations::Model) -> Result<Reservation, PortError> {
    Ok(Reservation {
        id: model.id,
        execution_id: model.execution_id,
        currency_code: model.currency_code,
        reserved_minor_units: u64::try_from(model.reserved_minor_units)
            .map_err(|_| accounting_invariant())?,
    })
}

fn validate_currency(value: &str) -> Result<(), PortError> {
    if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(PortError::validation(
            "ai.structured.currency_invalid",
            "structured execution currency must be a three-letter uppercase code",
        ));
    }
    Ok(())
}

fn canonical_allowed_classifications(
    classifications: &[AiTaskDataClassification],
) -> Result<Vec<AiTaskDataClassification>, PortError> {
    let unique = classifications.iter().copied().collect::<BTreeSet<_>>();
    if unique.is_empty() || unique.len() != classifications.len() {
        return Err(PortError::validation(
            "ai.structured.provider_egress_policy_invalid",
            "structured provider egress policy must allow one or more unique data classifications",
        ));
    }
    Ok(unique.into_iter().collect())
}

pub(crate) fn provider_allowed_classifications(
    policy: &ai_structured_provider_policies::Model,
) -> Result<Vec<AiTaskDataClassification>, PortError> {
    let classifications: Vec<AiTaskDataClassification> =
        serde_json::from_value(policy.allowed_classifications.clone())
            .map_err(|_| accounting_invariant())?;
    canonical_allowed_classifications(&classifications).map_err(|_| accounting_invariant())
}

fn parse_execution_classification(value: &str) -> Result<AiTaskDataClassification, PortError> {
    match value {
        "public" => Ok(AiTaskDataClassification::Public),
        "tenant_private" => Ok(AiTaskDataClassification::TenantPrivate),
        "personal" => Ok(AiTaskDataClassification::Personal),
        "sensitive" => Ok(AiTaskDataClassification::Sensitive),
        _ => Err(accounting_invariant()),
    }
}

fn provider_egress_classification_denied() -> PortError {
    PortError::forbidden(
        PROVIDER_EGRESS_CLASSIFICATION_DENIED_CODE,
        "no eligible structured generation provider permits this data classification",
    )
}

fn to_i64(value: u64) -> Result<i64, PortError> {
    i64::try_from(value).map_err(|_| accounting_limit())
}

fn accounting_unavailable() -> PortError {
    PortError::unavailable(
        "ai.structured.accounting_unavailable",
        "structured execution accounting is unavailable",
    )
}

fn accounting_limit() -> PortError {
    PortError::validation(
        "ai.structured.accounting_limit_exceeded",
        "structured execution accounting value exceeds the supported range",
    )
}

fn accounting_invariant() -> PortError {
    PortError::invariant_violation(
        "ai.structured.accounting_invalid",
        "structured execution accounting contains invalid evidence",
    )
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::Duration};

    use rustok_api::{PortActor, PortContext, PortErrorKind};
    use sea_orm::EntityTrait;
    use serde_json::json;
    use tokio::process::Command;

    use super::*;
    use crate::{
        AiStructuredTaskLimits, AiStructuredTaskRequest, AiTaskDataClassification,
        structured::StructuredExecutionLedger,
        structured_result::{StructuredResultKeyring, StructuredResultStore},
        structured_test_support,
    };

    async fn runtime(
        budget_limit: u64,
        max_concurrent: u32,
    ) -> (StructuredExecutionLedger, StructuredAccounting, Uuid, Uuid) {
        let database = structured_test_support::database().await;
        runtime_on(database, budget_limit, max_concurrent).await
    }

    async fn runtime_on(
        database: DatabaseConnection,
        budget_limit: u64,
        max_concurrent: u32,
    ) -> (StructuredExecutionLedger, StructuredAccounting, Uuid, Uuid) {
        let tenant_id = Uuid::new_v4();
        let provider_id = Uuid::new_v4();
        structured_test_support::insert_tenant(&database, tenant_id).await;
        structured_test_support::insert_provider_profile(
            &database,
            tenant_id,
            provider_id,
            "accounting-primary",
        )
        .await;
        let ledger = StructuredExecutionLedger::new(database.clone());
        let accounting = StructuredAccounting::new(database);
        accounting
            .put_budget(BudgetPolicy {
                tenant_id,
                currency_code: "USD".to_string(),
                limit_minor_units: budget_limit,
                max_concurrent,
            })
            .await
            .unwrap();
        accounting
            .put_provider_policy(ProviderPolicy {
                tenant_id,
                provider_profile_id: provider_id,
                allowed_classifications: vec![AiTaskDataClassification::TenantPrivate],
                currency_code: "USD".to_string(),
                input_cost_per_million_minor: 1_000_000,
                output_cost_per_million_minor: 2_000_000,
                max_concurrent: 1,
                is_active: true,
            })
            .await
            .unwrap();
        (ledger, accounting, tenant_id, provider_id)
    }

    fn context(tenant_id: Uuid, key: &str) -> PortContext {
        PortContext::new(
            tenant_id.to_string(),
            PortActor::service("translation-worker"),
            "en",
            format!("correlation-{key}"),
        )
        .with_idempotency_key(key)
        .with_deadline(Duration::from_secs(5))
    }

    fn request() -> AiStructuredTaskRequest {
        AiStructuredTaskRequest {
            owner: "translation".to_string(),
            task_slug: "machine_translation".to_string(),
            prompt_policy_digest: "a".repeat(64),
            input_schema_digest: "b".repeat(64),
            input: json!({"value": "bounded"}),
            output_schema: json!({"type": "object"}),
            classification: AiTaskDataClassification::TenantPrivate,
            evidence: Default::default(),
            limits: AiStructuredTaskLimits {
                max_output_bytes: 100,
                max_attempts: 3,
            },
        }
    }

    fn sealed(execution: &ai_structured_executions::Model) -> SealedStructuredResult {
        let created_at = Utc::now();
        SealedStructuredResult {
            tenant_id: execution.tenant_id,
            execution_id: execution.id,
            request_digest: execution.request_digest.clone(),
            output_digest: "c".repeat(64),
            key_id: "test-v1".to_string(),
            nonce: vec![1; 12],
            ciphertext: vec![2; 32],
            plaintext_bytes: 16,
            created_at,
            expires_at: created_at + chrono::Duration::minutes(5),
        }
    }

    #[tokio::test]
    async fn estimate_uses_active_price_snapshot_without_mutating_accounting_state() {
        let (_ledger, accounting, tenant_id, provider_id) = runtime(10_000, 1).await;

        let first = accounting
            .estimate(
                tenant_id,
                AiTaskDataClassification::TenantPrivate,
                25,
                100,
                3,
                &[provider_id],
            )
            .await
            .unwrap();
        let replay = accounting
            .estimate(
                tenant_id,
                AiTaskDataClassification::TenantPrivate,
                25,
                100,
                3,
                &[provider_id],
            )
            .await
            .unwrap();

        assert_eq!(first, replay);
        assert_eq!(first.input_tokens_upper_bound, 25);
        assert_eq!(first.output_tokens_upper_bound, 100);
        assert_eq!(first.attempts_upper_bound, 1);
        assert_eq!(first.cost_minor_units_upper_bound, 225);
        assert_eq!(first.currency_code, "USD");
        assert_eq!(first.price_snapshot_digest.len(), 64);
        assert_eq!(
            ai_structured_executions::Entity::find()
                .count(&accounting.database)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            ai_structured_reservations::Entity::find()
                .count(&accounting.database)
                .await
                .unwrap(),
            0
        );
        let budget = ai_structured_budgets::Entity::find()
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(budget.reserved_minor_units, 0);
        assert_eq!(budget.committed_minor_units, 0);
        assert_eq!(budget.in_flight, 0);
    }

    #[tokio::test]
    async fn provider_egress_allowlist_rejects_unapproved_classification_before_estimate() {
        let (_ledger, accounting, tenant_id, provider_id) = runtime(10_000, 1).await;

        let error = accounting
            .estimate(
                tenant_id,
                AiTaskDataClassification::Personal,
                25,
                100,
                3,
                &[provider_id],
            )
            .await
            .expect_err("personal data must not use a tenant-private-only provider");

        assert_eq!(error.kind, PortErrorKind::Forbidden);
        assert_eq!(error.code, PROVIDER_EGRESS_CLASSIFICATION_DENIED_CODE);
    }

    #[tokio::test]
    async fn provider_egress_allowlist_is_rechecked_before_attempt_egress() {
        let (ledger, accounting, tenant_id, provider_id) = runtime(10_000, 1).await;
        let execution = ledger
            .register(&context(tenant_id, "egress-policy-race"), &request())
            .await
            .unwrap()
            .execution;
        accounting
            .reserve(execution.id, &[provider_id])
            .await
            .unwrap();
        let lease = ledger
            .claim(execution.id, Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();

        accounting
            .put_provider_policy(ProviderPolicy {
                tenant_id,
                provider_profile_id: provider_id,
                currency_code: "USD".to_string(),
                input_cost_per_million_minor: 1_000_000,
                output_cost_per_million_minor: 2_000_000,
                max_concurrent: 1,
                is_active: true,
                allowed_classifications: vec![AiTaskDataClassification::Public],
            })
            .await
            .unwrap();

        let error = accounting
            .begin_attempt(
                execution.id,
                lease.token,
                provider_id,
                "openai_compatible",
                "test-model",
            )
            .await
            .expect_err("changed egress policy must stop the provider call");
        assert_eq!(error.kind, PortErrorKind::Forbidden);
        assert_eq!(error.code, PROVIDER_EGRESS_CLASSIFICATION_DENIED_CODE);
        assert!(
            ai_structured_attempts::Entity::find()
                .one(&accounting.database)
                .await
                .unwrap()
                .is_none(),
            "denied egress must not create an attempt"
        );
    }

    #[tokio::test]
    async fn provider_egress_policy_cannot_change_while_an_attempt_is_in_flight() {
        let (ledger, accounting, tenant_id, provider_id) = runtime(10_000, 1).await;
        let execution = ledger
            .register(&context(tenant_id, "egress-policy-in-flight"), &request())
            .await
            .unwrap()
            .execution;
        accounting
            .reserve(execution.id, &[provider_id])
            .await
            .unwrap();
        let lease = ledger
            .claim(execution.id, Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        let attempt = accounting
            .begin_attempt(
                execution.id,
                lease.token,
                provider_id,
                "openai_compatible",
                "test-model",
            )
            .await
            .unwrap();

        let error = accounting
            .put_provider_policy(ProviderPolicy {
                tenant_id,
                provider_profile_id: provider_id,
                allowed_classifications: vec![AiTaskDataClassification::Public],
                currency_code: "USD".to_string(),
                input_cost_per_million_minor: 1_000_000,
                output_cost_per_million_minor: 2_000_000,
                max_concurrent: 1,
                is_active: true,
            })
            .await
            .expect_err("in-flight provider egress policy must remain immutable");
        assert_eq!(error.kind, PortErrorKind::Conflict);
        assert_eq!(error.code, "ai.structured.provider_egress_policy_in_use");

        let policy = ai_structured_provider_policies::Entity::find()
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            provider_allowed_classifications(&policy).unwrap(),
            vec![AiTaskDataClassification::TenantPrivate]
        );
        assert!(policy.is_active);

        accounting
            .finish_attempt(
                execution.id,
                lease.token,
                attempt.model.id,
                AttemptOutcome::Failed {
                    usage: None,
                    error_code: "ai.structured.provider_timeout".to_string(),
                    retryable: true,
                    retry_after_ms: Some(100),
                },
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn successful_attempt_result_and_budget_settle_atomically() {
        let (ledger, accounting, tenant_id, provider_id) = runtime(10_000, 1).await;
        let execution = ledger
            .register(&context(tenant_id, "execute-a"), &request())
            .await
            .unwrap()
            .execution;

        let reserved = accounting
            .reserve(execution.id, &[provider_id])
            .await
            .unwrap();
        let replayed = accounting
            .reserve(execution.id, &[provider_id])
            .await
            .unwrap();
        assert_eq!(reserved, replayed);
        assert_eq!(
            reserved.reserved_minor_units,
            u64::try_from(execution.input_bytes).unwrap() + 200
        );

        let lease = ledger
            .claim(execution.id, Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        let attempt = accounting
            .begin_attempt(
                execution.id,
                lease.token,
                provider_id,
                "openai_compatible",
                "test-model",
            )
            .await
            .unwrap();
        let output = json!({"translated": "hello"});
        let keyring = StructuredResultKeyring::for_test(
            "test-v1",
            Duration::from_secs(300),
            std::collections::BTreeMap::from([("test-v1".to_string(), [7_u8; 32])]),
        );
        let encrypted = keyring
            .seal(tenant_id, execution.id, &execution.request_digest, &output)
            .await
            .unwrap();
        let finalized = accounting
            .complete_attempt(
                execution.id,
                lease.token,
                attempt.model.id,
                ProviderUsage::normalized(10, 5, None),
                encrypted,
            )
            .await
            .unwrap();
        assert_eq!(finalized.status, "completed");
        let store = StructuredResultStore::new(accounting.database.clone(), keyring);
        assert_eq!(
            store
                .replay(
                    tenant_id,
                    execution.id,
                    &execution.request_digest,
                    execution.max_output_bytes,
                )
                .await
                .unwrap(),
            output
        );
        assert!(
            ai_structured_results::Entity::find()
                .filter(ai_structured_results::Column::ExecutionId.eq(execution.id))
                .one(&accounting.database)
                .await
                .unwrap()
                .is_some()
        );
        let result = ai_structured_results::Entity::find()
            .filter(ai_structured_results::Column::ExecutionId.eq(execution.id))
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.replay_count, 1);
        assert!(result.last_replayed_at.is_some());

        let budget = ai_structured_budgets::Entity::find()
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(budget.reserved_minor_units, 0);
        assert_eq!(budget.committed_minor_units, 20);
        assert_eq!(budget.in_flight, 0);
        let reservation = ai_structured_reservations::Entity::find()
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reservation.state, "committed");
        assert_eq!(reservation.committed_minor_units, 20);
    }

    #[tokio::test]
    async fn result_insert_failure_rolls_back_attempt_slot_budget_and_execution() {
        let (ledger, accounting, tenant_id, provider_id) = runtime(10_000, 1).await;
        let execution = ledger
            .register(&context(tenant_id, "execute-rollback"), &request())
            .await
            .unwrap()
            .execution;
        accounting
            .reserve(execution.id, &[provider_id])
            .await
            .unwrap();
        let lease = ledger
            .claim(execution.id, Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        let attempt = accounting
            .begin_attempt(
                execution.id,
                lease.token,
                provider_id,
                "openai_compatible",
                "test-model",
            )
            .await
            .unwrap();
        let occupied = sealed(&execution);
        ai_structured_results::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(occupied.tenant_id),
            execution_id: Set(occupied.execution_id),
            request_digest: Set(occupied.request_digest),
            output_digest: Set(occupied.output_digest),
            key_id: Set(occupied.key_id),
            nonce: Set(occupied.nonce),
            ciphertext: Set(occupied.ciphertext),
            plaintext_bytes: Set(occupied.plaintext_bytes),
            replay_count: Set(0),
            created_at: Set(occupied.created_at.into()),
            expires_at: Set(occupied.expires_at.into()),
            last_replayed_at: Set(None),
        }
        .insert(&accounting.database)
        .await
        .unwrap();

        assert_eq!(
            accounting
                .complete_attempt(
                    execution.id,
                    lease.token,
                    attempt.model.id,
                    ProviderUsage::normalized(10, 5, None),
                    sealed(&execution),
                )
                .await
                .unwrap_err()
                .code,
            "ai.structured.accounting_unavailable"
        );
        let attempt = ai_structured_attempts::Entity::find_by_id(attempt.model.id)
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(attempt.status, "running");
        let execution = ai_structured_executions::Entity::find_by_id(execution.id)
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(execution.status, "running");
        let policy = ai_structured_provider_policies::Entity::find()
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(policy.in_flight, 1);
        let budget = ai_structured_budgets::Entity::find()
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert!(budget.reserved_minor_units > 0);
        assert_eq!(budget.committed_minor_units, 0);
        assert_eq!(budget.in_flight, 1);
    }

    #[tokio::test]
    async fn tenant_concurrency_fails_closed_until_first_execution_settles() {
        let (ledger, accounting, tenant_id, provider_id) = runtime(10_000, 1).await;
        let first = ledger
            .register(&context(tenant_id, "execute-a"), &request())
            .await
            .unwrap()
            .execution;
        let second = ledger
            .register(&context(tenant_id, "execute-b"), &request())
            .await
            .unwrap()
            .execution;
        accounting.reserve(first.id, &[provider_id]).await.unwrap();

        let error = accounting
            .reserve(second.id, &[provider_id])
            .await
            .unwrap_err();
        assert_eq!(error.kind, PortErrorKind::Unavailable);
        assert_eq!(error.code, "ai.structured.quota_exhausted");

        let lease = ledger
            .claim(first.id, Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        accounting
            .finalize(
                first.id,
                lease.token,
                TerminalOutcome::Failed {
                    error_code: "ai.structured.provider_unavailable".to_string(),
                    retryable: true,
                    retry_after_ms: Some(100),
                },
            )
            .await
            .unwrap();
        assert!(accounting.reserve(second.id, &[provider_id]).await.is_ok());
    }

    #[tokio::test]
    async fn queued_cancellation_atomically_releases_an_existing_reservation() {
        let (ledger, accounting, tenant_id, provider_id) = runtime(10_000, 1).await;
        let execution = ledger
            .register(&context(tenant_id, "execute-a"), &request())
            .await
            .unwrap()
            .execution;
        accounting
            .reserve(execution.id, &[provider_id])
            .await
            .unwrap();
        ledger
            .request_cancel(&context(tenant_id, "cancel-a"), execution.id)
            .await
            .unwrap();

        assert_eq!(accounting.recover_queued_cancellations().await.unwrap(), 1);
        assert_eq!(accounting.recover_queued_cancellations().await.unwrap(), 0);
        let cancelled = ai_structured_executions::Entity::find_by_id(execution.id)
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        let replayed = accounting.cancel_queued(execution.id).await.unwrap();

        assert_eq!(cancelled.status, "cancelled");
        assert_eq!(cancelled.id, replayed.id);
        let budget = ai_structured_budgets::Entity::find()
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(budget.in_flight, 0);
        assert_eq!(budget.reserved_minor_units, 0);
        let reservation = ai_structured_reservations::Entity::find()
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reservation.state, "released");
    }

    #[tokio::test]
    async fn budget_limit_rejects_before_any_reservation_is_persisted() {
        let (ledger, accounting, tenant_id, provider_id) = runtime(1, 1).await;
        let execution = ledger
            .register(&context(tenant_id, "execute-a"), &request())
            .await
            .unwrap()
            .execution;

        let error = accounting
            .reserve(execution.id, &[provider_id])
            .await
            .unwrap_err();
        assert_eq!(error.code, "ai.structured.quota_exhausted");
        assert!(
            ai_structured_reservations::Entity::find()
                .one(&accounting.database)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn attempt_captures_price_usage_cost_and_releases_provider_slot() {
        let (ledger, accounting, tenant_id, provider_id) = runtime(10_000, 1).await;
        let context = context(tenant_id, "execute-a");
        let execution = ledger
            .register(&context, &request())
            .await
            .unwrap()
            .execution;
        accounting
            .reserve(execution.id, &[provider_id])
            .await
            .unwrap();
        let lease = ledger
            .claim(execution.id, Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        let attempt = accounting
            .begin_attempt(
                execution.id,
                lease.token,
                provider_id,
                "openai_compatible",
                "test-model",
            )
            .await
            .unwrap();
        assert_eq!(attempt.model.attempt, 1);
        assert!(!attempt.model.fallback);
        assert_eq!(attempt.model.price_snapshot_digest.len(), 64);
        assert_eq!(
            accounting
                .begin_attempt(
                    execution.id,
                    lease.token,
                    provider_id,
                    "openai_compatible",
                    "test-model",
                )
                .await
                .unwrap_err()
                .code,
            "ai.structured.attempt_already_running"
        );

        let finalized = accounting
            .complete_attempt(
                execution.id,
                lease.token,
                attempt.model.id,
                ProviderUsage::normalized(10, 5, None),
                sealed(&execution),
            )
            .await
            .unwrap();
        assert_eq!(finalized.status, "completed");

        let attempt = ai_structured_attempts::Entity::find_by_id(attempt.model.id)
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(attempt.status, "completed");
        assert_eq!(attempt.input_tokens, Some(10));
        assert_eq!(attempt.output_tokens, Some(5));
        assert_eq!(attempt.total_tokens, Some(15));
        assert_eq!(attempt.cost_minor_units, Some(20));
        let policy = ai_structured_provider_policies::Entity::find()
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(policy.in_flight, 0);
        let budget = ai_structured_budgets::Entity::find()
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(budget.committed_minor_units, 20);
    }

    #[tokio::test]
    async fn provider_concurrency_fails_closed_and_reopens_after_failed_attempt() {
        let (ledger, accounting, tenant_id, provider_id) = runtime(20_000, 2).await;
        let first = ledger
            .register(&context(tenant_id, "execute-a"), &request())
            .await
            .unwrap()
            .execution;
        let second = ledger
            .register(&context(tenant_id, "execute-b"), &request())
            .await
            .unwrap()
            .execution;
        accounting.reserve(first.id, &[provider_id]).await.unwrap();
        accounting.reserve(second.id, &[provider_id]).await.unwrap();
        let first_lease = ledger
            .claim(first.id, Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        let second_lease = ledger
            .claim(second.id, Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        let first_attempt = accounting
            .begin_attempt(
                first.id,
                first_lease.token,
                provider_id,
                "openai_compatible",
                "test-model",
            )
            .await
            .unwrap();
        let error = accounting
            .begin_attempt(
                second.id,
                second_lease.token,
                provider_id,
                "openai_compatible",
                "test-model",
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, "ai.structured.provider_concurrency_exhausted");

        accounting
            .finish_attempt(
                first.id,
                first_lease.token,
                first_attempt.model.id,
                AttemptOutcome::Failed {
                    usage: None,
                    error_code: "ai.structured.provider_timeout".to_string(),
                    retryable: true,
                    retry_after_ms: Some(100),
                },
            )
            .await
            .unwrap();
        let second_attempt = accounting
            .begin_attempt(
                second.id,
                second_lease.token,
                provider_id,
                "openai_compatible",
                "test-model",
            )
            .await
            .unwrap();
        assert_eq!(second_attempt.model.attempt, 1);
    }

    #[tokio::test]
    async fn another_runtime_instance_recovers_and_reclaims_an_expired_execution() {
        let (ledger, accounting, tenant_id, provider_id) = runtime(10_000, 1).await;
        let recovery_accounting = StructuredAccounting::new(accounting.database.clone());
        let restarted_ledger = StructuredExecutionLedger::new(accounting.database.clone());
        let execution = ledger
            .register(&context(tenant_id, "execute-a"), &request())
            .await
            .unwrap()
            .execution;
        accounting
            .reserve(execution.id, &[provider_id])
            .await
            .unwrap();
        let lease = ledger
            .claim(execution.id, Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        let attempt = accounting
            .begin_attempt(
                execution.id,
                lease.token,
                provider_id,
                "openai_compatible",
                "test-model",
            )
            .await
            .unwrap();

        assert_eq!(
            recovery_accounting
                .recover_expired(Utc::now() + chrono::Duration::seconds(31))
                .await
                .unwrap(),
            1
        );

        let recovered = ai_structured_executions::Entity::find_by_id(execution.id)
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(recovered.status, "queued");
        assert_eq!(recovered.error_code.as_deref(), Some(RECOVERY_ERROR_CODE));
        assert!(recovered.retryable);
        assert!(recovered.lease_token.is_none());
        let attempt = ai_structured_attempts::Entity::find_by_id(attempt.model.id)
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(attempt.status, "failed");
        let provider = ai_structured_provider_policies::Entity::find()
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(provider.in_flight, 0);
        let budget = ai_structured_budgets::Entity::find()
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(budget.in_flight, 1);
        assert!(budget.reserved_minor_units > 0);
        let reservation = ai_structured_reservations::Entity::find()
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reservation.state, "reserved");

        let resumed = restarted_ledger
            .claim(execution.id, Duration::from_secs(30))
            .await
            .unwrap()
            .expect("a restarted runtime must reclaim the recovered execution");
        assert_ne!(resumed.token, lease.token);
    }

    #[tokio::test]
    async fn separate_process_recovers_and_reclaims_an_expired_execution() {
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace path");
        let evidence_dir = workspace.join("target/structured-process-tests");
        std::fs::create_dir_all(&evidence_dir).expect("structured process evidence directory");
        let evidence_dir = evidence_dir
            .canonicalize()
            .expect("structured process evidence path");
        assert!(evidence_dir.starts_with(workspace.join("target")));
        remove_stale_sqlite_test_files(&evidence_dir);
        let database_path =
            evidence_dir.join(format!("rustok-ai-structured-{}.sqlite", Uuid::new_v4()));
        let database = structured_test_support::database_at(&database_path).await;
        let (ledger, accounting, tenant_id, provider_id) = runtime_on(database, 10_000, 1).await;
        let execution = ledger
            .register(&context(tenant_id, "process-execute-a"), &request())
            .await
            .unwrap()
            .execution;
        accounting
            .reserve(execution.id, &[provider_id])
            .await
            .unwrap();
        let original_lease = ledger
            .claim(execution.id, Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        accounting
            .begin_attempt(
                execution.id,
                original_lease.token,
                provider_id,
                "openai_compatible",
                "test-model",
            )
            .await
            .unwrap();
        drop(ledger);
        accounting.database.clone().close().await.unwrap();
        drop(accounting);

        let output = Command::new(std::env::current_exe().expect("structured test executable"))
            .args([
                "--exact",
                "accounting::tests::structured_recovery_child_process",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("RUSTOK_AI_TEST_STRUCTURED_DB_PATH", &database_path)
            .env(
                "RUSTOK_AI_TEST_STRUCTURED_EXECUTION_ID",
                execution.id.to_string(),
            )
            .env(
                "RUSTOK_AI_TEST_STRUCTURED_ORIGINAL_LEASE",
                original_lease.token.to_string(),
            )
            .output()
            .await
            .expect("structured recovery child process");
        assert!(
            output.status.success(),
            "structured recovery child failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let database = structured_test_support::connect_file(&database_path, false).await;
        let recovered = ai_structured_executions::Entity::find_by_id(execution.id)
            .one(&database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(recovered.status, "running");
        assert!(recovered.error_code.is_none());
        assert_ne!(recovered.lease_token, Some(original_lease.token));
        let attempt = ai_structured_attempts::Entity::find()
            .filter(ai_structured_attempts::Column::ExecutionId.eq(execution.id))
            .one(&database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(attempt.status, "failed");
        assert_eq!(attempt.error_code.as_deref(), Some(RECOVERY_ERROR_CODE));
        let provider = ai_structured_provider_policies::Entity::find()
            .filter(ai_structured_provider_policies::Column::TenantId.eq(tenant_id))
            .one(&database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(provider.in_flight, 0);
        let budget = ai_structured_budgets::Entity::find()
            .filter(ai_structured_budgets::Column::TenantId.eq(tenant_id))
            .one(&database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(budget.in_flight, 1);
        assert!(budget.reserved_minor_units > 0);
        let reservation = ai_structured_reservations::Entity::find()
            .filter(ai_structured_reservations::Column::ExecutionId.eq(execution.id))
            .one(&database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reservation.state, "reserved");
        database.close().await.unwrap();
        remove_sqlite_test_files(&database_path);
    }

    #[tokio::test]
    #[ignore = "internal child process for structured recovery evidence"]
    async fn structured_recovery_child_process() {
        let Some(database_path) = std::env::var_os("RUSTOK_AI_TEST_STRUCTURED_DB_PATH") else {
            return;
        };
        let execution_id = Uuid::parse_str(
            &std::env::var("RUSTOK_AI_TEST_STRUCTURED_EXECUTION_ID")
                .expect("structured child execution id"),
        )
        .expect("structured child execution UUID");
        let original_lease = Uuid::parse_str(
            &std::env::var("RUSTOK_AI_TEST_STRUCTURED_ORIGINAL_LEASE")
                .expect("structured child original lease"),
        )
        .expect("structured child lease UUID");
        let database =
            structured_test_support::connect_file(std::path::Path::new(&database_path), false)
                .await;
        let accounting = StructuredAccounting::new(database.clone());
        assert_eq!(
            accounting
                .recover_expired(Utc::now() + chrono::Duration::seconds(31))
                .await
                .unwrap(),
            1
        );
        let lease = StructuredExecutionLedger::new(database.clone())
            .claim(execution_id, Duration::from_secs(30))
            .await
            .unwrap()
            .expect("child process must reclaim recovered execution");
        assert_ne!(lease.token, original_lease);
        database.close().await.unwrap();
    }

    fn remove_sqlite_test_files(database_path: &std::path::Path) {
        for path in [
            database_path.to_path_buf(),
            PathBuf::from(format!("{}-journal", database_path.display())),
            PathBuf::from(format!("{}-shm", database_path.display())),
            PathBuf::from(format!("{}-wal", database_path.display())),
        ] {
            if path.exists() {
                std::fs::remove_file(path).expect("remove structured process database artifact");
            }
        }
    }

    fn remove_stale_sqlite_test_files(evidence_dir: &std::path::Path) {
        for entry in
            std::fs::read_dir(evidence_dir).expect("read structured process evidence directory")
        {
            let path = entry.expect("structured process evidence entry").path();
            assert_eq!(path.parent(), Some(evidence_dir));
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            let base = name
                .strip_suffix("-journal")
                .or_else(|| name.strip_suffix("-shm"))
                .or_else(|| name.strip_suffix("-wal"))
                .unwrap_or(name);
            let Some(stem) = base.strip_suffix(".sqlite") else {
                continue;
            };
            let uuid = stem.strip_prefix("rustok-ai-structured-").unwrap_or(stem);
            if Uuid::parse_str(uuid).is_ok() {
                std::fs::remove_file(path)
                    .expect("remove stale structured process database artifact");
            }
        }
    }

    #[tokio::test]
    async fn recovery_terminalizes_non_retryable_attempt_without_rebilling() {
        let (ledger, accounting, tenant_id, provider_id) = runtime(10_000, 1).await;
        let execution = ledger
            .register(&context(tenant_id, "execute-non-retryable"), &request())
            .await
            .unwrap()
            .execution;
        accounting
            .reserve(execution.id, &[provider_id])
            .await
            .unwrap();
        let lease = ledger
            .claim(execution.id, Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        let attempt = accounting
            .begin_attempt(
                execution.id,
                lease.token,
                provider_id,
                "openai_compatible",
                "test-model",
            )
            .await
            .unwrap();
        accounting
            .finish_attempt(
                execution.id,
                lease.token,
                attempt.model.id,
                AttemptOutcome::Failed {
                    usage: None,
                    error_code: "ai.structured.provider_configuration_invalid".to_string(),
                    retryable: false,
                    retry_after_ms: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(
            accounting
                .recover_expired(Utc::now() + chrono::Duration::seconds(31))
                .await
                .unwrap(),
            1
        );
        let recovered = ai_structured_executions::Entity::find_by_id(execution.id)
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(recovered.status, "failed");
        assert_eq!(
            recovered.error_code.as_deref(),
            Some("ai.structured.provider_configuration_invalid")
        );
        assert!(!recovered.retryable);
        let reservation = ai_structured_reservations::Entity::find()
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reservation.state, "released");
    }

    #[tokio::test]
    async fn recovery_cancellation_atomically_releases_all_accounting() {
        let (ledger, accounting, tenant_id, provider_id) = runtime(10_000, 1).await;
        let execution = ledger
            .register(&context(tenant_id, "execute-a"), &request())
            .await
            .unwrap()
            .execution;
        accounting
            .reserve(execution.id, &[provider_id])
            .await
            .unwrap();
        let lease = ledger
            .claim(execution.id, Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        accounting
            .begin_attempt(
                execution.id,
                lease.token,
                provider_id,
                "openai_compatible",
                "test-model",
            )
            .await
            .unwrap();
        ledger
            .request_cancel(&context(tenant_id, "cancel-a"), execution.id)
            .await
            .unwrap();

        assert_eq!(
            accounting
                .recover_expired(Utc::now() + chrono::Duration::seconds(31))
                .await
                .unwrap(),
            1
        );

        let recovered = ai_structured_executions::Entity::find_by_id(execution.id)
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(recovered.status, "cancelled");
        assert!(recovered.completed_at.is_some());
        let provider = ai_structured_provider_policies::Entity::find()
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(provider.in_flight, 0);
        let budget = ai_structured_budgets::Entity::find()
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(budget.in_flight, 0);
        assert_eq!(budget.reserved_minor_units, 0);
        let reservation = ai_structured_reservations::Entity::find()
            .one(&accounting.database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reservation.state, "released");
    }

    #[test]
    fn token_cost_rounds_up_without_overflow() {
        assert_eq!(actual_cost(1, 0, 1, 0).unwrap(), 1);
        assert_eq!(actual_cost(1_000_000, 1_000_000, 3, 5).unwrap(), 8);
    }
}
