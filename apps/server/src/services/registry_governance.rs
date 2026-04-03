use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use anyhow::{anyhow, Context};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::models::registry_module_release::{
    self, ActiveModel as RegistryModuleReleaseActiveModel, Entity as RegistryModuleReleaseEntity,
    RegistryModuleReleaseStatus,
};
use crate::models::registry_publish_request::{
    self, ActiveModel as RegistryPublishRequestActiveModel, Entity as RegistryPublishRequestEntity,
    RegistryPublishRequestStatus,
};
use crate::modules::{CatalogManifestModule, CatalogModuleVersion};
use crate::services::marketplace_catalog::{
    RegistryPublishMarketplaceRequest, RegistryPublishRequest, RegistryPublishUiPackagesRequest,
    REGISTRY_MUTATION_SCHEMA_VERSION,
};

const REGISTRY_ARTIFACT_BUNDLE_TYPE: &str = "rustok-module-publish-bundle";
const REGISTRY_VALIDATOR_ACTOR: &str = "system:registry-validator";

#[derive(Debug, Clone)]
pub struct RegistryArtifactUpload {
    pub content_type: String,
    pub bytes: bytes::Bytes,
}

#[derive(Debug, Clone)]
pub struct RegistryGovernanceService {
    db: DatabaseConnection,
}

