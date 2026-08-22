//! Owner-composed authoring entrypoints for remote builds and publication.
//!
//! The CLI supplies validated authoring identity and a deterministic archive.
//! This service revalidates and publishes the archive into the deployment
//! source CAS, constructs immutable owner requests, and queues work. A
//! completed build can also be staged with a bounded metadata bundle for
//! governance review. These services never invoke a worker, Cargo, an OCI
//! registry, or a signing service.

use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use object_store::{ObjectStoreExt, PutMode, path::Path};
use rustok_api::is_valid_module_slug;
use rustok_build_source::{
    ArchiveLimits, CasArchiveError, CasArchivePublishReceipt, CasArchivePublisher,
};
use rustok_storage::{StorageConfig, StorageRuntime};
use sea_orm::DatabaseConnection;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    MODULE_BUILD_COMPONENT_TARGET, MODULE_BUILD_PROTOCOL_VERSION, MODULE_BUILD_RUNTIME_ABI,
    MODULE_BUILD_WIT_VERSION, MODULE_BUILD_WIT_WORLD, MODULE_PUBLISH_BUNDLE_CONTENT_TYPE,
    ModuleBuildAuthoring, ModuleBuildDependencyPolicy, ModuleBuildLimits, ModuleBuildNetworkPolicy,
    ModuleBuildRequest, ModuleBuildSource, ModuleBuildToolchain, ModuleBuildValidationProfile,
    ModuleBuildWitContract, ModuleCommandContext, ModulePublicationArtifactOrigin,
    ModulePublishArtifactAttachCommand, ModulePublishBundleValidation,
    ModulePublishPlatformBuildStageCommand, ModulePublishValidationContract,
    ModuleValidationJobEnqueueCommand, SeaOrmModuleBuildService, SeaOrmModuleGovernanceService,
    validate_module_publish_artifact,
};

pub const MODULE_AUTHORING_BUILD_MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
pub const MODULE_AUTHORING_BUILD_MAX_SOURCE_BYTES: u64 = 512 * 1024 * 1024;
pub const MODULE_AUTHORING_BUILD_MAX_SOURCE_ENTRIES: u32 = 16_384;

const AUTHORING_BUILD_CPU_CORES: u16 = 2;
const AUTHORING_BUILD_MEMORY_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const AUTHORING_BUILD_DISK_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const AUTHORING_BUILD_PROCESS_LIMIT: u16 = 64;
const AUTHORING_BUILD_OUTPUT_BYTES: u64 = 64 * 1024 * 1024;
const AUTHORING_BUILD_WALL_CLOCK_MS: u64 = 30 * 60 * 1_000;
const AUTHORING_PUBLICATION_OWNERSHIP: &str = "third_party";
const AUTHORING_PUBLICATION_TRUST_LEVEL: &str = "sandboxed";

/// Transport-neutral authoring facts accepted before the owner constructs the
/// complete immutable worker request. Resource, network, registry, validation,
/// WIT, ABI, and target policy are owner-selected and cannot be supplied by an
/// authoring client.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleAuthoringBuildCommand {
    pub context: ModuleCommandContext,
    pub project_id: String,
    pub source_digest: String,
    pub expected_module_slug: String,
    pub expected_version: String,
    pub rust_toolchain: String,
    pub sdk_version: String,
    pub template_version: String,
    pub dependency_lock_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleAuthoringBuildSubmission {
    pub request_id: Uuid,
    pub build_created: bool,
    pub source_created: bool,
    pub source_reference: String,
    pub source_digest: String,
    pub archive_bytes: u64,
    pub source_bytes: u64,
    pub entries: u32,
}

#[async_trait]
pub trait ModuleAuthoringBuildControl: Send + Sync {
    async fn submit_build(
        &self,
        command: ModuleAuthoringBuildCommand,
        archive_path: PathBuf,
    ) -> Result<ModuleAuthoringBuildSubmission, ModuleAuthoringBuildError>;
}

