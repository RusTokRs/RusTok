//! Publication execution and transactional release activation.

use sea_orm::{ConnectionTrait, Statement, TransactionTrait, Value};

use super::*;
use super::admissions::*;
use super::helpers::*;
use super::publication_verification::*;
use super::validation_evidence::*;

impl SeaOrmModuleGovernanceService {

    /// Publishes an approved request as one durable governance transition.
    ///
    /// The host performs authorization and assembles any override evidence;
    /// this owner transaction persists the complete release projection, owner
    /// binding, request finalization, and immutable audit facts together.
    pub async fn publish_request(
        &self,
        command: ModulePublishRequestPublicationCommand,
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
        let request_lock = if backend == sea_orm::DbBackend::Postgres {
            " FOR UPDATE"
        } else {
            ""
        };
        tx.query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT id FROM registry_publish_requests WHERE id = {}{request_lock}",
                mark(1)
            ),
            vec![command.request_id.clone().into()],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        let command_approval_override = command
            .approval_override
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(store_error)?;
        let existing_operation = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT CAST(actor_principal AS TEXT) AS actor_principal, \
                     CAST(publisher_principal AS TEXT) AS publisher_principal, \
                     CAST(allow_owner_rebind AS TEXT) AS allow_owner_rebind, \
                     CAST(approval_override AS TEXT) AS approval_override, \
                     CAST(actor_id AS TEXT) AS actor_id, trace_id, \
                     CAST(correlation_id AS TEXT) AS correlation_id, release_id \
                     FROM registry_publication_operations \
                     WHERE request_id = {} AND idempotency_key = {}{request_lock}",
                    mark(1),
                    mark(2),
                ),
                vec![
                    command.request_id.clone().into(),
                    registry_uuid_value(command.context.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(store_error)?;
        if let Some(operation) = existing_operation {
            let stored_actor: serde_json::Value = serde_json::from_str(
                &operation
                    .try_get::<String>("", "actor_principal")
                    .map_err(store_error)?,
            )
            .map_err(store_error)?;
            let stored_publisher: serde_json::Value = serde_json::from_str(
                &operation
                    .try_get::<String>("", "publisher_principal")
                    .map_err(store_error)?,
            )
            .map_err(store_error)?;
            let stored_override = operation
                .try_get::<Option<String>>("", "approval_override")
                .map_err(store_error)?
                .map(|value| serde_json::from_str::<serde_json::Value>(&value))
                .transpose()
                .map_err(store_error)?;
            let stored_allow_owner_rebind = operation
                .try_get::<String>("", "allow_owner_rebind")
                .map_err(store_error)?;
            let stored_actor_id = operation
                .try_get::<String>("", "actor_id")
                .map_err(store_error)?;
            let stored_trace_id = operation
                .try_get::<String>("", "trace_id")
                .map_err(store_error)?;
            let stored_correlation_id = operation
                .try_get::<String>("", "correlation_id")
                .map_err(store_error)?;
            let expected_allow_owner_rebind = if backend == sea_orm::DbBackend::Postgres {
                if command.allow_owner_rebind {
                    "true"
                } else {
                    "false"
                }
            } else if command.allow_owner_rebind {
                "1"
            } else {
                "0"
            };
            if stored_actor != command.actor_principal
                || stored_publisher != command.publisher_principal
                || stored_allow_owner_rebind != expected_allow_owner_rebind
                || stored_override != command_approval_override
                || stored_actor_id != command.context.actor_id.to_string()
                || stored_trace_id != command.context.trace_id
                || stored_correlation_id != command.context.correlation_id.to_string()
            {
                return Err(ModuleGovernanceError::PublicationIdempotencyConflict);
            }
            let release_id: String = operation.try_get("", "release_id").map_err(store_error)?;
            let release_exists = tx
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT release.id FROM registry_module_releases AS release \
                         INNER JOIN registry_module_release_artifacts AS artifact \
                           ON artifact.release_id = release.id \
                         WHERE release.id = {} LIMIT 1",
                        mark(1)
                    ),
                    vec![release_id.into()],
                ))
                .await
                .map_err(store_error)?
                .is_some();
            if !release_exists {
                return Err(ModuleGovernanceError::PublishedRequestMissingRelease);
            }
            tx.rollback().await.map_err(store_error)?;
            return Ok(());
        }

        let verified = match verify_publish_request_prerequisites(&tx, backend, &command).await {
            Ok(v) => v,
            Err(e) => {
                let _ = tx.rollback().await;
                return Err(e);
            }
        };
        let slug = verified.slug;
        let version = verified.version;
        let crate_name = verified.crate_name;
        let default_locale = verified.default_locale;
        let ownership = verified.ownership;
        let trust_level = verified.trust_level;
        let license = verified.license;
        let entry_type = verified.entry_type;
        let marketplace = verified.marketplace;
        let ui_packages = verified.ui_packages;
        let artifact_storage_key = verified.artifact_storage_key;
        let checksum_sha256 = verified.checksum_sha256;
        let artifact_size = verified.artifact_size;
        let artifact_origin = verified.artifact_origin;
        let translations = verified.translations;

        let marketplace_approval = ModulePublicationEvidenceCommand {
            request_id: command.request_id.clone(),
            expected_revision: command.expected_revision,
            authority: ModulePublicationEvidenceAuthority::MarketplaceApproval,
            subject_digest_sha256: checksum_sha256.clone(),
            evidence_reference: format!(
                "registry://publish-requests/{}/marketplace-approval",
                command.request_id
            ),
            issuer_identity: validation_stage_actor_label(&command.actor_principal)?,
            policy_revision: MARKETPLACE_APPROVAL_POLICY_REVISION.to_string(),
            signature_digest_sha256: None,
            actor_principal: command.actor_principal.clone(),
        };
        let marketplace_approval_digest = publication_evidence_digest_sha256(&marketplace_approval);
        let marketplace_approval_id = self.infrastructure.prefixed_id("rpe");
        let marketplace_approval_inserted = tx
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_publication_evidence \
                 (id, request_id, authority, subject_digest_sha256, evidence_reference, \
                  issuer_identity, policy_revision, signature_digest_sha256, evidence_digest_sha256, \
                  recorded_by_principal, created_at) \
                 VALUES ({}, {}, 'marketplace_approval', {}, {}, {}, {}, NULL, {}, {}, {now}) \
                 ON CONFLICT (request_id, evidence_digest_sha256) DO NOTHING",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                    mark(5),
                    mark(6),
                    mark(7),
                    mark(8),
                ),
                vec![
                    marketplace_approval_id.clone().into(),
                    command.request_id.clone().into(),
                    marketplace_approval.subject_digest_sha256.into(),
                    marketplace_approval.evidence_reference.into(),
                    marketplace_approval.issuer_identity.into(),
                    marketplace_approval.policy_revision.clone().into(),
                    marketplace_approval_digest.into(),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                ],
            ))
            .await
            .map_err(store_error)?;
        if marketplace_approval_inserted.rows_affected() == 1 {
            tx.execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_governance_events \
                     (id, slug, request_id, release_id, event_type, actor_principal, \
                      publisher_principal, details, created_at) \
                     VALUES ({}, {}, {}, NULL, 'marketplace_approval_recorded', {}, NULL, {}, {now})",
                    mark(1), mark(2), mark(3), mark(4), mark(5),
                ),
                vec![
                    self.infrastructure.prefixed_id("rge").into(),
                    slug.clone().into(),
                    command.request_id.clone().into(),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    Value::Json(Some(Box::new(serde_json::json!({
                        "evidence_id": marketplace_approval_id,
                        "authority": "marketplace_approval",
                        "subject_digest_sha256": checksum_sha256,
                        "policy_revision": MARKETPLACE_APPROVAL_POLICY_REVISION,
                    })))),
                ],
            )).await.map_err(store_error)?;
        }

        if let Some(override_evidence) = &command.approval_override {
            tx.execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_governance_events \
                     (id, slug, request_id, release_id, event_type, actor_principal, \
                      publisher_principal, details, created_at) \
                     VALUES ({}, {}, {}, NULL, 'publish_approval_override', {}, {}, {}, {now})",
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
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    Value::Json(Some(Box::new(command.publisher_principal.clone()))),
                    Value::Json(Some(Box::new(serde_json::json!({
                        "version": version,
                        "reason": override_evidence.reason,
                        "reason_code": override_evidence.reason_code,
                        "validation_stages": override_evidence.validation_stages,
                    })))),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        }

        let (artifact_contract, artifact_descriptor, artifact_lineage) =
            canonical_marketplace_artifact_contract(
                &tx,
                backend,
                &command.request_id,
                &slug,
                &version,
                artifact_origin,
                &checksum_sha256,
            )
            .await?;

        let release_id = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT id FROM registry_module_releases WHERE slug = {} AND version = {}",
                    mark(1),
                    mark(2),
                ),
                vec![slug.clone().into(), version.clone().into()],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .map(|row| {
                row.try_get::<String>("", "id")
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))
            })
            .transpose()?
            .unwrap_or_else(|| self.infrastructure.prefixed_id("rrel"));
        let release_exists = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT id FROM registry_module_releases WHERE id = {}",
                    mark(1)
                ),
                vec![release_id.clone().into()],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .is_some();
        if release_exists {
            tx.execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE registry_module_releases SET request_id = {}, crate_name = {}, \
                     default_locale = {}, ownership = {}, trust_level = {}, license = {}, \
                     entry_type = {}, artifact_origin = {}, marketplace = {}, ui_packages = {}, status = 'active', \
                     publisher_principal = {}, artifact_storage_key = {}, checksum_sha256 = {}, \
                     artifact_size = {}, yanked_reason = NULL, yanked_by_principal = NULL, \
                     yanked_at = NULL, published_at = {now}, updated_at = {now} WHERE id = {}",
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
                    mark(12),
                    mark(13),
                    mark(14),
                    mark(15),
                ),
                vec![
                    command.request_id.clone().into(),
                    crate_name.into(),
                    default_locale.into(),
                    ownership.into(),
                    trust_level.into(),
                    license.into(),
                    entry_type.into(),
                    artifact_origin.as_str().into(),
                    Value::Json(Some(Box::new(marketplace))),
                    Value::Json(Some(Box::new(ui_packages))),
                    Value::Json(Some(Box::new(command.publisher_principal.clone()))),
                    artifact_storage_key.into(),
                    checksum_sha256.clone().into(),
                    artifact_size.into(),
                    release_id.clone().into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        } else {
            tx.execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_module_releases \
                     (id, request_id, slug, version, crate_name, default_locale, ownership, trust_level, \
                      license, entry_type, artifact_origin, marketplace, ui_packages, status, publisher_principal, \
                      artifact_storage_key, checksum_sha256, artifact_size, yanked_reason, \
                      yanked_by_principal, yanked_at, published_at, created_at, updated_at) \
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, 'active', {}, {}, {}, {}, \
                             NULL, NULL, NULL, {now}, {now}, {now})",
                    mark(1), mark(2), mark(3), mark(4), mark(5), mark(6), mark(7), mark(8),
                    mark(9), mark(10), mark(11), mark(12), mark(13), mark(14), mark(15), mark(16), mark(17),
                ),
                vec![
                    release_id.clone().into(), command.request_id.clone().into(), slug.clone().into(),
                    version.clone().into(), crate_name.into(), default_locale.into(), ownership.into(),
                    trust_level.into(), license.into(), entry_type.into(),
                    artifact_origin.as_str().into(), Value::Json(Some(Box::new(marketplace))), Value::Json(Some(Box::new(ui_packages))),
                    Value::Json(Some(Box::new(command.publisher_principal.clone()))),
                    artifact_storage_key.into(), checksum_sha256.clone().into(), artifact_size.into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        }

        persist_published_artifact_contract(
            &tx,
            backend,
            &release_id,
            &command.request_id,
            &artifact_contract,
            &artifact_descriptor,
            &artifact_lineage,
        )
        .await?;

        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "DELETE FROM registry_module_release_translations WHERE release_id = {}",
                mark(1)
            ),
            vec![release_id.clone().into()],
        ))
        .await
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        for (locale, name, description) in translations {
            tx.execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_module_release_translations \
                     (release_id, locale, name, description, created_at, updated_at) \
                     VALUES ({}, {}, {}, {}, {now}, {now})",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                ),
                vec![
                    release_id.clone().into(),
                    locale.into(),
                    name.into(),
                    description.into(),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        }

        let existing_owner = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT CAST(owner_principal AS TEXT) AS owner_principal \
                     FROM registry_module_owners WHERE slug = {}",
                    mark(1)
                ),
                vec![slug.clone().into()],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let owner_transition = if let Some(existing_owner) = existing_owner {
            let previous_owner: serde_json::Value = serde_json::from_str(
                &existing_owner
                    .try_get::<String>("", "owner_principal")
                    .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?,
            )
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            if previous_owner == command.publisher_principal {
                tx.execute_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "UPDATE registry_module_owners SET bound_by_principal = {}, updated_at = {now} \
                         WHERE slug = {}",
                        mark(1), mark(2),
                    ),
                    vec![
                        Value::Json(Some(Box::new(command.actor_principal.clone()))),
                        slug.clone().into(),
                    ],
                ))
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                None
            } else {
                if !command.allow_owner_rebind {
                    return Err(ModuleGovernanceError::OwnerAlreadyBound);
                }
                tx.execute_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "UPDATE registry_module_owners SET owner_principal = {}, bound_by_principal = {}, \
                         bound_at = {now}, updated_at = {now} WHERE slug = {}",
                        mark(1), mark(2), mark(3),
                    ),
                    vec![
                        Value::Json(Some(Box::new(command.publisher_principal.clone()))),
                        Value::Json(Some(Box::new(command.actor_principal.clone()))),
                        slug.clone().into(),
                    ],
                ))
                .await
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
                Some(("rebind", Some(previous_owner)))
            }
        } else {
            tx.execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_module_owners \
                     (slug, owner_principal, bound_by_principal, bound_at, updated_at) \
                     VALUES ({}, {}, {}, {now}, {now})",
                    mark(1),
                    mark(2),
                    mark(3),
                ),
                vec![
                    slug.clone().into(),
                    Value::Json(Some(Box::new(command.publisher_principal.clone()))),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
            Some(("initial", None))
        };
        if let Some((mode, previous_owner)) = owner_transition {
            tx.execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_governance_events \
                     (id, slug, request_id, release_id, event_type, actor_principal, \
                      publisher_principal, details, created_at) \
                     VALUES ({}, {}, {}, {}, 'owner_bound', {}, {}, {}, {now})",
                    mark(1),
                    mark(2),
                    mark(3),
                    mark(4),
                    mark(5),
                    mark(6),
                    mark(7),
                ),
                vec![
                    self.infrastructure.prefixed_id("rge").into(),
                    slug.clone().into(),
                    command.request_id.clone().into(),
                    release_id.clone().into(),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                    Value::Json(Some(Box::new(command.publisher_principal.clone()))),
                    Value::Json(Some(Box::new(serde_json::json!({
                        "owner_transition": {
                            "previous_owner": previous_owner,
                            "new_owner": command.publisher_principal,
                            "bound_by": command.actor_principal,
                        },
                        "mode": mode,
                    })))),
                ],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        }

        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_publication_operations \
                 (operation_id, request_id, idempotency_key, actor_id, trace_id, correlation_id, \
                  actor_principal, publisher_principal, allow_owner_rebind, approval_override, \
                  release_id, committed_at) \
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {now})",
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
                registry_uuid_value(self.infrastructure.new_id(), backend),
                command.request_id.clone().into(),
                registry_uuid_value(command.context.idempotency_key, backend),
                registry_uuid_value(command.context.actor_id, backend),
                command.context.trace_id.clone().into(),
                registry_uuid_value(command.context.correlation_id, backend),
                Value::Json(Some(Box::new(command.actor_principal.clone()))),
                Value::Json(Some(Box::new(command.publisher_principal.clone()))),
                command.allow_owner_rebind.into(),
                Value::Json(command_approval_override.clone().map(Box::new)),
                release_id.clone().into(),
            ],
        ))
        .await
        .map_err(store_error)?;

        let request_updated = tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE registry_publish_requests SET status = 'published', approved_by_principal = {}, \
                 approved_at = {now}, published_at = {now}, revision = revision + 1, updated_at = {now} \
                 WHERE id = {} AND revision = {}",
                mark(1), mark(2), mark(3),
            ),
            vec![
                Value::Json(Some(Box::new(command.actor_principal.clone()))),
                command.request_id.clone().into(),
                command.expected_revision.into(),
            ],
        ))
        .await
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if request_updated.rows_affected() != 1 {
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
                 VALUES ({}, {}, {}, {}, 'release_published', {}, {}, {}, {now})",
                mark(1),
                mark(2),
                mark(3),
                mark(4),
                mark(5),
                mark(6),
                mark(7),
            ),
            vec![
                self.infrastructure.prefixed_id("rge").into(),
                slug.into(),
                command.request_id.into(),
                release_id.into(),
                Value::Json(Some(Box::new(command.actor_principal))),
                Value::Json(Some(Box::new(command.publisher_principal))),
                Value::Json(Some(Box::new(serde_json::json!({
                    "version": version,
                    "status": "published",
                    "artifact_origin": artifact_origin.as_str(),
                    "checksum_sha256": checksum_sha256,
                    "release_status": "active",
                })))),
            ],
        ))
        .await
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        Ok(())
    }


}