#[derive(Debug, Clone)]
pub struct RegistryPublishRequestSnapshot {
    pub id: String,
    pub status: String,
    pub requested_by: String,
    pub approved_by: Option<String>,
    pub rejected_by: Option<String>,
    pub rejection_reason: Option<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RegistryModuleReleaseSnapshot {
    pub version: String,
    pub status: String,
    pub publisher: String,
    pub checksum_sha256: Option<String>,
    pub published_at: String,
    pub yanked_reason: Option<String>,
    pub yanked_by: Option<String>,
    pub yanked_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RegistryModuleLifecycleSnapshot {
    pub latest_request: Option<RegistryPublishRequestSnapshot>,
    pub latest_release: Option<RegistryModuleReleaseSnapshot>,
}

#[derive(Debug, Default)]
struct RegistryArtifactValidation {
    warnings: Vec<String>,
    errors: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RegistryPublishArtifactBundle {
    schema_version: u32,
    artifact_type: String,
    module: RegistryPublishArtifactModule,
    files: RegistryPublishArtifactFiles,
}

#[derive(Debug, Deserialize)]
struct RegistryPublishArtifactModule {
    slug: String,
    version: String,
    crate_name: String,
    module_name: String,
    module_description: String,
    ownership: String,
    trust_level: String,
    license: String,
    module_entry_type: Option<String>,
    marketplace: RegistryPublishArtifactMarketplace,
    ui_packages: RegistryPublishArtifactUiPackages,
}

#[derive(Debug, Default, Deserialize)]
struct RegistryPublishArtifactMarketplace {
    category: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RegistryPublishArtifactUiPackages {
    admin: Option<RegistryPublishArtifactUiPackage>,
    storefront: Option<RegistryPublishArtifactUiPackage>,
}

#[derive(Debug, Deserialize)]
struct RegistryPublishArtifactUiPackage {
    crate_name: String,
    #[allow(dead_code)]
    manifest_path: String,
}

#[derive(Debug, Deserialize)]
struct RegistryPublishArtifactFiles {
    #[serde(rename = "rustok-module.toml")]
    package_manifest: Option<String>,
    #[serde(rename = "Cargo.toml")]
    crate_manifest: Option<String>,
    #[serde(rename = "admin/Cargo.toml")]
    admin_manifest: Option<String>,
    #[serde(rename = "storefront/Cargo.toml")]
    storefront_manifest: Option<String>,
}

impl RegistryGovernanceService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn create_publish_request(
        &self,
        request: &RegistryPublishRequest,
        requested_by: &str,
        warnings: &[String],
    ) -> anyhow::Result<registry_publish_request::Model> {
        let existing_active_release = RegistryModuleReleaseEntity::find()
            .filter(registry_module_release::Column::Slug.eq(&request.module.slug))
            .filter(registry_module_release::Column::Version.eq(&request.module.version))
            .filter(registry_module_release::Column::Status.eq(RegistryModuleReleaseStatus::Active))
            .one(&self.db)
            .await?;
        if existing_active_release.is_some() {
            anyhow::bail!(
                "Published release '{}@{}' already exists",
                request.module.slug,
                request.module.version
            );
        }

        let now = Utc::now();
        let request_id = format!("rpr_{}", uuid::Uuid::new_v4().simple());
        let model = registry_publish_request::Model {
            id: request_id.clone(),
            slug: request.module.slug.clone(),
            version: request.module.version.clone(),
            crate_name: request.module.crate_name.clone(),
            module_name: request.module.name.clone(),
            description: request.module.description.clone(),
            ownership: request.module.ownership.clone(),
            trust_level: request.module.trust_level.clone(),
            license: request.module.license.clone(),
            entry_type: request.module.entry_type.clone(),
            marketplace: serde_json::to_value(&request.module.marketplace)
                .context("failed to serialize registry publish marketplace metadata")?,
            ui_packages: serde_json::to_value(&request.module.ui_packages)
                .context("failed to serialize registry publish ui_packages metadata")?,
            status: RegistryPublishRequestStatus::Draft,
            requested_by: normalize_actor(requested_by),
            approved_by: None,
            rejected_by: None,
            rejection_reason: None,
            validation_warnings: serde_json::to_value(warnings)
                .context("failed to serialize registry publish warnings")?,
            validation_errors: serde_json::json!([]),
            artifact_path: None,
            artifact_url: None,
            artifact_checksum_sha256: None,
            artifact_size: None,
            artifact_content_type: None,
            submitted_at: None,
            validated_at: None,
            approved_at: None,
            published_at: None,
            created_at: now,
            updated_at: now,
        };

        let active_model = RegistryPublishRequestActiveModel {
            id: Set(model.id.clone()),
            slug: Set(model.slug.clone()),
            version: Set(model.version.clone()),
            crate_name: Set(model.crate_name.clone()),
            module_name: Set(model.module_name.clone()),
            description: Set(model.description.clone()),
            ownership: Set(model.ownership.clone()),
            trust_level: Set(model.trust_level.clone()),
            license: Set(model.license.clone()),
            entry_type: Set(model.entry_type.clone()),
            marketplace: Set(model.marketplace.clone()),
            ui_packages: Set(model.ui_packages.clone()),
            status: Set(model.status.clone()),
            requested_by: Set(model.requested_by.clone()),
            approved_by: Set(None),
            rejected_by: Set(None),
            rejection_reason: Set(None),
            validation_warnings: Set(model.validation_warnings.clone()),
            validation_errors: Set(model.validation_errors.clone()),
            artifact_path: Set(None),
            artifact_url: Set(None),
            artifact_checksum_sha256: Set(None),
            artifact_size: Set(None),
            artifact_content_type: Set(None),
            submitted_at: Set(None),
            validated_at: Set(None),
            approved_at: Set(None),
            published_at: Set(None),
            created_at: Set(model.created_at),
            updated_at: Set(model.updated_at),
        };

        active_model.insert(&self.db).await?;
        self.get_publish_request(&request_id)
            .await?
            .ok_or_else(|| anyhow!("registry publish request disappeared after insert"))
    }

    pub async fn get_publish_request(
        &self,
        request_id: &str,
    ) -> anyhow::Result<Option<registry_publish_request::Model>> {
        Ok(RegistryPublishRequestEntity::find_by_id(request_id)
            .one(&self.db)
            .await?)
    }

    pub async fn upload_and_publish_request(
        &self,
        request_id: &str,
        actor: &str,
        artifact: RegistryArtifactUpload,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let request = self
            .get_publish_request(request_id)
            .await?
            .ok_or_else(|| anyhow!("Registry publish request '{request_id}' was not found"))?;
        ensure_actor_can_manage_publish_request(actor, &request, "upload an artifact for")?;
        if request.status != RegistryPublishRequestStatus::Draft {
            anyhow::bail!(
                "Registry publish request '{}' is in status '{}' and can no longer accept an artifact upload",
                request_id,
                request_status_label(request.status.clone())
            );
        }

        let checksum = hex::encode(Sha256::digest(&artifact.bytes));
        let stored = store_registry_artifact(&request, &artifact)
            .await
            .context("failed to persist registry artifact")?;
        let artifact_uploaded_at = Utc::now();

        let mut request_active: RegistryPublishRequestActiveModel = request.clone().into();
        request_active.status = Set(RegistryPublishRequestStatus::ArtifactUploaded);
        request_active.artifact_path = Set(Some(stored.artifact_path.clone()));
        request_active.artifact_url = Set(Some(stored.artifact_url.clone()));
        request_active.artifact_checksum_sha256 = Set(Some(checksum.clone()));
        request_active.artifact_size = Set(Some(stored.artifact_size));
        request_active.artifact_content_type = Set(Some(artifact.content_type.clone()));
        request_active.updated_at = Set(artifact_uploaded_at);
        let request = request_active.update(&self.db).await?;

        let submitted_at = Utc::now();
        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(RegistryPublishRequestStatus::Submitted);
        request_active.submitted_at = Set(Some(submitted_at));
        request_active.updated_at = Set(submitted_at);
        let request = request_active.update(&self.db).await?;

        let validating_at = Utc::now();
        let mut request_active: RegistryPublishRequestActiveModel = request.clone().into();
        request_active.status = Set(RegistryPublishRequestStatus::Validating);
        request_active.updated_at = Set(validating_at);
        let request = request_active.update(&self.db).await?;

        let validation = validate_registry_artifact_bundle(&request, &artifact);
        let mut warnings = deserialize_message_list(&request.validation_warnings);
        warnings.extend(validation.warnings);
        let upload_actor = normalize_actor(actor);
        if upload_actor != request.requested_by {
            warnings.push(format!(
                "Artifact was uploaded by '{}' for publish request originally created by '{}'.",
                upload_actor, request.requested_by
            ));
        }
        let warnings = dedupe_message_list(warnings);

        if !validation.errors.is_empty() {
            let rejected_at = Utc::now();
            let errors = dedupe_message_list(validation.errors);
            let mut request_active: RegistryPublishRequestActiveModel = request.into();
            request_active.status = Set(RegistryPublishRequestStatus::Rejected);
            request_active.validation_warnings = Set(serde_json::to_value(&warnings)?);
            request_active.validation_errors = Set(serde_json::to_value(&errors)?);
            request_active.rejected_by = Set(Some(REGISTRY_VALIDATOR_ACTOR.to_string()));
            request_active.rejection_reason = Set(errors.first().cloned());
            request_active.validated_at = Set(Some(rejected_at));
            request_active.approved_by = Set(None);
            request_active.approved_at = Set(None);
            request_active.published_at = Set(None);
            request_active.updated_at = Set(rejected_at);
            return Ok(request_active.update(&self.db).await?);
        }

        let approved_at = Utc::now();
        let mut request_active: RegistryPublishRequestActiveModel = request.clone().into();
        request_active.status = Set(RegistryPublishRequestStatus::Approved);
        request_active.validation_warnings = Set(serde_json::to_value(&warnings)?);
        request_active.validation_errors = Set(serde_json::json!([]));
        request_active.rejected_by = Set(None);
        request_active.rejection_reason = Set(None);
        request_active.validated_at = Set(Some(approved_at));
        request_active.approved_by = Set(Some(REGISTRY_VALIDATOR_ACTOR.to_string()));
        request_active.approved_at = Set(Some(approved_at));
        request_active.updated_at = Set(approved_at);
        request_active.update(&self.db).await?;

        self.get_publish_request(request_id).await?.ok_or_else(|| {
            anyhow!("registry publish request '{request_id}' disappeared after validation")
        })
    }

    pub async fn approve_publish_request(
        &self,
        request_id: &str,
        actor: &str,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let request = self
            .get_publish_request(request_id)
            .await?
            .ok_or_else(|| anyhow!("Registry publish request '{request_id}' was not found"))?;
        ensure_actor_can_manage_publish_request(actor, &request, "approve")?;
        if request.status != RegistryPublishRequestStatus::Approved {
            anyhow::bail!(
                "Registry publish request '{}' is in status '{}' and cannot be approved",
                request_id,
                request_status_label(request.status.clone())
            );
        }

        let stored = StoredRegistryArtifact {
            artifact_path: request.artifact_path.clone().ok_or_else(|| {
                anyhow!("Registry publish request '{request_id}' is missing artifact_path")
            })?,
            artifact_url: request.artifact_url.clone().ok_or_else(|| {
                anyhow!("Registry publish request '{request_id}' is missing artifact_url")
            })?,
            artifact_size: request.artifact_size.ok_or_else(|| {
                anyhow!("Registry publish request '{request_id}' is missing artifact_size")
            })?,
        };
        let checksum = request.artifact_checksum_sha256.clone().ok_or_else(|| {
            anyhow!("Registry publish request '{request_id}' is missing artifact_checksum_sha256")
        })?;
        let published_at = Utc::now();
        self.upsert_release_from_request(
            request_id,
            actor,
            checksum,
            stored,
            published_at,
            &request,
        )
        .await?;

        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(RegistryPublishRequestStatus::Published);
        request_active.approved_by = Set(Some(normalize_actor(actor)));
        request_active.approved_at = Set(Some(published_at));
        request_active.published_at = Set(Some(published_at));
        request_active.updated_at = Set(published_at);
        Ok(request_active.update(&self.db).await?)
    }

    pub async fn reject_publish_request(
        &self,
        request_id: &str,
        actor: &str,
        reason: &str,
    ) -> anyhow::Result<registry_publish_request::Model> {
        let request = self
            .get_publish_request(request_id)
            .await?
            .ok_or_else(|| anyhow!("Registry publish request '{request_id}' was not found"))?;
        ensure_actor_can_manage_publish_request(actor, &request, "reject")?;
        if matches!(
            request.status,
            RegistryPublishRequestStatus::Published | RegistryPublishRequestStatus::Rejected
        ) {
            anyhow::bail!(
                "Registry publish request '{}' is in status '{}' and cannot be rejected",
                request_id,
                request_status_label(request.status.clone())
            );
        }

        let rejected_at = Utc::now();
        let mut errors = deserialize_message_list(&request.validation_errors);
        if !reason.trim().is_empty() {
            errors.push(format!("Governance rejection reason: {}", reason.trim()));
        }
        let mut request_active: RegistryPublishRequestActiveModel = request.into();
        request_active.status = Set(RegistryPublishRequestStatus::Rejected);
        request_active.rejected_by = Set(Some(normalize_actor(actor)));
        request_active.rejection_reason = Set(Some(reason.trim().to_string()));
        request_active.validation_errors = Set(serde_json::to_value(dedupe_message_list(errors))?);
        request_active.updated_at = Set(rejected_at);
        Ok(request_active.update(&self.db).await?)
    }

    pub async fn yank_release(
        &self,
        slug: &str,
        version: &str,
        reason: &str,
        actor: &str,
    ) -> anyhow::Result<registry_module_release::Model> {
        let release = RegistryModuleReleaseEntity::find()
            .filter(registry_module_release::Column::Slug.eq(slug))
            .filter(registry_module_release::Column::Version.eq(version))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Published release '{slug}@{version}' was not found"))?;
        ensure_actor_can_manage_release(actor, &release, "yank")?;

        let mut active: RegistryModuleReleaseActiveModel = release.into();
        active.status = Set(RegistryModuleReleaseStatus::Yanked);
        active.yanked_reason = Set(Some(reason.trim().to_string()));
        active.yanked_by = Set(Some(normalize_actor(actor)));
        active.yanked_at = Set(Some(Utc::now()));
        active.updated_at = Set(Utc::now());
        Ok(active.update(&self.db).await?)
    }

    pub async fn apply_catalog_projection(
        &self,
        modules: Vec<CatalogManifestModule>,
    ) -> anyhow::Result<Vec<CatalogManifestModule>> {
        let releases = RegistryModuleReleaseEntity::find()
            .order_by_desc(registry_module_release::Column::PublishedAt)
            .all(&self.db)
            .await?;

        if releases.is_empty() {
            return Ok(modules);
        }

        let mut release_map: HashMap<String, Vec<registry_module_release::Model>> = HashMap::new();
        for release in releases {
            release_map
                .entry(release.slug.clone())
                .or_default()
                .push(release);
        }

        let mut projected = modules;
        for module in &mut projected {
            let Some(releases) = release_map.get(&module.slug) else {
                continue;
            };

            let mut versions = releases
                .iter()
                .map(|release| CatalogModuleVersion {
                    version: release.version.clone(),
                    changelog: None,
                    yanked: release.status == RegistryModuleReleaseStatus::Yanked,
                    published_at: Some(release.published_at.to_rfc3339()),
                    checksum_sha256: release.checksum_sha256.clone(),
                    signature: None,
                })
                .collect::<Vec<_>>();
            versions.sort_by(|left, right| {
                left.yanked
                    .cmp(&right.yanked)
                    .then_with(|| right.published_at.cmp(&left.published_at))
                    .then_with(|| compare_semver_desc(&left.version, &right.version))
                    .then_with(|| right.version.cmp(&left.version))
            });

            if let Some(latest_active) = releases
                .iter()
                .find(|release| release.status == RegistryModuleReleaseStatus::Active)
            {
                module.version = Some(latest_active.version.clone());
                module.publisher = Some(latest_active.publisher.clone());
                module.checksum_sha256 = latest_active.checksum_sha256.clone();
            }
            module.versions = versions;
        }

        Ok(projected)
    }

    pub async fn lifecycle_snapshot(
        &self,
        slug: &str,
    ) -> anyhow::Result<Option<RegistryModuleLifecycleSnapshot>> {
        let latest_request = RegistryPublishRequestEntity::find()
            .filter(registry_publish_request::Column::Slug.eq(slug))
            .order_by_desc(registry_publish_request::Column::CreatedAt)
            .one(&self.db)
            .await?;
        let latest_release = RegistryModuleReleaseEntity::find()
            .filter(registry_module_release::Column::Slug.eq(slug))
            .order_by_desc(registry_module_release::Column::PublishedAt)
            .one(&self.db)
            .await?;

        if latest_request.is_none() && latest_release.is_none() {
            return Ok(None);
        }

        Ok(Some(RegistryModuleLifecycleSnapshot {
            latest_request: latest_request.map(|request| RegistryPublishRequestSnapshot {
                id: request.id,
                status: request_status_label(request.status).to_string(),
                requested_by: request.requested_by,
                approved_by: request.approved_by,
                rejected_by: request.rejected_by,
                rejection_reason: request.rejection_reason,
                warnings: deserialize_message_list(&request.validation_warnings),
                errors: deserialize_message_list(&request.validation_errors),
                created_at: request.created_at.to_rfc3339(),
                updated_at: request.updated_at.to_rfc3339(),
                published_at: request.published_at.map(|value| value.to_rfc3339()),
            }),
            latest_release: latest_release.map(|release| RegistryModuleReleaseSnapshot {
                version: release.version,
                status: release_status_label(release.status).to_string(),
                publisher: release.publisher,
                checksum_sha256: release.checksum_sha256,
                published_at: release.published_at.to_rfc3339(),
                yanked_reason: release.yanked_reason,
                yanked_by: release.yanked_by,
                yanked_at: release.yanked_at.map(|value| value.to_rfc3339()),
            }),
        }))
    }

    async fn upsert_release_from_request(
        &self,
        request_id: &str,
        actor: &str,
        checksum: String,
        stored: StoredRegistryArtifact,
        published_at: chrono::DateTime<Utc>,
        request: &registry_publish_request::Model,
    ) -> anyhow::Result<registry_module_release::Model> {
        let existing = RegistryModuleReleaseEntity::find()
            .filter(registry_module_release::Column::Slug.eq(&request.slug))
            .filter(registry_module_release::Column::Version.eq(&request.version))
            .one(&self.db)
            .await?;

        let marketplace = request.marketplace.clone();
        let ui_packages = request.ui_packages.clone();
        let publisher = normalize_actor(actor);

        if let Some(existing) = existing {
            let mut active: RegistryModuleReleaseActiveModel = existing.into();
            active.request_id = Set(Some(request_id.to_string()));
            active.crate_name = Set(request.crate_name.clone());
            active.module_name = Set(request.module_name.clone());
            active.description = Set(request.description.clone());
            active.ownership = Set(request.ownership.clone());
            active.trust_level = Set(request.trust_level.clone());
            active.license = Set(request.license.clone());
            active.entry_type = Set(request.entry_type.clone());
            active.marketplace = Set(marketplace);
            active.ui_packages = Set(ui_packages);
            active.status = Set(RegistryModuleReleaseStatus::Active);
            active.publisher = Set(publisher);
            active.artifact_path = Set(Some(stored.artifact_path));
            active.artifact_url = Set(Some(stored.artifact_url));
            active.checksum_sha256 = Set(Some(checksum));
            active.artifact_size = Set(Some(stored.artifact_size));
            active.yanked_reason = Set(None);
            active.yanked_by = Set(None);
            active.yanked_at = Set(None);
            active.published_at = Set(published_at);
            active.updated_at = Set(Utc::now());
            return Ok(active.update(&self.db).await?);
        }

        let id = format!("rrel_{}", uuid::Uuid::new_v4().simple());
        let active = RegistryModuleReleaseActiveModel {
            id: Set(id),
            request_id: Set(Some(request_id.to_string())),
            slug: Set(request.slug.clone()),
            version: Set(request.version.clone()),
            crate_name: Set(request.crate_name.clone()),
            module_name: Set(request.module_name.clone()),
            description: Set(request.description.clone()),
            ownership: Set(request.ownership.clone()),
            trust_level: Set(request.trust_level.clone()),
            license: Set(request.license.clone()),
            entry_type: Set(request.entry_type.clone()),
            marketplace: Set(marketplace),
            ui_packages: Set(ui_packages),
            status: Set(RegistryModuleReleaseStatus::Active),
            publisher: Set(publisher),
            artifact_path: Set(Some(stored.artifact_path)),
            artifact_url: Set(Some(stored.artifact_url)),
            checksum_sha256: Set(Some(checksum)),
            artifact_size: Set(Some(stored.artifact_size)),
            yanked_reason: Set(None),
            yanked_by: Set(None),
            yanked_at: Set(None),
            published_at: Set(published_at),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        };
        Ok(active.insert(&self.db).await?)
    }
}

#[derive(Debug, Clone)]
struct StoredRegistryArtifact {
    artifact_path: String,
    artifact_url: String,
    artifact_size: i64,
}

async fn store_registry_artifact(
    request: &registry_publish_request::Model,
    artifact: &RegistryArtifactUpload,
) -> anyhow::Result<StoredRegistryArtifact> {
    let artifact_dir = workspace_root()
        .join("storage")
        .join("registry-artifacts")
        .join(&request.id);
    tokio::fs::create_dir_all(&artifact_dir)
        .await
        .with_context(|| {
            format!(
                "failed to create registry artifact dir {}",
                artifact_dir.display()
            )
        })?;

    let artifact_path = artifact_dir.join(format!("{}-{}.crate", request.slug, request.version));
    tokio::fs::write(&artifact_path, &artifact.bytes)
        .await
        .with_context(|| {
            format!(
                "failed to write registry artifact {}",
                artifact_path.display()
            )
        })?;

    Ok(StoredRegistryArtifact {
        artifact_path: artifact_path.display().to_string(),
        artifact_url: artifact_path.display().to_string(),
        artifact_size: artifact.bytes.len() as i64,
    })
}

fn normalize_actor(value: &str) -> String {
    let actor = value.trim();
    if actor.is_empty() {
        "system:auto".to_string()
    } else {
        actor.to_string()
    }
}

fn dedupe_message_list(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for value in values {
        let value = value.trim().to_string();
        if value.is_empty() {
            continue;
        }
        if seen.insert(value.clone()) {
            deduped.push(value);
        }
    }
    deduped
}

fn deserialize_message_list(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(ToString::to_string))
        .collect()
}

fn compare_semver_desc(left: &str, right: &str) -> std::cmp::Ordering {
    match (semver::Version::parse(left), semver::Version::parse(right)) {
        (Ok(left), Ok(right)) => right.cmp(&left),
        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        (Err(_), Err(_)) => std::cmp::Ordering::Equal,
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .expect("workspace root should be resolvable from apps/server")
}

pub fn request_status_label(status: RegistryPublishRequestStatus) -> &'static str {
    match status {
        RegistryPublishRequestStatus::Draft => "draft",
        RegistryPublishRequestStatus::ArtifactUploaded => "artifact_uploaded",
        RegistryPublishRequestStatus::Submitted => "submitted",
        RegistryPublishRequestStatus::Validating => "validating",
        RegistryPublishRequestStatus::Approved => "approved",
        RegistryPublishRequestStatus::Rejected => "rejected",
        RegistryPublishRequestStatus::Published => "published",
    }
}

pub fn release_status_label(status: RegistryModuleReleaseStatus) -> &'static str {
    match status {
        RegistryModuleReleaseStatus::Active => "active",
        RegistryModuleReleaseStatus::Yanked => "yanked",
    }
}

pub fn request_ui_packages(
    request: &registry_publish_request::Model,
) -> RegistryPublishUiPackagesRequest {
    serde_json::from_value(request.ui_packages.clone()).unwrap_or_default()
}

pub fn actor_can_manage_registry_slug(actor: &str, slug: &str) -> bool {
    let actor = normalize_actor(actor);
    actor_is_registry_governance(&actor)
        || actor == "publisher:*"
        || actor == format!("publisher:{slug}")
}

fn validate_registry_artifact_bundle(
    request: &registry_publish_request::Model,
    artifact: &RegistryArtifactUpload,
) -> RegistryArtifactValidation {
    let mut validation = RegistryArtifactValidation::default();

    if !artifact
        .content_type
        .eq_ignore_ascii_case("application/json")
    {
        validation.warnings.push(format!(
            "Artifact upload content-type '{}' is accepted, but application/json is the canonical bundle content-type.",
            artifact.content_type
        ));
    }

    let bundle = match serde_json::from_slice::<RegistryPublishArtifactBundle>(&artifact.bytes) {
        Ok(bundle) => bundle,
        Err(error) => {
            validation.errors.push(format!(
                "Artifact bundle is not valid JSON for the registry publish contract: {error}"
            ));
            return validation;
        }
    };

    if bundle.schema_version != REGISTRY_MUTATION_SCHEMA_VERSION {
        validation.errors.push(format!(
            "Artifact bundle schema_version '{}' does not match registry mutation schema '{}'.",
            bundle.schema_version, REGISTRY_MUTATION_SCHEMA_VERSION
        ));
    }
    if bundle.artifact_type != REGISTRY_ARTIFACT_BUNDLE_TYPE {
        validation.errors.push(format!(
            "Artifact bundle type '{}' does not match expected '{}'.",
            bundle.artifact_type, REGISTRY_ARTIFACT_BUNDLE_TYPE
        ));
    }

    validate_artifact_module_contract(request, &bundle, &mut validation);
    validate_artifact_file_contract(request, &bundle, &mut validation);

    validation.warnings = dedupe_message_list(validation.warnings);
    validation.errors = dedupe_message_list(validation.errors);
    validation
}

fn validate_artifact_module_contract(
    request: &registry_publish_request::Model,
    bundle: &RegistryPublishArtifactBundle,
    validation: &mut RegistryArtifactValidation,
) {
    let request_marketplace: RegistryPublishMarketplaceRequest =
        serde_json::from_value(request.marketplace.clone()).unwrap_or_default();
    let request_ui = request_ui_packages(request);

    validate_exact_field(
        "module.slug",
        &bundle.module.slug,
        &request.slug,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.version",
        &bundle.module.version,
        &request.version,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.crate_name",
        &bundle.module.crate_name,
        &request.crate_name,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.name",
        &bundle.module.module_name,
        &request.module_name,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.description",
        &bundle.module.module_description,
        &request.description,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.ownership",
        &bundle.module.ownership,
        &request.ownership,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.trust_level",
        &bundle.module.trust_level,
        &request.trust_level,
        &mut validation.errors,
    );
    validate_exact_field(
        "module.license",
        &bundle.module.license,
        &request.license,
        &mut validation.errors,
    );
    validate_optional_field(
        "module.entry_type",
        bundle.module.module_entry_type.as_deref(),
        request.entry_type.as_deref(),
        &mut validation.errors,
    );
    validate_optional_field(
        "module.marketplace.category",
        bundle.module.marketplace.category.as_deref(),
        request_marketplace.category.as_deref(),
        &mut validation.errors,
    );

    if normalize_string_list(&bundle.module.marketplace.tags)
        != normalize_string_list(&request_marketplace.tags)
    {
        validation.errors.push(format!(
            "Artifact bundle module.marketplace.tags {:?} does not match publish request {:?}.",
            bundle.module.marketplace.tags, request_marketplace.tags
        ));
    }

    validate_optional_field(
        "module.ui_packages.admin.crate_name",
        bundle
            .module
            .ui_packages
            .admin
            .as_ref()
            .map(|ui| ui.crate_name.as_str()),
        request_ui.admin.as_ref().map(|ui| ui.crate_name.as_str()),
        &mut validation.errors,
    );
    validate_optional_field(
        "module.ui_packages.storefront.crate_name",
        bundle
            .module
            .ui_packages
            .storefront
            .as_ref()
            .map(|ui| ui.crate_name.as_str()),
        request_ui
            .storefront
            .as_ref()
            .map(|ui| ui.crate_name.as_str()),
        &mut validation.errors,
    );
}

fn validate_artifact_file_contract(
    request: &registry_publish_request::Model,
    bundle: &RegistryPublishArtifactBundle,
    validation: &mut RegistryArtifactValidation,
) {
    let request_marketplace: RegistryPublishMarketplaceRequest =
        serde_json::from_value(request.marketplace.clone()).unwrap_or_default();
    let request_ui = request_ui_packages(request);

    let package_manifest = require_bundle_file(
        "rustok-module.toml",
        bundle.files.package_manifest.as_deref(),
        &mut validation.errors,
    );
    let crate_manifest = require_bundle_file(
        "Cargo.toml",
        bundle.files.crate_manifest.as_deref(),
        &mut validation.errors,
    );

    match (&request_ui.admin, bundle.files.admin_manifest.as_deref()) {
        (Some(_), None) => validation.errors.push(
            "Artifact bundle must include admin/Cargo.toml because the publish request declares an admin UI package."
                .to_string(),
        ),
        (None, Some(_)) => validation.errors.push(
            "Artifact bundle includes admin/Cargo.toml, but the publish request does not declare an admin UI package."
                .to_string(),
        ),
        _ => {}
    }
    match (&request_ui.storefront, bundle.files.storefront_manifest.as_deref()) {
        (Some(_), None) => validation.errors.push(
            "Artifact bundle must include storefront/Cargo.toml because the publish request declares a storefront UI package."
                .to_string(),
        ),
        (None, Some(_)) => validation.errors.push(
            "Artifact bundle includes storefront/Cargo.toml, but the publish request does not declare a storefront UI package."
                .to_string(),
        ),
        _ => {}
    }

    if let Some(source) = package_manifest {
        validate_package_manifest_contract(
            source,
            request,
            &request_marketplace,
            &request_ui,
            validation,
        );
    }
    if let Some(source) = crate_manifest {
        validate_cargo_manifest_contract(
            "Cargo.toml",
            source,
            &request.crate_name,
            &request.version,
            Some(&request.license),
            validation,
        );
    }
    if let (Some(ui), Some(source)) = (&request_ui.admin, bundle.files.admin_manifest.as_deref()) {
        validate_cargo_manifest_contract(
            "admin/Cargo.toml",
            source,
            &ui.crate_name,
            &request.version,
            None,
            validation,
        );
    }
    if let (Some(ui), Some(source)) = (
        &request_ui.storefront,
        bundle.files.storefront_manifest.as_deref(),
    ) {
        validate_cargo_manifest_contract(
            "storefront/Cargo.toml",
            source,
            &ui.crate_name,
            &request.version,
            None,
            validation,
        );
    }
}

fn validate_package_manifest_contract(
    source: &str,
    request: &registry_publish_request::Model,
    request_marketplace: &RegistryPublishMarketplaceRequest,
    request_ui: &RegistryPublishUiPackagesRequest,
    validation: &mut RegistryArtifactValidation,
) {
    let manifest = match source.parse::<toml::Value>() {
        Ok(manifest) => manifest,
        Err(error) => {
            validation.errors.push(format!(
                "Artifact file rustok-module.toml is not valid TOML: {error}"
            ));
            return;
        }
    };

    validate_toml_string_field(
        &manifest,
        &["module", "slug"],
        "rustok-module.toml [module].slug",
        &request.slug,
        &mut validation.errors,
    );
    validate_toml_string_field(
        &manifest,
        &["module", "name"],
        "rustok-module.toml [module].name",
        &request.module_name,
        &mut validation.errors,
    );
    validate_toml_string_field(
        &manifest,
        &["module", "version"],
        "rustok-module.toml [module].version",
        &request.version,
        &mut validation.errors,
    );
    validate_toml_string_field(
        &manifest,
        &["module", "description"],
        "rustok-module.toml [module].description",
        &request.description,
        &mut validation.errors,
    );
    validate_toml_string_field(
        &manifest,
        &["module", "ownership"],
        "rustok-module.toml [module].ownership",
        &request.ownership,
        &mut validation.errors,
    );
    validate_toml_string_field(
        &manifest,
        &["module", "trust_level"],
        "rustok-module.toml [module].trust_level",
        &request.trust_level,
        &mut validation.errors,
    );
    validate_toml_optional_string_field(
        &manifest,
        &["marketplace", "category"],
        "rustok-module.toml [marketplace].category",
        request_marketplace.category.as_deref(),
        &mut validation.errors,
    );
    validate_toml_optional_string_field(
        &manifest,
        &["crate", "entry_type"],
        "rustok-module.toml [crate].entry_type",
        request.entry_type.as_deref(),
        &mut validation.errors,
    );

    if toml_string_list_field(&manifest, &["marketplace", "tags"])
        != normalize_string_list(&request_marketplace.tags)
    {
        validation.errors.push(format!(
            "Artifact file rustok-module.toml [marketplace].tags {:?} does not match publish request {:?}.",
            toml_string_list_field(&manifest, &["marketplace", "tags"]),
            request_marketplace.tags
        ));
    }

    validate_toml_optional_string_field(
        &manifest,
        &["provides", "admin_ui", "leptos_crate"],
        "rustok-module.toml [provides.admin_ui].leptos_crate",
        request_ui.admin.as_ref().map(|ui| ui.crate_name.as_str()),
        &mut validation.errors,
    );
    validate_toml_optional_string_field(
        &manifest,
        &["provides", "storefront_ui", "leptos_crate"],
        "rustok-module.toml [provides.storefront_ui].leptos_crate",
        request_ui
            .storefront
            .as_ref()
            .map(|ui| ui.crate_name.as_str()),
        &mut validation.errors,
    );
}

fn validate_cargo_manifest_contract(
    label: &str,
    source: &str,
    expected_name: &str,
    expected_version: &str,
    expected_license: Option<&str>,
    validation: &mut RegistryArtifactValidation,
) {
    let manifest = match source.parse::<toml::Value>() {
        Ok(manifest) => manifest,
        Err(error) => {
            validation
                .errors
                .push(format!("Artifact file {label} is not valid TOML: {error}"));
            return;
        }
    };

    validate_toml_string_field(
        &manifest,
        &["package", "name"],
        &format!("{label} [package].name"),
        expected_name,
        &mut validation.errors,
    );
    validate_toml_workspace_aware_string_field(
        &manifest,
        &["package", "version"],
        &format!("{label} [package].version"),
        expected_version,
        &mut validation.warnings,
        &mut validation.errors,
    );
    if let Some(expected_license) = expected_license {
        validate_toml_workspace_aware_string_field(
            &manifest,
            &["package", "license"],
            &format!("{label} [package].license"),
            expected_license,
            &mut validation.warnings,
            &mut validation.errors,
        );
    }
}

fn validate_exact_field(label: &str, actual: &str, expected: &str, errors: &mut Vec<String>) {
    if actual.trim() != expected.trim() {
        errors.push(format!(
            "Artifact bundle {label} '{}' does not match publish request '{}'.",
            actual, expected
        ));
    }
}

fn validate_optional_field(
    label: &str,
    actual: Option<&str>,
    expected: Option<&str>,
    errors: &mut Vec<String>,
) {
    let actual = actual.map(str::trim).filter(|value| !value.is_empty());
    let expected = expected.map(str::trim).filter(|value| !value.is_empty());
    if actual != expected {
        errors.push(format!(
            "Artifact bundle {label} {:?} does not match publish request {:?}.",
            actual, expected
        ));
    }
}

fn require_bundle_file<'a>(
    label: &str,
    source: Option<&'a str>,
    errors: &mut Vec<String>,
) -> Option<&'a str> {
    match source.map(str::trim) {
        Some(source) if !source.is_empty() => Some(source),
        _ => {
            errors.push(format!(
                "Artifact bundle must include non-empty file '{label}'."
            ));
            None
        }
    }
}

