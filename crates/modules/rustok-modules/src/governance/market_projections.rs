//! Published artifact contracts, Rhai workspaces, and marketplace metadata projections.

use sea_orm::{ConnectionTrait, Statement};
use semver::Version;

use super::helpers::*;
use super::projections::*;
use super::*;

impl SeaOrmModuleGovernanceService {
    pub async fn published_artifact_contracts(
        &self,
    ) -> Result<Vec<ModulePublishedArtifactContract>, ModuleGovernanceError> {
        let backend = self.db.get_database_backend();
        let rows = self
            .db
            .query_all_raw(Statement::from_string(
                backend,
                "SELECT artifact.release_id, CAST(artifact.artifact AS TEXT) AS artifact, \
                 CAST(artifact.descriptor AS TEXT) AS descriptor, CAST(artifact.lineage AS TEXT) AS lineage, \
                 admission.media_type AS payload_media_type, release.published_at \
                 FROM registry_module_release_artifacts AS artifact \
                 INNER JOIN registry_module_releases AS release \
                   ON release.id = artifact.release_id \
                 INNER JOIN registry_publish_platform_admissions AS admission \
                   ON admission.request_id = artifact.request_id \
                 WHERE release.status = 'active' \
                 ORDER BY release.slug, release.version, artifact.release_id"
                    .to_string(),
            ))
            .await
            .map_err(store_error)?;
        rows.into_iter()
            .map(|row| {
                let artifact = serde_json::from_str::<crate::ModuleMarketplaceArtifactRelease>(
                    &row.try_get::<String>("", "artifact").map_err(store_error)?,
                )
                .map_err(store_error)?;
                let descriptor = serde_json::from_str::<crate::ModuleArtifactDescriptor>(
                    &row.try_get::<String>("", "descriptor")
                        .map_err(store_error)?,
                )
                .map_err(store_error)?;
                let lineage = serde_json::from_str::<crate::ArtifactSourceLineage>(
                    &row.try_get::<String>("", "lineage").map_err(store_error)?,
                )
                .map_err(store_error)?;
                artifact.validate().map_err(|_| {
                    ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract
                })?;
                if crate::canonical_artifact_descriptor_digest(&descriptor)
                    != artifact.descriptor_digest
                    || descriptor.artifact_digest != artifact.payload_digest
                    || lineage.origin != crate::ArtifactOrigin::Marketplace
                    || lineage.source_digest != artifact.source_digest
                    || lineage
                        .parent_release
                        .as_ref()
                        .is_some_and(|parent| parent.validate().is_err())
                {
                    return Err(
                        ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract,
                    );
                }
                Ok(ModulePublishedArtifactContract {
                    release_id: row
                        .try_get::<String>("", "release_id")
                        .map_err(store_error)?,
                    artifact,
                    descriptor,
                    lineage,
                    payload_media_type: row
                        .try_get::<String>("", "payload_media_type")
                        .map_err(store_error)?,
                    published_at: row
                        .try_get::<chrono::DateTime<chrono::Utc>>("", "published_at")
                        .map_err(store_error)?,
                })
            })
            .collect()
    }

    /// Materializes one exact published Rhai workspace for continued Alloy
    /// development. The active release projection is the sole metadata
    /// authority and the admitted artifact CAS is the sole byte authority;
    /// neither catalog DTOs nor mutable OCI tags can substitute for either.
    pub async fn published_rhai_workspace(
        &self,
        release: &crate::ArtifactReleaseRef,
        blobs: &(dyn crate::ArtifactBlobStore + Send + Sync),
    ) -> Result<ModulePublishedRhaiWorkspace, ModuleGovernanceError> {
        release
            .validate()
            .map_err(|_| ModuleGovernanceError::ReleaseNotFound)?;
        let contract = self
            .published_artifact_contracts()
            .await?
            .into_iter()
            .find(|contract| {
                contract.descriptor.release_ref() == *release
                    && contract.artifact.payload_digest == release.digest
            })
            .ok_or(ModuleGovernanceError::ReleaseNotFound)?;
        if contract.descriptor.payload_kind != crate::ArtifactPayloadKind::Rhai
            || contract.artifact.runtime_kind != crate::ModuleMarketplaceRuntimeKind::Rhai
            || contract.artifact.source_digest != release.digest
            || contract.payload_media_type != rustok_sandbox::RHAI_WORKSPACE_MEDIA_TYPE
        {
            return Err(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract);
        }

        let bytes = blobs
            .get_verified(&release.digest)
            .await
            .map_err(|error| ModuleGovernanceError::Store(error.to_string()))?;
        if bytes.len() > crate::MODULE_PUBLISH_ALLOY_WORKSPACE_MAX_BYTES {
            return Err(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract);
        }
        let workspace = serde_json::from_slice::<rustok_sandbox::RhaiWorkspace>(&bytes)
            .map_err(|_| ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract)?;
        let canonical_bytes = workspace
            .canonical_bytes()
            .map_err(|_| ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract)?;
        if canonical_bytes != bytes {
            return Err(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract);
        }
        let source_digest = workspace
            .digest()
            .map_err(|_| ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract)?;
        if source_digest != release.digest || source_digest != contract.artifact.source_digest {
            return Err(ModuleGovernanceError::PublishRequestMissingCanonicalArtifactContract);
        }

        Ok(ModulePublishedRhaiWorkspace {
            release: crate::ArtifactRelease {
                descriptor: contract.descriptor,
                lineage: contract.lineage,
                published_at: contract.published_at,
            },
            workspace,
        })
    }

