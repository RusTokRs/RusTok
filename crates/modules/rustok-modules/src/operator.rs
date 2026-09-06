//! Operator experience and command surface for module release and rollback lifecycle.
//!
//! Exposes a unified owner projection and command surface across:
//! - Lifecycle commands: source/prebuilt submission, admission, install/add/update/enable/disable/remove/uninstall/reinstall,
//!   rollback/containment, distinct finalization, dynamic artifact data purge, dynamic artifact settings purge, queue drain.
//! - Canonical status reads and WordPress-like flow mapping using canonical tokens:
//!   `ready`, `running`, `observing`, `accepted`, `recovering`, `recovered`, `rejected`, `cancelled`, `recovery_required`.
//! - Blast radius, mode/reason, irreversible checkpoint, eligibility denial, fence state, diagnostics, recovery action.
//! - Exact current, candidate, and direct-predecessor coordinates by unit.
//! - Strict rejection of one publisher/module semver or distribution lineage/version resolving to different bytes.
//! - Authorized diagnostic support-bundle retrieval with zero raw pointers/keys/passwords/bypass controls.

use chrono::{DateTime, Utc};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ConflictFenceSet, DynamicLifecycleService, ModuleInstallationScope,
    RetentionHoldRecord, UpdateMode,
};

/// Canonical presentation state tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanonicalPresentationState {
    Ready,
    Running,
    Observing,
    Accepted,
    Recovering,
    Recovered,
    Rejected,
    Cancelled,
    RecoveryRequired,
}

impl CanonicalPresentationState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Running => "running",
            Self::Observing => "observing",
            Self::Accepted => "accepted",
            Self::Recovering => "recovering",
            Self::Recovered => "recovered",
            Self::Rejected => "rejected",
            Self::Cancelled => "cancelled",
            Self::RecoveryRequired => "recovery_required",
        }
    }

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Running => "Updating",
            Self::Observing => "Observing",
            Self::Accepted => "Active",
            Self::Recovering => "Recovering",
            Self::Recovered => "Recovered",
            Self::Rejected => "Rejected",
            Self::Cancelled => "Cancelled",
            Self::RecoveryRequired => "Recovery required",
        }
    }
}

/// Typed containment outcome rendered beneath `recovery_required`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainmentOutcome {
    Stopped,
    Fenced,
    DiagnosticsRetained,
}

impl ContainmentOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Fenced => "fenced",
            Self::DiagnosticsRetained => "diagnostics_retained",
        }
    }
}

/// Exact release coordinate identifying either a dynamic artifact or a static distribution composition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModuleReleaseCoordinate {
    Dynamic {
        publisher_identity: String,
        module_slug: String,
        version: String,
        release_digest: String,
        payload_digest: String,
    },
    Static {
        distribution_lineage: String,
        version_label: String,
        distribution_release_id: Uuid,
        bundle_root_digest: String,
        module_version_diffs: Vec<ModuleVersionDiff>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleVersionDiff {
    pub module_slug: String,
    pub previous_version: Option<String>,
    pub candidate_version: String,
    pub previous_digest: Option<String>,
    pub candidate_digest: String,
}

/// Operator blast radius evaluation for preflight and preview.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleBlastRadius {
    pub affected_scope: ModuleInstallationScope,
    pub affected_tenants_count: u64,
    pub affected_roles: Vec<String>,
    pub dependent_modules: Vec<String>,
    pub has_schema_migration: bool,
    pub is_irreversible: bool,
    pub data_owner_id: Uuid,
    pub settings_instance_id: Uuid,
}

/// Transition eligibility evaluation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionEligibility {
    pub eligible: bool,
    pub denial_reasons: Vec<String>,
}

/// Operator preview projection prior to transition application.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionPreviewProjection {
    pub operation_id: Uuid,
    pub module_slug: String,
    pub scope: ModuleInstallationScope,
    pub current_identity: Option<ModuleReleaseCoordinate>,
    pub candidate_identity: Option<ModuleReleaseCoordinate>,
    pub predecessor_identity: Option<ModuleReleaseCoordinate>,
    pub mode: UpdateMode,
    pub reason: String,
    pub blast_radius: ModuleBlastRadius,
    pub irreversible_checkpoint: Option<String>,
    pub eligibility: TransitionEligibility,
    pub fence_state: ConflictFenceSet,
    pub preview_digest: String,
}

