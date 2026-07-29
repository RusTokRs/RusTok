use std::time::Duration;

use chrono::{DateTime, Utc};
use rustok_api::{PortActorKind, PortContext, PortError, manifest_hash::hash_manifest};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, sea_query::Expr,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    AiStructuredTaskAttempt, AiStructuredTaskExecution, AiStructuredTaskRequest,
    AiStructuredTaskStatus, AiStructuredTaskUsage,
    entities::{ai_structured_attempts, ai_structured_executions},
};

const RECOVERY_ERROR_CODE: &str = "ai.structured.execution_lease_expired";

#[derive(Clone)]
pub(crate) struct StructuredExecutionLedger {
    database: DatabaseConnection,
}

#[derive(Debug, Clone)]
pub(crate) struct RegisteredExecution {
    pub execution: ai_structured_executions::Model,
    pub replayed: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ExecutionLease {
    pub execution: ai_structured_executions::Model,
    pub token: Uuid,
}

#[derive(Serialize)]
struct RequestDigestManifest<'a> {
    tenant_id: &'a str,
    actor_kind: &'static str,
    actor_id: &'a str,
    owner: &'a str,
    task_slug: &'a str,
    prompt_policy_digest: &'a str,
    input_schema_digest: &'a str,
    input_digest: &'a str,
    output_schema_digest: &'a str,
    classification: &'static str,
    evidence_digest: &'a str,
    max_output_bytes: u32,
    max_attempts: u16,
}

impl StructuredExecutionLedger {
    pub(crate) fn new(database: DatabaseConnection) -> Self {
        Self { database }
    }

    pub(crate) async fn register(
        &self,
        context: &PortContext,
        request: &AiStructuredTaskRequest,
    ) -> Result<RegisteredExecution, PortError> {
        request.validate(context)?;
        validate_context(context)?;
        let tenant_id = parse_uuid(&context.tenant_id, "tenant_id")?;
        let idempotency_key = context.idempotency_key.as_deref().ok_or_else(|| {
            PortError::validation(
                "port.idempotency_key_required",
                "write port calls require a non-empty idempotency key",
            )
        })?;
        let input_digest = hash(&request.input)?;
        let output_schema_digest = hash(&request.output_schema)?;
        let evidence_digest = hash(&request.evidence)?;
        let request_digest = hash(&RequestDigestManifest {
            tenant_id: &context.tenant_id,
            actor_kind: actor_kind(&context.actor.kind),
            actor_id: &context.actor.id,
            owner: &request.owner,
            task_slug: &request.task_slug,
            prompt_policy_digest: &request.prompt_policy_digest,
            input_schema_digest: &request.input_schema_digest,
            input_digest: &input_digest,
            output_schema_digest: &output_schema_digest,
            classification: classification_slug(request.classification),
            evidence_digest: &evidence_digest,
            max_output_bytes: request.limits.max_output_bytes,
            max_attempts: request.limits.max_attempts,
        })?;

        if let Some(existing) = self
            .find_by_idempotency(tenant_id, &request.owner, idempotency_key)
            .await?
        {
            reconcile_replay(&existing, &request_digest, context)?;
            return Ok(RegisteredExecution {
                execution: existing,
                replayed: true,
            });
        }

        let now = Utc::now();
        let active = ai_structured_executions::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            owner: Set(request.owner.clone()),
            task_slug: Set(request.task_slug.clone()),
            idempotency_key: Set(idempotency_key.to_string()),
            request_digest: Set(request_digest.clone()),
            prompt_policy_digest: Set(request.prompt_policy_digest.clone()),
            input_schema_digest: Set(request.input_schema_digest.clone()),
            input_digest: Set(input_digest),
            output_schema_digest: Set(output_schema_digest),
            classification: Set(classification_slug(request.classification).to_string()),
            evidence_digest: Set(evidence_digest),
            max_output_bytes: Set(i64::from(request.limits.max_output_bytes)),
            max_attempts: Set(i32::from(request.limits.max_attempts)),
            status: Set(status_slug(AiStructuredTaskStatus::Queued).to_string()),
            actor_kind: Set(actor_kind(&context.actor.kind).to_string()),
            actor_id: Set(context.actor.id.clone()),
            correlation_id: Set(context.correlation_id.clone()),
            causation_id: Set(context.causation_id.clone()),
            traceparent: Set(context.traceparent.clone()),
            error_code: Set(None),
            retryable: Set(false),
            retry_after_ms: Set(None),
            lease_token: Set(None),
            lease_expires_at: Set(None),
            cancel_requested_at: Set(None),
            cancel_idempotency_key: Set(None),
            cancel_request_digest: Set(None),
            cancel_actor_kind: Set(None),
            cancel_actor_id: Set(None),
            created_at: Set(now.into()),
            started_at: Set(None),
            completed_at: Set(None),
            updated_at: Set(now.into()),
        };

