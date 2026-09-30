//! Validation job results application and retry recording.

use sea_orm::{ConnectionTrait, Statement, TransactionTrait, Value};

use super::*;

impl SeaOrmModuleGovernanceService {
    pub async fn apply_validation_job_result(
        &self,
        command: ModuleValidationJobResultCommand,
    ) -> Result<String, ModuleGovernanceError> {
        command.validate()?;
        let automated_checks = normalize_governance_automated_checks(&command.automated_checks);
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
        let job = tx.query_one_raw(Statement::from_sql_and_values(
            backend,
            format!("SELECT j.status AS job_status, j.request_id, j.attempt_number, j.queue_reason, r.slug, r.version, r.artifact_origin, r.status AS request_status, r.revision AS request_revision FROM registry_validation_jobs j JOIN registry_publish_requests r ON r.id = j.request_id WHERE j.id = {}", mark(1)),
            vec![command.validation_job_id.clone().into()],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?.ok_or(ModuleGovernanceError::ValidationJobNotFound)?;
        let job_status: String = job
            .try_get("", "job_status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let request_id: String = job
            .try_get("", "request_id")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let request_status: String = job
            .try_get("", "request_status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let slug: String = job
            .try_get("", "slug")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let version: String = job
            .try_get("", "version")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let artifact_origin = ModulePublicationArtifactOrigin::parse(
            &job.try_get::<String>("", "artifact_origin")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?,
        )
        .ok_or(ModuleGovernanceError::PublishRequestArtifactOriginUnclassified)?;
        let attempt_number: i32 = job
            .try_get("", "attempt_number")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let queue_reason: String = job
            .try_get("", "queue_reason")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let (terminal_job_status, terminal_event_type, terminal_request_status) =
            match command.outcome {
                ModuleValidationJobResultOutcome::Passed => {
                    ("succeeded", "validation_job_succeeded", "approved")
                }
                ModuleValidationJobResultOutcome::Failed => {
                    ("failed", "validation_job_failed", "rejected")
                }
            };
        if job_status != "running" {
            if job_status == terminal_job_status && request_status == terminal_request_status {
                tx.commit()
                    .await
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                return Ok(request_id);
            }
            return Err(ModuleGovernanceError::ValidationJobNotRunning(job_status));
        }
        if request_status != "validating" {
            return Err(ModuleGovernanceError::ValidationJobRequestStateMismatch(
                request_status,
            ));
        }
        let current_revision: i64 = job
            .try_get("", "request_revision")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if command.expected_request_revision != current_revision {
            return Err(ModuleGovernanceError::PublishRequestRevisionConflict {
                expected: command.expected_request_revision,
                current: current_revision,
            });
        }

        let warnings = dedupe_validation_messages(command.warnings);
        let errors = dedupe_validation_messages(command.errors);
        let last_error = errors.first().cloned();
        let actor = validation_stage_actor_label(&command.actor_principal)?;
        let request_updated = match command.outcome {
            ModuleValidationJobResultOutcome::Passed => tx.execute_raw(Statement::from_sql_and_values(
                backend,
                format!("UPDATE registry_publish_requests SET status = 'approved', validation_warnings = {}, validation_errors = {}, rejected_by_principal = NULL, rejection_reason = NULL, validated_at = {now}, approved_by_principal = {}, approved_at = {now}, revision = revision + 1, updated_at = {now} WHERE id = {} AND status = 'validating' AND revision = {}", mark(1), mark(2), mark(3), mark(4), mark(5)),
                vec![Value::Json(Some(Box::new(serde_json::json!(warnings.clone())))), Value::Json(Some(Box::new(serde_json::json!([])))), Value::Json(Some(Box::new(command.actor_principal.clone()))), request_id.clone().into(), command.expected_request_revision.into()],
            )),
            ModuleValidationJobResultOutcome::Failed => tx.execute_raw(Statement::from_sql_and_values(
                backend,
                format!("UPDATE registry_publish_requests SET status = 'rejected', validation_warnings = {}, validation_errors = {}, rejected_by_principal = {}, rejection_reason = {}, validated_at = {now}, approved_by_principal = NULL, approved_at = NULL, published_at = NULL, revision = revision + 1, updated_at = {now} WHERE id = {} AND status = 'validating' AND revision = {}", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6)),
                vec![Value::Json(Some(Box::new(serde_json::json!(warnings.clone())))), Value::Json(Some(Box::new(serde_json::json!(errors.clone())))), Value::Json(Some(Box::new(command.actor_principal.clone()))), last_error.clone().into(), request_id.clone().into(), command.expected_request_revision.into()],
            )),
        }.await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if request_updated.rows_affected() != 1 {
            return Err(publish_request_revision_conflict(
                &tx,
                backend,
                &request_id,
                command.expected_request_revision,
            )
            .await?);
        }
        let job_updated = tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!("UPDATE registry_validation_jobs SET status = {}, finished_at = {now}, last_error = {}, updated_at = {now} WHERE id = {} AND status = 'running'", mark(1), mark(2), mark(3)),
            vec![terminal_job_status.into(), last_error.clone().into(), command.validation_job_id.clone().into()],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if job_updated.rows_affected() != 1 {
            return Err(ModuleGovernanceError::ValidationJobNotRunning(
                "concurrently changed".to_string(),
            ));
        }

        let mut stage_details = Vec::new();
        let mut follow_up_gates = Vec::new();
        if command.outcome == ModuleValidationJobResultOutcome::Passed {
            let platform_build_staged = artifact_origin
                == ModulePublicationArtifactOrigin::PlatformBuilt
                && tx
                    .query_one_raw(Statement::from_sql_and_values(
                        backend,
                        format!(
                            "SELECT 1 FROM registry_publish_build_staging WHERE request_id = {} LIMIT 1",
                            mark(1)
                        ),
                        vec![request_id.clone().into()],
                    ))
                    .await
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
                    .is_some();
            let external_supply_chain_evidence_current = artifact_origin
                == ModulePublicationArtifactOrigin::ExternalPrebuilt
                && external_prebuilt_supply_chain_evidence(&tx, backend, &request_id)
                    .await?
                    .is_some();
            let alloy_supply_chain_evidence_current = artifact_origin
                == ModulePublicationArtifactOrigin::AlloyAuthored
                && alloy_authored_supply_chain_evidence(&tx, backend, &request_id)
                    .await?
                    .is_some();
            for stage in publication_follow_up_stages(artifact_origin) {
                let stage_key = stage.key;
                let active_stage = tx.query_one_raw(Statement::from_sql_and_values(
                    backend,
                    format!("SELECT id FROM registry_validation_stages WHERE request_id = {} AND stage_key = {} AND status IN ('queued', 'running') LIMIT 1", mark(1), mark(2)),
                    vec![request_id.clone().into(), stage_key.into()],
                )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                if active_stage.is_some() {
                    continue;
                }
                let prior_attempt = tx.query_one_raw(Statement::from_sql_and_values(
                    backend,
                    format!("SELECT COALESCE(MAX(attempt_number), 0) AS attempt_number FROM registry_validation_stages WHERE request_id = {} AND stage_key = {}", mark(1), mark(2)),
                    vec![request_id.clone().into(), stage_key.into()],
                )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?.ok_or_else(|| ModuleGovernanceError::Store("missing validation stage attempt aggregate".to_string()))?;
                let attempt_number = prior_attempt
                    .try_get::<i64>("", "attempt_number")
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
                    as i32
                    + 1;
                let stage_id = self.infrastructure.prefixed_id("rvs");
                let initially_passed = stage.runner_kind == "owner_evidence"
                    && (platform_build_staged
                        || external_supply_chain_evidence_current
                        || alloy_supply_chain_evidence_current);
                let status = if initially_passed { "passed" } else { "queued" };
                let (detail, pass_reason_code) = match (
                    artifact_origin,
                    initially_passed,
                ) {
                    (ModulePublicationArtifactOrigin::PlatformBuilt, true) => (
                        format!(
                            "Platform build evidence satisfied validation stage '{stage_key}'."
                        ),
                        Some("platform_build_evidence_passed"),
                    ),
                    (ModulePublicationArtifactOrigin::ExternalPrebuilt, true) => (
                        "External provenance, quarantine, author-signature, and admitted signature/SBOM/SLSA/vulnerability evidence satisfied the security policy stage."
                            .to_string(),
                        Some("external_supply_chain_evidence_passed"),
                    ),
                    (ModulePublicationArtifactOrigin::AlloyAuthored, true) => (
                        "Reviewed Alloy source, capability-free production sandbox smoke, author-signature, and exact platform-admission evidence satisfied the security policy stage."
                            .to_string(),
                        Some("alloy_sandbox_evidence_passed"),
                    ),
                    _ => (
                        follow_up_validation_stage_detail(stage_key).to_string(),
                        None,
                    ),
                };
                let started_at = if initially_passed { now } else { "NULL" };
                let finished_at = if initially_passed { now } else { "NULL" };
                let runner_kind = if initially_passed {
                    None
                } else {
                    Some(stage.runner_kind.to_string())
                };
                tx.execute_raw(Statement::from_sql_and_values(
                    backend,
                    format!("INSERT INTO registry_validation_stages (id, request_id, slug, version, stage_key, status, triggered_by, queue_reason, attempt_number, detail, started_at, finished_at, last_error, claim_id, claimed_by, claim_expires_at, last_heartbeat_at, runner_kind, created_at, updated_at) VALUES ({}, {}, {}, {}, {}, {}, {}, 'validation_passed', {}, {}, {started_at}, {finished_at}, NULL, NULL, NULL, NULL, NULL, {}, {now}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7), mark(8), mark(9), mark(10)),
                    vec![stage_id.clone().into(), request_id.clone().into(), slug.clone().into(), version.clone().into(), stage_key.into(), status.into(), actor.clone().into(), attempt_number.into(), detail.clone().into(), runner_kind.into()],
                )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                let stage_value = serde_json::json!({"stage_id":stage_id,"stage_key":stage_key,"status":status,"detail":detail,"attempt_number":attempt_number,"queue_reason":"validation_passed","runner_kind":stage.runner_kind,"started_at":serde_json::Value::Null,"finished_at":serde_json::Value::Null});
                stage_details.push(stage_value.clone());
                let stage_event_type = if initially_passed {
                    "validation_stage_passed"
                } else {
                    "validation_stage_queued"
                };
                let gate_event_type = if initially_passed {
                    "follow_up_gate_passed"
                } else {
                    "follow_up_gate_queued"
                };
                let gate_status = if initially_passed {
                    "passed"
                } else {
                    "pending"
                };
                follow_up_gates.push(serde_json::json!({
                    "key": stage_key,
                    "status": gate_status,
                    "detail": detail,
                    "runner_kind": stage.runner_kind,
                }));
                for (event_type, details) in [
                    (
                        stage_event_type,
                        serde_json::json!({"stage_id":stage_id,"stage_key":stage_key,"status":status,"detail":detail,"attempt_number":attempt_number,"queue_reason":"validation_passed","request_status":"approved","version":version,"runner_kind":stage.runner_kind,"reason_code":pass_reason_code,"started_at":serde_json::Value::Null,"finished_at":serde_json::Value::Null}),
                    ),
                    (
                        gate_event_type,
                        serde_json::json!({"stage_key":stage_key,"status":gate_status,"detail":detail,"reason_code":pass_reason_code}),
                    ),
                ] {
                    tx.execute_raw(Statement::from_sql_and_values(backend,
                        format!("INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, {}, {}, NULL, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6)),
                        vec![self.infrastructure.prefixed_id("rge").into(), slug.clone().into(), request_id.clone().into(), event_type.into(), Value::Json(Some(Box::new(command.actor_principal.clone()))), Value::Json(Some(Box::new(details)))],
                    )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                }
            }
        }
        let result_event = match command.outcome {
            ModuleValidationJobResultOutcome::Passed => (
                "validation_passed",
                serde_json::json!({"version":version,"status":"approved","warnings":warnings,"automated_checks":automated_checks,"follow_up_gates":follow_up_gates,"validation_stages":stage_details}),
            ),
            ModuleValidationJobResultOutcome::Failed => (
                "validation_failed",
                serde_json::json!({"version":version,"status":"rejected","reason":last_error,"warnings":warnings,"errors":errors,"automated_checks":automated_checks}),
            ),
        };
        for (event_type, details) in [
            result_event,
            (
                terminal_event_type,
                serde_json::json!({"job_id":command.validation_job_id,"attempt_number":attempt_number,"queue_reason":queue_reason,"request_status":terminal_request_status,"version":version,"error":last_error}),
            ),
        ] {
            tx.execute_raw(Statement::from_sql_and_values(backend,
                format!("INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, {}, {}, NULL, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6)),
                vec![self.infrastructure.prefixed_id("rge").into(), slug.clone().into(), request_id.clone().into(), event_type.into(), Value::Json(Some(Box::new(command.actor_principal.clone()))), Value::Json(Some(Box::new(details)))],
            )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        }
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(request_id)
    }

    /// Records an in-flight worker retry observation as an owner-owned audit
    /// fact. It does not change the job or request state.
    pub async fn record_validation_job_retry(
        &self,
        command: ModuleValidationJobRetryCommand,
    ) -> Result<(), ModuleGovernanceError> {
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
        let job = tx.query_one_raw(Statement::from_sql_and_values(
            backend,
            format!("SELECT j.status, j.attempt_number, r.id AS request_id, r.slug, r.version, r.status AS request_status FROM registry_validation_jobs j JOIN registry_publish_requests r ON r.id = j.request_id WHERE j.id = {}", mark(1)),
            vec![command.validation_job_id.clone().into()],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?.ok_or(ModuleGovernanceError::ValidationJobNotFound)?;
        let status: String = job
            .try_get("", "status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if status != "running" {
            return Err(ModuleGovernanceError::ValidationJobNotRunning(status));
        }
        let request_id: String = job
            .try_get("", "request_id")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let slug: String = job
            .try_get("", "slug")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let version: String = job
            .try_get("", "version")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let request_status: String = job
            .try_get("", "request_status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let job_attempt: i32 = job
            .try_get("", "attempt_number")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let event_type = if command.retry_after_seconds.is_some() {
            "validation_retry_scheduled"
        } else {
            "validation_retry_exhausted"
        };
        let mut details = serde_json::json!({
            "job_id": command.validation_job_id,
            "job_attempt": job_attempt,
            "version": version,
            "status": request_status,
            "attempt": command.attempt,
            "error": VALIDATION_JOB_RETRY_ERROR,
        });
        if let Some(retry_after_seconds) = command.retry_after_seconds {
            details["next_attempt"] = serde_json::json!(command.attempt + 1);
            details["retry_after_seconds"] = serde_json::json!(retry_after_seconds);
        } else {
            details["max_attempts"] = serde_json::json!(command.attempt);
        }
        tx.execute_raw(Statement::from_sql_and_values(backend,
            format!("INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, {}, {}, NULL, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6)),
            vec![self.infrastructure.prefixed_id("rge").into(), slug.into(), request_id.into(), event_type.into(), Value::Json(Some(Box::new(command.actor_principal))), Value::Json(Some(Box::new(details)))],
        )).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(())
    }

    /// Renews a remote validation lease through a conditional update. The
    /// claim id, runner id, running state, remote ownership, and unexpired
    /// lease are one compare-and-swap predicate.

}