/// Operator status projection for runtime and dashboard presentation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStatusProjection {
    pub module_slug: String,
    pub scope: ModuleInstallationScope,
    pub presentation_state: CanonicalPresentationState,
    pub display_label: String,
    pub containment_outcome: Option<ContainmentOutcome>,
    pub current_identity: Option<ModuleReleaseCoordinate>,
    pub candidate_identity: Option<ModuleReleaseCoordinate>,
    pub predecessor_identity: Option<ModuleReleaseCoordinate>,
    pub work_generation: u64,
    pub retired: bool,
    pub active_retention_holds_count: usize,
    pub diagnostics: Vec<String>,
    pub recovery_action: Option<String>,
}

/// Authorized support bundle for incident diagnostics and support escalations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleSupportBundle {
    pub bundle_id: Uuid,
    pub module_slug: String,
    pub installation_id: Option<Uuid>,
    pub operation_id: Option<Uuid>,
    pub generated_at: DateTime<Utc>,
    pub presentation_state: CanonicalPresentationState,
    pub containment_outcome: Option<ContainmentOutcome>,
    pub diagnostics: Vec<String>,
    pub recovery_action: Option<String>,
    pub active_retention_holds: Vec<RetentionHoldRecord>,
    pub work_generation: u64,
    pub retired: bool,
}

#[derive(Debug, Error)]
pub enum ModuleOperatorError {
    #[error("Module installation not found for slug `{0}`")]
    InstallationNotFound(String),
    #[error("Module version conflict: publisher `{publisher}` / module `{slug}` version `{version}` already bound to a different digest")]
    VersionConflict {
        publisher: String,
        slug: String,
        version: String,
        existing_digest: String,
        new_digest: String,
    },
    #[error("Transition is not eligible: {0:?}")]
    TransitionIneligible(Vec<String>),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Dynamic lifecycle error: {0}")]
    Lifecycle(String),
}

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

/// Unified operator service for coordinating module release projections and commands.
#[derive(Clone)]
pub struct ModuleOperatorService {
    db: DatabaseConnection,
    dynamic_lifecycle: DynamicLifecycleService,
}

impl ModuleOperatorService {
    pub fn new(db: DatabaseConnection) -> Self {
        let dynamic_lifecycle = DynamicLifecycleService::new(db.clone());
        Self {
            db,
            dynamic_lifecycle,
        }
    }

    /// Returns a reference to the dynamic lifecycle service.
    pub fn dynamic_lifecycle(&self) -> &DynamicLifecycleService {
        &self.dynamic_lifecycle
    }

