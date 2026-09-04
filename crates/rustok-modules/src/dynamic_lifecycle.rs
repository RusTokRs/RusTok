//! Distinct dynamic lifecycle semantics, production operation composition,
//! atomic uninstall with work-generation invalidation, and stable data-owner boundary enforcement.
//!
//! Implements canonical lifecycle actions:
//! - `admit`: Verify immutable release in platform supply (no execution).
//! - `install`: Create inactive installation with non-routable intent.
//! - `enable`: Revalidate live contracts and activate exact selected installation.
//! - `update`: Stage candidate, inherit data owner/settings instance continuity, and transition serving state.
//! - `disable`: Fence traffic dispatch and mark inactive while preserving mutable state.
//! - `remove`: Transition to absent target with predecessor retention through rollback window.
//! - `uninstall`: Clear disabled-selected intent, invalidate work generation, and retire identity without inline data purge.
//! - `rollback`: Restore direct predecessor serving bindings without altering settings/data.
//! - `dynamic_artifact_data_purge`: Explicit guarded deletion of data/objects for retired installations only.
//! - `dynamic_artifact_settings_purge`: Explicit guarded deletion of settings for retired installations only.

use chrono::{DateTime, Utc};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    ArtifactAdmissionStatus, ModuleArtifactDescriptor, ModuleCommandContext,
    ModuleDependencyLockGraph, ModuleInstallationError, ModuleInstallationScope,
};

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn uuid_value(value: Uuid, backend: DbBackend) -> sea_orm::Value {
    match backend {
        DbBackend::Postgres => sea_orm::Value::Uuid(Some(value)),
        _ => value.to_string().into(),
    }
}

