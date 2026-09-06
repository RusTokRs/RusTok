//! Rhai authoring pipeline and immutable release packaging.
//!
//! Transforms a reviewed Alloy script revision into a deterministic canonical bounded-workspace
//! source object, create-only source-CAS receipt, and finalized immutable artifact descriptor
//! ready for production dispatch and OCI publication.

use std::{collections::BTreeMap, sync::Arc};

use chrono::{DateTime, Utc};
use rustok_api::is_valid_module_slug;
use rustok_sandbox::{
    CapabilityName, RHAI_SANDBOX_RUNTIME_ABI, RHAI_SOURCE_MEDIA_TYPE, RHAI_WORKSPACE_MEDIA_TYPE,
    RhaiWorkspace, RhaiWorkspaceError,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ArtifactBlobStore, ArtifactModuleKind, ArtifactPayloadKind, ArtifactPermissionDescriptor,
    ArtifactPersistenceContract, ArtifactSchemaDocument, ModuleArtifactDescriptor,
    ModuleArtifactError, ModuleRuntimeBinding,
    data::{placeholder, revision_value, uuid_value},
};

#[derive(Debug, Error)]
pub enum RhaiAuthoringError {
    #[error("Database error: {0}")]
    Storage(String),
    #[error("Invalid module slug: `{0}`")]
    InvalidSlug(String),
    #[error("Invalid semver version: `{0}`")]
    InvalidVersion(String),
    #[error("Rhai workspace validation failed: {0}")]
    Workspace(#[from] RhaiWorkspaceError),
    #[error("Module artifact descriptor error: {0}")]
    Descriptor(#[from] ModuleArtifactError),
    #[error("Workspace entrypoint `{0}` does not exist in workspace files")]
    MissingWorkspaceEntrypoint(String),
    #[error("Binding `{binding_id}` entrypoint `{entrypoint}` does not exist in workspace files")]
    MissingBindingEntrypoint {
        binding_id: String,
        entrypoint: String,
    },
    #[error("Binding `{binding_id}` references undeclared permission `{permission}`")]
    UndeclaredBindingPermission {
        binding_id: String,
        permission: String,
    },
    #[error("Schema digest `{0}` does not match any bundled schema documents")]
    SchemaDigestNotFound(String),
    #[error("CAS blob storage error: {0}")]
    BlobStore(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Idempotency conflict: package already exists for script `{alloy_script_id}` revision `{alloy_revision}` with differing content")]
    IdempotencyConflict {
        alloy_script_id: Uuid,
        alloy_revision: u32,
    },
}

/// Commands the authoring service to package a reviewed Alloy revision into an immutable release.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RhaiAuthoringPackageCommand {
    pub tenant_id: Uuid,
    pub actor_id: Uuid,
    pub slug: String,
    pub version: String,
    pub alloy_script_id: Uuid,
    pub alloy_revision: u32,
    pub review_decision_id: Uuid,
    pub review_digest: String,
    pub workspace: RhaiWorkspace,
    pub bindings: Vec<ModuleRuntimeBinding>,
    pub permissions: Vec<ArtifactPermissionDescriptor>,
    pub schema_documents: Vec<ArtifactSchemaDocument>,
    pub settings_schema_digest: Option<String>,
    pub data_schema_digest: Option<String>,
    pub persistence_contract: Option<ArtifactPersistenceContract>,
    pub capabilities: Vec<CapabilityName>,
    pub trace_id: String,
    pub idempotency_key: Uuid,
}

/// Receipt confirming publication of the canonical workspace into create-only source-CAS.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RhaiSourceCasReceipt {
    pub source_digest: String,
    pub size_bytes: u64,
    pub media_type: String,
    pub created: bool,
    pub published_at: DateTime<Utc>,
}

/// Canonical OCI layer descriptor describing the source payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RhaiOciPayload {
    pub media_type: String,
    pub digest: String,
    pub size_bytes: u64,
    pub annotations: BTreeMap<String, String>,
}

/// Finalized publishable release bundle combining descriptor, CAS receipt, and OCI payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RhaiAuthoringPublishableRelease {
    pub package_id: Uuid,
    pub descriptor: ModuleArtifactDescriptor,
    pub descriptor_digest: String,
    pub source_cas_receipt: RhaiSourceCasReceipt,
    pub oci_payload: RhaiOciPayload,
    pub created_at: DateTime<Utc>,
}

pub struct RhaiAuthoringService {
    db: DatabaseConnection,
    blob_store: Arc<dyn ArtifactBlobStore>,
}

impl RhaiAuthoringService {
    pub fn new(db: DatabaseConnection, blob_store: Arc<dyn ArtifactBlobStore>) -> Self {
        Self { db, blob_store }
    }

