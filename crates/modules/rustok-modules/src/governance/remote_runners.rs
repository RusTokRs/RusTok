//! Remote validation runner leases, heartbeats, claims, and claim expiration reconciliation.

use sea_orm::{ConnectionTrait, Statement, TransactionTrait, Value};

use super::helpers::*;
use super::*;

impl SeaOrmModuleGovernanceService {
    /// Renews a remote validation lease through a conditional update. The
    /// claim id, runner id, running state, remote ownership, and unexpired
    /// lease are one compare-and-swap predicate.
    pub async fn heartbeat_remote_validation_stage(
        &self,
        command: ModuleRemoteValidationHeartbeatCommand,
    ) -> Result<ModuleRemoteValidationStageTransition, ModuleGovernanceError> {
        command.validate()?;
        let now = self.infrastructure.now();
        let expires_at = now
            + chrono::Duration::milliseconds(
                command.lease_ttl_ms.max(1).min(i64::MAX as u64) as i64
            );
        let backend = self.db.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let updated = self
            .db
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_validation_stages SET last_heartbeat_at = {}, \
                     claim_expires_at = {}, updated_at = {} WHERE claim_id = {} \
                     AND claimed_by = {} AND runner_kind = 'remote' AND status = 'running' \
                     AND claim_expires_at >= {}",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                    mark(5),
                    mark(6),
                ),
                vec![
                    now.into(),
                    expires_at.into(),
                    now.into(),
                    command.claim_id.clone().into(),
                    command.runner_id.clone().into(),
                    now.into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if updated.rows_affected() == 1 {
            return Ok(ModuleRemoteValidationStageTransition {
                status: "running".to_string(),
            });
        }
        let stage = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT status, claimed_by, runner_kind FROM registry_validation_stages \
                     WHERE claim_id = {}",
                    mark(1)
                ),
                vec![command.claim_id.into()],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .ok_or(ModuleGovernanceError::RemoteValidationLeaseNotFound)?;
        let claimed_by: Option<String> = stage
            .try_get("", "claimed_by")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let runner_kind: Option<String> = stage
            .try_get("", "runner_kind")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if claimed_by.as_deref() != Some(command.runner_id.as_str())
            || runner_kind.as_deref() != Some("remote")
        {
            return Err(ModuleGovernanceError::RemoteValidationLeaseRunnerMismatch);
        }
        let status: String = stage
            .try_get("", "status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if status != "running" {
            return Err(ModuleGovernanceError::RemoteValidationLeaseNotRunning(
                status,
            ));
        }
        Err(ModuleGovernanceError::RemoteValidationLeaseExpired)
    }

    /// Claims the first eligible validation stage with a compare-and-swap and
    /// records the claim fact in the same transaction. Candidate discovery is
    /// advisory; the conditional update is the serialization point.
    pub async fn claim_remote_validation_stage(
        &self,
        command: ModuleRemoteValidationClaimCommand,
    ) -> Result<Option<ModuleRemoteValidationClaim>, ModuleGovernanceError> {
        let supported_stages = command.normalized_supported_stages()?;
        if supported_stages.is_empty() {
            return Ok(None);
        }
        let backend = self.db.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = self.infrastructure.now();
        let stage_marks = (1..=supported_stages.len())
            .map(&mark)
            .collect::<Vec<_>>()
            .join(", ");
        let mut candidate_values: Vec<Value> =
            supported_stages.iter().cloned().map(Value::from).collect();
        candidate_values.push(now.into());
        candidate_values.push(now.into());
        let candidates = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT id FROM registry_validation_stages WHERE stage_key IN ({stage_marks}) \
                     AND runner_kind = 'remote' \
                     AND ((status = 'queued' AND (claim_expires_at IS NULL OR claim_expires_at <= {})) \
                     OR (status = 'running' AND runner_kind = 'remote' AND claim_expires_at < {})) \
                     ORDER BY created_at ASC LIMIT {}",
                    mark(supported_stages.len() + 1),
                    mark(supported_stages.len() + 2),
                    MAX_REMOTE_VALIDATION_CLAIM_CANDIDATES,
                ),
                candidate_values,
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;

        for candidate in candidates {
            let stage_id: String = candidate
                .try_get("", "id")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let tx = self
                .db
                .begin()
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let tx_backend = tx.get_database_backend();
            let tx_mark = |n| {
                if tx_backend == sea_orm::DbBackend::Postgres {
                    format!("${n}")
                } else {
                    format!("?{n}")
                }
            };
            let tx_now = if tx_backend == sea_orm::DbBackend::Postgres {
                "NOW()"
            } else {
                "datetime('now')"
            };
            let Some(stage) = tx
                .query_one_raw(Statement::from_sql_and_values(
                    tx_backend,
                    format!(
                        "SELECT s.stage_key, s.status, s.claim_id, s.claimed_by, s.attempt_number, \
                         s.queue_reason, r.id AS request_id, r.revision AS request_revision, \
                         r.slug, r.version, r.status AS request_status, \
                         r.crate_name, r.artifact_storage_key, r.artifact_checksum_sha256 \
                         FROM registry_validation_stages s JOIN registry_publish_requests r ON r.id = s.request_id \
                         WHERE s.id = {}",
                        tx_mark(1)
                    ),
                    vec![stage_id.clone().into()],
                ))
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            else {
                tx.rollback()
                    .await
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                continue;
            };
            let request_status: String = stage
                .try_get("", "request_status")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let artifact_storage_key: Option<String> = stage
                .try_get("", "artifact_storage_key")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let artifact_checksum_sha256: Option<String> = stage
                .try_get("", "artifact_checksum_sha256")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let Some(artifact_checksum_sha256) = artifact_checksum_sha256 else {
                tx.rollback()
                    .await
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                continue;
            };
            if !matches!(request_status.as_str(), "approved" | "published")
                || artifact_storage_key.is_none()
            {
                tx.rollback()
                    .await
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                continue;
            }
            let stage_key: String = stage
                .try_get("", "stage_key")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let status: String = stage
                .try_get("", "status")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let previous_claim_id: Option<String> = stage
                .try_get("", "claim_id")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let previous_runner_id: Option<String> = stage
                .try_get("", "claimed_by")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let request_id: String = stage
                .try_get("", "request_id")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let request_revision: i64 = stage
                .try_get("", "request_revision")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let slug: String = stage
                .try_get("", "slug")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let version: String = stage
                .try_get("", "version")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let crate_name: String = stage
                .try_get("", "crate_name")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let attempt_number: i32 = stage
                .try_get("", "attempt_number")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let queue_reason: String = stage
                .try_get("", "queue_reason")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let claim_id = self.infrastructure.prefixed_id("rvc");
            let reclaimed = status == "running";
            let detail = if reclaimed {
                format!(
                    "Remote runner '{}' reclaimed expired validation stage '{}' from runner '{}'.",
                    command.runner_id.trim(),
                    stage_key,
                    previous_runner_id.as_deref().unwrap_or("unknown")
                )
            } else {
                format!(
                    "Remote runner '{}' claimed validation stage '{}'.",
                    command.runner_id.trim(),
                    stage_key
                )
            };
            let claim_expires_at = self.infrastructure.now()
                + chrono::Duration::milliseconds(
                    command.lease_ttl_ms.max(1).min(i64::MAX as u64) as i64
                );
            let updated = tx
                .execute_raw(Statement::from_sql_and_values(
                    tx_backend,
                    format!(
                        "UPDATE registry_validation_stages SET status = 'running', detail = {}, \
                         started_at = COALESCE(started_at, {tx_now}), finished_at = NULL, \
                         claim_id = {}, claimed_by = {}, claim_expires_at = {}, \
                         last_heartbeat_at = {tx_now}, runner_kind = 'remote', updated_at = {tx_now} \
                         WHERE id = {} AND runner_kind = 'remote' \
                         AND ((status = 'queued' AND (claim_expires_at IS NULL OR claim_expires_at <= {tx_now})) \
                         OR (status = 'running' AND runner_kind = 'remote' AND claim_expires_at < {tx_now})) \
                         AND EXISTS (SELECT 1 FROM registry_publish_requests r WHERE r.id = registry_validation_stages.request_id \
                         AND r.status IN ('approved', 'published') AND r.artifact_storage_key IS NOT NULL \
                         AND r.artifact_checksum_sha256 IS NOT NULL)",
                        tx_mark(1),
                        tx_mark(2),
                        tx_mark(3),
                        tx_mark(4),
                        tx_mark(5),
                    ),
                    vec![
                        detail.clone().into(),
                        claim_id.clone().into(),
                        command.runner_id.trim().to_string().into(),
                        claim_expires_at.into(),
                        stage_id.clone().into(),
                    ],
                ))
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            if updated.rows_affected() != 1 {
                tx.rollback()
                    .await
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                continue;
            }
            let request_updated = tx
                .execute_raw(Statement::from_sql_and_values(
                    tx_backend,
                    format!(
                        "UPDATE registry_publish_requests SET revision = revision + 1, updated_at = {tx_now} \
                         WHERE id = {} AND revision = {}",
                        tx_mark(1),
                        tx_mark(2),
                    ),
                    vec![request_id.clone().into(), request_revision.into()],
                ))
                .await
                .map_err(store_error)?;
            if request_updated.rows_affected() != 1 {
                tx.rollback().await.map_err(store_error)?;
                continue;
            }
            let actor = serde_json::json!({"kind":"remote_runner","id":command.runner_id.trim()});
            let details = serde_json::json!({
                "stage_id": stage_id,
                "stage_key": stage_key,
                "status": "running",
                "detail": detail,
                "attempt_number": attempt_number,
                "queue_reason": queue_reason,
                "request_status": request_status,
                "version": version,
                "claim_id": claim_id,
                "runner_id": command.runner_id.trim(),
                "runner_kind": "remote",
                "execution_mode": "local_workspace",
                "reclaimed_expired_lease": reclaimed,
                "previous_claim_id": previous_claim_id,
                "previous_runner_id": previous_runner_id,
            });
            tx.execute_raw(Statement::from_sql_and_values(
                tx_backend,
                format!(
                    "INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, 'validation_stage_running', {}, NULL, {}, {tx_now})",
                    tx_mark(1), tx_mark(2), tx_mark(3), tx_mark(4), tx_mark(5),
                ),
                vec![
                    self.infrastructure.prefixed_id("rge").into(),
                    slug.clone().into(),
                    request_id.clone().into(),
                    Value::Json(Some(Box::new(actor))),
                    Value::Json(Some(Box::new(details))),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            tx.commit()
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            return Ok(Some(ModuleRemoteValidationClaim {
                claim_id,
                request_id,
                request_revision: request_revision + 1,
                slug,
                version,
                stage_key: stage_key.clone(),
                execution_mode: "local_workspace".to_string(),
                requires_manual_confirmation: stage_key == "security_policy_review",
                allowed_terminal_reason_codes: REGISTRY_VALIDATION_STAGE_REASON_CODES
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect(),
                suggested_pass_reason_code: if stage_key == "security_policy_review" {
                    "manual_review_complete".to_string()
                } else {
                    "local_runner_passed".to_string()
                },
                suggested_failure_reason_code: match stage_key.as_str() {
                    "compile_smoke" => "build_failure",
                    "targeted_tests" => "test_failure",
                    "security_policy_review" => "policy_preflight_failed",
                    _ => "manual_override",
                }
                .to_string(),
                suggested_blocked_reason_code: if stage_key == "security_policy_review" {
                    "security_findings".to_string()
                } else {
                    "manual_override".to_string()
                },
                artifact_checksum_sha256,
                crate_name,
            }));
        }
        Ok(None)
    }

    /// Returns remote-runner lease counts from the registry owner. Expired
    /// claims are a subset of active claims: both require a remote runner and
    /// the durable `running` stage state.
    pub async fn remote_validation_runner_snapshot(
        &self,
    ) -> Result<ModuleRemoteValidationRunnerSnapshot, ModuleGovernanceError> {
        let backend = self.db.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = self.infrastructure.now();
        let snapshot = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT COUNT(*) AS active_claims, \
                     COALESCE(SUM(CASE WHEN claim_expires_at < {} THEN 1 ELSE 0 END), 0) \
                     AS expired_claims \
                     FROM registry_validation_stages \
                     WHERE runner_kind = 'remote' AND status = 'running'",
                    mark(1),
                ),
                vec![now.into()],
            ))
            .await
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?
            .ok_or_else(|| {
                ModuleGovernanceError::Store(
                    "remote validation runner snapshot did not return an aggregate row".to_string(),
                )
            })?;
        let active_claims: i64 = snapshot
            .try_get("", "active_claims")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;
        let expired_claims: i64 = snapshot
            .try_get("", "expired_claims")
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;

        Ok(ModuleRemoteValidationRunnerSnapshot {
            active_claims: u64::try_from(active_claims).map_err(|_| {
                ModuleGovernanceError::Store(
                    "remote validation active claim count is negative".to_string(),
                )
            })?,
            expired_claims: u64::try_from(expired_claims).map_err(|_| {
                ModuleGovernanceError::Store(
                    "remote validation expired claim count is negative".to_string(),
                )
            })?,
        })
    }

    /// Requeues every remote lease that is still expired when its transaction
    /// reaches the compare-and-swap. Blocking the old attempt, creating its
    /// successor, and recording both audit projections are one durable unit.
    pub async fn requeue_expired_remote_validation_claims(
        &self,
    ) -> Result<usize, ModuleGovernanceError> {
        let backend = self.db.get_database_backend();
        let mark = |n| {
            if backend == sea_orm::DbBackend::Postgres {
                format!("${n}")
            } else {
                format!("?{n}")
            }
        };
        let now = self.infrastructure.now();
        let candidates = self.db.query_all_raw(Statement::from_sql_and_values(
            backend,
            format!("SELECT id FROM registry_validation_stages WHERE status = 'running' AND runner_kind = 'remote' AND claim_expires_at < {} ORDER BY claim_expires_at ASC", mark(1)),
            vec![now.into()],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let mut requeued = 0;
        for candidate in candidates {
            let stage_id: String = candidate
                .try_get("", "id")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
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
            let Some(stage) = tx.query_one_raw(Statement::from_sql_and_values(
                backend,
                format!("SELECT s.stage_key, s.attempt_number, s.queue_reason, s.claim_id, s.claimed_by, r.id AS request_id, r.revision AS request_revision, r.slug, r.version, r.status AS request_status FROM registry_validation_stages s JOIN registry_publish_requests r ON r.id = s.request_id WHERE s.id = {}", mark(1)),
                vec![stage_id.clone().into()],
            )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))? else {
                tx.rollback().await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                continue;
            };
            let stage_key: String = stage
                .try_get("", "stage_key")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let attempt_number: i32 = stage
                .try_get("", "attempt_number")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let queue_reason: String = stage
                .try_get("", "queue_reason")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let claim_id: Option<String> = stage
                .try_get("", "claim_id")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let claimed_by: Option<String> = stage
                .try_get("", "claimed_by")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let request_id: String = stage
                .try_get("", "request_id")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let request_revision: i64 = stage
                .try_get("", "request_revision")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let slug: String = stage
                .try_get("", "slug")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let version: String = stage
                .try_get("", "version")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let request_status: String = stage
                .try_get("", "request_status")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let detail = format!(
                "Remote validation lease expired for runner '{}' (claim '{}'); stage attempt will be requeued.",
                claimed_by.as_deref().unwrap_or("unknown"),
                claim_id.as_deref().unwrap_or("unknown")
            );
            let blocked = tx.execute_raw(Statement::from_sql_and_values(
                backend,
                format!("UPDATE registry_validation_stages SET status = 'blocked', detail = {}, last_error = NULL, finished_at = {now}, claim_id = NULL, claimed_by = NULL, claim_expires_at = NULL, last_heartbeat_at = NULL, runner_kind = NULL, updated_at = {now} WHERE id = {} AND status = 'running' AND runner_kind = 'remote' AND claim_expires_at < {now}", mark(1), mark(2)),
                vec![detail.clone().into(), stage_id.clone().into()],
            )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            if blocked.rows_affected() != 1 {
                tx.rollback()
                    .await
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                continue;
            }
            let next_attempt = tx.query_one_raw(Statement::from_sql_and_values(
                backend,
                format!("SELECT COALESCE(MAX(attempt_number), 0) AS attempt_number FROM registry_validation_stages WHERE request_id = {} AND stage_key = {}", mark(1), mark(2)),
                vec![request_id.clone().into(), stage_key.clone().into()],
            )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?.ok_or_else(|| ModuleGovernanceError::Store("missing validation attempt aggregate".to_string()))?.try_get::<i64>("", "attempt_number").map_err(|e| ModuleGovernanceError::Store(e.to_string()))? as i32 + 1;
            let queued_id = self.infrastructure.prefixed_id("rvs");
            let queued_detail = format!(
                "Remote validation lease expired; retry attempt {} is queued for stage '{}'.",
                next_attempt, stage_key
            );
            tx.execute_raw(Statement::from_sql_and_values(
                backend,
                format!("INSERT INTO registry_validation_stages (id, request_id, slug, version, stage_key, status, triggered_by, queue_reason, attempt_number, detail, started_at, finished_at, last_error, claim_id, claimed_by, claim_expires_at, last_heartbeat_at, runner_kind, created_at, updated_at) VALUES ({}, {}, {}, {}, {}, 'queued', 'system:registry-runner-reaper', 'remote_lease_expired', {}, {}, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'remote', {now}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7)),
                vec![queued_id.clone().into(), request_id.clone().into(), slug.clone().into(), version.clone().into(), stage_key.clone().into(), next_attempt.into(), queued_detail.clone().into()],
            )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let request_updated = tx
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "UPDATE registry_publish_requests SET revision = revision + 1, updated_at = {now} \
                         WHERE id = {} AND revision = {}",
                        mark(1),
                        mark(2),
                    ),
                    vec![request_id.clone().into(), request_revision.into()],
                ))
                .await
                .map_err(store_error)?;
            if request_updated.rows_affected() != 1 {
                tx.rollback().await.map_err(store_error)?;
                continue;
            }
            let actor = serde_json::json!({"kind":"system","id":"registry-runner-reaper"});
            let events = [
                (
                    "validation_stage_blocked",
                    serde_json::json!({"stage_id":stage_id,"stage_key":stage_key,"status":"blocked","detail":detail,"attempt_number":attempt_number,"queue_reason":queue_reason,"request_status":request_status,"version":version,"reason_code":"other"}),
                ),
                (
                    "validation_stage_queued",
                    serde_json::json!({"stage_id":queued_id,"stage_key":stage_key,"status":"queued","detail":queued_detail,"attempt_number":next_attempt,"queue_reason":"remote_lease_expired","request_status":request_status,"version":version}),
                ),
                (
                    "follow_up_gate_queued",
                    serde_json::json!({"stage_key":stage_key,"status":"pending","detail":queued_detail}),
                ),
            ];
            for (event_type, details) in events {
                tx.execute_raw(Statement::from_sql_and_values(backend,
                    format!("INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, {}, {}, NULL, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6)),
                    vec![self.infrastructure.prefixed_id("rge").into(), slug.clone().into(), request_id.clone().into(), event_type.into(), Value::Json(Some(Box::new(actor.clone()))), Value::Json(Some(Box::new(details)))],
                )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            }
            tx.commit()
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            requeued += 1;
        }
        Ok(requeued)
    }
}
