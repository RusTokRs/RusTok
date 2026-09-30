//! Author signature evidence, build attestations, and platform admission recording.

use sea_orm::{ConnectionTrait, DatabaseTransaction, DbBackend, Statement, TransactionTrait, Value};

use super::*;
use super::admissions::*;
use super::helpers::*;
use super::mapping::*;
use super::receipts::*;
use super::staging_alloy::*;
use super::staging_external::*;
use super::validation_work_items::*;

impl SeaOrmModuleGovernanceService {

    /// Records one operator-supplied author signature for the exact artifact
    /// currently attached to a publish request. The signed subject is loaded
    /// under the owner request lock rather than trusted from the transport.
    pub async fn record_author_signature_evidence(
        &self,
        command: ModuleAuthorSignatureEvidenceCommand,
    ) -> Result<ModulePublicationEvidenceResult, ModuleGovernanceError> {
        command.validate()?;
        self.record_publication_evidence_inner(
            ModulePublicationEvidenceCommand {
                request_id: command.request_id.clone(),
                expected_revision: command.expected_revision,
                authority: ModulePublicationEvidenceAuthority::AuthorSignature,
                subject_digest_sha256: "0".repeat(64),
                evidence_reference: command.evidence_reference.clone(),
                issuer_identity: command.signer_identity.clone(),
                policy_revision: command.policy_revision.clone(),
                signature_digest_sha256: None,
                actor_principal: command.actor_principal.clone(),
            },
            None,
            Some(&command),
        )
        .await
    }

    /// Records a build-service attestation only after the receipt's OCI
    /// identities and declared signature authority have been validated.
    pub async fn record_build_service_attestation(
        &self,
        command: ModuleBuildServiceAttestationCommand,
    ) -> Result<ModulePublicationEvidenceResult, ModuleGovernanceError> {
        command.validate()?;
        self.record_publication_evidence_inner(command.publication_evidence()?, None, None)
            .await
    }