        match active.insert(&self.database).await {
            Ok(execution) => Ok(RegisteredExecution {
                execution,
                replayed: false,
            }),
            Err(error) => {
                let existing = self
                    .find_by_idempotency(tenant_id, &request.owner, idempotency_key)
                    .await?;
                #[cfg(test)]
                if existing.is_none() {
                    panic!("structured execution insert failed: {error}");
                }
                let existing = existing.ok_or_else(database_unavailable)?;
                reconcile_replay(&existing, &request_digest, context)?;
                Ok(RegisteredExecution {
                    execution: existing,
                    replayed: true,
                })
            }
        }
    }

    pub(crate) async fn load(
        &self,
        context: &PortContext,
        execution_id: Uuid,
    ) -> Result<ai_structured_executions::Model, PortError> {
        context.require_read_semantics()?;
        let tenant_id = parse_uuid(&context.tenant_id, "tenant_id")?;
        ai_structured_executions::Entity::find_by_id(execution_id)
            .filter(ai_structured_executions::Column::TenantId.eq(tenant_id))
            .one(&self.database)
            .await
            .map_err(|_| database_unavailable())?
            .ok_or_else(|| {
                PortError::not_found(
                    "ai.structured.execution_not_found",
                    "structured task execution was not found",
                )
            })
    }

    pub(crate) async fn view(
        &self,
        context: &PortContext,
        execution_id: Uuid,
    ) -> Result<AiStructuredTaskExecution, PortError> {
        let execution = self.load(context, execution_id).await?;
        let attempts = ai_structured_attempts::Entity::find()
            .filter(ai_structured_attempts::Column::TenantId.eq(execution.tenant_id))
            .filter(ai_structured_attempts::Column::ExecutionId.eq(execution.id))
            .order_by_asc(ai_structured_attempts::Column::Attempt)
            .all(&self.database)
            .await
            .map_err(|_| database_unavailable())?;
        map_execution(execution, attempts)
    }

    pub(crate) async fn claim(
        &self,
        execution_id: Uuid,
        lease_duration: Duration,
    ) -> Result<Option<ExecutionLease>, PortError> {
        if lease_duration.is_zero() {
            return Err(PortError::validation(
                "ai.structured.lease_duration_invalid",
                "structured execution lease duration must be positive",
            ));
        }
        let Some(execution) = ai_structured_executions::Entity::find_by_id(execution_id)
            .one(&self.database)
            .await
            .map_err(|_| database_unavailable())?
        else {
            return Ok(None);
        };
        if execution.status != status_slug(AiStructuredTaskStatus::Queued)
            || execution.cancel_requested_at.is_some()
        {
            return Ok(None);
        }
        let now = Utc::now();
        let expires_at = now
            + chrono::Duration::from_std(lease_duration).map_err(|_| {
                PortError::validation(
                    "ai.structured.lease_duration_invalid",
                    "structured execution lease duration is out of range",
                )
            })?;
        let token = Uuid::new_v4();
        let started_at = execution.started_at.unwrap_or_else(|| now.into());
        let claimed = ai_structured_executions::Entity::update_many()
            .col_expr(
                ai_structured_executions::Column::Status,
                Expr::value(status_slug(AiStructuredTaskStatus::Running)),
            )
            .col_expr(
                ai_structured_executions::Column::LeaseToken,
                Expr::value(Some(token)),
            )
            .col_expr(
                ai_structured_executions::Column::LeaseExpiresAt,
                Expr::value(Some(expires_at)),
            )
            .col_expr(
                ai_structured_executions::Column::StartedAt,
                Expr::value(Some(started_at)),
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
                ai_structured_executions::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(ai_structured_executions::Column::Id.eq(execution_id))
            .filter(
                ai_structured_executions::Column::Status
                    .eq(status_slug(AiStructuredTaskStatus::Queued)),
            )
            .filter(ai_structured_executions::Column::CancelRequestedAt.is_null())
            .exec(&self.database)
            .await
            .map_err(|_| database_unavailable())?;
        if claimed.rows_affected != 1 {
            return Ok(None);
        }
        let execution = ai_structured_executions::Entity::find_by_id(execution_id)
            .one(&self.database)
            .await
            .map_err(|_| database_unavailable())?
            .ok_or_else(ledger_invariant)?;
        Ok(Some(ExecutionLease { execution, token }))
    }

    pub(crate) async fn request_cancel(
        &self,
        context: &PortContext,
        execution_id: Uuid,
    ) -> Result<ai_structured_executions::Model, PortError> {
        context.require_write_semantics()?;
        validate_context(context)?;
        let mut execution = self.load(context, execution_id).await?;
        let idempotency_key = context.idempotency_key.as_deref().unwrap_or_default();
        let request_digest = hash(&(
            context.tenant_id.as_str(),
            execution_id,
            actor_kind(&context.actor.kind),
            context.actor.id.as_str(),
        ))?;

        if let Some(existing_key) = execution.cancel_idempotency_key.as_deref() {
            if existing_key != idempotency_key
                || execution.cancel_request_digest.as_deref() != Some(request_digest.as_str())
                || execution.cancel_actor_kind.as_deref() != Some(actor_kind(&context.actor.kind))
                || execution.cancel_actor_id.as_deref() != Some(context.actor.id.as_str())
            {
                return Err(PortError::conflict(
                    "ai.structured.cancel_conflict",
                    "structured execution already has a different cancellation request",
                ));
            }
            return Ok(execution);
        }

        let now = Utc::now();
        let queued = execution.status == status_slug(AiStructuredTaskStatus::Queued);
        let terminal = is_terminal(&execution.status);
        let mut update = ai_structured_executions::Entity::update_many()
            .col_expr(
                ai_structured_executions::Column::CancelRequestedAt,
                Expr::value(Some(now)),
            )
            .col_expr(
                ai_structured_executions::Column::CancelIdempotencyKey,
                Expr::value(Some(idempotency_key.to_string())),
            )
            .col_expr(
                ai_structured_executions::Column::CancelRequestDigest,
                Expr::value(Some(request_digest.clone())),
            )
            .col_expr(
                ai_structured_executions::Column::CancelActorKind,
                Expr::value(Some(actor_kind(&context.actor.kind).to_string())),
            )
            .col_expr(
                ai_structured_executions::Column::CancelActorId,
                Expr::value(Some(context.actor.id.clone())),
            )
            .col_expr(
                ai_structured_executions::Column::UpdatedAt,
                Expr::value(now),
            );
        if queued {
            update = update
                .col_expr(
                    ai_structured_executions::Column::Status,
                    Expr::value(status_slug(AiStructuredTaskStatus::Cancelled)),
                )
                .col_expr(
                    ai_structured_executions::Column::CompletedAt,
                    Expr::value(Some(now)),
                );
        }
        let changed = update
            .filter(ai_structured_executions::Column::Id.eq(execution_id))
            .filter(ai_structured_executions::Column::TenantId.eq(execution.tenant_id))
            .filter(ai_structured_executions::Column::Status.eq(execution.status.clone()))
            .filter(ai_structured_executions::Column::CancelIdempotencyKey.is_null())
            .exec(&self.database)
            .await
            .map_err(|_| database_unavailable())?;
        if changed.rows_affected == 0 && !terminal {
            execution = self.load(context, execution_id).await?;
            if execution.cancel_idempotency_key.is_none() {
                return Err(PortError::conflict(
                    "ai.structured.cancel_state_conflict",
                    "structured execution changed while cancellation was requested",
                ));
            }
        } else {
            execution = self.load(context, execution_id).await?;
        }
        if execution.cancel_idempotency_key.as_deref() != Some(idempotency_key)
            || execution.cancel_request_digest.as_deref() != Some(request_digest.as_str())
        {
            return Err(PortError::conflict(
                "ai.structured.cancel_conflict",
                "structured execution already has a different cancellation request",
            ));
        }
        Ok(execution)
    }

    pub(crate) async fn recover_expired(&self, now: DateTime<Utc>) -> Result<u64, PortError> {
        let expired = ai_structured_executions::Entity::find()
            .filter(
                ai_structured_executions::Column::Status
                    .eq(status_slug(AiStructuredTaskStatus::Running)),
            )
            .filter(ai_structured_executions::Column::LeaseExpiresAt.lt(now))
            .all(&self.database)
            .await
            .map_err(|_| database_unavailable())?;
        let mut recovered = 0_u64;
        for execution in expired {
            let cancelled = execution.cancel_requested_at.is_some();
            let result = ai_structured_executions::Entity::update_many()
                .col_expr(
                    ai_structured_executions::Column::Status,
                    Expr::value(status_slug(if cancelled {
                        AiStructuredTaskStatus::Cancelled
                    } else {
                        AiStructuredTaskStatus::Queued
                    })),
                )
                .col_expr(
                    ai_structured_executions::Column::ErrorCode,
                    Expr::value((!cancelled).then(|| RECOVERY_ERROR_CODE.to_string())),
                )
                .col_expr(
                    ai_structured_executions::Column::Retryable,
                    Expr::value(!cancelled),
                )
                .col_expr(
                    ai_structured_executions::Column::RetryAfterMs,
                    Expr::value((!cancelled).then_some(0_i64)),
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
                    Expr::value(cancelled.then_some(now)),
                )
                .col_expr(
                    ai_structured_executions::Column::UpdatedAt,
                    Expr::value(now),
                )
                .filter(ai_structured_executions::Column::Id.eq(execution.id))
                .filter(
                    ai_structured_executions::Column::Status
                        .eq(status_slug(AiStructuredTaskStatus::Running)),
                )
                .filter(ai_structured_executions::Column::LeaseToken.eq(execution.lease_token))
                .filter(ai_structured_executions::Column::LeaseExpiresAt.lt(now))
                .exec(&self.database)
                .await
                .map_err(|_| database_unavailable())?;
            if result.rows_affected == 1 {
                recovered = recovered.saturating_add(1);
            }
        }
        Ok(recovered)
    }

    async fn find_by_idempotency(
        &self,
        tenant_id: Uuid,
        owner: &str,
        idempotency_key: &str,
    ) -> Result<Option<ai_structured_executions::Model>, PortError> {
        ai_structured_executions::Entity::find()
            .filter(ai_structured_executions::Column::TenantId.eq(tenant_id))
            .filter(ai_structured_executions::Column::Owner.eq(owner))
            .filter(ai_structured_executions::Column::IdempotencyKey.eq(idempotency_key))
            .one(&self.database)
            .await
            .map_err(|_| database_unavailable())
    }
}

