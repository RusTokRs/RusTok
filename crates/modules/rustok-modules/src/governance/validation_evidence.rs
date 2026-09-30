//! Owner validation stage evidence passes and transition detail contracts.

use sea_orm::{ConnectionTrait, DatabaseTransaction, DbBackend, Statement, Value};

use super::*;


pub(crate) fn validation_stage_actor_label(
    principal: &serde_json::Value,
) -> Result<String, ModuleGovernanceError> {
    principal
        .get("id")
        .or_else(|| principal.get("subject"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or(ModuleGovernanceError::InvalidValidationStageReportCommand)
}

pub(crate) struct OwnerEvidenceStage<'a> {
    request_id: &'a str,
    slug: &'a str,
    version: &'a str,
    stage_key: &'a str,
    actor_principal: &'a serde_json::Value,
    detail: &'a str,
    reason_code: &'a str,
}

pub(crate) async fn pass_owner_evidence_validation_stage(
    infrastructure: &ControlPlaneInfrastructure,
    tx: &DatabaseTransaction,
    backend: DbBackend,
    stage: OwnerEvidenceStage<'_>,
) -> Result<(), ModuleGovernanceError> {
    let OwnerEvidenceStage {
        request_id,
        slug,
        version,
        stage_key,
        actor_principal,
        detail,
        reason_code,
    } = stage;
    let mark = |n| placeholder(backend, n);
    let now = database_now(backend);
    let Some(stage) = tx
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT id, status, attempt_number, queue_reason, runner_kind \
                 FROM registry_validation_stages WHERE request_id = {} AND stage_key = {} \
                 ORDER BY attempt_number DESC, created_at DESC LIMIT 1",
                mark(1),
                mark(2),
            ),
            vec![request_id.to_string().into(), stage_key.to_string().into()],
        ))
        .await
        .map_err(store_error)?
    else {
        // Origin evidence may be recorded before artifact validation finishes.
        // The later validation transaction creates the stage from the same
        // durable owner facts.
        return Ok(());
    };
    let status: String = stage.try_get("", "status").map_err(store_error)?;
    if status == "passed" {
        return Ok(());
    }
    let runner_kind: Option<String> = stage.try_get("", "runner_kind").map_err(store_error)?;
    if runner_kind.as_deref() != Some("owner_evidence") {
        return Err(ModuleGovernanceError::Store(format!(
            "validation stage '{stage_key}' is not owned by immutable owner evidence"
        )));
    }
    let prior_attempt: i32 = stage.try_get("", "attempt_number").map_err(store_error)?;
    let prior_queue_reason: String = stage.try_get("", "queue_reason").map_err(store_error)?;
    let detail = detail.to_string();
    let (stage_id, attempt_number, queue_reason) = if status == "failed" {
        let stage_id = infrastructure.prefixed_id("rvs");
        let attempt_number = prior_attempt + 1;
        let queue_reason = reason_code.to_string();
        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_validation_stages \
                 (id, request_id, slug, version, stage_key, status, triggered_by, queue_reason, \
                  attempt_number, detail, started_at, finished_at, last_error, claim_id, claimed_by, \
                  claim_expires_at, last_heartbeat_at, runner_kind, created_at, updated_at) \
                 VALUES ({}, {}, {}, {}, {}, 'passed', {}, {}, {}, {}, {now}, {now}, NULL, NULL, \
                         NULL, NULL, NULL, NULL, {now}, {now})",
                mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7), mark(8), mark(9),
            ),
            vec![
                stage_id.clone().into(),
                request_id.to_string().into(),
                slug.to_string().into(),
                version.to_string().into(),
                stage_key.to_string().into(),
                validation_stage_actor_label(actor_principal)?.into(),
                queue_reason.clone().into(),
                attempt_number.into(),
                detail.clone().into(),
            ],
        ))
        .await
        .map_err(store_error)?;
        (stage_id, attempt_number, queue_reason)
    } else {
        let stage_id: String = stage.try_get("", "id").map_err(store_error)?;
        let updated = tx
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_validation_stages SET status = 'passed', detail = {}, \
                     last_error = NULL, started_at = COALESCE(started_at, {now}), \
                     finished_at = {now}, claim_id = NULL, claimed_by = NULL, \
                     claim_expires_at = NULL, last_heartbeat_at = NULL, runner_kind = NULL, \
                     updated_at = {now} WHERE id = {} AND status IN ('queued', 'running', 'blocked') \
                     AND runner_kind = 'owner_evidence'",
                    mark(1),
                    mark(2),
                ),
                vec![detail.clone().into(), stage_id.clone().into()],
            ))
            .await
            .map_err(store_error)?;
        if updated.rows_affected() != 1 {
            return Err(ModuleGovernanceError::Store(format!(
                "validation stage '{stage_key}' changed while owner evidence was applied"
            )));
        }
        (stage_id, prior_attempt, prior_queue_reason)
    };

    let actor = actor_principal.clone();
    for (event_type, details) in [
        (
            "validation_stage_passed",
            serde_json::json!({
                "stage_id": stage_id,
                "stage_key": stage_key,
                "status": "passed",
                "detail": detail,
                "attempt_number": attempt_number,
                "queue_reason": queue_reason,
                "request_status": "approved",
                "version": version,
                "reason_code": reason_code,
                "runner_kind": "owner_evidence",
            }),
        ),
        (
            "follow_up_gate_passed",
            serde_json::json!({
                "stage_key": stage_key,
                "status": "passed",
                "detail": detail,
                "reason_code": reason_code,
            }),
        ),
    ] {
        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_governance_events \
                 (id, slug, request_id, release_id, event_type, actor_principal, \
                  publisher_principal, details, created_at) \
                 VALUES ({}, {}, {}, NULL, {}, {}, NULL, {}, {now})",
                mark(1),
                mark(2),
                mark(3),
                mark(4),
                mark(5),
                mark(6),
            ),
            vec![
                infrastructure.prefixed_id("rge").into(),
                slug.to_string().into(),
                request_id.to_string().into(),
                event_type.into(),
                Value::Json(Some(Box::new(actor.clone()))),
                Value::Json(Some(Box::new(details))),
            ],
        ))
        .await
        .map_err(store_error)?;
    }
    Ok(())
}