    pub(crate) async fn marketplace_release_projections(
        &self,
    ) -> Result<Vec<RegistryMarketplaceReleaseProjection>, ModuleGovernanceError> {
        let backend = self.db.get_database_backend();
        let rows = self
            .db
            .query_all_raw(Statement::from_string(
                backend,
                "SELECT id, slug, version, status, \
                        CAST(publisher_principal AS TEXT) AS publisher_principal, \
                        checksum_sha256, default_locale, published_at \
                 FROM registry_module_releases \
                 ORDER BY published_at DESC, id DESC"
                    .to_string(),
            ))
            .await
            .map_err(store_error)?;
        rows.into_iter()
            .map(|row| {
                let raw_principal = required_column::<String>(&row, "publisher_principal")?;
                let publisher_principal = serde_json::from_str(&raw_principal).map_err(|_| {
                    ModuleGovernanceError::Store(
                        "registry release publisher principal is not valid JSON".to_string(),
                    )
                })?;
                Ok(RegistryMarketplaceReleaseProjection {
                    id: required_column(&row, "id")?,
                    slug: required_column(&row, "slug")?,
                    version: required_column(&row, "version")?,
                    status: required_column(&row, "status")?,
                    publisher_principal,
                    checksum_sha256: optional_column(&row, "checksum_sha256")?,
                    default_locale: required_column(&row, "default_locale")?,
                    published_at: required_timestamp(&row, "published_at")?,
                })
            })
            .collect()
    }

    pub(crate) async fn marketplace_release_metadata(
        &self,
        release_id: &str,
        preferred_locale: Option<&str>,
        fallback_locale: Option<&str>,
    ) -> Result<RegistryMarketplaceTranslationProjection, ModuleGovernanceError> {
        let backend = self.db.get_database_backend();
        let mark = |position| placeholder(backend, position);
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT locale, name, description \
                     FROM registry_module_release_translations \
                     WHERE release_id = {} ORDER BY locale ASC",
                    mark(1)
                ),
                vec![release_id.to_string().into()],
            ))
            .await
            .map_err(store_error)?;
        let translations = rows
            .into_iter()
            .map(|row| {
                Ok(RegistryMarketplaceTranslationProjection {
                    locale: required_column(&row, "locale")?,
                    name: required_column(&row, "name")?,
                    description: required_column(&row, "description")?,
                })
            })
            .collect::<Result<Vec<_>, ModuleGovernanceError>>()?;
        marketplace_translation_for_locales(&translations, preferred_locale, fallback_locale)
            .ok_or_else(|| {
                ModuleGovernanceError::Store(
                    "active registry release is missing metadata translations".to_string(),
                )
            })
    }
}

pub(crate) fn marketplace_translation_for_locales(
    translations: &[RegistryMarketplaceTranslationProjection],
    preferred_locale: Option<&str>,
    fallback_locale: Option<&str>,
) -> Option<RegistryMarketplaceTranslationProjection> {
    let candidates = rustok_api::build_locale_candidates(
        [
            preferred_locale,
            fallback_locale,
            Some(rustok_api::PLATFORM_FALLBACK_LOCALE),
        ],
        true,
    );
    for candidate in candidates {
        if let Some(translation) = translations
            .iter()
            .find(|translation| rustok_api::locale_tags_match(&translation.locale, &candidate))
        {
            return Some(RegistryMarketplaceTranslationProjection {
                locale: translation.locale.clone(),
                name: translation.name.clone(),
                description: translation.description.clone(),
            });
        }
    }
    translations
        .first()
        .map(|translation| RegistryMarketplaceTranslationProjection {
            locale: translation.locale.clone(),
            name: translation.name.clone(),
            description: translation.description.clone(),
        })
}

pub(crate) fn marketplace_principal_label(
    principal: &serde_json::Value,
) -> Result<String, ModuleGovernanceError> {
    principal
        .get("display_label")
        .or_else(|| principal.get("subject"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| principal.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            ModuleGovernanceError::Store(
                "registry release publisher principal has no stable display label".to_string(),
            )
        })
}

pub(crate) fn marketplace_version_desc(left: &str, right: &str) -> std::cmp::Ordering {
    match (Version::parse(left), Version::parse(right)) {
        (Ok(left), Ok(right)) => right.cmp(&left),
        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        (Err(_), Err(_)) => std::cmp::Ordering::Equal,
    }
}