fn reconcile_replay(
    existing: &ai_structured_executions::Model,
    request_digest: &str,
    context: &PortContext,
) -> Result<(), PortError> {
    if existing.request_digest != request_digest
        || existing.actor_kind != actor_kind(&context.actor.kind)
        || existing.actor_id != context.actor.id
    {
        return Err(PortError::conflict(
            "ai.structured.idempotency_conflict",
            "structured execution idempotency key was reused with another request",
        ));
    }
    Ok(())
}

fn map_execution(
    execution: ai_structured_executions::Model,
    attempts: Vec<ai_structured_attempts::Model>,
) -> Result<AiStructuredTaskExecution, PortError> {
    let status = parse_status(&execution.status)?;
    let attempt_evidence = attempts
        .iter()
        .map(|attempt| {
            Ok(AiStructuredTaskAttempt {
                attempt: u16::try_from(attempt.attempt).map_err(|_| ledger_invariant())?,
                provider_profile_id: attempt.provider_profile_id.to_string(),
                provider_slug: attempt.provider_slug.clone(),
                model: attempt.model.clone(),
                fallback: attempt.fallback,
                status: parse_status(&attempt.status)?,
                error_code: attempt.error_code.clone(),
            })
        })
        .collect::<Result<Vec<_>, PortError>>()?;
    let usage = aggregate_usage(&attempts)?;
    Ok(AiStructuredTaskExecution {
        execution_id: execution.id.to_string(),
        request_digest: execution.request_digest,
        status,
        output: None,
        attempts: attempt_evidence,
        usage,
        retry_after_ms: execution
            .retry_after_ms
            .map(u64::try_from)
            .transpose()
            .map_err(|_| ledger_invariant())?,
    })
}

