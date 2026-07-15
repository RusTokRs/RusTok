use chrono::{Duration, Utc};
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set, TransactionTrait,
};

use crate::models::{
    registry_governance_event, registry_publish_request, registry_validation_stage,
};
use crate::services::registry_governance::{
    request_status_label, validation_stage_status_label, REGISTRY_VALIDATION_STAGE_REASON_CODES,
};
use crate::services::registry_principal::RegistryPrincipalRef;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteTerminalOutcome {
    Passed,
    Failed,
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryRemoteTransitionError {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Internal(String),
}

pub async fn heartbeat_remote_validation_stage_atomic(
    db: &DatabaseConnection,
    claim_id: &str,
    runner_id: &str,
    lease_ttl_ms: u64,
) -> Result<registry_validation_stage::Model, RegistryRemoteTransitionError> {
    let claim_id = required(claim_id, "claim_id")?;
    let runner_id = required(runner_id, "runner_id")?;
    let now = Utc::now();
    let expires_at = now + lease_ttl(lease_ttl_ms);

    let updated = registry_validation_stage::Entity::update_many()
        .col_expr(
            registry_validation_stage::Column::LastHeartbeatAt,
            Expr::value(Some(now)),
        )
        .col_expr(
            registry_validation_stage::Column::ClaimExpiresAt,
            Expr::value(Some(expires_at)),
        )
        .col_expr(
            registry_validation_stage::Column::UpdatedAt,
            Expr::value(now),
        )
        .filter(registry_validation_stage::Column::ClaimId.eq(claim_id))
        .filter(registry_validation_stage::Column::ClaimedBy.eq(runner_id))
        .filter(
            registry_validation_stage::Column::Status
                .eq(registry_validation_stage::RegistryValidationStageStatus::Running),
        )
        .filter(registry_validation_stage::Column::ClaimExpiresAt.gte(now))
        .exec(db)
        .await
        .map_err(internal)?;

    if updated.rows_affected != 1 {
        return Err(classify_claim_failure(db, claim_id, runner_id, now).await);
    }

    registry_validation_stage::Entity::find()
        .filter(registry_validation_stage::Column::ClaimId.eq(claim_id))
        .one(db)
        .await
        .map_err(internal)?
        .ok_or_else(|| {
            RegistryRemoteTransitionError::Internal(
                "Heartbeat succeeded but registry validation stage disappeared".to_string(),
            )
        })
}

pub async fn finish_remote_validation_stage_atomic(
    db: &DatabaseConnection,
    claim_id: &str,
    runner_id: &str,
    outcome: RemoteTerminalOutcome,
    detail: Option<&str>,
    reason_code: Option<&str>,
) -> Result<registry_validation_stage::Model, RegistryRemoteTransitionError> {
    let claim_id = required(claim_id, "claim_id")?;
    let runner_id = required(runner_id, "runner_id")?;
    let now = Utc::now();
    let current = registry_validation_stage::Entity::find()
        .filter(registry_validation_stage::Column::ClaimId.eq(claim_id))
        .one(db)
        .await
        .map_err(internal)?
        .ok_or_else(|| {
            RegistryRemoteTransitionError::NotFound(format!(
                "Remote validation claim '{claim_id}' was not found"
            ))
        })?;
    validate_claim_state(&current, runner_id, now)?;

    let request = registry_publish_request::Entity::find_by_id(&current.request_id)
        .one(db)
        .await
        .map_err(internal)?
        .ok_or_else(|| {
            RegistryRemoteTransitionError::Internal(format!(
                "Remote validation claim '{}' points to missing request '{}'",
                claim_id, current.request_id
            ))
        })?;
    let reason_code = normalize_reason_code(
        reason_code.unwrap_or_else(|| default_reason_code(&current.stage_key, outcome)),
    )?;
    let detail = detail
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| default_detail(&current.stage_key, &request.slug, outcome));
    let terminal_status = match outcome {
        RemoteTerminalOutcome::Passed => {
            registry_validation_stage::RegistryValidationStageStatus::Passed
        }
        RemoteTerminalOutcome::Failed => {
            registry_validation_stage::RegistryValidationStageStatus::Failed
        }
    };

    let tx = db.begin().await.map_err(internal)?;
    let updated = registry_validation_stage::Entity::update_many()
        .col_expr(
            registry_validation_stage::Column::Status,
            Expr::value(terminal_status.clone()),
        )
        .col_expr(
            registry_validation_stage::Column::Detail,
            Expr::value(detail.clone()),
        )
        .col_expr(
            registry_validation_stage::Column::LastError,
            Expr::value(match outcome {
                RemoteTerminalOutcome::Passed => Option::<String>::None,
                RemoteTerminalOutcome::Failed => Some(detail.clone()),
            }),
        )
        .col_expr(
            registry_validation_stage::Column::StartedAt,
            Expr::value(current.started_at.or(Some(now))),
        )
        .col_expr(
            registry_validation_stage::Column::FinishedAt,
            Expr::value(Some(now)),
        )
        .col_expr(
            registry_validation_stage::Column::ClaimId,
            Expr::value(Option::<String>::None),
        )
        .col_expr(
            registry_validation_stage::Column::ClaimedBy,
            Expr::value(Option::<String>::None),
        )
        .col_expr(
            registry_validation_stage::Column::ClaimExpiresAt,
            Expr::value(Option::<chrono::DateTime<Utc>>::None),
        )
        .col_expr(
            registry_validation_stage::Column::LastHeartbeatAt,
            Expr::value(Option::<chrono::DateTime<Utc>>::None),
        )
        .col_expr(
            registry_validation_stage::Column::RunnerKind,
            Expr::value(Option::<String>::None),
        )
        .col_expr(
            registry_validation_stage::Column::UpdatedAt,
            Expr::value(now),
        )
        .filter(registry_validation_stage::Column::Id.eq(&current.id))
        .filter(registry_validation_stage::Column::ClaimId.eq(claim_id))
        .filter(registry_validation_stage::Column::ClaimedBy.eq(runner_id))
        .filter(
            registry_validation_stage::Column::Status
                .eq(registry_validation_stage::RegistryValidationStageStatus::Running),
        )
        .filter(registry_validation_stage::Column::ClaimExpiresAt.gte(now))
        .exec(&tx)
        .await
        .map_err(internal)?;

    if updated.rows_affected != 1 {
        tx.rollback().await.map_err(internal)?;
        return Err(classify_claim_failure(db, claim_id, runner_id, now).await);
    }

    let stage = registry_validation_stage::Entity::find_by_id(&current.id)
        .one(&tx)
        .await
        .map_err(internal)?
        .ok_or_else(|| {
            RegistryRemoteTransitionError::Internal(
                "Terminal update succeeded but registry validation stage disappeared".to_string(),
            )
        })?;
    let actor = format!("remote-runner:{runner_id}");
    insert_stage_event(
        &tx,
        &request,
        &stage,
        &actor,
        outcome,
        &detail,
        &reason_code,
    )
    .await?;
    insert_gate_event(
        &tx,
        &request,
        &stage,
        &actor,
        outcome,
        &detail,
        &reason_code,
    )
    .await?;
    tx.commit().await.map_err(internal)?;

    Ok(stage)
}

