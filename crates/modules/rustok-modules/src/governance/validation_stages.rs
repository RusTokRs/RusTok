//! Validation stage reporting and remote stage completion.

use sea_orm::{ConnectionTrait, Statement, TransactionTrait, Value};

use super::helpers::*;
use super::receipts::*;
use super::receipts_reviews::*;
use super::validation_evidence::*;
use super::*;

impl SeaOrmModuleGovernanceService {
    /// Persists a manual validation-stage transition or a fresh queued attempt
    /// with its stage and follow-up audit facts in one transaction.
    pub async fn report_validation_stage(
        &self,
        command: ModuleValidationStageReportCommand,
    ) -> Result<(), ModuleGovernanceError> {
        let command = command.normalized()?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let backend = tx.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = if backend == sea_orm::DbBackend::Postgres {
            "NOW()"
        } else {
            "datetime('now')"
        };
        let receipt = ValidationStageReportReceipt {
            request_id: &command.request_id,
            expected_revision: command.expected_revision,
            context: &command.context,
            actor_principal: &command.actor_principal,
            stage_key: &command.stage_key,
            status: &command.status,
            reason_code: command.reason_code.as_deref(),
            requeue: command.requeue,
        };
        lock_publish_request(&tx, backend, &command.request_id).await?;
        if validation_stage_report_replay(&tx, backend, &receipt).await? {
            tx.commit().await.map_err(store_error)?;
            return Ok(());
        }
        let request_lock = if backend == sea_orm::DbBackend::Postgres {
            " FOR UPDATE"
        } else {
            ""
        };
        let request = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT slug, version, revision, status, artifact_origin \
                     FROM registry_publish_requests WHERE id = {}{request_lock}",
                    mark(1),
                ),
                vec![command.request_id.clone().into()],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        let slug: String = request
            .try_get("", "slug")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let version: String = request
            .try_get("", "version")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let current_revision: i64 = request
            .try_get("", "revision")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if command.expected_revision != current_revision {
            return Err(ModuleGovernanceError::PublishRequestRevisionConflict {
                expected: command.expected_revision,
                current: current_revision,
            });
        }
        let request_status: String = request
            .try_get("", "status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let artifact_origin = ModulePublicationArtifactOrigin::parse(
            &request
                .try_get::<String>("", "artifact_origin")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?,
        )
        .ok_or(ModuleGovernanceError::PublishRequestArtifactOriginUnclassified)?;
        let stage_spec = publication_follow_up_stage(artifact_origin, &command.stage_key)
            .ok_or_else(
                || ModuleGovernanceError::ValidationStageNotRequiredForArtifactOrigin {
                    stage_key: command.stage_key.clone(),
                    artifact_origin: artifact_origin.as_str().to_string(),
                },
            )?;
        if stage_spec.runner_kind == "owner_evidence" {
            return Err(
                ModuleGovernanceError::OwnerEvidenceValidationStageCannotBeReported(
                    command.stage_key.clone(),
                ),
            );
        }
        if !matches!(request_status.as_str(), "approved" | "published") {
            return Err(
                ModuleGovernanceError::PublishRequestCannotReportValidationStage(request_status),
            );
        }