fn aggregate_usage(
    attempts: &[ai_structured_attempts::Model],
) -> Result<Option<AiStructuredTaskUsage>, PortError> {
    let accounted = attempts
        .iter()
        .filter(|attempt| attempt.cost_minor_units.is_some())
        .collect::<Vec<_>>();
    if accounted.is_empty() {
        return Ok(None);
    }
    let currency = accounted[0].currency_code.clone();
    if accounted
        .iter()
        .any(|attempt| attempt.currency_code != currency)
    {
        return Err(ledger_invariant());
    }
    let mut input_tokens = 0_u64;
    let mut output_tokens = 0_u64;
    let mut cost_minor_units = 0_u64;
    let mut price_snapshots = Vec::with_capacity(accounted.len());
    for attempt in accounted {
        input_tokens = input_tokens
            .checked_add(nonnegative(attempt.input_tokens)?)
            .ok_or_else(ledger_invariant)?;
        output_tokens = output_tokens
            .checked_add(nonnegative(attempt.output_tokens)?)
            .ok_or_else(ledger_invariant)?;
        cost_minor_units = cost_minor_units
            .checked_add(nonnegative(attempt.cost_minor_units)?)
            .ok_or_else(ledger_invariant)?;
        price_snapshots.push(&attempt.price_snapshot_digest);
    }
    Ok(Some(AiStructuredTaskUsage {
        input_tokens,
        output_tokens,
        total_tokens: input_tokens
            .checked_add(output_tokens)
            .ok_or_else(ledger_invariant)?,
        cost_minor_units,
        currency_code: currency,
        price_snapshot_digest: hash(&price_snapshots)?,
    }))
}