async fn classify_claim_failure(
    db: &DatabaseConnection,
    claim_id: &str,
    runner_id: &str,
    now: chrono::DateTime<Utc>,
) -> RegistryRemoteTransitionError {
    match registry_validation_stage::Entity::find()
        .filter(registry_validation_stage::Column::ClaimId.eq(claim_id))
        .one(db)
        .await
    {
        Err(error) => internal(error),
        Ok(None) => RegistryRemoteTransitionError::NotFound(format!(
            "Remote validation claim '{claim_id}' was not found or was already completed"
        )),
        Ok(Some(stage)) => validate_claim_state(&stage, runner_id, now)
            .err()
            .unwrap_or_else(|| {
                RegistryRemoteTransitionError::Conflict(format!(
                    "Remote validation claim '{claim_id}' changed concurrently"
                ))
            }),
    }
}

fn validate_claim_state(
    stage: &registry_validation_stage::Model,
    runner_id: &str,
    now: chrono::DateTime<Utc>,
) -> Result<(), RegistryRemoteTransitionError> {
    if stage.claimed_by.as_deref() != Some(runner_id) {
        return Err(RegistryRemoteTransitionError::Forbidden(format!(
            "Remote validation claim '{}' belongs to another runner",
            stage.claim_id.as_deref().unwrap_or("unknown")
        )));
    }
    if stage.status != registry_validation_stage::RegistryValidationStageStatus::Running {
        return Err(RegistryRemoteTransitionError::Conflict(format!(
            "Remote validation claim '{}' is in status '{}'",
            stage.claim_id.as_deref().unwrap_or("unknown"),
            validation_stage_status_label(stage.status.clone())
        )));
    }
    if stage
        .claim_expires_at
        .as_ref()
        .is_none_or(|expires_at| *expires_at < now)
    {
        return Err(RegistryRemoteTransitionError::Conflict(format!(
            "Remote validation claim '{}' has expired",
            stage.claim_id.as_deref().unwrap_or("unknown")
        )));
    }
    Ok(())
}