fn normalize_string_list(values: &[String]) -> Vec<String> {
    let mut values = values
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn toml_value_at_path<'a>(value: &'a toml::Value, path: &[&str]) -> Option<&'a toml::Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn toml_string_field(value: &toml::Value, path: &[&str]) -> Option<String> {
    toml_value_at_path(value, path)
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn toml_string_list_field(value: &toml::Value, path: &[&str]) -> Vec<String> {
    toml_value_at_path(value, path)
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::trim).map(ToString::to_string))
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .map(|mut values| {
            values.sort();
            values.dedup();
            values
        })
        .unwrap_or_default()
}

fn toml_is_workspace_inherited(value: &toml::Value, path: &[&str]) -> bool {
    toml_value_at_path(value, path)
        .and_then(toml::Value::as_table)
        .and_then(|table| table.get("workspace"))
        .and_then(toml::Value::as_bool)
        == Some(true)
}

fn validate_toml_string_field(
    manifest: &toml::Value,
    path: &[&str],
    label: &str,
    expected: &str,
    errors: &mut Vec<String>,
) {
    let actual = toml_string_field(manifest, path);
    if actual.as_deref() != Some(expected.trim()) {
        errors.push(format!(
            "Artifact file {label} {:?} does not match publish request '{}'.",
            actual, expected
        ));
    }
}