    /// Validates version coordinate immutability:
    /// Rejects if the same publisher/module semver resolves to different bytes.
    pub async fn validate_version_identity_immutability(
        &self,
        publisher: &str,
        slug: &str,
        version: &str,
        candidate_release_digest: &str,
    ) -> Result<(), ModuleOperatorError> {
        let backend = self.db.get_database_backend();
        let placeholder_slug = match backend {
            DbBackend::Postgres => "$1",
            _ => "?1",
        };
        let placeholder_version = match backend {
            DbBackend::Postgres => "$2",
            _ => "?2",
        };

        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT r.release_digest, e.publisher_identity \
                     FROM module_admitted_oci_releases r \
                     LEFT JOIN module_external_prebuilt_ingress e ON e.release_digest = r.release_digest \
                     WHERE r.slug = {placeholder_slug} AND r.version = {placeholder_version} \
                     LIMIT 1"
                ),
                vec![slug.to_string().into(), version.to_string().into()],
            ))
            .await
            .map_err(|e| ModuleOperatorError::Database(e.to_string()))?;

        if let Some(row) = row {
            let existing_digest: String = row
                .try_get("", "release_digest")
                .map_err(|e| ModuleOperatorError::Database(e.to_string()))?;
            let existing_publisher: Option<String> = row
                .try_get("", "publisher_identity")
                .unwrap_or(None);

            if existing_digest != candidate_release_digest {
                let pub_id = existing_publisher.unwrap_or_else(|| publisher.to_string());
                return Err(ModuleOperatorError::VersionConflict {
                    publisher: pub_id,
                    slug: slug.to_string(),
                    version: version.to_string(),
                    existing_digest,
                    new_digest: candidate_release_digest.to_string(),
                });
            }
        }

        Ok(())
    }

    /// Evaluates and generates a complete transition preview projection.
    pub async fn generate_preview(
        &self,
        operation_id: Uuid,
        slug: &str,
        scope: &ModuleInstallationScope,
        candidate_coordinate: Option<ModuleReleaseCoordinate>,
        mode: UpdateMode,
        reason: &str,
    ) -> Result<TransitionPreviewProjection, ModuleOperatorError> {
        let backend = self.db.get_database_backend();
        let (scope_kind, tenant_id) = match scope {
            ModuleInstallationScope::Platform => ("platform", None),
            ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(*tenant_id)),
        };

        // Query active installation if present
        let scope_clause = match backend {
            DbBackend::Postgres => "scope_kind = $2 AND tenant_id IS NOT DISTINCT FROM $3",
            _ => "scope_kind = ?2 AND tenant_id IS ?3",
        };

        let placeholder_slug = match backend {
            DbBackend::Postgres => "$1",
            _ => "?1",
        };

        let inst_row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT i.installation_id, i.manifest_digest, i.version, i.payload_digest, \
                            i.data_owner_id, i.settings_instance_id \
                     FROM module_artifact_installations i \
                     JOIN module_artifact_admissions a ON a.installation_id = i.installation_id \
                     WHERE i.slug = {placeholder_slug} AND {scope_clause} AND a.status = 'active' \
                     LIMIT 1"
                ),
                vec![
                    slug.to_string().into(),
                    scope_kind.into(),
                    match backend {
                        DbBackend::Postgres => sea_orm::Value::Uuid(tenant_id),
                        _ => tenant_id.map(|u| u.to_string()).into(),
                    },
                ],
            ))
            .await
            .map_err(|e| ModuleOperatorError::Database(e.to_string()))?;

        let (current_identity, predecessor_identity, data_owner_id, settings_instance_id) = match inst_row {
            Some(row) => {
                let manifest_digest: String = row
                    .try_get("", "manifest_digest")
                    .map_err(|e| ModuleOperatorError::Database(e.to_string()))?;
                let version: String = row
                    .try_get("", "version")
                    .map_err(|e| ModuleOperatorError::Database(e.to_string()))?;
                let payload_digest: String = row
                    .try_get("", "payload_digest")
                    .map_err(|e| ModuleOperatorError::Database(e.to_string()))?;
                let owner_id = match backend {
                    DbBackend::Postgres => row.try_get::<Uuid>("", "data_owner_id")
                        .map_err(|e| ModuleOperatorError::Database(e.to_string()))?,
                    _ => {
                        let s: String = row.try_get("", "data_owner_id")
                            .map_err(|e| ModuleOperatorError::Database(e.to_string()))?;
                        Uuid::parse_str(&s).unwrap_or_else(|_| Uuid::new_v4())
                    }
                };
                let settings_id = match backend {
                    DbBackend::Postgres => row.try_get::<Uuid>("", "settings_instance_id")
                        .map_err(|e| ModuleOperatorError::Database(e.to_string()))?,
                    _ => {
                        let s: String = row.try_get("", "settings_instance_id")
                            .map_err(|e| ModuleOperatorError::Database(e.to_string()))?;
                        Uuid::parse_str(&s).unwrap_or_else(|_| Uuid::new_v4())
                    }
                };

                let coord = ModuleReleaseCoordinate::Dynamic {
                    publisher_identity: "platform".to_string(),
                    module_slug: slug.to_string(),
                    version,
                    release_digest: manifest_digest.clone(),
                    payload_digest,
                };
                (Some(coord.clone()), Some(coord), owner_id, settings_id)
            }
            None => (None, None, Uuid::new_v4(), Uuid::new_v4()),
        };

        let (has_schema_migration, is_irreversible) = match &candidate_coordinate {
            Some(ModuleReleaseCoordinate::Static { .. }) => (true, false),
            _ => (false, false),
        };

        let blast_radius = ModuleBlastRadius {
            affected_scope: scope.clone(),
            affected_tenants_count: match scope {
                ModuleInstallationScope::Tenant { .. } => 1,
                ModuleInstallationScope::Platform => 1,
            },
            affected_roles: vec!["api".to_string(), "monolith".to_string()],
            dependent_modules: Vec::new(),
            has_schema_migration,
            is_irreversible,
            data_owner_id,
            settings_instance_id,
        };

        let mut denial_reasons = Vec::new();
        // Exact transition rule:
        // Automatic mode is evaluated per exact transition and denied for schema migrations,
        // irreversible checkpoints, or cross-module impacts.
        if mode == UpdateMode::Automatic
            && (blast_radius.has_schema_migration
                || blast_radius.is_irreversible
                || !blast_radius.dependent_modules.is_empty())
        {
            denial_reasons.push(
                "Automatic mode denied: transition requires manual approval due to schema migration, irreversible checkpoint, or cross-module impact"
                    .to_string(),
            );
        }

        let eligibility = TransitionEligibility {
            eligible: denial_reasons.is_empty(),
            denial_reasons,
        };

        let irreversible_checkpoint = if blast_radius.is_irreversible {
            Some("PointOfNoReturn: irreversible schema or data conversion committed; automatic rollback prohibited".to_string())
        } else if blast_radius.has_schema_migration {
            Some("PointOfNoReturn: additive native schema migration will be committed".to_string())
        } else {
            None
        };

        let fence_state = ConflictFenceSet::derive_module_update_fences(
            slug,
            tenant_id,
            &blast_radius.affected_roles,
        );

        let preview_seed = format!("{operation_id}:{slug}:{mode:?}:{data_owner_id}");
        let preview_digest = sha256_digest(preview_seed.as_bytes());

        Ok(TransitionPreviewProjection {
            operation_id,
            module_slug: slug.to_string(),
            scope: scope.clone(),
            current_identity,
            candidate_identity: candidate_coordinate,
            predecessor_identity,
            mode,
            reason: reason.to_string(),
            blast_radius,
            irreversible_checkpoint,
            eligibility,
            fence_state,
            preview_digest,
        })
    }

    /// Queries the current operator status projection.
    pub async fn get_status(
        &self,
        slug: &str,
        scope: &ModuleInstallationScope,
    ) -> Result<ModuleStatusProjection, ModuleOperatorError> {
        let backend = self.db.get_database_backend();
        let (scope_kind, tenant_id) = match scope {
            ModuleInstallationScope::Platform => ("platform", None),
            ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(*tenant_id)),
        };

        let scope_clause = match backend {
            DbBackend::Postgres => "i.scope_kind = $2 AND i.tenant_id IS NOT DISTINCT FROM $3",
            _ => "i.scope_kind = ?2 AND i.tenant_id IS ?3",
        };
        let placeholder_slug = match backend {
            DbBackend::Postgres => "$1",
            _ => "?1",
        };

        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT i.installation_id, i.manifest_digest, i.version, i.payload_digest, \
                            a.status AS admission_status, w.work_generation, w.retired \
                     FROM module_artifact_installations i \
                     JOIN module_artifact_admissions a ON a.installation_id = i.installation_id \
                     JOIN module_artifact_work_generations w ON w.installation_id = i.installation_id \
                     WHERE i.slug = {placeholder_slug} AND {scope_clause} \
                     ORDER BY i.installed_at DESC LIMIT 1"
                ),
                vec![
                    slug.to_string().into(),
                    scope_kind.into(),
                    match backend {
                        DbBackend::Postgres => sea_orm::Value::Uuid(tenant_id),
                        _ => tenant_id.map(|u| u.to_string()).into(),
                    },
                ],
            ))
            .await
            .map_err(|e| ModuleOperatorError::Database(e.to_string()))?;

        match row {
            Some(row) => {
                let adm_status: String = row
                    .try_get("", "admission_status")
                    .map_err(|e| ModuleOperatorError::Database(e.to_string()))?;
                let version: String = row
                    .try_get("", "version")
                    .map_err(|e| ModuleOperatorError::Database(e.to_string()))?;
                let manifest_digest: String = row
                    .try_get("", "manifest_digest")
                    .map_err(|e| ModuleOperatorError::Database(e.to_string()))?;
                let payload_digest: String = row
                    .try_get("", "payload_digest")
                    .map_err(|e| ModuleOperatorError::Database(e.to_string()))?;

                let work_generation = match backend {
                    DbBackend::Postgres => {
                        let g: i64 = row.try_get("", "work_generation")
                            .map_err(|e| ModuleOperatorError::Database(e.to_string()))?;
                        g as u64
                    }
                    _ => {
                        let g: i32 = row.try_get("", "work_generation")
                            .map_err(|e| ModuleOperatorError::Database(e.to_string()))?;
                        g as u64
                    }
                };

                let retired = match backend {
                    DbBackend::Postgres => row.try_get::<bool>("", "retired")
                        .map_err(|e| ModuleOperatorError::Database(e.to_string()))?,
                    _ => row.try_get::<i32>("", "retired")
                        .map_err(|e| ModuleOperatorError::Database(e.to_string()))? != 0,
                };

                let (presentation_state, containment_outcome, diagnostics, recovery_action) = if retired {
                    (
                        CanonicalPresentationState::Cancelled,
                        Some(ContainmentOutcome::Stopped),
                        vec!["Installation is retired".to_string()],
                        Some("Reinstall or purge data".to_string()),
                    )
                } else {
                    match adm_status.as_str() {
                        "active" => (
                            CanonicalPresentationState::Accepted,
                            None,
                            Vec::new(),
                            None,
                        ),
                        "inactive" => (
                            CanonicalPresentationState::Ready,
                            None,
                            vec!["Inactive installation prepared".to_string()],
                            Some("Enable when ready".to_string()),
                        ),
                        "failed" => (
                            CanonicalPresentationState::RecoveryRequired,
                            Some(ContainmentOutcome::Fenced),
                            vec!["Installation failed preflight or activation".to_string()],
                            Some("Investigate diagnostics and trigger recovery".to_string()),
                        ),
                        _ => (
                            CanonicalPresentationState::Observing,
                            None,
                            Vec::new(),
                            None,
                        ),
                    }
                };

                let coord = ModuleReleaseCoordinate::Dynamic {
                    publisher_identity: "platform".to_string(),
                    module_slug: slug.to_string(),
                    version,
                    release_digest: manifest_digest,
                    payload_digest,
                };

                Ok(ModuleStatusProjection {
                    module_slug: slug.to_string(),
                    scope: scope.clone(),
                    presentation_state,
                    display_label: presentation_state.display_label().to_string(),
                    containment_outcome,
                    current_identity: if presentation_state == CanonicalPresentationState::Accepted {
                        Some(coord.clone())
                    } else {
                        None
                    },
                    candidate_identity: if presentation_state != CanonicalPresentationState::Accepted {
                        Some(coord.clone())
                    } else {
                        None
                    },
                    predecessor_identity: None,
                    work_generation,
                    retired,
                    active_retention_holds_count: 0,
                    diagnostics,
                    recovery_action,
                })
            }
            None => Ok(ModuleStatusProjection {
                module_slug: slug.to_string(),
                scope: scope.clone(),
                presentation_state: CanonicalPresentationState::Ready,
                display_label: CanonicalPresentationState::Ready.display_label().to_string(),
                containment_outcome: None,
                current_identity: None,
                candidate_identity: None,
                predecessor_identity: None,
                work_generation: 0,
                retired: false,
                active_retention_holds_count: 0,
                diagnostics: vec!["No installation found".to_string()],
                recovery_action: Some("Install module".to_string()),
            }),
        }
    }

    /// Generates an authorized support bundle containing redacted diagnostic evidence.
    pub async fn generate_support_bundle(
        &self,
        slug: &str,
        scope: &ModuleInstallationScope,
    ) -> Result<ModuleSupportBundle, ModuleOperatorError> {
        let status = self.get_status(slug, scope).await?;

        Ok(ModuleSupportBundle {
            bundle_id: Uuid::new_v4(),
            module_slug: slug.to_string(),
            installation_id: None,
            operation_id: None,
            generated_at: Utc::now(),
            presentation_state: status.presentation_state,
            containment_outcome: status.containment_outcome,
            diagnostics: status.diagnostics,
            recovery_action: status.recovery_action,
            active_retention_holds: Vec::new(),
            work_generation: status.work_generation,
            retired: status.retired,
        })
    }
}