async fn insert_stage_event<C>(
    db: &C,
    request: &registry_publish_request::Model,
    stage: &registry_validation_stage::Model,
    actor: &str,
    outcome: RemoteTerminalOutcome,
    detail: &str,
    reason_code: &str,
) -> Result<(), RegistryRemoteTransitionError>
where
    C: sea_orm::ConnectionTrait,
{
    let event_type = match outcome {
        RemoteTerminalOutcome::Passed => "validation_stage_passed",
        RemoteTerminalOutcome::Failed => "validation_stage_failed",
    };
    registry_governance_event::ActiveModel {
        id: Set(format!("rge_{}", uuid::Uuid::new_v4().simple())),
        slug: Set(request.slug.clone()),
        request_id: Set(Some(request.id.clone())),
        release_id: Set(None),
        event_type: Set(event_type.to_string()),
        actor: Set(RegistryPrincipalRef::from_legacy_value(actor).to_json_value()),
        publisher: Set(None),
        details: Set(serde_json::json!({
            "stage_id": stage.id.clone(),
            "stage_key": stage.stage_key.clone(),
            "status": validation_stage_status_label(stage.status.clone()),
            "detail": detail,
            "attempt_number": stage.attempt_number,
            "queue_reason": stage.queue_reason.clone(),
            "request_status": request_status_label(request.status.clone()),
            "version": request.version.clone(),
            "started_at": stage.started_at.as_ref().map(|value| value.to_rfc3339()),
            "finished_at": stage.finished_at.as_ref().map(|value| value.to_rfc3339()),
            "reason_code": reason_code,
        })),
        created_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .map_err(internal)?;
    Ok(())
}

async fn insert_gate_event<C>(
    db: &C,
    request: &registry_publish_request::Model,
    stage: &registry_validation_stage::Model,
    actor: &str,
    outcome: RemoteTerminalOutcome,
    detail: &str,
    reason_code: &str,
) -> Result<(), RegistryRemoteTransitionError>
where
    C: sea_orm::ConnectionTrait,
{
    let (event_type, status) = match outcome {
        RemoteTerminalOutcome::Passed => ("follow_up_gate_passed", "passed"),
        RemoteTerminalOutcome::Failed => ("follow_up_gate_failed", "failed"),
    };
    registry_governance_event::ActiveModel {
        id: Set(format!("rge_{}", uuid::Uuid::new_v4().simple())),
        slug: Set(request.slug.clone()),
        request_id: Set(Some(request.id.clone())),
        release_id: Set(None),
        event_type: Set(event_type.to_string()),
        actor: Set(RegistryPrincipalRef::from_legacy_value(actor).to_json_value()),
        publisher: Set(None),
        details: Set(serde_json::json!({
            "stage_key": stage.stage_key.clone(),
            "status": status,
            "detail": detail,
            "reason_code": reason_code,
        })),
        created_at: Set(Utc::now()),
    }
    .insert(db)
    .await
    .map_err(internal)?;
    Ok(())
}

fn required<'a>(value: &'a str, field: &str) -> Result<&'a str, RegistryRemoteTransitionError> {
    let value = value.trim();
    if value.is_empty() {
        Err(RegistryRemoteTransitionError::Invalid(format!(
            "Remote validation transition requires non-empty {field}"
        )))
    } else {
        Ok(value)
    }
}

fn normalize_reason_code(value: &str) -> Result<String, RegistryRemoteTransitionError> {
    let value = value.trim().to_ascii_lowercase();
    if REGISTRY_VALIDATION_STAGE_REASON_CODES
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(&value))
    {
        Ok(value)
    } else {
        Err(RegistryRemoteTransitionError::Invalid(format!(
            "Unsupported remote validation reason_code '{}'; expected one of {}",
            value,
            REGISTRY_VALIDATION_STAGE_REASON_CODES.join(", ")
        )))
    }
}

fn lease_ttl(lease_ttl_ms: u64) -> Duration {
    Duration::milliseconds(lease_ttl_ms.max(1).min(i64::MAX as u64) as i64)
}

fn default_reason_code(stage_key: &str, outcome: RemoteTerminalOutcome) -> &'static str {
    match (stage_key, outcome) {
        ("compile_smoke", RemoteTerminalOutcome::Passed)
        | ("targeted_tests", RemoteTerminalOutcome::Passed) => "local_runner_passed",
        ("security_policy_review", RemoteTerminalOutcome::Passed) => "manual_review_complete",
        ("compile_smoke", RemoteTerminalOutcome::Failed) => "build_failure",
        ("targeted_tests", RemoteTerminalOutcome::Failed) => "test_failure",
        ("security_policy_review", RemoteTerminalOutcome::Failed) => "policy_preflight_failed",
        (_, RemoteTerminalOutcome::Passed) => "manual_review_complete",
        (_, RemoteTerminalOutcome::Failed) => "manual_override",
    }
}

fn default_detail(stage_key: &str, slug: &str, outcome: RemoteTerminalOutcome) -> String {
    let result = match outcome {
        RemoteTerminalOutcome::Passed => "passed",
        RemoteTerminalOutcome::Failed => "failed",
    };
    format!("Remote validation stage '{stage_key}' {result} for registry module '{slug}'.")
}

fn internal(error: impl std::fmt::Display) -> RegistryRemoteTransitionError {
    RegistryRemoteTransitionError::Internal(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{default_reason_code, RemoteTerminalOutcome};

    #[test]
    fn terminal_reason_defaults_match_stage_semantics() {
        assert_eq!(
            default_reason_code("compile_smoke", RemoteTerminalOutcome::Failed),
            "build_failure"
        );
        assert_eq!(
            default_reason_code("targeted_tests", RemoteTerminalOutcome::Failed),
            "test_failure"
        );
        assert_eq!(
            default_reason_code("security_policy_review", RemoteTerminalOutcome::Passed),
            "manual_review_complete"
        );
    }
}