    /// Records an admitted platform trust decision only after its exact OCI
    /// manifest, verified payload, policy revisions, and mandatory verification
    /// outcomes are bound to one immutable evidence fingerprint.
    pub async fn record_platform_admission(
        &self,
        command: ModulePlatformAdmissionCommand,
    ) -> Result<ModulePublicationEvidenceResult, ModuleGovernanceError> {
        command.validate()?;
        let backend = self.db.get_database_backend();
        let artifact_origin = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT artifact_origin FROM registry_publish_requests WHERE id = {}",
                    placeholder(backend, 1),
                ),
                vec![command.request_id.clone().into()],
            ))
            .await
            .map_err(store_error)?
            .ok_or(ModuleGovernanceError::PublishRequestNotFound)?
            .try_get::<String>("", "artifact_origin")
            .map_err(store_error)?;
        let artifact_origin = ModulePublicationArtifactOrigin::parse(&artifact_origin)
            .ok_or(ModuleGovernanceError::PublishRequestArtifactOriginUnclassified)?;
        let evidence = command.publication_evidence(artifact_origin)?;
        self.record_publication_evidence_inner(evidence, Some(&command), None)
            .await
    }

    async fn record_publication_evidence_inner(
        &self,
        mut command: ModulePublicationEvidenceCommand,
        platform_admission: Option<&ModulePlatformAdmissionCommand>,
        author_signature: Option<&ModuleAuthorSignatureEvidenceCommand>,
    ) -> Result<ModulePublicationEvidenceResult, ModuleGovernanceError> {
        let tx = self.db.begin().await.map_err(store_error)?;
        let backend = tx.get_database_backend();
        let mark = |n| placeholder(backend, n);
        let now = database_now(backend);
        let request_lock = if backend == DbBackend::Postgres {
            " FOR UPDATE"
        } else {
            ""
        };
        let request = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT slug, version, revision, status, artifact_origin, artifact_checksum_sha256, \
                            CAST(requested_by_principal AS TEXT) AS requested_by_principal, \
                            CAST(publisher_principal AS TEXT) AS publisher_principal \
                     FROM registry_publish_requests WHERE id = {}{request_lock}",
                    mark(1),
                ),
                vec![command.request_id.clone().into()],
            ))
            .await
            .map_err(store_error)?
            .ok_or(ModuleGovernanceError::PublishRequestNotFound)?;
        let slug: String = request.try_get("", "slug").map_err(store_error)?;
        let version: String = request.try_get("", "version").map_err(store_error)?;
        let request_revision: i64 = request.try_get("", "revision").map_err(store_error)?;
        let status: String = request.try_get("", "status").map_err(store_error)?;
        let artifact_origin: String = request
            .try_get("", "artifact_origin")
            .map_err(store_error)?;
        if let Some(author_signature) = author_signature {
            let requested_by_principal = required_json_text(&request, "requested_by_principal")?;
            let publisher_principal = optional_json_text(&request, "publisher_principal")?;
            let owner_principal = if author_signature.actor_can_manage_modules {
                None
            } else {
                governance_owner_principal_for_slug(&tx, backend, &slug, true).await?
            };
            if !governance_actor_can_manage_request_principals(
                &requested_by_principal,
                publisher_principal.as_ref(),
                owner_principal.as_ref(),
                &author_signature.actor_principal,
                author_signature.actor_can_manage_modules,
            ) {
                return Err(ModuleGovernanceError::PublishRequestAuthorSignatureUnauthorized);
            }
        }
        let author_signature_receipt = if let Some(author_signature) = author_signature {
            let subject_digest_sha256: Option<String> = request
                .try_get("", "artifact_checksum_sha256")
                .map_err(store_error)?;
            let subject_digest_sha256 = subject_digest_sha256
                .filter(|digest| is_sha256_hex(digest))
                .ok_or(ModuleGovernanceError::PublishRequestMissingArtifactChecksum)?;
            command = author_signature.publication_evidence(subject_digest_sha256.clone());
            let receipt = AuthorSignatureEvidenceReceipt {
                command: author_signature,
                subject_digest_sha256,
            };
            if let Some(result) = author_signature_evidence_replay(&tx, backend, &receipt).await? {
                tx.commit().await.map_err(store_error)?;
                return Ok(result);
            }
            Some(receipt)
        } else {
            None
        };
        if status == "rejected" {
            return Err(
                ModuleGovernanceError::PublishRequestCannotRecordPublicationEvidence(status),
            );
        }
        if let Some(platform_admission) = platform_admission {
            if platform_admission.descriptor.slug != slug
                || platform_admission.descriptor.version != version
            {
                return Err(ModuleGovernanceError::InvalidPlatformAdmissionCommand);
            }
            match artifact_origin.as_str() {
                origin if origin == ModulePublicationArtifactOrigin::AlloyAuthored.as_str() => {
                    let checksum = request
                        .try_get::<Option<String>>("", "artifact_checksum_sha256")
                        .map_err(store_error)?
                        .filter(|checksum| is_sha256_hex(checksum))
                        .ok_or(ModuleGovernanceError::PublishRequestMissingArtifactChecksum)?;
                    let artifact_digest = format!("sha256:{checksum}");
                    if platform_admission.evidence.payload_digest != artifact_digest
                        || platform_admission.descriptor.artifact_digest != artifact_digest
                        || platform_admission.evidence.media_type
                            != rustok_sandbox::RHAI_WORKSPACE_MEDIA_TYPE
                    {
                        return Err(ModuleGovernanceError::InvalidPlatformAdmissionCommand);
                    }
                    let staged_descriptor = alloy_descriptor_for_validation_work_item(
                        &tx,
                        backend,
                        &command.request_id,
                        &slug,
                        &version,
                        &artifact_digest,
                    )
                    .await?;
                    if staged_descriptor != platform_admission.descriptor {
                        return Err(ModuleGovernanceError::InvalidPlatformAdmissionCommand);
                    }
                }
                origin if origin == ModulePublicationArtifactOrigin::ExternalPrebuilt.as_str() => {
                    let checksum = request
                        .try_get::<Option<String>>("", "artifact_checksum_sha256")
                        .map_err(store_error)?
                        .filter(|checksum| is_sha256_hex(checksum))
                        .ok_or(ModuleGovernanceError::PublishRequestMissingArtifactChecksum)?;
                    let artifact_digest = format!("sha256:{checksum}");
                    if platform_admission.evidence.payload_digest != artifact_digest
                        || platform_admission.descriptor.artifact_digest != artifact_digest
                    {
                        return Err(ModuleGovernanceError::InvalidPlatformAdmissionCommand);
                    }
                }
                origin if origin == ModulePublicationArtifactOrigin::PlatformBuilt.as_str() => {}
                _ => return Err(ModuleGovernanceError::PublishRequestArtifactOriginUnclassified),
            }
        }

        let authority = command.authority.as_str();
        let evidence_digest_sha256 = publication_evidence_digest_sha256(&command);
        let evidence_id = self.infrastructure.prefixed_id("rpe");
        let inserted = tx
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO registry_publication_evidence \
                 (id, request_id, authority, subject_digest_sha256, evidence_reference, \
                  issuer_identity, policy_revision, signature_digest_sha256, evidence_digest_sha256, \
                  recorded_by_principal, created_at) \
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {now}) \
                 ON CONFLICT (request_id, evidence_digest_sha256) DO NOTHING",
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
                ),
                vec![
                    evidence_id.clone().into(),
                    command.request_id.clone().into(),
                    authority.into(),
                    command.subject_digest_sha256.clone().into(),
                    command.evidence_reference.clone().into(),
                    command.issuer_identity.clone().into(),
                    command.policy_revision.clone().into(),
                    command.signature_digest_sha256.clone().into(),
                    evidence_digest_sha256.clone().into(),
                    Value::Json(Some(Box::new(command.actor_principal.clone()))),
                ],
            ))
            .await
            .map_err(store_error)?;
        if let Some(platform_admission) = platform_admission {
            if platform_admission.descriptor.slug != slug
                || platform_admission.descriptor.version != version
            {
                return Err(ModuleGovernanceError::InvalidPlatformAdmissionCommand);
            }
            persist_platform_admission_contract(
                &tx,
                backend,
                platform_admission,
                &command.evidence_reference,
                &evidence_digest_sha256,
            )
            .await?;
        }
        if inserted.rows_affected() == 0 {
            let existing = tx
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT id FROM registry_publication_evidence \
                         WHERE request_id = {} AND evidence_digest_sha256 = {}",
                        mark(1),
                        mark(2),
                    ),
                    vec![
                        command.request_id.clone().into(),
                        evidence_digest_sha256.into(),
                    ],
                ))
                .await
                .map_err(store_error)?
                .ok_or_else(|| {
                    ModuleGovernanceError::Store(
                        "publication evidence conflict did not expose its existing record"
                            .to_string(),
                    )
                })?;
            let evidence_id: String = existing.try_get("", "id").map_err(store_error)?;
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
            let result = ModulePublicationEvidenceResult {
                evidence_id,
                recorded: false,
                request_revision,
            };
            if let Some(receipt) = author_signature_receipt.as_ref() {
                record_author_signature_evidence_receipt(
                    &self.infrastructure,
                    &tx,
                    backend,
                    now,
                    receipt,
                    &result,
                )
                .await?;
            }
            tx.commit().await.map_err(store_error)?;
            return Ok(result);
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
        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_governance_events \
                 (id, slug, request_id, release_id, event_type, actor_principal, \
                  publisher_principal, details, created_at) \
                 VALUES ({}, {}, {}, NULL, 'publication_evidence_recorded', {}, NULL, {}, {now})",
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
                    "evidence_id": evidence_id.clone(),
                    "authority": authority,
                    "subject_digest_sha256": command.subject_digest_sha256.clone(),
                    "policy_revision": command.policy_revision.clone(),
                })))),
            ],
        ))
        .await
        .map_err(store_error)?;
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
        let result = ModulePublicationEvidenceResult {
            evidence_id,
            recorded: true,
            request_revision: request_revision + 1,
        };
        if let Some(receipt) = author_signature_receipt.as_ref() {
            record_author_signature_evidence_receipt(
                &self.infrastructure,
                &tx,
                backend,
                now,
                receipt,
                &result,
            )
            .await?;
        }
        tx.commit().await.map_err(store_error)?;
        Ok(result)
    }

}

pub(crate) async fn load_publication_evidence_contract(
    transaction: &DatabaseTransaction,
    backend: DbBackend,
    request_id: &str,
    authority: ModulePublicationEvidenceAuthority,
    subject_digest_sha256: &str,
) -> Result<(String, String), ModuleGovernanceError> {
    let mark = |position| placeholder(backend, position);
    let row = transaction
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT evidence_reference, evidence_digest_sha256 \
                 FROM registry_publication_evidence \
                 WHERE request_id = {} AND authority = {} AND subject_digest_sha256 = {} \
                 ORDER BY created_at DESC, id DESC LIMIT 1",
                mark(1),
                mark(2),
                mark(3),
            ),
            vec![
                request_id.to_string().into(),
                authority.as_str().into(),
                subject_digest_sha256.to_string().into(),
            ],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract)?;
    let reference = row
        .try_get::<String>("", "evidence_reference")
        .map_err(store_error)?;
    let digest = row
        .try_get::<String>("", "evidence_digest_sha256")
        .map_err(store_error)?;
    if !is_sha256_hex(&digest) {
        return Err(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract);
    }
    Ok((reference, format!("sha256:{digest}")))
}