fn nonnegative(value: Option<i64>) -> Result<u64, PortError> {
    value
        .ok_or_else(ledger_invariant)
        .and_then(|value| u64::try_from(value).map_err(|_| ledger_invariant()))
}

fn validate_context(context: &PortContext) -> Result<(), PortError> {
    for (field, value, limit) in [
        ("actor_id", context.actor.id.as_str(), 191_usize),
        ("correlation_id", context.correlation_id.as_str(), 191),
    ] {
        if value.trim().is_empty() || value.len() > limit {
            return Err(PortError::validation(
                format!("ai.structured.{field}_invalid"),
                format!("{field} must contain 1..={limit} non-whitespace bytes"),
            ));
        }
    }
    if context
        .idempotency_key
        .as_deref()
        .is_some_and(|value| value.len() > 191)
    {
        return Err(PortError::validation(
            "ai.structured.idempotency_key_too_long",
            "structured execution idempotency key exceeds 191 bytes",
        ));
    }
    Ok(())
}

fn parse_uuid(value: &str, field: &str) -> Result<Uuid, PortError> {
    Uuid::parse_str(value).map_err(|_| {
        PortError::validation(
            format!("ai.structured.{field}_invalid"),
            format!("{field} must be a UUID"),
        )
    })
}

fn hash<T: Serialize>(value: &T) -> Result<String, PortError> {
    hash_manifest(value).map_err(|_| {
        PortError::invariant_violation(
            "ai.structured.digest_failed",
            "structured execution evidence could not be hashed",
        )
    })
}