#[derive(Clone)]
pub struct SharedModuleAuthoringBuildControl(pub Arc<dyn ModuleAuthoringBuildControl>);

/// Author-supplied marketplace metadata for a completed platform build. The
/// owner fixes origin, ownership, trust classification, runtime kind, UI
/// package policy, actor principal shape, and validation delivery.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleAuthoringPublishCommand {
    pub context: ModuleCommandContext,
    pub build_request_id: Uuid,
    pub slug: String,
    pub version: String,
    pub crate_name: String,
    pub default_locale: String,
    pub name: String,
    pub description: String,
    pub license: String,
    pub marketplace_category: Option<String>,
    pub marketplace_tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleAuthoringPublishSubmission {
    pub request_id: String,
    pub build_request_id: Uuid,
    pub staging_id: String,
    pub stage_created: bool,
    pub validation_job_id: Option<String>,
    pub validation_queued: bool,
    pub bundle_storage_key: String,
    pub bundle_checksum_sha256: String,
    pub bundle_bytes: u64,
}

#[async_trait]
pub trait ModuleAuthoringPublishControl: Send + Sync {
    async fn submit_publish_request(
        &self,
        command: ModuleAuthoringPublishCommand,
        bundle: Vec<u8>,
    ) -> Result<ModuleAuthoringPublishSubmission, ModuleAuthoringPublishError>;
}

#[derive(Clone)]
pub struct SharedModuleAuthoringPublishControl(pub Arc<dyn ModuleAuthoringPublishControl>);

#[derive(Clone)]
pub struct SeaOrmModuleAuthoringPublishService {
    builds: SeaOrmModuleBuildService,
    governance: SeaOrmModuleGovernanceService,
    storage: StorageRuntime,
}

impl SeaOrmModuleAuthoringPublishService {
    pub fn new(db: DatabaseConnection, storage: StorageRuntime) -> Self {
        Self {
            builds: SeaOrmModuleBuildService::new(db.clone()),
            governance: SeaOrmModuleGovernanceService::new(db),
            storage,
        }
    }

    pub async fn from_storage_settings(
        db: DatabaseConnection,
        settings: serde_json::Value,
        local_storage_root: &std::path::Path,
    ) -> Result<Self, ModuleAuthoringPublishError> {
        let mut config: StorageConfig = serde_json::from_value(settings)
            .map_err(|error| ModuleAuthoringPublishError::Storage(error.to_string()))?;
        config.bind_local_base_dir(local_storage_root);
        let storage = StorageRuntime::from_config(&config)
            .await
            .map_err(|error| ModuleAuthoringPublishError::Storage(error.to_string()))?;
        Ok(Self::new(db, storage))
    }

    fn create_command(
        command: &ModuleAuthoringPublishCommand,
    ) -> Result<crate::ModulePublishRequestCreateCommand, ModuleAuthoringPublishError> {
        let request = crate::ModulePublishRequestCreateCommand {
            slug: command.slug.clone(),
            version: command.version.clone(),
            crate_name: command.crate_name.clone(),
            default_locale: command.default_locale.clone(),
            ownership: AUTHORING_PUBLICATION_OWNERSHIP.to_string(),
            trust_level: AUTHORING_PUBLICATION_TRUST_LEVEL.to_string(),
            license: command.license.clone(),
            entry_type: None,
            artifact_origin: ModulePublicationArtifactOrigin::PlatformBuilt,
            marketplace: serde_json::json!({
                "category": command.marketplace_category.clone(),
                "tags": command.marketplace_tags.clone(),
            }),
            ui_packages: serde_json::json!({"admin": null, "storefront": null}),
            name: command.name.clone(),
            description: command.description.clone(),
            actor_principal: author_principal(command.context.actor_id),
            actor_can_manage_modules: false,
        };
        request.validate()?;
        Ok(request)
    }