pub(crate) fn follow_up_validation_stage_detail(stage_key: &str) -> &'static str {
    match stage_key {
        "compile_smoke" => "Compile smoke awaits exact platform build-worker validation evidence.",
        "targeted_tests" => "Targeted tests await exact platform build-worker validation evidence.",
        "security_policy_review" => {
            "Security and policy review await exact origin-specific owner evidence."
        }
        _ => "External follow-up gate is still pending.",
    }
}

pub(crate) fn content_free_validation_stage_detail(
    stage_key: &str,
    status: &str,
    reason_code: Option<&str>,
) -> String {
    let reason_code = reason_code.unwrap_or("not_reported");
    format!(
        "Registry validation stage '{stage_key}' entered status '{status}' with reason code '{reason_code}'."
    )
}

pub(crate) fn validation_stage_transition_allowed(
    current: &str,
    next: &str,
    stage_key: &str,
) -> Result<(), ModuleGovernanceError> {
    let allowed = match current {
        "queued" | "running" | "blocked" => {
            matches!(next, "running" | "passed" | "failed" | "blocked")
        }
        "passed" | "failed" => false,
        _ => false,
    };
    if allowed {
        return Ok(());
    }
    Err(ModuleGovernanceError::InvalidValidationStageTransition {
        stage_key: stage_key.to_string(),
        current: current.to_string(),
        next: next.to_string(),
    })
}

/// Transport-neutral classification for the canonical module-governance error
/// contract. Hosts map this category to their own envelopes without recreating
/// the owner lifecycle taxonomy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