fn validate_toml_optional_string_field(
    manifest: &toml::Value,
    path: &[&str],
    label: &str,
    expected: Option<&str>,
    errors: &mut Vec<String>,
) {
    let actual = toml_string_field(manifest, path);
    let expected = expected
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if actual != expected {
        errors.push(format!(
            "Artifact file {label} {:?} does not match publish request {:?}.",
            actual, expected
        ));
    }
}

fn validate_toml_workspace_aware_string_field(
    manifest: &toml::Value,
    path: &[&str],
    label: &str,
    expected: &str,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    if let Some(actual) = toml_string_field(manifest, path) {
        if actual != expected.trim() {
            errors.push(format!(
                "Artifact file {label} '{}' does not match publish request '{}'.",
                actual, expected
            ));
        }
        return;
    }

    if toml_is_workspace_inherited(manifest, path) {
        warnings.push(format!(
            "Artifact file {label} uses workspace inheritance, so the registry validator cannot verify it from the uploaded bundle alone."
        ));
        return;
    }

    warnings.push(format!(
        "Artifact file {label} is missing, so the registry validator could not verify it from the uploaded bundle."
    ));
}

fn ensure_actor_can_manage_publish_request(
    actor: &str,
    request: &registry_publish_request::Model,
    action: &str,
) -> anyhow::Result<()> {
    let actor = normalize_actor(actor);
    if actor_is_registry_governance(&actor)
        || actor == request.requested_by
        || actor_can_manage_registry_slug(&actor, &request.slug)
    {
        return Ok(());
    }

    anyhow::bail!(
        "Actor '{}' is not allowed to {} registry publish request '{}' for slug '{}'",
        actor,
        action,
        request.id,
        request.slug
    )
}