    fn validation_contract(
        command: &ModuleAuthoringPublishCommand,
    ) -> Result<ModulePublishValidationContract, ModuleAuthoringPublishError> {
        command.validation_contract()
    }

    async fn store_bundle(
        &self,
        storage_key: &str,
        bundle: &[u8],
        checksum_sha256: &str,
    ) -> Result<(), ModuleAuthoringPublishError> {
        let path = Path::from(storage_key);
        let mut options = self.storage.put_options(MODULE_PUBLISH_BUNDLE_CONTENT_TYPE);
        options.mode = PutMode::Create;
        let created = match self
            .storage
            .objects
            .put_opts(&path, bytes::Bytes::copy_from_slice(bundle).into(), options)
            .await
        {
            Ok(_) => true,
            Err(
                object_store::Error::AlreadyExists { .. }
                | object_store::Error::Precondition { .. },
            ) => false,
            Err(error) => return Err(ModuleAuthoringPublishError::Storage(error.to_string())),
        };
        if !created {
            let existing = self
                .storage
                .objects
                .get(&path)
                .await
                .map_err(|error| ModuleAuthoringPublishError::Storage(error.to_string()))?
                .bytes()
                .await
                .map_err(|error| ModuleAuthoringPublishError::Storage(error.to_string()))?;
            if existing.len() != bundle.len()
                || hex::encode(Sha256::digest(existing.as_ref())) != checksum_sha256
            {
                return Err(ModuleAuthoringPublishError::BundleCollision);
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct SeaOrmModuleAuthoringBuildService {
    builds: SeaOrmModuleBuildService,
    publisher: CasArchivePublisher,
    archive_limits: ArchiveLimits,
}

impl SeaOrmModuleAuthoringBuildService {
    pub fn new(
        db: DatabaseConnection,
        source_cas_root: PathBuf,
    ) -> Result<Self, ModuleAuthoringBuildError> {
        let publisher = CasArchivePublisher::new(source_cas_root)?;
        let archive_limits = ArchiveLimits::new(
            MODULE_AUTHORING_BUILD_MAX_ARCHIVE_BYTES,
            MODULE_AUTHORING_BUILD_MAX_SOURCE_BYTES,
            MODULE_AUTHORING_BUILD_MAX_SOURCE_ENTRIES,
        )?;
        Ok(Self {
            builds: SeaOrmModuleBuildService::new(db),
            publisher,
            archive_limits,
        })
    }

    fn immutable_request(
        command: &ModuleAuthoringBuildCommand,
    ) -> Result<ModuleBuildRequest, ModuleAuthoringBuildError> {
        command.validate()?;
        let request = ModuleBuildRequest {
            protocol_version: MODULE_BUILD_PROTOCOL_VERSION,
            request_id: deterministic_request_id(command)?,
            context: command.context.clone(),
            project_id: command.project_id.clone(),
            source: ModuleBuildSource {
                reference: format!("cas://{}", command.source_digest),
                digest: command.source_digest.clone(),
            },
            expected_module_slug: command.expected_module_slug.clone(),
            expected_version: command.expected_version.clone(),
            runtime_abi: MODULE_BUILD_RUNTIME_ABI.to_string(),
            wit: ModuleBuildWitContract {
                world: MODULE_BUILD_WIT_WORLD.to_string(),
                version: MODULE_BUILD_WIT_VERSION.to_string(),
            },
            toolchain: ModuleBuildToolchain {
                rust_toolchain: command.rust_toolchain.clone(),
                component_target: MODULE_BUILD_COMPONENT_TARGET.to_string(),
            },
            authoring: ModuleBuildAuthoring {
                sdk_version: command.sdk_version.clone(),
                template_version: command.template_version.clone(),
            },
            dependency_policy: ModuleBuildDependencyPolicy {
                lock_digest: command.dependency_lock_digest.clone(),
                allowed_registries: vec!["https://crates.io".to_string()],
                allow_git_dependencies: false,
                allow_build_scripts: false,
                allow_native_links: false,
            },
            limits: ModuleBuildLimits {
                cpu_cores: AUTHORING_BUILD_CPU_CORES,
                memory_bytes: AUTHORING_BUILD_MEMORY_BYTES,
                disk_bytes: AUTHORING_BUILD_DISK_BYTES,
                process_limit: AUTHORING_BUILD_PROCESS_LIMIT,
                output_bytes: AUTHORING_BUILD_OUTPUT_BYTES,
                wall_clock_ms: AUTHORING_BUILD_WALL_CLOCK_MS,
            },
            network_policy: ModuleBuildNetworkPolicy::ScopedDependencyMaterialization {
                endpoints: vec![
                    "https://index.crates.io".to_string(),
                    "https://static.crates.io".to_string(),
                ],
            },
            validation_profiles: vec![
                ModuleBuildValidationProfile::Format,
                ModuleBuildValidationProfile::Check,
                ModuleBuildValidationProfile::Lint,
                ModuleBuildValidationProfile::Test,
                ModuleBuildValidationProfile::DependencyPolicy,
                ModuleBuildValidationProfile::Vulnerability,
            ],
            attempt: 1,
        };
        request.validate()?;
        Ok(request)
    }
}

#[async_trait]
impl ModuleAuthoringBuildControl for SeaOrmModuleAuthoringBuildService {
    async fn submit_build(
        &self,
        command: ModuleAuthoringBuildCommand,
        archive_path: PathBuf,
    ) -> Result<ModuleAuthoringBuildSubmission, ModuleAuthoringBuildError> {
        let request = Self::immutable_request(&command)?;
        let publisher = self.publisher.clone();
        let archive_limits = self.archive_limits;
        let expected_digest = command.source_digest.clone();
        let published = tokio::task::spawn_blocking(move || {
            publisher.publish(&archive_path, &expected_digest, archive_limits)
        })
        .await
        .map_err(|error| ModuleAuthoringBuildError::SourceTask(error.to_string()))??;
        let submission = self.builds.submit(request.clone()).await?;
        Ok(submission_from_receipts(
            request,
            submission.created,
            published,
        ))
    }
}

#[async_trait]
impl ModuleAuthoringPublishControl for SeaOrmModuleAuthoringPublishService {
    async fn submit_publish_request(
        &self,
        command: ModuleAuthoringPublishCommand,
        bundle: Vec<u8>,
    ) -> Result<ModuleAuthoringPublishSubmission, ModuleAuthoringPublishError> {
        command.validate()?;
        let tenant_id = command
            .context
            .tenant_id
            .ok_or(ModuleAuthoringPublishError::InvalidCommand)?;
        let idempotency_key = command.context.idempotency_key;
        let completed = self
            .builds
            .load_completed(tenant_id, command.build_request_id)
            .await?;
        if !matches!(
            completed.result.outcome,
            crate::ModuleBuildOutcome::Succeeded
        ) || completed.result.publication.is_none()
            || completed.request.expected_module_slug != command.slug
            || completed.request.expected_version != command.version
        {
            return Err(ModuleAuthoringPublishError::BuildIdentityMismatch);
        }
        let contract = Self::validation_contract(&command)?;
        let validation = validate_module_publish_artifact(
            ModulePublicationArtifactOrigin::PlatformBuilt,
            &contract,
            MODULE_PUBLISH_BUNDLE_CONTENT_TYPE,
            &bundle,
        );
        if !validation.errors.is_empty() {
            return Err(ModuleAuthoringPublishError::InvalidBundle(validation));
        }
        let bundle_checksum_sha256 = hex::encode(Sha256::digest(&bundle));
        let request_id = self
            .governance
            .create_publish_request(Self::create_command(&command)?)
            .await?;
        let actor_principal = author_principal(command.context.actor_id);
        let artifact_command = ModulePublishArtifactAttachCommand {
            request_id: request_id.clone(),
            expected_revision: 1,
            actor_principal: actor_principal.clone(),
            actor_can_manage_modules: false,
            checksum_sha256: bundle_checksum_sha256.clone(),
            artifact_size: i64::try_from(bundle.len())
                .map_err(|_| ModuleAuthoringPublishError::InvalidCommand)?,
            content_type: MODULE_PUBLISH_BUNDLE_CONTENT_TYPE.to_string(),
        };
        let upload_slot = self
            .governance
            .prepare_publish_artifact_upload(&artifact_command)
            .await?;
        if !upload_slot.artifact_already_attached {
            self.store_bundle(
                &upload_slot.artifact_storage_key,
                &bundle,
                &bundle_checksum_sha256,
            )
            .await?;
        }
        let attached = self
            .governance
            .attach_publish_artifact(artifact_command)
            .await?;
        if attached.artifact_storage_key != upload_slot.artifact_storage_key {
            return Err(ModuleAuthoringPublishError::BundleCollision);
        }
        let stage = self
            .governance
            .stage_platform_build(ModulePublishPlatformBuildStageCommand {
                request_id: request_id.clone(),
                expected_revision: 2,
                tenant_id,
                build_request_id: command.build_request_id,
                idempotency_key,
                actor_principal: actor_principal.clone(),
                actor_can_manage_modules: false,
            })
            .await?;
        let validation_job = self
            .governance
            .enqueue_validation_job(ModuleValidationJobEnqueueCommand {
                request_id: request_id.clone(),
                expected_revision: stage.request_revision,
                actor_principal,
                allow_rejected_retry: false,
            })
            .await?;
        Ok(ModuleAuthoringPublishSubmission {
            request_id,
            build_request_id: command.build_request_id,
            staging_id: stage.staging_id,
            stage_created: stage.created,
            validation_job_id: validation_job.validation_job_id,
            validation_queued: validation_job.queued,
            bundle_storage_key: attached.artifact_storage_key,
            bundle_checksum_sha256,
            bundle_bytes: bundle.len() as u64,
        })
    }
}

impl ModuleAuthoringBuildCommand {
    pub fn validate(&self) -> Result<(), ModuleAuthoringBuildError> {
        self.context
            .validate()
            .map_err(|_| ModuleAuthoringBuildError::InvalidCommand)?;
        if !matches!(self.context.tenant_id, Some(tenant_id) if !tenant_id.is_nil())
            || self.project_id.trim().is_empty()
            || self.project_id.len() > 256
            || self.project_id.contains(char::is_control)
            || !is_valid_module_slug(&self.expected_module_slug)
            || Version::parse(&self.expected_version).is_err()
            || Version::parse(&self.rust_toolchain).is_err()
            || Version::parse(&self.sdk_version).is_err()
            || Version::parse(&self.template_version).is_err()
            || !valid_digest(&self.source_digest)
            || !valid_digest(&self.dependency_lock_digest)
        {
            return Err(ModuleAuthoringBuildError::InvalidCommand);
        }
        Ok(())
    }
}

impl ModuleAuthoringPublishCommand {
    pub fn validate(&self) -> Result<(), ModuleAuthoringPublishError> {
        self.context
            .validate()
            .map_err(|_| ModuleAuthoringPublishError::InvalidCommand)?;
        let mut tags = self.marketplace_tags.clone();
        tags.sort();
        tags.dedup();
        if !matches!(self.context.tenant_id, Some(tenant_id) if !tenant_id.is_nil())
            || self.context.idempotency_key.is_nil()
            || self.build_request_id.is_nil()
            || !is_valid_module_slug(&self.slug)
            || Version::parse(&self.version).is_err()
            || self.crate_name.trim().is_empty()
            || self.crate_name.len() > 128
            || self.crate_name.contains(char::is_control)
            || rustok_api::normalize_locale_tag(&self.default_locale).is_none()
            || self.license.trim().is_empty()
            || self.marketplace_tags != tags
        {
            return Err(ModuleAuthoringPublishError::InvalidCommand);
        }
        SeaOrmModuleAuthoringPublishService::create_command(self).map(|_| ())
    }

    pub fn validation_contract(
        &self,
    ) -> Result<ModulePublishValidationContract, ModuleAuthoringPublishError> {
        self.validate()?;
        Ok(ModulePublishValidationContract {
            slug: self.slug.clone(),
            version: self.version.clone(),
            crate_name: self.crate_name.clone(),
            module_name: self.name.clone(),
            module_description: self.description.clone(),
            ownership: AUTHORING_PUBLICATION_OWNERSHIP.to_string(),
            trust_level: AUTHORING_PUBLICATION_TRUST_LEVEL.to_string(),
            license: self.license.clone(),
            entry_type: None,
            marketplace_category: self.marketplace_category.clone(),
            marketplace_tags: self.marketplace_tags.clone(),
            admin_ui_crate_name: None,
            storefront_ui_crate_name: None,
        })
    }
}

fn author_principal(actor_id: Uuid) -> serde_json::Value {
    serde_json::json!({"kind": "user", "id": actor_id})
}

fn deterministic_request_id(
    command: &ModuleAuthoringBuildCommand,
) -> Result<Uuid, ModuleAuthoringBuildError> {
    let encoded = serde_json::to_vec(command)
        .map_err(|error| ModuleAuthoringBuildError::Encoding(error.to_string()))?;
    let digest = Sha256::digest(
        [
            b"rustok.module.authoring-build.request.v1\0".as_slice(),
            encoded.as_slice(),
        ]
        .concat(),
    );
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(Uuid::from_bytes(bytes))
}

fn submission_from_receipts(
    request: ModuleBuildRequest,
    build_created: bool,
    source: CasArchivePublishReceipt,
) -> ModuleAuthoringBuildSubmission {
    ModuleAuthoringBuildSubmission {
        request_id: request.request_id,
        build_created,
        source_created: source.created,
        source_reference: request.source.reference,
        source_digest: source.source_digest,
        archive_bytes: source.archive_bytes,
        source_bytes: source.extracted_bytes,
        entries: source.entries,
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Error)]
pub enum ModuleAuthoringBuildError {
    #[error("module authoring build command is invalid")]
    InvalidCommand,
    #[error("module authoring build command encoding failed: {0}")]
    Encoding(String),
    #[error("module source CAS operation failed: {0}")]
    Source(#[from] CasArchiveError),
    #[error("module source CAS task failed: {0}")]
    SourceTask(String),
    #[error("module build submission failed: {0}")]
    Build(#[from] crate::ModuleBuildProtocolError),
}

#[derive(Debug, Error)]
pub enum ModuleAuthoringPublishError {
    #[error("module authoring publish command is invalid")]
    InvalidCommand,
    #[error("completed module build does not match the publication request")]
    BuildIdentityMismatch,
    #[error("module publication bundle is invalid: {0:?}")]
    InvalidBundle(ModulePublishBundleValidation),
    #[error("module publication bundle storage failed: {0}")]
    Storage(String),
    #[error("module publication bundle CAS contains conflicting bytes")]
    BundleCollision,
    #[error("module publication bundle object key is invalid: {0}")]
    StorageKey(#[from] rustok_storage::KeyError),
    #[error("module build lookup failed: {0}")]
    Build(#[from] crate::ModuleBuildProtocolError),
    #[error("module publication governance failed: {0}")]
    Governance(#[from] crate::ModuleGovernanceError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(marker: char) -> String {
        format!("sha256:{}", marker.to_string().repeat(64))
    }

    fn command() -> ModuleAuthoringBuildCommand {
        ModuleAuthoringBuildCommand {
            context: ModuleCommandContext {
                actor_id: Uuid::new_v4(),
                tenant_id: Some(Uuid::new_v4()),
                trace_id: "trace:authoring-build".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
            project_id: "project:sample".to_string(),
            source_digest: digest('a'),
            expected_module_slug: "sample_module".to_string(),
            expected_version: "1.0.0".to_string(),
            rust_toolchain: "1.85.0".to_string(),
            sdk_version: "1.0.0".to_string(),
            template_version: "1.0.0".to_string(),
            dependency_lock_digest: digest('b'),
        }
    }

    fn publish_command() -> ModuleAuthoringPublishCommand {
        ModuleAuthoringPublishCommand {
            context: ModuleCommandContext {
                actor_id: Uuid::new_v4(),
                tenant_id: Some(Uuid::new_v4()),
                trace_id: "trace:authoring-publish".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
            build_request_id: Uuid::new_v4(),
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            crate_name: "sample-module".to_string(),
            default_locale: "en".to_string(),
            name: "Sample module".to_string(),
            description: "A standalone sample module for publication.".to_string(),
            license: "MIT".to_string(),
            marketplace_category: Some("developer-tools".to_string()),
            marketplace_tags: vec!["example".to_string(), "standalone".to_string()],
        }
    }

    #[test]
    fn owner_constructs_one_deterministic_fail_closed_request() {
        let command = command();
        let first = SeaOrmModuleAuthoringBuildService::immutable_request(&command)
            .expect("immutable request");
        let repeated = SeaOrmModuleAuthoringBuildService::immutable_request(&command)
            .expect("repeated immutable request");

        assert_eq!(first, repeated);
        assert_eq!(first.request_id.get_version_num(), 8);
        assert_eq!(
            first.source.reference,
            format!("cas://{}", command.source_digest)
        );
        assert!(matches!(
            first.network_policy,
            ModuleBuildNetworkPolicy::ScopedDependencyMaterialization { .. }
        ));
        assert_eq!(first.validation_profiles.len(), 6);
        assert!(!first.dependency_policy.allow_git_dependencies);
        assert!(!first.dependency_policy.allow_build_scripts);
        assert!(!first.dependency_policy.allow_native_links);
    }

    #[test]
    fn authoring_command_requires_explicit_scope_and_canonical_digests() {
        assert!(command().validate().is_ok());

        let mut missing_tenant = command();
        missing_tenant.context.tenant_id = None;
        assert!(missing_tenant.validate().is_err());

        let mut uppercase_digest = command();
        uppercase_digest.source_digest = format!("sha256:{}", "A".repeat(64));
        assert!(uppercase_digest.validate().is_err());
    }

    #[test]
    fn publication_owner_fixes_origin_trust_and_native_ui_policy() {
        let command = publish_command();
        let request =
            SeaOrmModuleAuthoringPublishService::create_command(&command).expect("publish request");

        assert_eq!(
            request.artifact_origin,
            ModulePublicationArtifactOrigin::PlatformBuilt
        );
        assert_eq!(request.ownership, AUTHORING_PUBLICATION_OWNERSHIP);
        assert_eq!(request.trust_level, AUTHORING_PUBLICATION_TRUST_LEVEL);
        assert_eq!(request.entry_type, None);
        assert_eq!(
            request.ui_packages,
            serde_json::json!({"admin": null, "storefront": null})
        );
        assert_eq!(
            request.actor_principal,
            serde_json::json!({"kind": "user", "id": command.context.actor_id})
        );
    }

    #[test]
    fn publication_command_rejects_noncanonical_identity_and_taxonomy() {
        assert!(publish_command().validate().is_ok());

        let mut hyphenated_slug = publish_command();
        hyphenated_slug.slug = "sample-module".to_string();
        assert!(hyphenated_slug.validate().is_err());

        let mut unsorted_tags = publish_command();
        unsorted_tags.marketplace_tags.reverse();
        assert!(unsorted_tags.validate().is_err());

        let mut nil_idempotency = publish_command();
        nil_idempotency.context.idempotency_key = Uuid::nil();
        assert!(nil_idempotency.validate().is_err());
    }
}