    /// Validates inputs, canonicalizes the workspace, stores into source-CAS,
    /// verifies idempotency, and records the immutable published release.
    pub async fn package_release(
        &self,
        command: RhaiAuthoringPackageCommand,
    ) -> Result<RhaiAuthoringPublishableRelease, RhaiAuthoringError> {
        // 1. Validate module slug and version
        if !is_valid_module_slug(&command.slug) {
            return Err(RhaiAuthoringError::InvalidSlug(command.slug));
        }
        Version::parse(&command.version)
            .map_err(|_| RhaiAuthoringError::InvalidVersion(command.version.clone()))?;

        // 2. Validate workspace
        command.workspace.validate()?;

        // Verify entrypoint exists in workspace files
        if !command
            .workspace
            .files
            .iter()
            .any(|f| f.path == command.workspace.entrypoint)
        {
            return Err(RhaiAuthoringError::MissingWorkspaceEntrypoint(
                command.workspace.entrypoint.clone(),
            ));
        }

        // 3. Validate bindings against workspace files and permissions
        for binding in &command.bindings {
            if !command
                .workspace
                .files
                .iter()
                .any(|f| f.path == binding.entrypoint)
            {
                return Err(RhaiAuthoringError::MissingBindingEntrypoint {
                    binding_id: binding.id.clone(),
                    entrypoint: binding.entrypoint.clone(),
                });
            }
            if !command
                .permissions
                .iter()
                .any(|p| p.key == binding.permission)
            {
                return Err(RhaiAuthoringError::UndeclaredBindingPermission {
                    binding_id: binding.id.clone(),
                    permission: binding.permission.clone(),
                });
            }
        }

        // 4. Validate schema references
        if let Some(ref digest) = command.settings_schema_digest {
            if !command.schema_documents.iter().any(|s| &s.digest == digest) {
                return Err(RhaiAuthoringError::SchemaDigestNotFound(digest.clone()));
            }
        }
        if let Some(ref digest) = command.data_schema_digest {
            if !command.schema_documents.iter().any(|s| &s.digest == digest) {
                return Err(RhaiAuthoringError::SchemaDigestNotFound(digest.clone()));
            }
        }

        // 5. Compute deterministic canonical bounded-workspace source object
        let canonical_bytes = command.workspace.canonical_bytes()?;
        let source_digest = command.workspace.digest()?;

        // 6. Build finalized immutable ModuleArtifactDescriptor
        let descriptor = ModuleArtifactDescriptor {
            schema_version: crate::MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
            slug: command.slug.clone(),
            version: command.version.clone(),
            payload_kind: ArtifactPayloadKind::Rhai,
            module_kind: ArtifactModuleKind::Optional,
            runtime_abi: RHAI_SANDBOX_RUNTIME_ABI.to_string(),
            platform_compatibility: "*".to_string(),
            required_features: Vec::new(),
            artifact_digest: source_digest.clone(),
            entrypoint: command.workspace.entrypoint.clone(),
            capabilities: command.capabilities.clone(),
            bindings: command.bindings.clone(),
            dependencies: Vec::new(),
            permissions: command.permissions.clone(),
            schema_documents: command.schema_documents.clone(),
            settings_schema_digest: command.settings_schema_digest.clone(),
            data_schema_digest: command.data_schema_digest.clone(),
            localization_catalogs: Vec::new(),
            ui_contributions: Vec::new(),
            persistence_contract: command.persistence_contract.clone(),
        };

        descriptor.validate()?;

        let descriptor_json = serde_json::to_string(&descriptor)
            .map_err(|e| RhaiAuthoringError::Serialization(e.to_string()))?;
        let descriptor_digest = format!(
            "sha256:{}",
            hex::encode(Sha256::digest(descriptor_json.as_bytes()))
        );

        // 7. Check idempotency in database
        let backend = self.db.get_database_backend();
        if let Some(row) = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT package_id, source_digest, descriptor_digest, descriptor_json, created_at \
                     FROM module_artifact_rhai_authoring_packages \
                     WHERE tenant_id = {} AND alloy_script_id = {} AND alloy_revision = {} \
                       AND idempotency_key = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                vec![
                    uuid_value(command.tenant_id, backend),
                    uuid_value(command.alloy_script_id, backend),
                    revision_value(command.alloy_revision as u64)
                        .map_err(|e| RhaiAuthoringError::Storage(e.to_string()))?,
                    uuid_value(command.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(|e| RhaiAuthoringError::Storage(e.to_string()))?
        {
            let existing_package_id_str: String = row
                .try_get("", "package_id")
                .map_err(|e| RhaiAuthoringError::Storage(e.to_string()))?;
            let existing_package_id = Uuid::parse_str(&existing_package_id_str)
                .map_err(|e| RhaiAuthoringError::Storage(e.to_string()))?;
            let existing_source_digest: String = row
                .try_get("", "source_digest")
                .map_err(|e| RhaiAuthoringError::Storage(e.to_string()))?;
            let existing_descriptor_digest: String = row
                .try_get("", "descriptor_digest")
                .map_err(|e| RhaiAuthoringError::Storage(e.to_string()))?;
            let existing_created_at: DateTime<Utc> = row
                .try_get("", "created_at")
                .map_err(|e| RhaiAuthoringError::Storage(e.to_string()))?;

            if existing_source_digest != source_digest
                || existing_descriptor_digest != descriptor_digest
            {
                return Err(RhaiAuthoringError::IdempotencyConflict {
                    alloy_script_id: command.alloy_script_id,
                    alloy_revision: command.alloy_revision,
                });
            }

            let mut annotations = BTreeMap::new();
            annotations.insert(
                "org.opencontainers.image.title".to_string(),
                command.slug.clone(),
            );
            annotations.insert(
                "org.opencontainers.image.version".to_string(),
                command.version.clone(),
            );
            annotations.insert(
                "io.rustok.alloy.script_id".to_string(),
                command.alloy_script_id.to_string(),
            );
            annotations.insert(
                "io.rustok.alloy.revision".to_string(),
                command.alloy_revision.to_string(),
            );
            annotations.insert(
                "io.rustok.descriptor.digest".to_string(),
                descriptor_digest.clone(),
            );

            return Ok(RhaiAuthoringPublishableRelease {
                package_id: existing_package_id,
                descriptor,
                descriptor_digest,
                source_cas_receipt: RhaiSourceCasReceipt {
                    source_digest: source_digest.clone(),
                    size_bytes: canonical_bytes.len() as u64,
                    media_type: RHAI_WORKSPACE_MEDIA_TYPE.to_string(),
                    created: false,
                    published_at: existing_created_at,
                },
                oci_payload: RhaiOciPayload {
                    media_type: RHAI_SOURCE_MEDIA_TYPE.to_string(),
                    digest: source_digest,
                    size_bytes: canonical_bytes.len() as u64,
                    annotations,
                },
                created_at: existing_created_at,
            });
        }

        // 8. Publish canonical bytes into create-only source-CAS
        let blob_exists = self
            .blob_store
            .get_verified(&source_digest)
            .await
            .is_ok();

        let created = if !blob_exists {
            self.blob_store
                .put_verified(&source_digest, &canonical_bytes)
                .await
                .map_err(|e| RhaiAuthoringError::BlobStore(e.to_string()))?;
            true
        } else {
            false
        };

        let now = Utc::now();
        let package_id = Uuid::new_v4();

        // 9. Persist immutable authoring package record in database
        self.db
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_rhai_authoring_packages (\
                        package_id, tenant_id, slug, version, alloy_script_id, alloy_revision, \
                        review_decision_id, review_digest, source_digest, descriptor_digest, \
                        descriptor_json, idempotency_key, actor_id, trace_id, created_at\
                    ) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                    placeholder(backend, 7),
                    placeholder(backend, 8),
                    placeholder(backend, 9),
                    placeholder(backend, 10),
                    placeholder(backend, 11),
                    placeholder(backend, 12),
                    placeholder(backend, 13),
                    placeholder(backend, 14),
                    placeholder(backend, 15),
                ),
                vec![
                    uuid_value(package_id, backend),
                    uuid_value(command.tenant_id, backend),
                    command.slug.clone().into(),
                    command.version.clone().into(),
                    uuid_value(command.alloy_script_id, backend),
                    revision_value(command.alloy_revision as u64)
                        .map_err(|e| RhaiAuthoringError::Storage(e.to_string()))?,
                    uuid_value(command.review_decision_id, backend),
                    command.review_digest.into(),
                    source_digest.clone().into(),
                    descriptor_digest.clone().into(),
                    descriptor_json.into(),
                    uuid_value(command.idempotency_key, backend),
                    uuid_value(command.actor_id, backend),
                    command.trace_id.into(),
                    now.into(),
                ],
            ))
            .await
            .map_err(|e| RhaiAuthoringError::Storage(e.to_string()))?;

