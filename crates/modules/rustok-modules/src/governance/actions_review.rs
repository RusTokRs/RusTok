//! Governance transitions for publish request rejection, change requests, holds, and resumes.

use sea_orm::{ConnectionTrait, Statement, TransactionTrait, Value};

use super::*;

impl SeaOrmModuleGovernanceService {
    pub async fn reject_publish_request(
        &self,
        command: ModulePublishRequestRejectCommand,
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
        let receipt = PublishRequestReviewReceipt {
            operation_kind: "reject",
            request_id: &command.request_id,
            expected_revision: command.expected_revision,
            context: &command.context,
            actor_principal: &command.actor_principal,
            reason: &command.reason,
            reason_code: &command.reason_code,
        };
        lock_publish_request(&tx, backend, &command.request_id).await?;
        if publish_request_review_replay(&tx, backend, &receipt).await? {
            tx.commit().await.map_err(store_error)?;
            return Ok(());
        }
        let request = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT slug, version, revision, status, CAST(validation_errors AS TEXT) AS validation_errors \
                     FROM registry_publish_requests WHERE id = {}",
                    mark(1)
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
        let status: String = request
            .try_get("", "status")
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
        if matches!(status.as_str(), "published" | "rejected" | "on_hold") {
            return Err(ModuleGovernanceError::PublishRequestCannotBeRejected(
                status,
            ));
        }
        let stored_errors: serde_json::Value = serde_json::from_str(
            &request
                .try_get::<String>("", "validation_errors")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?,
        )
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let mut errors = stored_errors
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str())
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let rejection_error = format!("Governance rejection reason: {}", command.reason);
        if !errors.iter().any(|value| value == &rejection_error) {
            errors.push(rejection_error);
        }
        let mut deduplicated = Vec::new();
        for error in errors {
            if !deduplicated.iter().any(|value| value == &error) {
                deduplicated.push(error);
            }
        }
        let validation_errors = serde_json::json!(deduplicated);
        let update = tx
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_publish_requests \
                     SET status = 'rejected', rejected_by_principal = {}, rejection_reason = {}, \
                     validation_errors = {}, revision = revision + 1, updated_at = {now} \
                     WHERE id = {} AND revision = {}",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                    mark(5),
                ),
                vec![
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    command.reason.clone().into(),
                    Value::Json(Some(Box::new(validation_errors.clone()))),
                    command.request_id.clone().into(),
                    command.expected_revision.into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if update.rows_affected() != 1 {
            return Err(publish_request_revision_conflict(
                &tx,
                backend,
                &command.request_id,
                command.expected_revision,
            )
            .await?);
        }
        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_governance_events \
                 (id, slug, request_id, release_id, event_type, actor_principal, \
                  publisher_principal, details, created_at) \
                 VALUES ({}, {}, {}, NULL, 'request_rejected', {}, NULL, {}, {now})",
                mark(1),
                mark(2),
                mark(3),
                mark(4),
                mark(5),
            ),
            vec![
                self.infrastructure.prefixed_id("rge").into(),
                slug.into(),
                command.request_id.clone().into(),
                Value::Json(Some(Box::new(command.actor_principal.clone()))),
                Value::Json(Some(Box::new(serde_json::json!({
                    "version": version,
                    "status": "rejected",
                    "reason": command.reason,
                    "reason_code": command.reason_code,
                    "errors": validation_errors,
                })))),
            ],
        ))
        .await
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        record_publish_request_review_receipt(
            &self.infrastructure,
            &tx,
            backend,
            now,
            &receipt,
            "rejected",
            command.expected_revision + 1,
        )
        .await?;
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(())
    }

    /// Moves an approved publish request back to the publisher and records the
    /// review decision atomically.
    pub async fn request_publish_request_changes(
        &self,
        command: ModulePublishRequestChangesCommand,
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
        let receipt = PublishRequestReviewReceipt {
            operation_kind: "request_changes",
            request_id: &command.request_id,
            expected_revision: command.expected_revision,
            context: &command.context,
            actor_principal: &command.actor_principal,
            reason: &command.reason,
            reason_code: &command.reason_code,
        };
        lock_publish_request(&tx, backend, &command.request_id).await?;
        if publish_request_review_replay(&tx, backend, &receipt).await? {
            tx.commit().await.map_err(store_error)?;
            return Ok(());
        }
        let request = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT slug, version, revision, status, CAST(publisher_principal AS TEXT) AS publisher_principal \
                     FROM registry_publish_requests WHERE id = {}",
                    mark(1)
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
        let status: String = request
            .try_get("", "status")
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
        if status != "approved" {
            return Err(ModuleGovernanceError::PublishRequestCannotRequestChanges(
                status,
            ));
        }
        let publisher: Option<serde_json::Value> = request
            .try_get::<Option<String>>("", "publisher_principal")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let update = tx
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_publish_requests \
                     SET status = 'changes_requested', changes_requested_by_principal = {}, \
                         changes_requested_reason = {}, changes_requested_reason_code = {}, \
                         changes_requested_at = {now}, revision = revision + 1, updated_at = {now} \
                     WHERE id = {} AND revision = {}",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                    mark(5),
                ),
                vec![
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    command.reason.clone().into(),
                    command.reason_code.clone().into(),
                    command.request_id.clone().into(),
                    command.expected_revision.into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if update.rows_affected() != 1 {
            return Err(publish_request_revision_conflict(
                &tx,
                backend,
                &command.request_id,
                command.expected_revision,
            )
            .await?);
        }
        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_governance_events \
                 (id, slug, request_id, release_id, event_type, actor_principal, \
                  publisher_principal, details, created_at) \
                 VALUES ({}, {}, {}, NULL, 'changes_requested', {}, {}, {}, {now})",
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
                Value::Json(Some(Box::new(command.actor_principal.clone()))),
                Value::Json(publisher.map(Box::new)),
                Value::Json(Some(Box::new(serde_json::json!({
                    "version": version,
                    "status": "changes_requested",
                    "reason": command.reason,
                    "reason_code": command.reason_code,
                })))),
            ],
        ))
        .await
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        record_publish_request_review_receipt(
            &self.infrastructure,
            &tx,
            backend,
            now,
            &receipt,
            "changes_requested",
            command.expected_revision + 1,
        )
        .await?;
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(())
    }

    /// Places an eligible publish request on hold while retaining the exact
    /// predecessor state required for a later resume.
    pub async fn hold_publish_request(
        &self,
        command: ModulePublishRequestHoldCommand,
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
        let receipt = PublishRequestReviewReceipt {
            operation_kind: "hold",
            request_id: &command.request_id,
            expected_revision: command.expected_revision,
            context: &command.context,
            actor_principal: &command.actor_principal,
            reason: &command.reason,
            reason_code: &command.reason_code,
        };
        lock_publish_request(&tx, backend, &command.request_id).await?;
        if publish_request_review_replay(&tx, backend, &receipt).await? {
            tx.commit().await.map_err(store_error)?;
            return Ok(());
        }
        let request = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT slug, version, revision, status, CAST(publisher_principal AS TEXT) AS publisher_principal \
                     FROM registry_publish_requests WHERE id = {}",
                    mark(1)
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
        let previous_status: String = request
            .try_get("", "status")
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
        if !matches!(
            previous_status.as_str(),
            "submitted" | "approved" | "changes_requested"
        ) {
            return Err(ModuleGovernanceError::PublishRequestCannotBeHeld(
                previous_status,
            ));
        }
        let publisher: Option<serde_json::Value> = request
            .try_get::<Option<String>>("", "publisher_principal")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let update = tx
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_publish_requests \
                     SET status = 'on_hold', held_by_principal = {}, held_reason = {}, \
                         held_reason_code = {}, held_at = {now}, held_from_status = {}, \
                         revision = revision + 1, updated_at = {now} \
                     WHERE id = {} AND revision = {}",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                    mark(5),
                    mark(6),
                ),
                vec![
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    command.reason.clone().into(),
                    command.reason_code.clone().into(),
                    previous_status.clone().into(),
                    command.request_id.clone().into(),
                    command.expected_revision.into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if update.rows_affected() != 1 {
            return Err(publish_request_revision_conflict(
                &tx,
                backend,
                &command.request_id,
                command.expected_revision,
            )
            .await?);
        }
        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_governance_events \
                 (id, slug, request_id, release_id, event_type, actor_principal, \
                  publisher_principal, details, created_at) \
                 VALUES ({}, {}, {}, NULL, 'request_held', {}, {}, {}, {now})",
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
                Value::Json(Some(Box::new(command.actor_principal.clone()))),
                Value::Json(publisher.map(Box::new)),
                Value::Json(Some(Box::new(serde_json::json!({
                    "version": version,
                    "status": "on_hold",
                    "held_from_status": previous_status,
                    "reason": command.reason,
                    "reason_code": command.reason_code,
                })))),
            ],
        ))
        .await
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        record_publish_request_review_receipt(
            &self.infrastructure,
            &tx,
            backend,
            now,
            &receipt,
            "on_hold",
            command.expected_revision + 1,
        )
        .await?;
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(())
    }

    pub async fn resume_publish_request(
        &self,
        command: ModulePublishRequestResumeCommand,
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
        let receipt = PublishRequestReviewReceipt {
            operation_kind: "resume",
            request_id: &command.request_id,
            expected_revision: command.expected_revision,
            context: &command.context,
            actor_principal: &command.actor_principal,
            reason: &command.reason,
            reason_code: &command.reason_code,
        };
        lock_publish_request(&tx, backend, &command.request_id).await?;
        if publish_request_review_replay(&tx, backend, &receipt).await? {
            tx.commit().await.map_err(store_error)?;
            return Ok(());
        }
        let request = tx.query_one_raw(Statement::from_sql_and_values(backend, format!("SELECT slug, version, revision, status, artifact_origin, held_from_status, CAST(publisher_principal AS TEXT) AS publisher_principal FROM registry_publish_requests WHERE id = {}", mark(1)), vec![command.request_id.clone().into()])).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?.ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
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
        if command.expected_revision != current_revision {
            return Err(ModuleGovernanceError::PublishRequestRevisionConflict {
                expected: command.expected_revision,
                current: current_revision,
            });
        }
        let artifact_origin: String = request
            .try_get("", "artifact_origin")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if status != "on_hold" {
            return Err(ModuleGovernanceError::PublishRequestCannotBeResumed(status));
        }
        let resumed_status: String = request
            .try_get::<Option<String>>("", "held_from_status")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .filter(|value| {
                matches!(
                    value.as_str(),
                    "submitted" | "approved" | "changes_requested"
                )
            })
            .ok_or(ModuleGovernanceError::PublishRequestInvalidHeldFromStatus)?;
        let publisher: Option<serde_json::Value> = request
            .try_get::<Option<String>>("", "publisher_principal")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let update = tx.execute_raw(Statement::from_sql_and_values(backend, format!("UPDATE registry_publish_requests SET status = {}, revision = revision + 1, updated_at = {now} WHERE id = {} AND revision = {}", mark(1), mark(2), mark(3)), vec![resumed_status.clone().into(), command.request_id.clone().into(), command.expected_revision.into()])).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if update.rows_affected() != 1 {
            return Err(publish_request_revision_conflict(
                &tx,
                backend,
                &command.request_id,
                command.expected_revision,
            )
            .await?);
        }
        tx.execute_raw(Statement::from_sql_and_values(backend, format!("INSERT INTO registry_governance_events (id, slug, request_id, release_id, event_type, actor_principal, publisher_principal, details, created_at) VALUES ({}, {}, {}, NULL, 'request_resumed', {}, {}, {}, {now})", mark(1), mark(2), mark(3), mark(4), mark(5), mark(6)), vec![self.infrastructure.prefixed_id("rge").into(), slug.into(), command.request_id.clone().into(), Value::Json(Some(Box::new(command.actor_principal.clone()))), Value::Json(publisher.map(Box::new)), Value::Json(Some(Box::new(serde_json::json!({"version": version, "status": resumed_status.clone(), "resumed_from_hold": true, "resumed_to_status": resumed_status, "reason": command.reason, "reason_code": command.reason_code}))))])).await.map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if resumed_status == "approved" {
            if artifact_origin == ModulePublicationArtifactOrigin::ExternalPrebuilt.as_str() {
                reconcile_external_prebuilt_security_stage(
                    &self.infrastructure,
                    &tx,
                    backend,
                    &command.request_id,
                    &command.actor_principal,
                )
                .await?;
            } else if artifact_origin == ModulePublicationArtifactOrigin::AlloyAuthored.as_str() {
                reconcile_alloy_authored_security_stage(
                    &self.infrastructure,
                    &tx,
                    backend,
                    &command.request_id,
                    &command.actor_principal,
                )
                .await?;
            }
        }
        record_publish_request_review_receipt(
            &self.infrastructure,
            &tx,
            backend,
            now,
            &receipt,
            &resumed_status,
            command.expected_revision + 1,
        )
        .await?;
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(())
    }

    /// Persists a manual validation-stage transition or a fresh queued attempt
    /// with its stage and follow-up audit facts in one transaction.

}