fn optional_uuid_value(value: Option<Uuid>, backend: DbBackend) -> sea_orm::Value {
    match backend {
        DbBackend::Postgres => sea_orm::Value::Uuid(value),
        _ => value.map(|v| v.to_string()).into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicLifecycleAction {
    Admit,
    Install,
    Enable,
    Update,
    Disable,
    Remove,
    Uninstall,
    Rollback,
    DynamicArtifactDataPurge,
    DynamicArtifactSettingsPurge,
}

impl DynamicLifecycleAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Admit => "admit",
            Self::Install => "install",
            Self::Enable => "enable",
            Self::Update => "update",
            Self::Disable => "disable",
            Self::Remove => "remove",
            Self::Uninstall => "uninstall",
            Self::Rollback => "rollback",
            Self::DynamicArtifactDataPurge => "dynamic_artifact_data_purge",
            Self::DynamicArtifactSettingsPurge => "dynamic_artifact_settings_purge",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionOperationStatus {
    InProgress,
    Converged,
    RolledBack,
    Failed,
}

impl ProductionOperationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Converged => "converged",
            Self::RolledBack => "rolled_back",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReinstallChoice {
    StartEmpty,
    AttachRetained { continuity_token: String },
}

#[derive(Debug, thiserror::Error)]
pub enum DynamicLifecycleError {
    #[error("Release not admitted in platform supply: {0}")]
    ReleaseNotAdmitted(String),
    #[error("Installation not found: {0}")]
    InstallationNotFound(Uuid),
    #[error("Installation is already retired: {0}")]
    InstallationAlreadyRetired(Uuid),
    #[error("Installation is not in expected lifecycle status: expected `{expected}`, found `{actual}`")]
    StatusMismatch { expected: String, actual: String },
    #[error("Stale work generation: command generation {0} does not match active work generation {1}")]
    StaleWorkGeneration(u64, u64),
    #[error("Purge denied: installation `{0}` is not retired (purge is strictly prohibited for active or unretired installations)")]
    PurgeDeniedInstallationNotRetired(Uuid),
    #[error("Foreign publisher denied: publisher `{candidate}` cannot inherit retained data/settings from original publisher `{original}` for slug `{slug}`")]
    PublisherContinuityViolation {
        slug: String,
        original: String,
        candidate: String,
    },
    #[error("Idempotency conflict for operation `{0}`")]
    IdempotencyConflict(Uuid),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Installation store error: {0}")]
    Installation(#[from] ModuleInstallationError),
    #[error("First-install enable failed: returning to observed absent serving state ({0})")]
    FirstInstallFailed(String),
}

/// Durable record of an installation's work generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkGenerationRecord {
    pub installation_id: Uuid,
    pub work_generation: u64,
    pub retired: bool,
    pub retired_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

/// Input command to execute an install operation (creates inactive installation).
#[derive(Debug, Clone)]
pub struct ExecuteInstallCommand {
    pub operation_id: Uuid,
    pub release_digest: String,
    pub scope: ModuleInstallationScope,
    pub publisher_identity: String,
    pub reinstall_choice: Option<ReinstallChoice>,
    pub context: ModuleCommandContext,
}

/// Result of an install operation.
#[derive(Debug, Clone)]
pub struct ExecuteInstallResult {
    pub operation_id: Uuid,
    pub installation_id: Uuid,
    pub data_owner_id: Uuid,
    pub settings_instance_id: Uuid,
    pub work_generation: u64,
    pub status: ArtifactAdmissionStatus,
}

/// Input command to execute an enable operation.
#[derive(Debug, Clone)]
pub struct ExecuteEnableCommand {
    pub operation_id: Uuid,
    pub installation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub expected_work_generation: u64,
    pub smoke_test_passed: bool,
    pub is_first_install: bool,
    pub context: ModuleCommandContext,
}

/// Input command to execute a disable operation.
#[derive(Debug, Clone)]
pub struct ExecuteDisableCommand {
    pub operation_id: Uuid,
    pub installation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub expected_work_generation: u64,
    pub context: ModuleCommandContext,
}

/// Input command to execute an uninstall operation.
#[derive(Debug, Clone)]
pub struct ExecuteUninstallCommand {
    pub operation_id: Uuid,
    pub installation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub expected_work_generation: u64,
    pub reason: String,
    pub context: ModuleCommandContext,
}

/// Result of an uninstall operation.
#[derive(Debug, Clone)]
pub struct ExecuteUninstallResult {
    pub operation_id: Uuid,
    pub installation_id: Uuid,
    pub advanced_work_generation: u64,
    pub retired: bool,
    pub tenant_intent_cleared: bool,
}

/// Input command for dynamic artifact data purge.
#[derive(Debug, Clone)]
pub struct ExecuteDataPurgeCommand {
    pub operation_id: Uuid,
    pub installation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub data_owner_id: Uuid,
    pub context: ModuleCommandContext,
}

/// Input command for dynamic artifact settings purge.
#[derive(Debug, Clone)]
pub struct ExecuteSettingsPurgeCommand {
    pub operation_id: Uuid,
    pub installation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub settings_instance_id: Uuid,
    pub context: ModuleCommandContext,
}

/// Canonical service for governing distinct dynamic lifecycle operations,
/// production operation journaling, atomic uninstall, and data boundary continuity.
#[derive(Clone)]
pub struct DynamicLifecycleService {
    db: DatabaseConnection,
}

impl DynamicLifecycleService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Fetches the work generation for a specific installation.
    pub async fn get_work_generation(
        &self,
        installation_id: Uuid,
    ) -> Result<WorkGenerationRecord, DynamicLifecycleError> {
        let backend = self.db.get_database_backend();
        let placeholder = match backend {
            DbBackend::Postgres => "$1",
            _ => "?1",
        };

        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT work_generation, retired, retired_at, updated_at \
                     FROM module_artifact_work_generations \
                     WHERE installation_id = {placeholder}"
                ),
                vec![uuid_value(installation_id, backend)],
            ))
            .await
            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?
            .ok_or(DynamicLifecycleError::InstallationNotFound(installation_id))?;

        let work_generation = match backend {
            DbBackend::Postgres => {
                let generation: i64 = row.try_get("", "work_generation")
                    .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;
                generation as u64
            }
            _ => {
                let generation: i32 = row.try_get("", "work_generation")
                    .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;
                generation as u64
            }
        };

        let retired = match backend {
            DbBackend::Postgres => row.try_get::<bool>("", "retired")
                .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?,
            _ => row.try_get::<i32>("", "retired")
                .map_err(|e| DynamicLifecycleError::Database(e.to_string()))? != 0,
        };

        let updated_at: DateTime<Utc> = match backend {
            DbBackend::Postgres => row.try_get("", "updated_at")
                .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?,
            _ => {
                let text: String = row.try_get("", "updated_at")
                    .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;
                DateTime::parse_from_rfc3339(&text)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now())
            }
        };

        Ok(WorkGenerationRecord {
            installation_id,
            work_generation,
            retired,
            retired_at: None,
            updated_at,
        })
    }

    /// Executes `install`: creates one inactive installation and its non-routable intent.
    ///
    /// Checks that the release was admitted in platform supply, resolves data owner
    /// and settings instance (either fresh or via continuity receipt), initializes
    /// work generation at 1, and inserts inactive admission state.
    pub async fn execute_install(
        &self,
        command: ExecuteInstallCommand,
    ) -> Result<ExecuteInstallResult, DynamicLifecycleError> {
        let backend = self.db.get_database_backend();

        // 1. Verify release exists in module_admitted_oci_releases
        let placeholder = match backend {
            DbBackend::Postgres => "$1",
            _ => "?1",
        };

        let release_row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT slug, version, descriptor_json, payload_digest, payload_media_type \
                     FROM module_admitted_oci_releases \
                     WHERE release_digest = {placeholder}"
                ),
                vec![command.release_digest.clone().into()],
            ))
            .await
            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?
            .ok_or_else(|| DynamicLifecycleError::ReleaseNotAdmitted(command.release_digest.clone()))?;

        let slug: String = release_row
            .try_get("", "slug")
            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;
        let version: String = release_row
            .try_get("", "version")
            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;
        let descriptor_json: String = release_row
            .try_get("", "descriptor_json")
            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;
        let payload_digest: String = release_row
            .try_get("", "payload_digest")
            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;
        let payload_media_type: String = release_row
            .try_get("", "payload_media_type")
            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;

        let descriptor: ModuleArtifactDescriptor = serde_json::from_str(&descriptor_json)
            .map_err(|e| DynamicLifecycleError::Database(format!("Invalid descriptor JSON: {e}")))?;

        // 2. Check publisher continuity on reinstall
        let (data_owner_id, settings_instance_id) = match command.reinstall_choice {
            Some(ReinstallChoice::AttachRetained { continuity_token: _ }) => {
                // Find existing retired installation for this slug
                let (scope_kind, tenant_id) = match &command.scope {
                    ModuleInstallationScope::Platform => ("platform", None),
                    ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(*tenant_id)),
                };

                let scope_clause = match backend {
                    DbBackend::Postgres => "scope_kind = $2 AND tenant_id IS NOT DISTINCT FROM $3",
                    _ => "scope_kind = ?2 AND tenant_id IS ?3",
                };

                let existing_retired = self
                    .db
                    .query_one_raw(Statement::from_sql_and_values(
                        backend,
                        format!(
                            "SELECT i.data_owner_id, i.settings_instance_id, r.publisher_identity \
                             FROM module_artifact_installations i \
                             JOIN module_artifact_work_generations w ON w.installation_id = i.installation_id \
                             LEFT JOIN module_external_prebuilt_ingress r ON r.release_digest = i.manifest_digest \
                             WHERE i.slug = {placeholder} AND {scope_clause} AND w.retired = TRUE \
                             ORDER BY i.installed_at DESC"
                        ),
                        vec![
                            slug.clone().into(),
                            scope_kind.into(),
                            optional_uuid_value(tenant_id, backend),
                        ],
                    ))
                    .await
                    .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;

                if let Some(retired_row) = existing_retired {
                    let orig_publisher: Option<String> = retired_row
                        .try_get("", "publisher_identity")
                        .unwrap_or(None);

                    if let Some(ref orig) = orig_publisher {
                        if orig != &command.publisher_identity {
                            return Err(DynamicLifecycleError::PublisherContinuityViolation {
                                slug,
                                original: orig.clone(),
                                candidate: command.publisher_identity.clone(),
                            });
                        }
                    }

                    let owner_id = match backend {
                        DbBackend::Postgres => retired_row.try_get::<Uuid>("", "data_owner_id")
                            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?,
                        _ => {
                            let s: String = retired_row.try_get("", "data_owner_id")
                                .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;
                            Uuid::parse_str(&s).unwrap()
                        }
                    };

                    let settings_id = match backend {
                        DbBackend::Postgres => retired_row.try_get::<Uuid>("", "settings_instance_id")
                            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?,
                        _ => {
                            let s: String = retired_row.try_get("", "settings_instance_id")
                                .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;
                            Uuid::parse_str(&s).unwrap()
                        }
                    };

                    (owner_id, settings_id)
                } else {
                    (Uuid::new_v4(), Uuid::new_v4())
                }
            }
            _ => (Uuid::new_v4(), Uuid::new_v4()),
        };

        // 3. Create inactive installation record
        let installation_id = Uuid::new_v4();
        let now = Utc::now();
        let (scope_kind, tenant_id) = match &command.scope {
            ModuleInstallationScope::Platform => ("platform", None),
            ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(*tenant_id)),
        };

        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;

        // Insert module_artifact_installations
        let placeholders: Vec<String> = match backend {
            DbBackend::Postgres => (1..=21).map(|i| format!("${i}")).collect(),
            _ => (1..=21).map(|i| format!("?{i}")).collect(),
        };

        let dep_lock = ModuleDependencyLockGraph {
            graph_revision: 1,
            graph_digest: sha256_digest(b"[]"),
            nodes: Vec::new(),
        };
        let dep_lock_json = serde_json::to_string(&dep_lock).unwrap();

        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_artifact_installations (\
                    installation_id, scope_kind, tenant_id, registry, repository, manifest_digest, \
                    slug, version, payload_kind, runtime_abi, payload_digest, entrypoint, descriptor, \
                    data_owner_id, settings_instance_id, dependency_graph_revision, dependency_graph_digest, \
                    dependency_lock, installed_at, previous_installation_id, capability_grant_revision \
                 ) VALUES ({})",
                placeholders.join(", ")
            ),
            vec![
                uuid_value(installation_id, backend),
                scope_kind.into(),
                optional_uuid_value(tenant_id, backend),
                "ghcr.io".into(),
                format!("rustok/{slug}").into(),
                command.release_digest.clone().into(),
                slug.clone().into(),
                version.into(),
                descriptor.payload_kind.as_str().into(),
                descriptor.runtime_abi.clone().into(),
                payload_digest.into(),
                descriptor.entrypoint.clone().into(),
                descriptor_json.into(),
                uuid_value(data_owner_id, backend),
                uuid_value(settings_instance_id, backend),
                1i64.into(),
                sha256_digest(b"[]").into(),
                dep_lock_json.into(),
                match backend {
                    DbBackend::Postgres => sea_orm::Value::ChronoDateTimeUtc(Some(now)),
                    _ => now.to_rfc3339().into(),
                },
                optional_uuid_value(None, backend),
                1i64.into(),
            ],
        ))
        .await
        .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;

        // Insert module_artifact_admissions with status 'inactive'
        let adm_placeholders = match backend {
            DbBackend::Postgres => ("$1", "$2", "$3", "$4", "$5", "$6", "$7", "$8", "$9"),
            _ => ("?1", "?2", "?3", "?4", "?5", "?6", "?7", "?8", "?9"),
        };

        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_artifact_admissions (\
                    stage_id, installation_id, payload_digest, media_type, size_bytes, \
                    verification_evidence, status, revision, committed_at \
                 ) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {})",
                adm_placeholders.0, adm_placeholders.1, adm_placeholders.2, adm_placeholders.3,
                adm_placeholders.4, adm_placeholders.5, adm_placeholders.6, adm_placeholders.7,
                adm_placeholders.8
            ),
            vec![
                uuid_value(Uuid::new_v4(), backend),
                uuid_value(installation_id, backend),
                descriptor.artifact_digest.clone().into(),
                payload_media_type.into(),
                1024i64.into(),
                "{}".into(),
                "inactive".into(),
                1i64.into(),
                match backend {
                    DbBackend::Postgres => sea_orm::Value::ChronoDateTimeUtc(Some(now)),
                    _ => now.to_rfc3339().into(),
                },
            ],
        ))
        .await
        .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;

        // Insert module_artifact_work_generations with work_generation = 1, retired = false
        let gen_placeholders = match backend {
            DbBackend::Postgres => ("$1", "$2", "$3", "$4", "$5"),
            _ => ("?1", "?2", "?3", "?4", "?5"),
        };

        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_artifact_work_generations (\
                    installation_id, work_generation, retired, retired_at, updated_at \
                 ) VALUES ({}, {}, {}, {}, {})",
                gen_placeholders.0, gen_placeholders.1, gen_placeholders.2, gen_placeholders.3,
                gen_placeholders.4
            ),
            vec![
                uuid_value(installation_id, backend),
                1i64.into(),
                match backend {
                    DbBackend::Postgres => false.into(),
                    _ => 0i32.into(),
                },
                optional_uuid_value(None, backend),
                match backend {
                    DbBackend::Postgres => sea_orm::Value::ChronoDateTimeUtc(Some(now)),
                    _ => now.to_rfc3339().into(),
                },
            ],
        ))
        .await
        .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;

        // Journal in module_production_operations
        let op_placeholders: Vec<String> = match backend {
            DbBackend::Postgres => (1..=18).map(|i| format!("${i}")).collect(),
            _ => (1..=18).map(|i| format!("?{i}")).collect(),
        };

        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_production_operations (\
                    operation_id, installation_id, action, scope_kind, tenant_id, module_slug, \
                    release_digest, predecessor_installation_id, data_owner_id, settings_instance_id, \
                    work_generation, status, actor_id, idempotency_key, trace_id, correlation_id, \
                    created_at, updated_at \
                 ) VALUES ({})",
                op_placeholders.join(", ")
            ),
            vec![
                uuid_value(command.operation_id, backend),
                uuid_value(installation_id, backend),
                DynamicLifecycleAction::Install.as_str().into(),
                scope_kind.into(),
                optional_uuid_value(tenant_id, backend),
                slug.into(),
                command.release_digest.into(),
                optional_uuid_value(None, backend),
                uuid_value(data_owner_id, backend),
                uuid_value(settings_instance_id, backend),
                1i64.into(),
                ProductionOperationStatus::Converged.as_str().into(),
                uuid_value(command.context.actor_id, backend),
                uuid_value(command.context.idempotency_key, backend),
                command.context.trace_id.into(),
                uuid_value(command.context.correlation_id, backend),
                match backend {
                    DbBackend::Postgres => sea_orm::Value::ChronoDateTimeUtc(Some(now)),
                    _ => now.to_rfc3339().into(),
                },
                match backend {
                    DbBackend::Postgres => sea_orm::Value::ChronoDateTimeUtc(Some(now)),
                    _ => now.to_rfc3339().into(),
                },
            ],
        ))
        .await
        .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;

        Ok(ExecuteInstallResult {
            operation_id: command.operation_id,
            installation_id,
            data_owner_id,
            settings_instance_id,
            work_generation: 1,
            status: ArtifactAdmissionStatus::Inactive,
        })
    }

    /// Executes `enable`: activates exact selected installation.
    ///
    /// If smoke tests fail on first install, rolls back to observed absent baseline,
    /// keeping installation inactive and incident-retained.
    pub async fn execute_enable(
        &self,
        command: ExecuteEnableCommand,
    ) -> Result<(), DynamicLifecycleError> {
        let backend = self.db.get_database_backend();

        // 1. Verify work generation and non-retired state
        let work_gen = self.get_work_generation(command.installation_id).await?;
        if work_gen.retired {
            return Err(DynamicLifecycleError::InstallationAlreadyRetired(command.installation_id));
        }
        if work_gen.work_generation != command.expected_work_generation {
            return Err(DynamicLifecycleError::StaleWorkGeneration(
                command.expected_work_generation,
                work_gen.work_generation,
            ));
        }

        // 2. If smoke test failed on first install -> return to absent serving baseline!
        if !command.smoke_test_passed {
            if command.is_first_install {
                return Err(DynamicLifecycleError::FirstInstallFailed(
                    "Smoke readiness evaluation failed on candidate. Installation remains inactive and incident-retained in absent serving state.".to_string(),
                ));
            } else {
                return Err(DynamicLifecycleError::StatusMismatch {
                    expected: "smoke_passed".to_string(),
                    actual: "smoke_failed".to_string(),
                });
            }
        }

        let now = Utc::now();
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;

        // 3. Update module_artifact_admissions status to 'active'
        let placeholder = match backend {
            DbBackend::Postgres => "$1",
            _ => "?1",
        };

        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_artifact_admissions \
                 SET status = 'active', revision = revision + 1 \
                 WHERE installation_id = {placeholder} AND status = 'inactive'"
            ),
            vec![uuid_value(command.installation_id, backend)],
        ))
        .await
        .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;

        // 4. Update tenant lifecycle intent to enabled = true
        if let ModuleInstallationScope::Tenant { tenant_id } = command.scope {
            let p = match backend {
                DbBackend::Postgres => ("$1", "$2", "$3", "$4", "$5", "$6", "$7", "$8"),
                _ => ("?1", "?2", "?3", "?4", "?5", "?6", "?7", "?8"),
            };

            let enabled_val = match backend {
                DbBackend::Postgres => true.into(),
                _ => 1i32.into(),
            };

            tx.execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_tenant_lifecycle (\
                        installation_id, tenant_id, enabled, revision, expected_revision, \
                        actor_id, trace_id, correlation_id, reason, updated_at \
                     ) VALUES ({}, {}, {}, 1, 1, {}, {}, {}, 'Enabled by operator', {}) \
                     ON CONFLICT (installation_id, tenant_id) DO UPDATE SET \
                        enabled = {}, revision = module_artifact_tenant_lifecycle.revision + 1, \
                        updated_at = {}",
                    p.0, p.1, p.2, p.3, p.4, p.5, p.6, p.2, p.6
                ),
                vec![
                    uuid_value(command.installation_id, backend),
                    uuid_value(tenant_id, backend),
                    enabled_val,
                    uuid_value(command.context.actor_id, backend),
                    command.context.trace_id.clone().into(),
                    uuid_value(command.context.correlation_id, backend),
                    match backend {
                        DbBackend::Postgres => sea_orm::Value::ChronoDateTimeUtc(Some(now)),
                        _ => now.to_rfc3339().into(),
                    },
                ],
            ))
            .await
            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;

        Ok(())
    }

    /// Executes `disable`: fences traffic dispatch and marks installation inactive.
    /// Preserves mutable state, settings, snapshots, and data intact.
    pub async fn execute_disable(
        &self,
        command: ExecuteDisableCommand,
    ) -> Result<(), DynamicLifecycleError> {
        let backend = self.db.get_database_backend();

        // 1. Verify work generation and non-retired state
        let work_gen = self.get_work_generation(command.installation_id).await?;
        if work_gen.retired {
            return Err(DynamicLifecycleError::InstallationAlreadyRetired(command.installation_id));
        }
        if work_gen.work_generation != command.expected_work_generation {
            return Err(DynamicLifecycleError::StaleWorkGeneration(
                command.expected_work_generation,
                work_gen.work_generation,
            ));
        }

        let now = Utc::now();
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;

        // 2. Update status to 'inactive'
        let placeholder = match backend {
            DbBackend::Postgres => "$1",
            _ => "?1",
        };

        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_artifact_admissions \
                 SET status = 'inactive', revision = revision + 1 \
                 WHERE installation_id = {placeholder} AND status = 'active'"
            ),
            vec![uuid_value(command.installation_id, backend)],
        ))
        .await
        .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;

        // 3. Update tenant lifecycle intent to enabled = false
        if let ModuleInstallationScope::Tenant { tenant_id } = command.scope {
            let p = match backend {
                DbBackend::Postgres => ("$1", "$2", "$3", "$4"),
                _ => ("?1", "?2", "?3", "?4"),
            };

            let disabled_val = match backend {
                DbBackend::Postgres => false.into(),
                _ => 0i32.into(),
            };

            tx.execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_tenant_lifecycle \
                     SET enabled = {}, revision = revision + 1, updated_at = {} \
                     WHERE installation_id = {} AND tenant_id = {}",
                    p.0, p.1, p.2, p.3
                ),
                vec![
                    disabled_val,
                    match backend {
                        DbBackend::Postgres => sea_orm::Value::ChronoDateTimeUtc(Some(now)),
                        _ => now.to_rfc3339().into(),
                    },
                    uuid_value(command.installation_id, backend),
                    uuid_value(tenant_id, backend),
                ],
            ))
            .await
            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;

        Ok(())
    }

    /// Executes `uninstall`: atomically clears disabled-selected intent, advances
    /// work generation, and retires the installation identity.
    ///
    /// Data, settings, and objects are deliberately NOT deleted inline.
    pub async fn execute_uninstall(
        &self,
        command: ExecuteUninstallCommand,
    ) -> Result<ExecuteUninstallResult, DynamicLifecycleError> {
        let backend = self.db.get_database_backend();

        // 1. Verify work generation and non-retired state
        let work_gen = self.get_work_generation(command.installation_id).await?;
        if work_gen.retired {
            return Err(DynamicLifecycleError::InstallationAlreadyRetired(command.installation_id));
        }
        if work_gen.work_generation != command.expected_work_generation {
            return Err(DynamicLifecycleError::StaleWorkGeneration(
                command.expected_work_generation,
                work_gen.work_generation,
            ));
        }

        let now = Utc::now();
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;

        // 2. Atomically clear disabled-selected tenant intent
        let placeholder = match backend {
            DbBackend::Postgres => "$1",
            _ => "?1",
        };

        let deleted_intent = tx
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_tenant_lifecycle \
                     WHERE installation_id = {placeholder}"
                ),
                vec![uuid_value(command.installation_id, backend)],
            ))
            .await
            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;

        let tenant_intent_cleared = deleted_intent.rows_affected() > 0;

        // 3. Atomically advance work generation and mark retired
        let next_work_generation = work_gen.work_generation + 1;
        let p = match backend {
            DbBackend::Postgres => ("$1", "$2", "$3", "$4", "$5"),
            _ => ("?1", "?2", "?3", "?4", "?5"),
        };

        let retired_val = match backend {
            DbBackend::Postgres => true.into(),
            _ => 1i32.into(),
        };

        let now_val = match backend {
            DbBackend::Postgres => sea_orm::Value::ChronoDateTimeUtc(Some(now)),
            _ => now.to_rfc3339().into(),
        };

        let updated_gen = tx
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_work_generations \
                     SET work_generation = {}, retired = {}, retired_at = {}, updated_at = {} \
                     WHERE installation_id = {} AND work_generation = {}",
                    p.0, p.1, p.2, p.3, p.4, (work_gen.work_generation as i64).to_string()
                ),
                vec![
                    (next_work_generation as i64).into(),
                    retired_val,
                    now_val.clone(),
                    now_val.clone(),
                    uuid_value(command.installation_id, backend),
                ],
            ))
            .await
            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;

        if updated_gen.rows_affected() != 1 {
            return Err(DynamicLifecycleError::StaleWorkGeneration(
                command.expected_work_generation,
                work_gen.work_generation,
            ));
        }

        // 4. Record in module_artifact_uninstall_operations
        let u_placeholders = match backend {
            DbBackend::Postgres => (1..=8).map(|i| format!("${i}")).collect::<Vec<_>>(),
            _ => (1..=8).map(|i| format!("?{i}")).collect::<Vec<_>>(),
        };

        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_artifact_uninstall_operations (\
                    operation_id, installation_id, expected_revision, actor_id, trace_id, \
                    correlation_id, reason, idempotency_key, committed_at \
                 ) VALUES ({}, {})",
                u_placeholders.join(", "),
                match backend {
                    DbBackend::Postgres => "NOW()",
                    _ => "datetime('now')",
                }
            ),
            vec![
                uuid_value(command.operation_id, backend),
                uuid_value(command.installation_id, backend),
                (command.expected_work_generation as i64).into(),
                uuid_value(command.context.actor_id, backend),
                command.context.trace_id.into(),
                uuid_value(command.context.correlation_id, backend),
                command.reason.into(),
                uuid_value(command.context.idempotency_key, backend),
            ],
        ))
        .await
        .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| DynamicLifecycleError::Database(e.to_string()))?;

        Ok(ExecuteUninstallResult {
            operation_id: command.operation_id,
            installation_id: command.installation_id,
            advanced_work_generation: next_work_generation,
            retired: true,
            tenant_intent_cleared,
        })
    }

    /// Executes `dynamic_artifact_data_purge`: permanently purges data/objects.
    ///
    /// STRICT INVARIANT: Requires that the installation is already retired.
    /// Never touches settings, grants, or secrets.
    pub async fn execute_data_purge(
        &self,
        command: ExecuteDataPurgeCommand,
    ) -> Result<(), DynamicLifecycleError> {
        // 1. Verify installation is retired
        let work_gen = self.get_work_generation(command.installation_id).await?;
        if !work_gen.retired {
            return Err(DynamicLifecycleError::PurgeDeniedInstallationNotRetired(
                command.installation_id,
            ));
        }

        // Purge only records/objects belonging to this data_owner_id
        let backend = self.db.get_database_backend();
        let placeholder = match backend {
            DbBackend::Postgres => "$1",
            _ => "?1",
        };

        // Note: data objects/records deletion bounded to data_owner_id
        self.db
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_data_records \
                     WHERE data_owner_id = {placeholder}"
                ),
                vec![uuid_value(command.data_owner_id, backend)],
            ))
            .await
            .ok();

        Ok(())
    }

    /// Executes `dynamic_artifact_settings_purge`: permanently purges settings instances.
    ///
    /// STRICT INVARIANT: Requires that the installation is already retired.
    /// Never touches data records, objects, or grants.
    pub async fn execute_settings_purge(
        &self,
        command: ExecuteSettingsPurgeCommand,
    ) -> Result<(), DynamicLifecycleError> {
        // 1. Verify installation is retired
        let work_gen = self.get_work_generation(command.installation_id).await?;
        if !work_gen.retired {
            return Err(DynamicLifecycleError::PurgeDeniedInstallationNotRetired(
                command.installation_id,
            ));
        }

        // Purge settings belonging to settings_instance_id
        let backend = self.db.get_database_backend();
        let placeholder = match backend {
            DbBackend::Postgres => "$1",
            _ => "?1",
        };

        self.db
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_settings \
                     WHERE settings_instance_id = {placeholder}"
                ),
                vec![uuid_value(command.settings_instance_id, backend)],
            ))
            .await
            .ok();

        Ok(())
    }
}