        let detail = content_free_validation_stage_detail(
            &command.stage_key,
            &command.status,
            command.reason_code.as_deref(),
        );
        let actor_label = validation_stage_actor_label(&command.actor_principal)?;
        let (stage_id, attempt_number, queue_reason, event_type) = if command.requeue {
            let next_attempt =
                tx.query_one_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT COALESCE(MAX(attempt_number), 0) AS attempt_number \
                         FROM registry_validation_stages WHERE request_id = {} AND stage_key = {}",
                        mark(1),
                        mark(2),
                    ),
                    vec![
                        command.request_id.clone().into(),
                        command.stage_key.clone().into(),
                    ],
                ))
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
                .ok_or_else(|| {
                    ModuleGovernanceError::Store("missing validation attempt aggregate".to_string())
                })?
                .try_get::<i64>("", "attempt_number")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))? as i32
                    + 1;
            let stage_id = self.infrastructure.prefixed_id("rvs");
            let queue_reason = "manual_requeue".to_string();
            tx.execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_validation_stages \
                     (id, request_id, slug, version, stage_key, status, triggered_by, queue_reason, \
                      attempt_number, detail, started_at, finished_at, last_error, claim_id, claimed_by, \
                      claim_expires_at, last_heartbeat_at, runner_kind, created_at, updated_at) \
                     VALUES ({}, {}, {}, {}, {}, 'queued', {}, {}, {}, {}, NULL, NULL, NULL, NULL, NULL, \
                             NULL, NULL, {}, {now}, {now})",
                    mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7), mark(8), mark(9), mark(10),
                ),
                vec![
                    stage_id.clone().into(), command.request_id.clone().into(), slug.clone().into(),
                    version.clone().into(), command.stage_key.clone().into(), actor_label.into(),
                    queue_reason.clone().into(), next_attempt.into(), detail.clone().into(),
                    stage_spec.runner_kind.into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            (
                stage_id,
                next_attempt,
                queue_reason,
                "validation_stage_queued",
            )
        } else {
            let stage = tx
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT id, status, attempt_number, queue_reason \
                         FROM registry_validation_stages WHERE request_id = {} AND stage_key = {} \
                         ORDER BY attempt_number DESC, created_at DESC LIMIT 1",
                        mark(1),
                        mark(2),
                    ),
                    vec![
                        command.request_id.clone().into(),
                        command.stage_key.clone().into(),
                    ],
                ))
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
                .ok_or(ModuleGovernanceError::ValidationStageNotFound)?;
            let stage_id: String = stage
                .try_get("", "id")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let current_status: String = stage
                .try_get("", "status")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            validation_stage_transition_allowed(
                &current_status,
                &command.status,
                &command.stage_key,
            )?;
            let attempt_number: i32 = stage
                .try_get("", "attempt_number")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let queue_reason: String = stage
                .try_get("", "queue_reason")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            tx.execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_validation_stages SET status = {}, detail = {}, \
                     last_error = CASE WHEN {} = 'failed' THEN {} ELSE NULL END, \
                     started_at = COALESCE(started_at, {now}), \
                     finished_at = CASE WHEN {} IN ('passed', 'failed', 'blocked') THEN {now} ELSE NULL END, \
                     claim_id = CASE WHEN {} IN ('passed', 'failed', 'blocked') THEN NULL ELSE claim_id END, \
                     claimed_by = CASE WHEN {} IN ('passed', 'failed', 'blocked') THEN NULL ELSE claimed_by END, \
                     claim_expires_at = CASE WHEN {} IN ('passed', 'failed', 'blocked') THEN NULL ELSE claim_expires_at END, \
                     last_heartbeat_at = CASE WHEN {} IN ('passed', 'failed', 'blocked') THEN NULL ELSE last_heartbeat_at END, \
                     runner_kind = CASE WHEN {} IN ('passed', 'failed', 'blocked') THEN NULL ELSE runner_kind END, \
                     updated_at = {now} WHERE id = {}",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                    mark(5),
                    mark(6),
                    mark(7),
                    mark(8),
                    mark(9),
                    mark(10),
                    mark(11),
                ),
                vec![
                    command.status.clone().into(), detail.clone().into(),
                    command.status.clone().into(), detail.clone().into(),
                    command.status.clone().into(), command.status.clone().into(),
                    command.status.clone().into(), command.status.clone().into(),
                    command.status.clone().into(), command.status.clone().into(),
                    stage_id.clone().into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let event_type = match command.status.as_str() {
                "running" => "validation_stage_running",
                "passed" => "validation_stage_passed",
                "failed" => "validation_stage_failed",
                "blocked" => "validation_stage_blocked",
                _ => unreachable!("validated non-requeue status"),
            };
            (stage_id, attempt_number, queue_reason, event_type)
        };

        let request_updated = tx
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_publish_requests SET revision = revision + 1, updated_at = {now} \
                     WHERE id = {} AND revision = {}",
                    mark(1),
                    mark(2),
                ),
                vec![
                    command.request_id.clone().into(),
                    command.expected_revision.into(),
                ],
            ))
            .await
            .map_err(store_error)?;
        if request_updated.rows_affected() != 1 {
            return Err(publish_request_revision_conflict(
                &tx,
                backend,
                &command.request_id,
                command.expected_revision,
            )
            .await?);
        }

        let mut stage_details = serde_json::json!({
            "stage_id": stage_id,
            "stage_key": command.stage_key,
            "status": command.status,
            "detail": detail.clone(),
            "attempt_number": attempt_number,
            "queue_reason": queue_reason,
            "request_status": request_status,
            "version": version,
        });
        if let Some(reason_code) = &command.reason_code {
            stage_details["reason_code"] = serde_json::Value::String(reason_code.clone());
        }
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
                self.infrastructure.prefixed_id("rge").into(),
                slug.clone().into(),
                command.request_id.clone().into(),
                event_type.into(),
                Value::Json(Some(Box::new(command.actor_principal.clone()))),
                Value::Json(Some(Box::new(stage_details))),
            ],
        ))
        .await
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let gate = match command.status.as_str() {
            "queued" => Some(("follow_up_gate_queued", "pending")),
            "passed" => Some(("follow_up_gate_passed", "passed")),
            "failed" => Some(("follow_up_gate_failed", "failed")),
            _ => None,
        };
        if let Some((event_type, gate_status)) = gate {
            let mut gate_details = serde_json::json!({
                "stage_key": command.stage_key,
                "status": gate_status,
                "detail": detail.clone(),
            });
            if let Some(reason_code) = &command.reason_code {
                gate_details["reason_code"] = serde_json::Value::String(reason_code.clone());
            }
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
                    self.infrastructure.prefixed_id("rge").into(),
                    slug.into(),
                    command.request_id.clone().into(),
                    event_type.into(),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    Value::Json(Some(Box::new(gate_details))),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        }
        record_validation_stage_report_receipt(
            &self.infrastructure,
            &tx,
            backend,
            now,
            &receipt,
            &stage_id,
            command.expected_revision + 1,
        )
        .await?;
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(())
    }

    /// Completes a remote lease and emits the terminal stage and follow-up gate
    /// facts in the same transaction. Returns the canonical terminal state for
    /// a host adapter that needs to shape a transport response.
    pub async fn complete_remote_validation_stage(
        &self,
        command: ModuleRemoteValidationTerminalCommand,
    ) -> Result<ModuleRemoteValidationStageTransition, ModuleGovernanceError> {
        command.validate()?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let backend = tx.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = if backend == sea_orm::DbBackend::Postgres {
            "NOW()"
        } else {
            "datetime('now')"
        };
        let row = tx.query_one_raw(Statement::from_sql_and_values(backend, format!(
            "SELECT s.id, s.stage_key, s.status, s.claimed_by, s.runner_kind, s.attempt_number, s.queue_reason, r.id AS request_id, r.revision AS request_revision, r.slug, r.version, r.status AS request_status \
             FROM registry_validation_stages s JOIN registry_publish_requests r ON r.id = s.request_id WHERE s.claim_id = {}", mark(1)), vec![command.claim_id.clone().into()]))
            .await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .ok_or(ModuleGovernanceError::RemoteValidationLeaseNotFound)?;
        let stage_id: String = row
            .try_get("", "id")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let stage_key: String = row
            .try_get("", "stage_key")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let status: String = row
            .try_get("", "status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let claimed_by: Option<String> = row
            .try_get("", "claimed_by")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let runner_kind: Option<String> = row
            .try_get("", "runner_kind")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if claimed_by.as_deref() != Some(command.runner_id.as_str())
            || runner_kind.as_deref() != Some("remote")
        {
            return Err(ModuleGovernanceError::RemoteValidationLeaseRunnerMismatch);
        }
        if status != "running" {
            return Err(ModuleGovernanceError::RemoteValidationLeaseNotRunning(
                status,
            ));
        }
        let slug: String = row
            .try_get("", "slug")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let request_id: String = row
            .try_get("", "request_id")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let request_revision: i64 = row
            .try_get("", "request_revision")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if command.expected_request_revision != request_revision {
            return Err(ModuleGovernanceError::PublishRequestRevisionConflict {
                expected: command.expected_request_revision,
                current: request_revision,
            });
        }
        let version: String = row
            .try_get("", "version")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let request_status: String = row
            .try_get("", "request_status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let attempt_number: i32 = row
            .try_get("", "attempt_number")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let queue_reason: String = row
            .try_get("", "queue_reason")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let (terminal_status, event_type, gate_event, gate_status, default_reason) =
            match command.outcome {
                ModuleRemoteValidationTerminalOutcome::Passed => (
                    "passed",
                    "validation_stage_passed",
                    "follow_up_gate_passed",
                    "passed",
                    if stage_key == "security_policy_review" {
                        "manual_review_complete"
                    } else {
                        "local_runner_passed"
                    },
                ),
                ModuleRemoteValidationTerminalOutcome::Failed => (
                    "failed",
                    "validation_stage_failed",
                    "follow_up_gate_failed",
                    "failed",
                    match stage_key.as_str() {
                        "compile_smoke" => "build_failure",
                        "targeted_tests" => "test_failure",
                        "security_policy_review" => "policy_preflight_failed",
                        _ => "manual_override",
                    },
                ),
            };
        let reason_code = command
            .reason_code
            .unwrap_or_else(|| default_reason.to_string());
        let detail = content_free_validation_stage_detail(
            &stage_key,
            terminal_status,
            Some(reason_code.as_str()),
        );
        let update = tx.execute_raw(Statement::from_sql_and_values(backend, format!(
            "UPDATE registry_validation_stages SET status = {}, detail = {}, last_error = {}, started_at = COALESCE(started_at, {now}), finished_at = {now}, claim_id = NULL, claimed_by = NULL, claim_expires_at = NULL, last_heartbeat_at = NULL, runner_kind = NULL, updated_at = {now} WHERE id = {} AND claim_id = {} AND claimed_by = {} AND status = 'running' AND claim_expires_at >= {now}", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6)),
            vec![terminal_status.into(), detail.clone().into(), if terminal_status == "failed" { Some(detail.clone()).into() } else { Option::<String>::None.into() }, stage_id.clone().into(), command.claim_id.clone().into(), command.runner_id.clone().into()]))
            .await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if update.rows_affected() != 1 {
            let current = tx
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT status, claimed_by, runner_kind FROM registry_validation_stages \
                         WHERE claim_id = {}",
                        mark(1)
                    ),
                    vec![command.claim_id.clone().into()],
                ))
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
                .ok_or(ModuleGovernanceError::RemoteValidationLeaseNotFound)?;
            let current_claimed_by: Option<String> = current
                .try_get("", "claimed_by")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let current_runner_kind: Option<String> = current
                .try_get("", "runner_kind")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            if current_claimed_by.as_deref() != Some(command.runner_id.as_str())
                || current_runner_kind.as_deref() != Some("remote")
            {
                return Err(ModuleGovernanceError::RemoteValidationLeaseRunnerMismatch);
            }
            let current_status: String = current
                .try_get("", "status")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            if current_status != "running" {
                return Err(ModuleGovernanceError::RemoteValidationLeaseNotRunning(
                    current_status,
                ));
            }
            return Err(ModuleGovernanceError::RemoteValidationLeaseExpired);
        }
        let request_updated = tx
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_publish_requests SET revision = revision + 1, updated_at = {now} \
                     WHERE id = {} AND revision = {}",
                    mark(1),
                    mark(2),
                ),
                vec![
                    request_id.clone().into(),
                    command.expected_request_revision.into(),
                ],
            ))
            .await
            .map_err(store_error)?;
        if request_updated.rows_affected() != 1 {
            return Err(publish_request_revision_conflict(
                &tx,
                backend,
                &request_id,
                command.expected_request_revision,
            )
            .await?);
        }
        let actor = serde_json::json!({"kind":"remote_runner","id":command.runner_id});
        for (event_type, details) in [
            (
                event_type,
                serde_json::json!({"stage_id":stage_id,"stage_key":stage_key,"status":terminal_status,"detail":detail,"attempt_number":attempt_number,"queue_reason":queue_reason,"request_status":request_status,"version":version,"reason_code":reason_code}),
            ),
            (
                gate_event,
                serde_json::json!({"stage_key":stage_key,"status":gate_status,"detail":detail,"reason_code":reason_code}),
            ),
        ] {
            tx.execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, {}, {}, NULL, {}, {now})",
                    mark(1), mark(2), mark(3), mark(4), mark(5), mark(6),
                ),
                vec![
                    self.infrastructure.prefixed_id("rge").into(),
                    slug.clone().into(),
                    request_id.clone().into(),
                    event_type.into(),
                    Value::Json(Some(Box::new(actor.clone()))),
                    Value::Json(Some(Box::new(details))),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        }
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(ModuleRemoteValidationStageTransition {
            status: terminal_status.to_string(),
        })
    }
}
