//! Artifact contract generation, admission persistence, and marketplace installation contracts.

use sea_orm::{ConnectionTrait, DatabaseTransaction, DbBackend, Statement, Value};

use super::*;

pub(crate) async fn canonical_marketplace_artifact_contract(
    transaction: &DatabaseTransaction,
    backend: DbBackend,
    request_id: &str,
    slug: &str,
    version: &str,
    artifact_origin: ModulePublicationArtifactOrigin,
    checksum_sha256: &str,
) -> Result<
    (
        crate::ModuleMarketplaceArtifactRelease,
        crate::ModuleArtifactDescriptor,
        crate::ArtifactSourceLineage,
    ),
    ModuleGovernanceError,
> {
    let mark = |position| placeholder(backend, position);
    let admitted = transaction
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT registry_id, repository, manifest_digest, payload_digest, \
                 descriptor_digest, CAST(descriptor AS TEXT) AS descriptor, runtime_kind, \
                 signature_reference, signature_digest, provenance_reference, provenance_digest, \
                 sbom_reference, sbom_digest, admission_reference, admission_digest \
                 FROM registry_publish_platform_admissions \
                 WHERE request_id = {} LIMIT 1",
                mark(1),
            ),
            vec![request_id.to_string().into()],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract)?;
    let descriptor = serde_json::from_str::<crate::ModuleArtifactDescriptor>(
        &admitted
            .try_get::<String>("", "descriptor")
            .map_err(store_error)?,
    )
    .map_err(store_error)?;
    let descriptor_digest = admitted
        .try_get::<String>("", "descriptor_digest")
        .map_err(store_error)?;
    let payload_digest = admitted
        .try_get::<String>("", "payload_digest")
        .map_err(store_error)?;
    let manifest_digest = admitted
        .try_get::<String>("", "manifest_digest")
        .map_err(store_error)?;
    let expected_uploaded_payload_digest = format!("sha256:{checksum_sha256}");
    if descriptor.slug != slug
        || descriptor.version != version
        || descriptor.artifact_digest != payload_digest
        || (artifact_origin != ModulePublicationArtifactOrigin::PlatformBuilt
            && payload_digest != expected_uploaded_payload_digest)
        || crate::canonical_artifact_descriptor_digest(&descriptor) != descriptor_digest
    {
        return Err(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract);
    }

    let runtime_kind = match admitted
        .try_get::<String>("", "runtime_kind")
        .map_err(store_error)?
        .as_str()
    {
        "rhai" => crate::ModuleMarketplaceRuntimeKind::Rhai,
        "wasm_component" => crate::ModuleMarketplaceRuntimeKind::WasmComponent,
        "sidecar" => crate::ModuleMarketplaceRuntimeKind::Sidecar,
        _ => {
            return Err(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract);
        }
    };
    let (source_reference, source_digest, parent_release) = match artifact_origin {
        ModulePublicationArtifactOrigin::PlatformBuilt => {
            let source = transaction
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT source_reference, source_digest, \
                                parent_release_slug, parent_release_version, parent_release_digest \
                         FROM registry_publish_build_staging \
                         WHERE request_id = {} AND component_digest = {} \
                           AND artifact_manifest_digest = {} \
                           AND staged_at >= ( \
                               SELECT submitted_at FROM registry_publish_requests \
                               WHERE id = registry_publish_build_staging.request_id \
                           ) \
                         ORDER BY staged_at DESC, id DESC LIMIT 1",
                        mark(1),
                        mark(2),
                        mark(3),
                    ),
                    vec![
                        request_id.to_string().into(),
                        payload_digest.clone().into(),
                        manifest_digest.clone().into(),
                    ],
                ))
                .await
                .map_err(store_error)?
                .ok_or(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract)?;
            (
                source
                    .try_get::<Option<String>>("", "source_reference")
                    .map_err(store_error)?
                    .filter(|value| !value.trim().is_empty())
                    .ok_or(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract)?,
                source
                    .try_get::<String>("", "source_digest")
                    .map_err(store_error)?,
                parent_release_from_stage_row(&source)?,
            )
        }
        ModulePublicationArtifactOrigin::ExternalPrebuilt => {
            let source = transaction
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT source_reference, source_digest \
                         FROM registry_publish_external_staging \
                         WHERE request_id = {} AND artifact_digest = {} \
                           AND source_evidence_kind = 'reproducible' \
                         ORDER BY staged_at DESC LIMIT 1",
                        mark(1),
                        mark(2),
                    ),
                    vec![request_id.to_string().into(), payload_digest.clone().into()],
                ))
                .await
                .map_err(store_error)?
                .ok_or(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract)?;
            (
                source
                    .try_get::<Option<String>>("", "source_reference")
                    .map_err(store_error)?
                    .filter(|value| !value.trim().is_empty())
                    .ok_or(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract)?,
                source
                    .try_get::<Option<String>>("", "source_digest")
                    .map_err(store_error)?
                    .ok_or(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract)?,
                None,
            )
        }
        ModulePublicationArtifactOrigin::AlloyAuthored => {
            let source = transaction
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT CAST(alloy_tenant_id AS TEXT) AS alloy_tenant_id, \
                         CAST(alloy_script_id AS TEXT) AS alloy_script_id, \
                         source_revision, source_digest, \
                         CAST(descriptor AS TEXT) AS descriptor, descriptor_digest, \
                         parent_release_slug, parent_release_version, parent_release_digest \
                         FROM registry_publish_alloy_staging \
                         WHERE request_id = {} AND artifact_digest = {} \
                         ORDER BY staged_at DESC LIMIT 1",
                        mark(1),
                        mark(2),
                    ),
                    vec![request_id.to_string().into(), payload_digest.clone().into()],
                ))
                .await
                .map_err(store_error)?
                .ok_or(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract)?;
            let tenant_id = source
                .try_get::<String>("", "alloy_tenant_id")
                .map_err(store_error)?;
            let script_id = source
                .try_get::<String>("", "alloy_script_id")
                .map_err(store_error)?;
            let revision = source
                .try_get::<i64>("", "source_revision")
                .map_err(store_error)?;
            let staged_descriptor = serde_json::from_str::<crate::ModuleArtifactDescriptor>(
                &source
                    .try_get::<String>("", "descriptor")
                    .map_err(store_error)?,
            )
            .map_err(|_| ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract)?;
            let staged_descriptor_digest = source
                .try_get::<String>("", "descriptor_digest")
                .map_err(store_error)?;
            let staged_source_digest = source
                .try_get::<String>("", "source_digest")
                .map_err(store_error)?;
            if staged_descriptor != descriptor
                || crate::canonical_artifact_descriptor_digest(&staged_descriptor)
                    != staged_descriptor_digest
                || staged_source_digest != payload_digest
            {
                return Err(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract);
            }
            (
                format!("alloy://{tenant_id}/{script_id}/{revision}"),
                staged_source_digest,
                parent_release_from_stage_row(&source)?,
            )
        }
    };
    if let Some(parent_release) = &parent_release {
        let parent_version = Version::parse(&parent_release.version)
            .expect("validated artifact release version must parse");
        let child_version = Version::parse(version)
            .map_err(|_| ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract)?;
        if parent_release.slug != slug
            || parent_release.digest == source_digest
            || child_version <= parent_version
            || !active_published_rhai_parent_exists(transaction, backend, parent_release).await?
        {
            return Err(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract);
        }
    }

    let (author_reference, author_digest) = load_publication_evidence_contract(
        transaction,
        backend,
        request_id,
        ModulePublicationEvidenceAuthority::AuthorSignature,
        checksum_sha256,
    )
    .await?;
    let (approval_reference, approval_digest) = load_publication_evidence_contract(
        transaction,
        backend,
        request_id,
        ModulePublicationEvidenceAuthority::MarketplaceApproval,
        checksum_sha256,
    )
    .await?;
    let mut evidence = vec![
        crate::ModuleMarketplaceEvidenceReference {
            kind: crate::ModuleMarketplaceEvidenceKind::AuthorSignature,
            reference: author_reference,
            digest: author_digest,
        },
        crate::ModuleMarketplaceEvidenceReference {
            kind: crate::ModuleMarketplaceEvidenceKind::Sbom,
            reference: admitted
                .try_get::<String>("", "sbom_reference")
                .map_err(store_error)?,
            digest: admitted
                .try_get::<String>("", "sbom_digest")
                .map_err(store_error)?,
        },
        crate::ModuleMarketplaceEvidenceReference {
            kind: crate::ModuleMarketplaceEvidenceKind::Provenance,
            reference: admitted
                .try_get::<String>("", "provenance_reference")
                .map_err(store_error)?,
            digest: admitted
                .try_get::<String>("", "provenance_digest")
                .map_err(store_error)?,
        },
        crate::ModuleMarketplaceEvidenceReference {
            kind: crate::ModuleMarketplaceEvidenceKind::PlatformAdmission,
            reference: admitted
                .try_get::<String>("", "admission_reference")
                .map_err(store_error)?,
            digest: format!(
                "sha256:{}",
                admitted
                    .try_get::<String>("", "admission_digest")
                    .map_err(store_error)?
            ),
        },
        crate::ModuleMarketplaceEvidenceReference {
            kind: crate::ModuleMarketplaceEvidenceKind::MarketplaceApproval,
            reference: approval_reference,
            digest: approval_digest,
        },
    ];
    if artifact_origin == ModulePublicationArtifactOrigin::PlatformBuilt {
        let manifest_subject = receipt_digest_sha256(&manifest_digest)
            .map_err(|_| ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract)?;
        let (reference, digest) = load_publication_evidence_contract(
            transaction,
            backend,
            request_id,
            ModulePublicationEvidenceAuthority::BuildServiceAttestation,
            manifest_subject,
        )
        .await?;
        evidence.push(crate::ModuleMarketplaceEvidenceReference {
            kind: crate::ModuleMarketplaceEvidenceKind::BuildServiceAttestation,
            reference,
            digest,
        });
    }
    let origin = match artifact_origin {
        ModulePublicationArtifactOrigin::PlatformBuilt => {
            crate::ModuleMarketplaceArtifactOrigin::PlatformBuilt
        }
        ModulePublicationArtifactOrigin::ExternalPrebuilt => {
            crate::ModuleMarketplaceArtifactOrigin::ExternalPrebuilt
        }
        ModulePublicationArtifactOrigin::AlloyAuthored => {
            crate::ModuleMarketplaceArtifactOrigin::AlloyAuthored
        }
    };
    let artifact = crate::ModuleMarketplaceArtifactRelease {
        registry_id: admitted
            .try_get::<String>("", "registry_id")
            .map_err(store_error)?,
        repository: admitted
            .try_get::<String>("", "repository")
            .map_err(store_error)?,
        origin,
        runtime_kind,
        oci_manifest_digest: manifest_digest,
        payload_digest,
        descriptor_digest,
        source_reference,
        source_digest: source_digest.clone(),
        evidence,
    };
    artifact
        .validate()
        .map_err(|_| ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract)?;
    Ok((
        artifact,
        descriptor,
        crate::ArtifactSourceLineage {
            origin: crate::ArtifactOrigin::Marketplace,
            source_digest,
            parent_release,
        },
    ))
}

