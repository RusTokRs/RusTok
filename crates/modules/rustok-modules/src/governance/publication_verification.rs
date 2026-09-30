//! Publication prerequisites verification.

use sea_orm::{ConnectionTrait, DatabaseTransaction, DbBackend, Statement};

use super::*;

#[derive(Clone, Debug)]
pub(crate) struct VerifiedPublicationRequest {
    pub slug: String,
    pub version: String,
    pub crate_name: String,
    pub default_locale: String,
    pub ownership: String,
    pub trust_level: String,
    pub license: String,
    pub entry_type: String,
    pub marketplace_json: serde_json::Value,
    pub checksum_sha256: String,
    pub delivery_media_type: String,
    pub delivery_payload_digest: String,
    pub delivery_storage_key: String,
    pub delivery_size_bytes: i64,
    pub artifact_origin: ModulePublicationArtifactOrigin,
    pub command_approval_override: Option<serde_json::Value>,
    pub translations: Vec<(String, String, String)>,
}

pub(crate) async fn verify_publish_request_prerequisites(
    tx: &DatabaseTransaction,
    backend: DbBackend,
    command: &ModulePublishRequestPublicationCommand,
) -> Result<VerifiedPublicationRequest, ModuleGovernanceError> {
    let mark = |n| placeholder(backend, n);
    let request_lock = if backend == sea_orm::DbBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };

        let request = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT slug, version, revision, crate_name, default_locale, ownership, trust_level, license, \
                     entry_type, CAST(marketplace AS TEXT) AS marketplace, \
                     CAST(ui_packages AS TEXT) AS ui_packages, status, artifact_storage_key, \
                     artifact_checksum_sha256, artifact_size, artifact_origin \
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
        if status == "published" {
            let release_exists = tx
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT release.id FROM registry_module_releases AS release \
                         INNER JOIN registry_module_release_artifacts AS artifact \
                           ON artifact.release_id = release.id \
                         WHERE release.request_id = {} AND release.slug = {} \
                           AND release.version = {} LIMIT 1",
                        mark(1),
                        mark(2),
                        mark(3),
                    ),
                    vec![
                        command.request_id.clone().into(),
                        slug.clone().into(),
                        version.clone().into(),
                    ],
                ))
                .await
                .map_err(store_error)?
                .is_some();
            if !release_exists {
                return Err(ModuleGovernanceError::PublishedRequestMissingRelease);
            }
            tx.rollback().await.map_err(store_error)?;
            return Err(ModuleGovernanceError::PublishedRequestMissingIdempotencyRecord);
        }
        if status != "approved" {
            return Err(ModuleGovernanceError::PublishRequestCannotBePublished(
                status,
            ));
        }
        let artifact_origin: String = request
            .try_get("", "artifact_origin")
            .map_err(store_error)?;
        let artifact_origin = ModulePublicationArtifactOrigin::parse(&artifact_origin)
            .ok_or(ModuleGovernanceError::PublishRequestArtifactOriginUnclassified)?;
        let crate_name: String = request
            .try_get("", "crate_name")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let default_locale: String = request
            .try_get("", "default_locale")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let ownership: String = request
            .try_get("", "ownership")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let trust_level: String = request
            .try_get("", "trust_level")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let license: String = request
            .try_get("", "license")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let entry_type: Option<String> = request
            .try_get("", "entry_type")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let marketplace: serde_json::Value = serde_json::from_str(
            &request
                .try_get::<String>("", "marketplace")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?,
        )
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let ui_packages: serde_json::Value = serde_json::from_str(
            &request
                .try_get::<String>("", "ui_packages")
                .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?,
        )
        .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        let artifact_storage_key = request
            .try_get::<Option<String>>("", "artifact_storage_key")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .filter(|value| !value.trim().is_empty())
            .ok_or(ModuleGovernanceError::PublishRequestMissingArtifactStorageKey)?;
        let checksum_sha256 = request
            .try_get::<Option<String>>("", "artifact_checksum_sha256")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .filter(|value| !value.trim().is_empty())
            .ok_or(ModuleGovernanceError::PublishRequestMissingArtifactChecksum)?;
        if !is_sha256_hex(&checksum_sha256) {
            return Err(ModuleGovernanceError::PublishRequestInvalidArtifactChecksum);
        }
        let artifact_size = request
            .try_get::<Option<i64>>("", "artifact_size")
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?
            .filter(|value| *value >= 0)
            .ok_or(ModuleGovernanceError::PublishRequestMissingArtifactSize)?;

        let platform_build_manifest = match artifact_origin {
            ModulePublicationArtifactOrigin::PlatformBuilt => {
                let platform_build_manifest = tx
                    .query_one_raw(Statement::from_sql_and_values(
                        backend,
                        format!(
                            "SELECT artifact_manifest_digest FROM registry_publish_build_staging AS stage \
                             WHERE stage.request_id = {} \
                               AND stage.staged_at >= ( \
                                   SELECT request.submitted_at FROM registry_publish_requests AS request \
                                   WHERE request.id = stage.request_id \
                               ) \
                             ORDER BY stage.staged_at DESC, stage.id DESC LIMIT 1",
                            mark(1),
                        ),
                        vec![command.request_id.clone().into()],
                    ))
                    .await
                    .map_err(store_error)?
                    .map(|row| row.try_get::<String>("", "artifact_manifest_digest"))
                    .transpose()
                    .map_err(store_error)?;
                let Some(platform_build_manifest) = platform_build_manifest else {
                    return Err(ModuleGovernanceError::PublishRequestMissingPlatformBuildStage);
                };
                let platform_build_manifest = receipt_digest_sha256(&platform_build_manifest)
                    .map_err(|_| ModuleGovernanceError::PublishRequestMissingPlatformBuildStage)?;
                Some(platform_build_manifest.to_string())
            }
            ModulePublicationArtifactOrigin::ExternalPrebuilt => {
                let external_prebuilt_staged = tx
                    .query_one_raw(Statement::from_sql_and_values(
                        backend,
                        format!(
                            "SELECT 1 FROM registry_publish_external_staging AS stage \
                             WHERE stage.request_id = {} \
                               AND stage.artifact_digest = {} \
                               AND stage.staged_at >= ( \
                                   SELECT request.submitted_at FROM registry_publish_requests AS request \
                                   WHERE request.id = stage.request_id \
                               ) \
                             LIMIT 1",
                            mark(1),
                            mark(2),
                        ),
                        vec![
                            command.request_id.clone().into(),
                            format!("sha256:{checksum_sha256}").into(),
                        ],
                    ))
                    .await
                    .map_err(store_error)?
                    .is_some();
                if !external_prebuilt_staged {
                    return Err(ModuleGovernanceError::PublishRequestMissingExternalPrebuiltStage);
                }
                None
            }
            ModulePublicationArtifactOrigin::AlloyAuthored => {
                let alloy_authored_staged = tx
                    .query_one_raw(Statement::from_sql_and_values(
                        backend,
                        format!(
                            "SELECT 1 FROM registry_publish_alloy_staging AS stage \
                             WHERE stage.request_id = {} \
                               AND stage.artifact_digest = {} \
                               AND stage.staged_at >= ( \
                                   SELECT request.submitted_at FROM registry_publish_requests AS request \
                                   WHERE request.id = stage.request_id \
                               ) \
                             LIMIT 1",
                            mark(1),
                            mark(2),
                        ),
                        vec![
                            command.request_id.clone().into(),
                            format!("sha256:{checksum_sha256}").into(),
                        ],
                    ))
                    .await
                    .map_err(store_error)?
                    .is_some();
                if !alloy_authored_staged {
                    return Err(ModuleGovernanceError::PublishRequestMissingAlloyAuthoredStage);
                }
                None
            }
        };

        let author_signature_recorded = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT 1 FROM registry_publication_evidence AS author \
                     WHERE author.request_id = {} AND author.authority = 'author_signature' \
                     AND author.subject_digest_sha256 = {} \
                     AND author.signature_digest_sha256 IS NOT NULL \
                     AND author.created_at >= ( \
                         SELECT request.submitted_at FROM registry_publish_requests AS request \
                         WHERE request.id = author.request_id \
                     ) \
                     LIMIT 1",
                    mark(1),
                    mark(2),
                ),
                vec![
                    command.request_id.clone().into(),
                    checksum_sha256.clone().into(),
                ],
            ))
            .await
            .map_err(store_error)?
            .is_some();
        if !author_signature_recorded {
            return Err(ModuleGovernanceError::PublishRequestMissingAuthorSignature);
        }
        match artifact_origin {
            ModulePublicationArtifactOrigin::PlatformBuilt => {
                let platform_build_manifest = platform_build_manifest
                    .expect("platform-built staging must provide an OCI manifest digest");
                let matched_build_and_platform_evidence = tx
                    .query_one_raw(Statement::from_sql_and_values(
                        backend,
                        format!(
                            "SELECT 1 FROM registry_publication_evidence AS build \
                             WHERE build.request_id = {} \
                               AND build.authority = 'build_service_attestation' \
                               AND build.subject_digest_sha256 = {} \
                               AND build.created_at >= ( \
                                   SELECT request.submitted_at FROM registry_publish_requests AS request \
                                   WHERE request.id = build.request_id \
                               ) \
                               AND EXISTS ( \
                                   SELECT 1 FROM registry_publication_evidence AS platform \
                                   WHERE platform.request_id = build.request_id \
                                     AND platform.authority = 'platform_admission' \
                                     AND platform.subject_digest_sha256 = build.subject_digest_sha256 \
                                     AND platform.created_at >= ( \
                                         SELECT request.submitted_at FROM registry_publish_requests AS request \
                                         WHERE request.id = platform.request_id \
                                     ) \
                               ) \
                             LIMIT 1",
                            mark(1),
                            mark(2),
                        ),
                        vec![
                            command.request_id.clone().into(),
                            platform_build_manifest.into(),
                        ],
                    ))
                    .await
                    .map_err(store_error)?
                    .is_some();
                if !matched_build_and_platform_evidence {
                    return Err(
                        ModuleGovernanceError::PublishRequestMissingBuildOrPlatformAdmission,
                    );
                }
            }
            ModulePublicationArtifactOrigin::ExternalPrebuilt
            | ModulePublicationArtifactOrigin::AlloyAuthored => {
                let platform_admission_recorded = tx
                    .query_one_raw(Statement::from_sql_and_values(
                        backend,
                        format!(
                            "SELECT 1 FROM registry_publication_evidence AS platform \
                             WHERE platform.request_id = {} \
                               AND platform.authority = 'platform_admission' \
                               AND platform.subject_digest_sha256 = {} \
                               AND platform.created_at >= ( \
                                   SELECT request.submitted_at FROM registry_publish_requests AS request \
                                   WHERE request.id = platform.request_id \
                               ) \
                             LIMIT 1",
                            mark(1),
                            mark(2),
                        ),
                        vec![command.request_id.clone().into(), checksum_sha256.clone().into()],
                    ))
                    .await
                    .map_err(store_error)?
                    .is_some();
                if !platform_admission_recorded {
                    return Err(match artifact_origin {
                        ModulePublicationArtifactOrigin::ExternalPrebuilt => {
                            ModuleGovernanceError::PublishRequestMissingExternalPlatformAdmission
                        }
                        ModulePublicationArtifactOrigin::AlloyAuthored => {
                            ModuleGovernanceError::PublishRequestMissingAlloyPlatformAdmission
                        }
                        ModulePublicationArtifactOrigin::PlatformBuilt => unreachable!(
                            "platform-built releases do not use the external admission branch"
                        ),
                    });
                }
            }
        }

        let translations = tx
            .query_all_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT locale, name, description FROM registry_publish_request_translations \
                     WHERE request_id = {} ORDER BY locale",
                    mark(1)
                ),
                vec![command.request_id.clone().into()],
            ))
            .await
            .map_err(|e| ModuleGovernanceError::Store(e.to_string()))?;
        if translations.is_empty() {
            return Err(ModuleGovernanceError::PublishRequestMissingTranslations);
        }
        let translations = translations
            .into_iter()
            .map(|translation| {
                Ok((
                    translation
                        .try_get::<String>("", "locale")
                        .map_err(store_error)?,
                    translation
                        .try_get::<String>("", "name")
                        .map_err(store_error)?,
                    translation
                        .try_get::<String>("", "description")
                        .map_err(store_error)?,
                ))
            })
            .collect::<Result<Vec<_>, ModuleGovernanceError>>()?;
        if !valid_publication_translations(&default_locale, &translations) {
            return Err(ModuleGovernanceError::PublishRequestInvalidTranslations);
        }



    Ok(VerifiedPublicationRequest {
        slug,
        version,
        crate_name,
        default_locale,
        ownership,
        trust_level,
        license,
        entry_type,
        marketplace_json,
        checksum_sha256,
        delivery_media_type,
        delivery_payload_digest,
        delivery_storage_key,
        delivery_size_bytes,
        artifact_origin,
        command_approval_override,
        translations,
    })
}