const fn actor_kind(kind: &PortActorKind) -> &'static str {
    match kind {
        PortActorKind::User => "user",
        PortActorKind::Service => "service",
        PortActorKind::System => "system",
    }
}

const fn classification_slug(classification: crate::AiTaskDataClassification) -> &'static str {
    match classification {
        crate::AiTaskDataClassification::Public => "public",
        crate::AiTaskDataClassification::TenantPrivate => "tenant_private",
        crate::AiTaskDataClassification::Personal => "personal",
        crate::AiTaskDataClassification::Sensitive => "sensitive",
    }
}

const fn status_slug(status: AiStructuredTaskStatus) -> &'static str {
    match status {
        AiStructuredTaskStatus::Queued => "queued",
        AiStructuredTaskStatus::Running => "running",
        AiStructuredTaskStatus::Completed => "completed",
        AiStructuredTaskStatus::Failed => "failed",
        AiStructuredTaskStatus::Cancelled => "cancelled",
    }
}

fn parse_status(value: &str) -> Result<AiStructuredTaskStatus, PortError> {
    match value {
        "queued" => Ok(AiStructuredTaskStatus::Queued),
        "running" => Ok(AiStructuredTaskStatus::Running),
        "completed" => Ok(AiStructuredTaskStatus::Completed),
        "failed" => Ok(AiStructuredTaskStatus::Failed),
        "cancelled" => Ok(AiStructuredTaskStatus::Cancelled),
        _ => Err(ledger_invariant()),
    }
}

fn is_terminal(status: &str) -> bool {
    matches!(status, "completed" | "failed" | "cancelled")
}

fn database_unavailable() -> PortError {
    PortError::unavailable(
        "ai.structured.persistence_unavailable",
        "structured execution persistence is unavailable",
    )
}

fn ledger_invariant() -> PortError {
    PortError::invariant_violation(
        "ai.structured.ledger_invalid",
        "structured execution ledger contains invalid evidence",
    )
}

#[cfg(test)]
mod tests {
    use rustok_api::{PortActor, PortErrorKind};
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    use sea_orm_migration::{MigrationTrait, SchemaManager};
    use serde_json::json;

    use super::*;
    use crate::{
        AiStructuredTaskLimits, AiTaskDataClassification,
        migrations::m20260729_000001_structured_execution::Migration,
    };