pub(crate) async fn persist_published_artifact_contract(
    transaction: &DatabaseTransaction,
    backend: DbBackend,
    release_id: &str,
    request_id: &str,
    artifact: &crate::ModuleMarketplaceArtifactRelease,
    descriptor: &crate::ModuleArtifactDescriptor,
    lineage: &crate::ArtifactSourceLineage,
) -> Result<(), ModuleGovernanceError> {
    let mark = |position| placeholder(backend, position);
    let now = database_now(backend);
    let artifact_json = serde_json::to_value(artifact).map_err(store_error)?;
    let descriptor_json = serde_json::to_value(descriptor).map_err(store_error)?;
    let lineage_json = serde_json::to_value(lineage).map_err(store_error)?;
    transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_module_release_artifacts \
                 (release_id, request_id, artifact, descriptor, lineage, created_at) \
                 VALUES ({}, {}, {}, {}, {}, {now}) \
                 ON CONFLICT (release_id) DO NOTHING",
                mark(1),
                mark(2),
                mark(3),
                mark(4),
                mark(5),
            ),
            vec![
                release_id.to_string().into(),
                request_id.to_string().into(),
                Value::Json(Some(Box::new(artifact_json))),
                Value::Json(Some(Box::new(descriptor_json))),
                Value::Json(Some(Box::new(lineage_json))),
            ],
        ))
        .await
        .map_err(store_error)?;
    let stored = transaction
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT request_id, CAST(artifact AS TEXT) AS artifact, \
                 CAST(descriptor AS TEXT) AS descriptor, CAST(lineage AS TEXT) AS lineage \
                 FROM registry_module_release_artifacts WHERE release_id = {}",
                mark(1),
            ),
            vec![release_id.to_string().into()],
        ))
        .await
        .map_err(store_error)?
        .ok_or(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract)?;
    let stored_artifact = serde_json::from_str::<crate::ModuleMarketplaceArtifactRelease>(
        &stored
            .try_get::<String>("", "artifact")
            .map_err(store_error)?,
    )
    .map_err(store_error)?;
    let stored_descriptor = serde_json::from_str::<crate::ModuleArtifactDescriptor>(
        &stored
            .try_get::<String>("", "descriptor")
            .map_err(store_error)?,
    )
    .map_err(store_error)?;
    let stored_lineage = serde_json::from_str::<crate::ArtifactSourceLineage>(
        &stored
            .try_get::<String>("", "lineage")
            .map_err(store_error)?,
    )
    .map_err(store_error)?;
    if stored
        .try_get::<String>("", "request_id")
        .map_err(store_error)?
        != request_id
        || stored_artifact != *artifact
        || stored_descriptor != *descriptor
        || stored_lineage != *lineage
    {
        return Err(ModuleGovernanceError::PublicationIdempotencyConflict);
    }
    Ok(())
}

