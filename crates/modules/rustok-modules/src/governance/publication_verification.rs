//! Publication prerequisites verification.

use sea_orm::{ConnectionTrait, DatabaseTransaction, Statement};

use super::helpers::*;
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
    pub entry_type: Option<String>,
    pub marketplace: serde_json::Value,
    pub ui_packages: serde_json::Value,
    pub artifact_storage_key: String,
    pub checksum_sha256: String,
    pub artifact_size: i64,
    pub artifact_origin: ModulePublicationArtifactOrigin,
    pub translations: Vec<(String, String, String)>,
}

pub(crate) async fn verify_publish_request_prerequisites(
    tx: &DatabaseTransaction,
    backend: SupportedBackend,
    command: &ModulePublishRequestPublicationCommand,
) -> Result<VerifiedPublicationRequest, ModuleGovernanceError> {
    let mark = |n| backend.placeholder(n);
    let request_lock = if backend == SupportedBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };

    let request = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
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
                backend.db_backend(),
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
                        backend.db_backend(),
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
                        backend.db_backend(),
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
                        backend.db_backend(),
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
            backend.db_backend(),
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
    // Marketplace approval was checked for independence from the requester
    // and the then-stored publisher when it was recorded, but publication
    // binds the release to the host-supplied `command.publisher_principal`.
    // Re-check independence against that final publisher here: an approval
    // recorded by the principal that will own the release is not a valid
    // independent approval, even if the stored publisher differed earlier.
    let marketplace_approval_recorders = tx
        .query_all_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT CAST(approval.recorded_by_principal AS TEXT) AS recorded_by_principal \
                     FROM registry_publication_evidence AS approval \
                     WHERE approval.request_id = {} AND approval.authority = 'marketplace_approval' \
                     AND approval.subject_digest_sha256 = {} \
                     AND approval.policy_revision = {} \
                     AND approval.created_at >= ( \
                         SELECT request.submitted_at FROM registry_publish_requests AS request \
                         WHERE request.id = approval.request_id \
                     )",
                mark(1),
                mark(2),
                mark(3),
            ),
            vec![
                command.request_id.clone().into(),
                checksum_sha256.clone().into(),
                MARKETPLACE_APPROVAL_POLICY_REVISION.into(),
            ],
        ))
        .await
        .map_err(store_error)?
        .iter()
        .map(|row| required_json_text(row, "recorded_by_principal"))
        .collect::<Result<Vec<_>, _>>()?;
    if !marketplace_approval_recorders
        .iter()
        .any(|recorded_by| recorded_by != &command.publisher_principal)
    {
        return Err(ModuleGovernanceError::PublishRequestMissingMarketplaceApproval);
    }
    match artifact_origin {
        ModulePublicationArtifactOrigin::PlatformBuilt => {
            let platform_build_manifest = platform_build_manifest
                .expect("platform-built staging must provide an OCI manifest digest");
            let matched_build_and_platform_evidence = tx
                    .query_one_raw(Statement::from_sql_and_values(
                        backend.db_backend(),
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
                return Err(ModuleGovernanceError::PublishRequestMissingBuildOrPlatformAdmission);
            }
        }
        ModulePublicationArtifactOrigin::ExternalPrebuilt
        | ModulePublicationArtifactOrigin::AlloyAuthored => {
            let platform_admission_recorded = tx
                    .query_one_raw(Statement::from_sql_and_values(
                        backend.db_backend(),
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

    verify_approval_override_gate(tx, backend, command, artifact_origin).await?;

    let translations = tx
        .query_all_raw(Statement::from_sql_and_values(
            backend.db_backend(),
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
        marketplace,
        ui_packages,
        artifact_storage_key,
        checksum_sha256,
        artifact_size,
        artifact_origin,
        translations,
    })
}

async fn verify_approval_override_gate(
    tx: &DatabaseTransaction,
    backend: SupportedBackend,
    command: &ModulePublishRequestPublicationCommand,
    artifact_origin: ModulePublicationArtifactOrigin,
) -> Result<(), ModuleGovernanceError> {
    let mark = |n| backend.placeholder(n);
    let rows = tx
        .query_all_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT stage_key, status, attempt_number \
                     FROM registry_validation_stages WHERE request_id = {} \
                     ORDER BY attempt_number DESC, created_at DESC, id DESC",
                mark(1),
            ),
            vec![command.request_id.clone().into()],
        ))
        .await
        .map_err(store_error)?;
    let mut latest_by_key = std::collections::HashMap::<String, (String, i64)>::new();
    for row in rows {
        let stage_key: String = row.try_get("", "stage_key").map_err(store_error)?;
        let status: String = row.try_get("", "status").map_err(store_error)?;
        let attempt_number: i32 = row.try_get("", "attempt_number").map_err(store_error)?;
        latest_by_key
            .entry(stage_key.to_ascii_lowercase())
            .or_insert((status.to_ascii_lowercase(), i64::from(attempt_number)));
    }

    let mut incomplete = Vec::new();
    for required_stage in publication_follow_up_stages(artifact_origin) {
        let (status, attempt_number) = latest_by_key
            .get(required_stage.key)
            .cloned()
            .unwrap_or_else(|| ("queued".to_string(), 0));
        if status != "passed" {
            incomplete.push((required_stage.key.to_string(), status, attempt_number));
        }
    }

    if incomplete.is_empty() {
        if command.approval_override.is_some() {
            return Err(ModuleGovernanceError::InvalidPublishApprovalOverride);
        }
        return Ok(());
    }
    let Some(approval_override) = &command.approval_override else {
        return Err(ModuleGovernanceError::PublishRequestApprovalOverrideRequired);
    };
    let Some(override_stages) = approval_override.validation_stages.as_array() else {
        return Err(ModuleGovernanceError::InvalidPublishApprovalOverride);
    };
    let mut recorded = Vec::with_capacity(override_stages.len());
    for stage in override_stages {
        let Some(stage_key) = stage
            .get("stage_key")
            .and_then(serde_json::Value::as_str)
            .map(str::to_ascii_lowercase)
        else {
            return Err(ModuleGovernanceError::PublishApprovalOverrideStageMismatch);
        };
        let Some(status) = stage
            .get("status")
            .and_then(serde_json::Value::as_str)
            .map(str::to_ascii_lowercase)
        else {
            return Err(ModuleGovernanceError::PublishApprovalOverrideStageMismatch);
        };
        let Some(attempt_number) = stage
            .get("attempt_number")
            .and_then(serde_json::Value::as_i64)
        else {
            return Err(ModuleGovernanceError::PublishApprovalOverrideStageMismatch);
        };
        recorded.push((stage_key, status, attempt_number));
    }
    incomplete.sort();
    recorded.sort();
    if recorded != incomplete {
        return Err(ModuleGovernanceError::PublishApprovalOverrideStageMismatch);
    }
    Ok(())
}
