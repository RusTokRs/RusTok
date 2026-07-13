use chrono::{Duration, Utc};
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};

use crate::models::{
    registry_governance_event, registry_publish_request, registry_validation_stage,
};
use crate::services::registry_governance::{
    request_status_label, RegistryRemoteValidationClaim, REGISTRY_VALIDATION_STAGE_REASON_CODES,
};
use crate::services::registry_principal::RegistryPrincipalRef;

const FOLLOW_UP_STAGES: &[&str] =
    &["compile_smoke", "targeted_tests", "security_policy_review"];
const MAX_CLAIM_CANDIDATES: u64 = 128;

/// Claim the first eligible remote validation stage with a database
/// compare-and-swap.
///
/// Candidate discovery is intentionally advisory. The conditional update is
/// the serialization point: only one runner can move a stage from `queued`
/// with an expired/no lease to `running`. The claim and its governance event
/// are committed in the same transaction.
pub async fn claim_remote_validation_stage_atomic(
    db: &DatabaseConnection,
    runner_id: &str,
    supported_stages: &[String],
    lease_ttl_ms: u64,
) -> anyhow::Result<Option<RegistryRemoteValidationClaim>> {
    let runner_id = runner_id.trim();
    if runner_id.is_empty() {
        anyhow::bail!("Remote validation runner must provide a non-empty runner_id");
    }
    let supported_stages = normalize_supported_stages(supported_stages)?;
    if supported_stages.is_empty() {
        return Ok(None);
    }

    let discovery_now = Utc::now();
    let candidates = registry_validation_stage::Entity::find()
        .filter(
            Condition::all()
                .add(
                    registry_validation_stage::Column::Status
                        .eq(registry_validation_stage::RegistryValidationStageStatus::Queued),
                )
                .add(registry_validation_stage::Column::StageKey.is_in(supported_stages))
                .add(
                    Condition::any()
                        .add(registry_validation_stage::Column::ClaimExpiresAt.is_null())
                        .add(
                            registry_validation_stage::Column::ClaimExpiresAt.lte(discovery_now),
                        ),
                ),
        )
        .order_by_asc(registry_validation_stage::Column::CreatedAt)
        .limit(MAX_CLAIM_CANDIDATES)
        .all(db)
        .await?;

    for candidate in candidates {
        let tx = db.begin().await?;
        let Some(request) = registry_publish_request::Entity::find_by_id(&candidate.request_id)
            .one(&tx)
            .await?
        else {
            tx.rollback().await?;
            continue;
        };
        if !matches!(
            request.status,
            registry_publish_request::RegistryPublishRequestStatus::Approved
                | registry_publish_request::RegistryPublishRequestStatus::Published
        ) {
            tx.rollback().await?;
            continue;
        }
        if request.artifact_storage_key.is_none() {
            tx.rollback().await?;
            continue;
        }
        let Some(artifact_checksum_sha256) = request.artifact_checksum_sha256.clone() else {
            tx.rollback().await?;
            continue;
        };

        let now = Utc::now();
        let claim_id = format!("rvc_{}", uuid::Uuid::new_v4().simple());
        let claim_expires_at = now + remote_validation_lease_ttl(lease_ttl_ms);
        let detail = format!(
            "Remote runner '{}' claimed validation stage '{}'.",
            runner_id, candidate.stage_key
        );

        let claimed = registry_validation_stage::Entity::update_many()
            .col_expr(
                registry_validation_stage::Column::Status,
                Expr::value(registry_validation_stage::RegistryValidationStageStatus::Running),
            )
            .col_expr(
                registry_validation_stage::Column::Detail,
                Expr::value(detail.clone()),
            )
            .col_expr(
                registry_validation_stage::Column::StartedAt,
                Expr::value(candidate.started_at.or(Some(now))),
            )
            .col_expr(
                registry_validation_stage::Column::FinishedAt,
                Expr::value(Option::<chrono::DateTime<Utc>>::None),
            )
            .col_expr(
                registry_validation_stage::Column::ClaimId,
                Expr::value(Some(claim_id.clone())),
            )
            .col_expr(
                registry_validation_stage::Column::ClaimedBy,
                Expr::value(Some(runner_id.to_string())),
            )
            .col_expr(
                registry_validation_stage::Column::ClaimExpiresAt,
                Expr::value(Some(claim_expires_at)),
            )
            .col_expr(
                registry_validation_stage::Column::LastHeartbeatAt,
                Expr::value(Some(now)),
            )
            .col_expr(
                registry_validation_stage::Column::RunnerKind,
                Expr::value(Some("remote".to_string())),
            )
            .col_expr(
                registry_validation_stage::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(registry_validation_stage::Column::Id.eq(&candidate.id))
            .filter(
                registry_validation_stage::Column::Status
                    .eq(registry_validation_stage::RegistryValidationStageStatus::Queued),
            )
            .filter(
                Condition::any()
                    .add(registry_validation_stage::Column::ClaimExpiresAt.is_null())
                    .add(registry_validation_stage::Column::ClaimExpiresAt.lte(now)),
            )
            .exec(&tx)
            .await?;

        if claimed.rows_affected != 1 {
            tx.rollback().await?;
            continue;
        }

        let stage = registry_validation_stage::Entity::find_by_id(&candidate.id)
            .one(&tx)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Claimed registry validation stage disappeared"))?;
        let actor = format!("remote-runner:{runner_id}");
        registry_governance_event::ActiveModel {
            id: Set(format!("rge_{}", uuid::Uuid::new_v4().simple())),
            slug: Set(request.slug.clone()),
            request_id: Set(Some(request.id.clone())),
            release_id: Set(None),
            event_type: Set("validation_stage_running".to_string()),
            actor: Set(RegistryPrincipalRef::from_legacy_value(&actor).to_json_value()),
            publisher: Set(None),
            details: Set(serde_json::json!({
                "stage_id": stage.id.clone(),
                "stage_key": stage.stage_key.clone(),
                "status": "running",
                "detail": stage.detail.clone(),
                "attempt_number": stage.attempt_number,
                "queue_reason": stage.queue_reason.clone(),
                "request_status": request_status_label(request.status.clone()),
                "version": request.version.clone(),
                "started_at": stage.started_at.as_ref().map(|value| value.to_rfc3339()),
                "finished_at": stage.finished_at.as_ref().map(|value| value.to_rfc3339()),
                "claim_id": claim_id.clone(),
                "runner_id": runner_id,
                "runner_kind": "remote",
                "execution_mode": "local_workspace",
            })),
            created_at: Set(now),
        }
        .insert(&tx)
        .await?;
        tx.commit().await?;

        return Ok(Some(RegistryRemoteValidationClaim {
            claim_id,
            request_id: request.id.clone(),
            slug: request.slug,
            version: request.version,
            stage_key: stage.stage_key.clone(),
            execution_mode: "local_workspace".to_string(),
            runnable: true,
            requires_manual_confirmation: stage.stage_key == "security_policy_review",
            allowed_terminal_reason_codes: REGISTRY_VALIDATION_STAGE_REASON_CODES
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            suggested_pass_reason_code: Some(pass_reason_code(&stage.stage_key).to_string()),
            suggested_failure_reason_code: Some(failure_reason_code(&stage.stage_key).to_string()),
            suggested_blocked_reason_code: Some(blocked_reason_code(&stage.stage_key).to_string()),
            artifact_download_url: format!(
                "/v2/catalog/publish/{}/artifact/download",
                request.id
            ),
            artifact_checksum_sha256,
            crate_name: request.crate_name,
        }));
    }

    Ok(None)
}

fn normalize_supported_stages(values: &[String]) -> anyhow::Result<Vec<String>> {
    let mut normalized = Vec::new();
    for value in values {
        let value = value.trim();
        let Some(canonical) = FOLLOW_UP_STAGES
            .iter()
            .find(|candidate| candidate.eq_ignore_ascii_case(value))
        else {
            anyhow::bail!(
                "Unsupported validation stage '{}'; expected one of {}",
                value,
                FOLLOW_UP_STAGES.join(", ")
            );
        };
        let canonical = (*canonical).to_string();
        if !normalized.contains(&canonical) {
            normalized.push(canonical);
        }
    }
    Ok(normalized)
}

fn remote_validation_lease_ttl(lease_ttl_ms: u64) -> Duration {
    Duration::milliseconds(lease_ttl_ms.max(1).min(i64::MAX as u64) as i64)
}

fn pass_reason_code(stage_key: &str) -> &'static str {
    match stage_key {
        "security_policy_review" => "manual_review_complete",
        _ => "local_runner_passed",
    }
}

fn failure_reason_code(stage_key: &str) -> &'static str {
    match stage_key {
        "compile_smoke" => "build_failure",
        "targeted_tests" => "test_failure",
        "security_policy_review" => "policy_preflight_failed",
        _ => "manual_override",
    }
}

fn blocked_reason_code(stage_key: &str) -> &'static str {
    match stage_key {
        "security_policy_review" => "security_findings",
        _ => "manual_override",
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_supported_stages;

    #[test]
    fn supported_stages_are_canonicalized_and_deduplicated() {
        assert_eq!(
            normalize_supported_stages(&[
                "COMPILE_SMOKE".to_string(),
                "compile_smoke".to_string(),
                "targeted_tests".to_string(),
            ])
            .unwrap(),
            vec!["compile_smoke".to_string(), "targeted_tests".to_string()]
        );
    }

    #[test]
    fn unknown_stage_is_rejected() {
        assert!(normalize_supported_stages(&["arbitrary".to_string()]).is_err());
    }
}