pub(crate) async fn persist_platform_admission_contract(
    transaction: &DatabaseTransaction,
    backend: DbBackend,
    command: &ModulePlatformAdmissionCommand,
    admission_reference: &str,
    admission_digest: &str,
) -> Result<(), ModuleGovernanceError> {
    let descriptor_digest = crate::canonical_artifact_descriptor_digest(&command.descriptor);
    let runtime_kind = match command.descriptor.payload_kind {
        crate::ArtifactPayloadKind::Rhai => "rhai",
        crate::ArtifactPayloadKind::WasmComponent => "wasm_component",
        crate::ArtifactPayloadKind::Sidecar => "sidecar",
        crate::ArtifactPayloadKind::StaticPromoted => {
            return Err(ModuleGovernanceError::InvalidPlatformAdmissionCommand);
        }
    };
    let evidence = |kind| {
        command
            .evidence
            .evidence
            .iter()
            .find(|evidence| evidence.kind == kind)
            .ok_or(ModuleGovernanceError::InvalidPlatformAdmissionCommand)
    };
    let signature = evidence(crate::TrustEvidenceKind::Signature)?;
    let provenance = evidence(crate::TrustEvidenceKind::Provenance)?;
    let sbom = evidence(crate::TrustEvidenceKind::Sbom)?;
    let descriptor = serde_json::to_value(&command.descriptor).map_err(store_error)?;
    let mark = |position| placeholder(backend, position);
    let now = database_now(backend);
    transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO registry_publish_platform_admissions \
                 (request_id, registry_id, registry, repository, manifest_digest, payload_digest, \
                  descriptor_digest, descriptor, runtime_kind, media_type, \
                  signature_reference, signature_digest, provenance_reference, \
                  provenance_digest, sbom_reference, sbom_digest, \
                  admission_reference, admission_digest, recorded_at) \
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {now}) \
                 ON CONFLICT (request_id) DO NOTHING",
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
                mark(16),
                mark(17),
                mark(18),
            ),
            vec![
                command.request_id.clone().into(),
                command.registry_id.clone().into(),
                command.reference.registry.clone().into(),
                command.reference.repository.clone().into(),
                command.reference.digest.clone().into(),
                command.evidence.payload_digest.clone().into(),
                descriptor_digest.clone().into(),
                Value::Json(Some(Box::new(descriptor))),
                runtime_kind.into(),
                command.evidence.media_type.clone().into(),
                signature.reference.clone().into(),
                signature.digest.clone().into(),
                provenance.reference.clone().into(),
                provenance.digest.clone().into(),
                sbom.reference.clone().into(),
                sbom.digest.clone().into(),
                admission_reference.to_string().into(),
                admission_digest.to_string().into(),
            ],
        ))
        .await
        .map_err(store_error)?;

    let stored = transaction
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT registry_id, registry, repository, manifest_digest, payload_digest, \
                 descriptor_digest, CAST(descriptor AS TEXT) AS descriptor, runtime_kind, \
                 media_type, signature_reference, signature_digest, provenance_reference, \
                 provenance_digest, sbom_reference, sbom_digest, admission_reference, \
                 admission_digest \
                 FROM registry_publish_platform_admissions WHERE request_id = {}",
                mark(1),
            ),
            vec![command.request_id.clone().into()],
        ))
        .await
        .map_err(store_error)?
        .ok_or_else(|| {
            ModuleGovernanceError::Store(
                "platform admission insert completed without a durable contract".to_string(),
            )
        })?;
    let stored_descriptor = serde_json::from_str::<crate::ModuleArtifactDescriptor>(
        &stored
            .try_get::<String>("", "descriptor")
            .map_err(store_error)?,
    )
    .map_err(store_error)?;
    let matches = stored
        .try_get::<String>("", "registry_id")
        .map_err(store_error)?
        == command.registry_id
        && stored
            .try_get::<String>("", "registry")
            .map_err(store_error)?
            == command.reference.registry
        && stored
            .try_get::<String>("", "repository")
            .map_err(store_error)?
            == command.reference.repository
        && stored
            .try_get::<String>("", "manifest_digest")
            .map_err(store_error)?
            == command.reference.digest
        && stored
            .try_get::<String>("", "payload_digest")
            .map_err(store_error)?
            == command.evidence.payload_digest
        && stored
            .try_get::<String>("", "descriptor_digest")
            .map_err(store_error)?
            == descriptor_digest
        && stored_descriptor == command.descriptor
        && stored
            .try_get::<String>("", "runtime_kind")
            .map_err(store_error)?
            == runtime_kind
        && stored
            .try_get::<String>("", "media_type")
            .map_err(store_error)?
            == command.evidence.media_type
        && stored
            .try_get::<String>("", "signature_reference")
            .map_err(store_error)?
            == signature.reference
        && stored
            .try_get::<String>("", "signature_digest")
            .map_err(store_error)?
            == signature.digest
        && stored
            .try_get::<String>("", "provenance_reference")
            .map_err(store_error)?
            == provenance.reference
        && stored
            .try_get::<String>("", "provenance_digest")
            .map_err(store_error)?
            == provenance.digest
        && stored
            .try_get::<String>("", "sbom_reference")
            .map_err(store_error)?
            == sbom.reference
        && stored
            .try_get::<String>("", "sbom_digest")
            .map_err(store_error)?
            == sbom.digest
        && stored
            .try_get::<String>("", "admission_reference")
            .map_err(store_error)?
            == admission_reference
        && stored
            .try_get::<String>("", "admission_digest")
            .map_err(store_error)?
            == admission_digest;
    if !matches {
        return Err(ModuleGovernanceError::PublicationIdempotencyConflict);
    }
    Ok(())
}

