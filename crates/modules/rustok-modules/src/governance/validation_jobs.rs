//! Validation job queue enqueue and leasing.

use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait, Value};

use super::*;
use super::helpers::*;
use super::receipts::*;
use super::validation_evidence::*;
use super::validation_work_items::*;

impl SeaOrmModuleGovernanceService {

    /// Enqueues at most one active automated validation job and records its
    /// request/job facts atomically. The worker is intentionally outside this
    /// transaction and may begin only after the host observes this result.
    pub async fn enqueue_validation_job(
        &self,
        command: ModuleValidationJobEnqueueCommand,
    ) -> Result<ModuleValidationJobEnqueueResult, ModuleGovernanceError> {
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
        let request = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT id, slug, version, revision, status, artifact_origin FROM registry_publish_requests WHERE id = {}{}",
                    mark(1),
                    if backend == DbBackend::Postgres { " FOR UPDATE" } else { "" },
                ),
                vec![command.request_id.clone().into()],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        let request_id: String = request
            .try_get("", "id")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let slug: String = request
            .try_get("", "slug")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let version: String = request
            .try_get("", "version")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let status: String = request
            .try_get("", "status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let current_revision: i64 = request
            .try_get("", "revision")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if let Some(result) = validation_job_enqueue_replay(&tx, backend, &command).await? {
            tx.commit().await.map_err(store_error)?;
            return Ok(result);
        }
        if command.expected_revision != current_revision {
            return Err(ModuleGovernanceError::PublishRequestRevisionConflict {
                expected: command.expected_revision,
                current: current_revision,
            });
        }
        let artifact_origin = ModulePublicationArtifactOrigin::parse(
            &request
                .try_get::<String>("", "artifact_origin")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?,
        )
        .ok_or(ModuleGovernanceError::PublishRequestArtifactOriginUnclassified)?;
        if matches!(
            status.as_str(),
            "draft" | "changes_requested" | "on_hold" | "approved" | "published"
        ) || (status == "rejected" && !command.allow_rejected_retry)
        {
            return Err(ModuleGovernanceError::PublishRequestCannotQueueValidation(
                status,
            ));
        }
        let actor = validation_stage_actor_label(&command.actor_principal)?;
        let actor_json = command.actor_principal.clone();
        let stale_predicate = if backend == sea_orm::DbBackend::Postgres {
            format!(
                "started_at <= NOW() - INTERVAL '{} seconds'",
                VALIDATION_JOB_STALE_AFTER_SECONDS
            )
        } else {
            format!(
                "datetime(started_at) <= datetime('now', '-{} seconds')",
                VALIDATION_JOB_STALE_AFTER_SECONDS
            )
        };
        let stale_job = tx.query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT id FROM registry_validation_jobs WHERE request_id = {} AND status = 'running' AND {stale_predicate} ORDER BY started_at ASC LIMIT 1",
                mark(1)
            ),
            vec![request_id.clone().into()],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let recovered_stale_job = if let Some(job) = stale_job {
            let stale_job_id: String = job
                .try_get("", "id")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let updated = tx.execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_validation_jobs SET status = 'failed', finished_at = {now}, last_error = 'validation_worker_lease_expired', updated_at = {now} WHERE id = {} AND status = 'running' AND {stale_predicate}",
                    mark(1)
                ),
                vec![stale_job_id.clone().into()],
            )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            if updated.rows_affected() == 1 {
                let details = serde_json::json!({
                    "job_id": stale_job_id,
                    "reason_code": "validation_worker_lease_expired",
                    "lease_timeout_seconds": VALIDATION_JOB_STALE_AFTER_SECONDS,
                });
                tx.execute_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, 'validation_job_recovered', {}, NULL, {}, {now})",
                        mark(1), mark(2), mark(3), mark(4), mark(5)
                    ),
                    vec![
                        self.infrastructure.prefixed_id("rge").into(),
                        slug.clone().into(),
                        request_id.clone().into(),
                        Value::Json(Some(Box::new(actor_json.clone()))),
                        Value::Json(Some(Box::new(details))),
                    ],
                )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                true
            } else {
                false
            }
        } else {
            false
        };
        let active_job = tx.query_one_raw(Statement::from_sql_and_values(
            backend,
            format!("SELECT id FROM registry_validation_jobs WHERE request_id = {} AND status IN ('queued', 'running') ORDER BY created_at DESC LIMIT 1", mark(1)),
            vec![request_id.clone().into()],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if let Some(job) = active_job {
            let job_id: String = job
                .try_get("", "id")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            let result = ModuleValidationJobEnqueueResult {
                request_id,
                request_status: status,
                queued: false,
                validation_job_id: Some(job_id),
            };
            record_validation_job_enqueue_receipt(
                &self.infrastructure,
                &tx,
                backend,
                now,
                &command,
                &result,
            )
            .await?;
            tx.commit()
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            return Ok(result);
        }
        let requeued = status == "rejected";
        let queue_reason = if recovered_stale_job {
            "requeued_after_validation_lease_expired"
        } else if status == "validating" {
            "validation_resumed"
        } else if requeued {
            "requeued_after_validation_failed"
        } else {
            "initial_validation"
        };
        if status != "validating" {
            let request_updated = tx.execute_raw(Statement::from_sql_and_values(
                backend,
                format!("UPDATE registry_publish_requests SET status = 'validating', validation_errors = {}, rejected_by_principal = NULL, rejection_reason = NULL, validated_at = NULL, revision = revision + 1, updated_at = {now} WHERE id = {} AND revision = {}", mark(1), mark(2), mark(3)),
                vec![Value::Json(Some(Box::new(serde_json::json!([])))), request_id.clone().into(), command.expected_revision.into()],
            )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            if request_updated.rows_affected() != 1 {
                return Err(publish_request_revision_conflict(
                    &tx,
                    backend,
                    &command.request_id,
                    command.expected_revision,
                )
                .await?);
            }
        }
        let attempt = tx.query_one_raw(Statement::from_sql_and_values(
            backend,
            format!("SELECT COALESCE(MAX(attempt_number), 0) AS attempt_number FROM registry_validation_jobs WHERE request_id = {}", mark(1)),
            vec![request_id.clone().into()],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?.ok_or_else(|| ModuleGovernanceError::Store("missing validation job attempt aggregate".to_string()))?.try_get::<i64>("", "attempt_number").map_err(|e| ModuleGovernanceError::Store(e.to_string()))? as i32 + 1;
        let job_id = self.infrastructure.prefixed_id("rvj");
        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!("INSERT INTO registry_validation_jobs (id, request_id, slug, version, status, triggered_by, queue_reason, attempt_number, started_at, finished_at, last_error, created_at, updated_at) VALUES ({}, {}, {}, {}, 'queued', {}, {}, {}, NULL, NULL, NULL, {now}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7)),
            vec![job_id.clone().into(), request_id.clone().into(), slug.clone().into(), version.clone().into(), actor.clone().into(), queue_reason.into(), attempt.into()],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let events = [
            (
                if recovered_stale_job {
                    "validation_recovered"
                } else if requeued {
                    "validation_requeued"
                } else if status == "validating" {
                    "validation_resumed"
                } else {
                    "validation_queued"
                },
                serde_json::json!({"job_id":job_id,"attempt_number":attempt,"queue_reason":queue_reason,"version":version,"status":"validating","requeued":requeued,"artifact_origin":artifact_origin.as_str(),"follow_up_gates":publication_follow_up_stages(artifact_origin).iter().map(|stage| stage.key).collect::<Vec<_>>() }),
            ),
            (
                "validation_job_queued",
                serde_json::json!({"job_id":job_id,"attempt_number":attempt,"queue_reason":queue_reason,"request_status":"validating","version":version}),
            ),
        ];
        for (event_type, details) in events {
            tx.execute_raw(Statement::from_sql_and_values(backend,
                format!("INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, {}, {}, NULL, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6)),
                vec![self.infrastructure.prefixed_id("rge").into(), slug.clone().into(), request_id.clone().into(), event_type.into(), Value::Json(Some(Box::new(actor_json.clone()))), Value::Json(Some(Box::new(details)))],
            )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        }
        let result = ModuleValidationJobEnqueueResult {
            request_id,
            request_status: "validating".to_string(),
            queued: true,
            validation_job_id: Some(job_id),
        };
        record_validation_job_enqueue_receipt(
            &self.infrastructure,
            &tx,
            backend,
            now,
            &command,
            &result,
        )
        .await?;
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(result)
    }

    /// Claims a queued validation job through a conditional update and emits
    /// the started fact in the same transaction. A non-queued job is observed
    /// but never re-executed by this worker invocation.
    pub async fn claim_validation_job(
        &self,
        command: ModuleValidationJobClaimCommand,
    ) -> Result<Option<ModuleValidationJobClaimResult>, ModuleGovernanceError> {
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
        let Some(job) = tx.query_one_raw(Statement::from_sql_and_values(
            backend,
            format!("SELECT j.status, j.request_id, j.attempt_number, j.queue_reason, r.slug, r.version, r.revision AS request_revision, r.crate_name, r.ownership, r.trust_level, r.license, r.entry_type, r.artifact_origin, r.marketplace, r.ui_packages, r.validation_warnings, r.status AS request_status, r.artifact_storage_key, r.artifact_checksum_sha256, r.artifact_size, r.artifact_content_type, t.name AS module_name, t.description AS module_description FROM registry_validation_jobs j JOIN registry_publish_requests r ON r.id = j.request_id LEFT JOIN registry_publish_request_translations t ON t.request_id = r.id AND t.locale = r.default_locale WHERE j.id = {}", mark(1)),
            vec![command.validation_job_id.clone().into()],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))? else {
            return Ok(None);
        };
        let status: String = job
            .try_get("", "status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let request_id: String = job
            .try_get("", "request_id")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if status != "queued" {
            tx.commit()
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            return Ok(Some(ModuleValidationJobClaimResult {
                request_id,
                should_run: false,
                work_item: None,
            }));
        }
        let slug: String = job
            .try_get("", "slug")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let version: String = job
            .try_get("", "version")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let request_status: String = job
            .try_get("", "request_status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let request_revision: i64 = job
            .try_get("", "request_revision")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let attempt_number: i32 = job
            .try_get("", "attempt_number")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let queue_reason: String = job
            .try_get("", "queue_reason")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let work_item_result =
            (|| -> Result<ModuleValidationJobWorkItem, ModuleGovernanceError> {
                let artifact_storage_key = job
                    .try_get::<Option<String>>("", "artifact_storage_key")
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
                    .filter(|value| !value.trim().is_empty())
                    .ok_or(ModuleGovernanceError::PublishRequestMissingArtifactStorageKey)?;
                let artifact_checksum_sha256 = job
                    .try_get::<Option<String>>("", "artifact_checksum_sha256")
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
                    .filter(|value| !value.trim().is_empty())
                    .ok_or(ModuleGovernanceError::PublishRequestMissingArtifactChecksum)?;
                if !is_sha256_hex(&artifact_checksum_sha256) {
                    return Err(ModuleGovernanceError::PublishRequestInvalidArtifactChecksum);
                }
                let artifact_size = job
                    .try_get::<Option<i64>>("", "artifact_size")
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
                    .filter(|value| *value >= 0)
                    .ok_or(ModuleGovernanceError::PublishRequestMissingArtifactSize)?;
                let artifact_content_type = job
                    .try_get::<Option<String>>("", "artifact_content_type")
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
                    .filter(|value| !value.trim().is_empty())
                    .ok_or(ModuleGovernanceError::InvalidPublishArtifactAttachCommand)?;
                let crate_name: String = job
                    .try_get("", "crate_name")
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                let artifact_origin = ModulePublicationArtifactOrigin::parse(
                    &job.try_get::<String>("", "artifact_origin")
                        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?,
                )
                .ok_or(ModuleGovernanceError::PublishRequestArtifactOriginUnclassified)?;
                Ok(ModuleValidationJobWorkItem {
                    validation_job_id: command.validation_job_id.clone(),
                    request_id: request_id.clone(),
                    expected_request_revision: request_revision,
                    slug: slug.clone(),
                    version: version.clone(),
                    crate_name,
                    artifact_origin,
                    alloy_descriptor: None,
                    artifact_storage_key,
                    artifact_checksum_sha256,
                    artifact_size: artifact_size as u64,
                    artifact_content_type,
                    existing_warnings: validation_warnings_from_row(&job)?,
                    contract: module_publish_validation_contract_from_row(&job)?,
                })
            })();
        let work_item_result = match work_item_result {
            Ok(mut work_item)
                if work_item.artifact_origin == ModulePublicationArtifactOrigin::AlloyAuthored =>
            {
                alloy_descriptor_for_validation_work_item(
                    &tx,
                    backend,
                    &work_item.request_id,
                    &work_item.slug,
                    &work_item.version,
                    &format!("sha256:{}", work_item.artifact_checksum_sha256),
                )
                .await
                .map(|descriptor| {
                    work_item.alloy_descriptor = Some(descriptor);
                    work_item
                })
            }
            Ok(work_item) => Ok(work_item),
            Err(error) => Err(error),
        };
        let updated = tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!("UPDATE registry_validation_jobs SET status = 'running', started_at = {now}, finished_at = NULL, last_error = NULL, updated_at = {now} WHERE id = {} AND status = 'queued'", mark(1)),
            vec![command.validation_job_id.clone().into()],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if updated.rows_affected() != 1 {
            tx.commit()
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            return Ok(Some(ModuleValidationJobClaimResult {
                request_id,
                should_run: false,
                work_item: None,
            }));
        }
        let work_item = match work_item_result {
            Ok(work_item) => work_item,
            Err(_) => {
                terminalize_invalid_validation_work_item(
                    &self.infrastructure,
                    &tx,
                    backend,
                    InvalidValidationWorkItem {
                        command: &command,
                        request_id: &request_id,
                        expected_request_revision: request_revision,
                        slug: &slug,
                        version: &version,
                        attempt_number,
                        queue_reason: &queue_reason,
                    },
                )
                .await?;
                tx.commit()
                    .await
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                return Ok(Some(ModuleValidationJobClaimResult {
                    request_id,
                    should_run: false,
                    work_item: None,
                }));
            }
        };
        tx.execute_raw(Statement::from_sql_and_values(backend,
            format!("INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, 'validation_job_started', {}, NULL, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5)),
            vec![self.infrastructure.prefixed_id("rge").into(), slug.into(), request_id.clone().into(), Value::Json(Some(Box::new(command.actor_principal))), Value::Json(Some(Box::new(serde_json::json!({"job_id":command.validation_job_id,"attempt_number":attempt_number,"queue_reason":queue_reason,"request_status":request_status,"version":version}))))],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(Some(ModuleValidationJobClaimResult {
            request_id,
            should_run: true,
            work_item: Some(work_item),
        }))
    }

    /// Claims one oldest queued validation job for an independent worker.
    ///
    /// The initial lookup is intentionally only a candidate selection; the
    /// existing conditional claim remains the authority. A concurrent worker
    /// may win the candidate, in which case this method retries once before
    /// returning no work. The worker never needs an HTTP server callback or a
    /// synthetic tenant-scoped event to find durable queued work.
    pub async fn claim_next_validation_job(
        &self,
        actor_principal: serde_json::Value,
    ) -> Result<Option<ModuleValidationJobClaimResult>, ModuleGovernanceError> {
        let command = ModuleValidationJobClaimCommand {
            validation_job_id: "candidate".to_string(),
            actor_principal: actor_principal.clone(),
        };
        command.validate()?;
        for _ in 0..2 {
            let backend = self.db.get_database_backend();
            let candidate = self
                .db
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    "SELECT id FROM registry_validation_jobs WHERE status = 'queued' \
                     ORDER BY created_at ASC, id ASC LIMIT 1"
                        .to_string(),
                    Vec::new(),
                ))
                .await
                .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;
            let Some(candidate) = candidate else {
                return Ok(None);
            };
            let validation_job_id: String = candidate
                .try_get("", "id")
                .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;
            let claim = self
                .claim_validation_job(ModuleValidationJobClaimCommand {
                    validation_job_id,
                    actor_principal: actor_principal.clone(),
                })
                .await?;
            if claim.as_ref().is_some_and(|claim| claim.should_run) {
                return Ok(claim);
            }
        }
        Ok(None)
    }

}