        // 10. Construct canonical OCI payload
        let mut annotations = BTreeMap::new();
        annotations.insert(
            "org.opencontainers.image.title".to_string(),
            command.slug.clone(),
        );
        annotations.insert(
            "org.opencontainers.image.version".to_string(),
            command.version.clone(),
        );
        annotations.insert(
            "io.rustok.alloy.script_id".to_string(),
            command.alloy_script_id.to_string(),
        );
        annotations.insert(
            "io.rustok.alloy.revision".to_string(),
            command.alloy_revision.to_string(),
        );
        annotations.insert(
            "io.rustok.descriptor.digest".to_string(),
            descriptor_digest.clone(),
        );

        Ok(RhaiAuthoringPublishableRelease {
            package_id,
            descriptor,
            descriptor_digest,
            source_cas_receipt: RhaiSourceCasReceipt {
                source_digest: source_digest.clone(),
                size_bytes: canonical_bytes.len() as u64,
                media_type: RHAI_WORKSPACE_MEDIA_TYPE.to_string(),
                created,
                published_at: now,
            },
            oci_payload: RhaiOciPayload {
                media_type: RHAI_SOURCE_MEDIA_TYPE.to_string(),
                digest: source_digest,
                size_bytes: canonical_bytes.len() as u64,
                annotations,
            },
            created_at: now,
        })
    }
}