    async fn ledger() -> (StructuredExecutionLedger, Uuid) {
        let database = Database::connect("sqlite::memory:").await.unwrap();
        database
            .execute_unprepared(
                "PRAGMA foreign_keys = ON; \
                 CREATE TABLE tenants (id UUID PRIMARY KEY); \
                 CREATE TABLE ai_provider_profiles (id UUID PRIMARY KEY)",
            )
            .await
            .unwrap();
        Migration.up(&SchemaManager::new(&database)).await.unwrap();
        let tenant_id = Uuid::new_v4();
        database
            .execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO tenants (id) VALUES (?)".to_string(),
                vec![tenant_id.into()],
            ))
            .await
            .unwrap();
        (StructuredExecutionLedger::new(database), tenant_id)
    }

    fn context(tenant_id: Uuid, idempotency_key: &str) -> PortContext {
        PortContext::new(
            tenant_id.to_string(),
            PortActor::service("translation-worker"),
            "en",
            "correlation-a",
        )
        .with_idempotency_key(idempotency_key)
        .with_deadline(Duration::from_secs(5))
    }

    fn request(input: serde_json::Value) -> AiStructuredTaskRequest {
        AiStructuredTaskRequest {
            owner: "translation".to_string(),
            task_slug: "machine_translation".to_string(),
            prompt_policy_digest: "a".repeat(64),
            input_schema_digest: "b".repeat(64),
            input,
            output_schema: json!({"type": "object"}),
            classification: AiTaskDataClassification::TenantPrivate,
            evidence: [("job_id".to_string(), "job-a".to_string())]
                .into_iter()
                .collect(),
            limits: AiStructuredTaskLimits {
                max_output_bytes: 4096,
                max_attempts: 3,
            },
        }
    }

    #[tokio::test]
    async fn replay_preserves_identity_without_persisting_payload() {
        let (ledger, tenant_id) = ledger().await;
        let context = context(tenant_id, "idem-a");
        let request = request(json!({"secret_source_text": "Do not persist me"}));

        let created = ledger.register(&context, &request).await.unwrap();
        let replayed = ledger.register(&context, &request).await.unwrap();

        assert!(!created.replayed);
        assert!(replayed.replayed);
        assert_eq!(created.execution.id, replayed.execution.id);
        assert_ne!(
            created.execution.input_digest, "Do not persist me",
            "only a digest may enter the execution ledger"
        );
        let serialized = serde_json::to_string(&created.execution).unwrap();
        assert!(!serialized.contains("Do not persist me"));
        assert!(!serialized.contains("job-a"));
    }

    #[tokio::test]
    async fn replay_rejects_request_hash_conflict() {
        let (ledger, tenant_id) = ledger().await;
        let context = context(tenant_id, "idem-a");
        ledger
            .register(&context, &request(json!({"value": "one"})))
            .await
            .unwrap();

        let error = ledger
            .register(&context, &request(json!({"value": "two"})))
            .await
            .unwrap_err();
        assert_eq!(error.kind, PortErrorKind::Conflict);
        assert_eq!(error.code, "ai.structured.idempotency_conflict");
    }

    #[tokio::test]
    async fn queued_cancel_is_durable_and_idempotent() {
        let (ledger, tenant_id) = ledger().await;
        let execute_context = context(tenant_id, "execute-a");
        let execution = ledger
            .register(&execute_context, &request(json!({"value": "one"})))
            .await
            .unwrap()
            .execution;
        let cancel_context = context(tenant_id, "cancel-a");

        let cancelled = ledger
            .request_cancel(&cancel_context, execution.id)
            .await
            .unwrap();
        let replayed = ledger
            .request_cancel(&cancel_context, execution.id)
            .await
            .unwrap();

        assert_eq!(cancelled.status, "cancelled");
        assert_eq!(
            cancelled.cancel_idempotency_key.as_deref(),
            Some("cancel-a")
        );
        assert_eq!(cancelled.id, replayed.id);
        assert!(replayed.completed_at.is_some());
    }

    #[tokio::test]
    async fn expired_lease_requeues_without_losing_execution_identity() {
        let (ledger, tenant_id) = ledger().await;
        let context = context(tenant_id, "execute-a");
        let execution = ledger
            .register(&context, &request(json!({"value": "one"})))
            .await
            .unwrap()
            .execution;
        let lease = ledger
            .claim(execution.id, Duration::from_millis(1))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(lease.execution.status, "running");
        assert_ne!(lease.token, Uuid::nil());
        let recovered = ledger
            .recover_expired(Utc::now() + chrono::Duration::seconds(1))
            .await
            .unwrap();
        assert_eq!(recovered, 1);

        let requeued = ledger.load(&context, execution.id).await.unwrap();
        assert_eq!(requeued.status, "queued");
        assert_eq!(requeued.error_code.as_deref(), Some(RECOVERY_ERROR_CODE));
        assert!(requeued.retryable);
        assert!(requeued.lease_token.is_none());
    }
}