fn ensure_actor_can_manage_release(
    actor: &str,
    release: &registry_module_release::Model,
    action: &str,
) -> anyhow::Result<()> {
    let actor = normalize_actor(actor);
    if actor_is_registry_governance(&actor)
        || actor == release.publisher
        || actor_can_manage_registry_slug(&actor, &release.slug)
    {
        return Ok(());
    }

    anyhow::bail!(
        "Actor '{}' is not allowed to {} published release '{}@{}'",
        actor,
        action,
        release.slug,
        release.version
    )
}

fn actor_is_registry_governance(actor: &str) -> bool {
    actor.starts_with("system:")
        || actor.starts_with("xtask:")
        || actor == "registry:admin"
        || actor.starts_with("registry:")
        || actor == "governance:moderator"
        || actor.starts_with("moderator:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_bundle_validation_accepts_matching_bundle() {
        let request = sample_publish_request_model();
        let artifact = RegistryArtifactUpload {
            content_type: "application/json".to_string(),
            bytes: bytes::Bytes::from(sample_publish_artifact_json("blog", true).into_bytes()),
        };

        let validation = validate_registry_artifact_bundle(&request, &artifact);

        assert!(validation.errors.is_empty(), "{:?}", validation.errors);
    }

    #[test]
    fn artifact_bundle_validation_rejects_mismatched_slug() {
        let request = sample_publish_request_model();
        let artifact = RegistryArtifactUpload {
            content_type: "application/json".to_string(),
            bytes: bytes::Bytes::from(sample_publish_artifact_json("forum", true).into_bytes()),
        };

        let validation = validate_registry_artifact_bundle(&request, &artifact);

        assert!(
            validation
                .errors
                .iter()
                .any(|error| error.contains("module.slug")),
            "{:?}",
            validation.errors
        );
    }

    #[test]
    fn artifact_bundle_validation_rejects_missing_requested_admin_manifest() {
        let request = sample_publish_request_model();
        let artifact = RegistryArtifactUpload {
            content_type: "application/json".to_string(),
            bytes: bytes::Bytes::from(sample_publish_artifact_json("blog", false).into_bytes()),
        };

        let validation = validate_registry_artifact_bundle(&request, &artifact);

        assert!(
            validation
                .errors
                .iter()
                .any(|error| error.contains("admin/Cargo.toml")),
            "{:?}",
            validation.errors
        );
    }

    fn sample_publish_request_model() -> registry_publish_request::Model {
        registry_publish_request::Model {
            id: "rpr_test".to_string(),
            slug: "blog".to_string(),
            version: "0.1.0".to_string(),
            crate_name: "rustok-blog".to_string(),
            module_name: "Blog".to_string(),
            description: "Blog module description long enough for validation.".to_string(),
            ownership: "first_party".to_string(),
            trust_level: "core".to_string(),
            license: "MIT".to_string(),
            entry_type: Some("backend".to_string()),
            marketplace: serde_json::json!({
                "category": "content",
                "tags": ["blog", "content"]
            }),
            ui_packages: serde_json::json!({
                "admin": { "crate_name": "rustok-blog-admin" },
                "storefront": { "crate_name": "rustok-blog-storefront" }
            }),
            status: RegistryPublishRequestStatus::Draft,
            requested_by: "xtask:module-publish".to_string(),
            approved_by: None,
            rejected_by: None,
            rejection_reason: None,
            validation_warnings: serde_json::json!([]),
            validation_errors: serde_json::json!([]),
            artifact_path: None,
            artifact_url: None,
            artifact_checksum_sha256: None,
            artifact_size: None,
            artifact_content_type: None,
            submitted_at: None,
            validated_at: None,
            approved_at: None,
            published_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn sample_publish_artifact_json(slug: &str, include_admin_manifest: bool) -> String {
        let package_manifest = r#"
[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
description = "Blog module description long enough for validation."
ownership = "first_party"
trust_level = "core"

[marketplace]
category = "content"
tags = ["blog", "content"]

[crate]
entry_type = "backend"

[provides.admin_ui]
leptos_crate = "rustok-blog-admin"

[provides.storefront_ui]
leptos_crate = "rustok-blog-storefront"
"#;
        let crate_manifest = r#"
[package]
name = "rustok-blog"
version = "0.1.0"
license = "MIT"
"#;
        let admin_manifest = include_admin_manifest.then_some(
            r#"
[package]
name = "rustok-blog-admin"
version = "0.1.0"
"#,
        );
        let storefront_manifest = Some(
            r#"
[package]
name = "rustok-blog-storefront"
version = "0.1.0"
"#,
        );

        serde_json::json!({
            "schema_version": REGISTRY_MUTATION_SCHEMA_VERSION,
            "artifact_type": REGISTRY_ARTIFACT_BUNDLE_TYPE,
            "module": {
                "slug": slug,
                "version": "0.1.0",
                "crate_name": "rustok-blog",
                "module_name": "Blog",
                "module_description": "Blog module description long enough for validation.",
                "ownership": "first_party",
                "trust_level": "core",
                "license": "MIT",
                "module_entry_type": "backend",
                "marketplace": {
                    "category": "content",
                    "tags": ["blog", "content"]
                },
                "ui_packages": {
                    "admin": {
                        "crate_name": "rustok-blog-admin",
                        "manifest_path": "crates/rustok-blog/admin/Cargo.toml"
                    },
                    "storefront": {
                        "crate_name": "rustok-blog-storefront",
                        "manifest_path": "crates/rustok-blog/storefront/Cargo.toml"
                    }
                }
            },
            "files": {
                "rustok-module.toml": package_manifest,
                "Cargo.toml": crate_manifest,
                "admin/Cargo.toml": admin_manifest,
                "storefront/Cargo.toml": storefront_manifest
            }
        })
        .to_string()
    }
}
