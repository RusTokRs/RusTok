//! Operator read-model projections for the module release and rollback lifecycle.
//!
//! This module is a read-only projection layer over canonical control-plane
//! state. It executes no lifecycle commands: submission, admission,
//! installation, enable/disable, rollback, finalization, and purge effects are
//! owned by the canonical services in `installation`, `lifecycle`,
//! `transition_service`, and `data`. Every projected value is derived from
//! those owners' tables (`module_artifact_installations`,
//! `module_artifact_admissions`, `module_admitted_oci_releases`,
//! `module_external_prebuilt_ingress`, `module_artifact_work_generations`,
//! `module_retention_holds`, `module_transition_checkpoints`,
//! `module_artifact_node_reconciliation_assignments`); where no canonical
//! source exists, the projection reports `None` or an empty collection
//! instead of an invented value.
//!
//! Status (2026-10-21): not wired into any transport. A caller that exposes
//! these projections must authorize the request first; the support bundle in
//! particular aggregates diagnostics and must not be treated as
//! pre-authorized.

use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, QueryResult, Statement, TransactionTrait};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::backend::SupportedBackend;
use crate::installation::sha256_digest;
use crate::transition_store::RetentionHoldStore;
use crate::{
    ConflictFenceSet, ModuleArtifactDescriptor, ModuleInstallationScope, RetentionHoldRecord,
    UpdateMode,
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

/// Exact release coordinate identifying either a dynamic artifact or a static
/// distribution composition.
///
/// For dynamic artifacts, `release_digest` is the admitted release (OCI
/// manifest) digest recorded on the installation, and `publisher_identity` is
/// the externally attested publisher from `module_external_prebuilt_ingress`.
/// It is `None` for internally built releases, which carry no ingress record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModuleReleaseCoordinate {
    Dynamic {
        publisher_identity: Option<String>,
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
///
/// Every field is derived from canonical state:
/// - `affected_tenants_count`: for a tenant-scoped transition, exactly the
///   targeted tenant (1); for a platform-scoped preview, the number of
///   distinct tenants that have their own installation of the module.
/// - `affected_nodes`: distinct node identifiers holding a reconciliation
///   assignment for the active installation (empty before any assignment).
/// - `dependent_modules`: slugs of other installations in the same scope
///   whose admitted descriptor declares a dependency on this module.
/// - `has_schema_migration`: comparison of the active and candidate admitted
///   descriptors' data-schema fingerprints; `true` by policy for static
///   candidates, whose native migrations cannot be introspected here.
/// - `data_owner_id` / `settings_instance_id`: the active installation's
///   owners; `None` when nothing is installed yet.
///
/// Irreversibility is deliberately not projected here: it is an operation
/// property derived from live evidence by
/// `data_upgrade::evaluate_data_upgrade_decision`, not a descriptor fact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleBlastRadius {
    pub affected_scope: ModuleInstallationScope,
    pub affected_tenants_count: u64,
    pub affected_nodes: Vec<String>,
    pub dependent_modules: Vec<String>,
    pub has_schema_migration: bool,
    pub data_owner_id: Option<Uuid>,
    pub settings_instance_id: Option<Uuid>,
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
    pub point_of_no_return: Option<String>,
    pub eligibility: TransitionEligibility,
    pub fence_state: ConflictFenceSet,
    pub preview_digest: String,
}

/// Operator status projection for runtime and dashboard presentation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStatusProjection {
    pub module_slug: String,
    pub scope: ModuleInstallationScope,
    pub installation_id: Option<Uuid>,
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

/// Support bundle aggregating redacted diagnostic evidence for incident
/// handling. Assembling a bundle performs no authorization; the exposing
/// caller must authorize the request before returning it.
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
    #[error(
        "Module version conflict: publisher `{publisher}` / module `{slug}` version `{version}` already bound to a different digest"
    )]
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
}

fn database_error(error: impl std::fmt::Display) -> ModuleOperatorError {
    ModuleOperatorError::Database(error.to_string())
}

fn scope_parts(scope: &ModuleInstallationScope) -> (&'static str, Option<Uuid>) {
    match scope {
        ModuleInstallationScope::Platform => ("platform", None),
        ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(*tenant_id)),
    }
}

fn scope_key(scope: &ModuleInstallationScope) -> (&'static str, String) {
    let (kind, tenant) = scope_parts(scope);
    (
        kind,
        tenant.map_or_else(|| "platform".to_string(), |id| id.to_string()),
    )
}

/// SQL fragment matching the nullable tenant column against an optional
/// tenant value, for `tenant_id` columns expressed without a table alias.
fn tenant_clause(backend: SupportedBackend, column: &str, index: usize) -> String {
    match backend {
        SupportedBackend::Postgres => format!("{column} IS NOT DISTINCT FROM ${index}"),
        SupportedBackend::Sqlite => format!("{column} IS ?{index}"),
    }
}

/// Reads a `NOT NULL` UUID column. A malformed stored value is a typed
/// database error; projections never substitute a fabricated identifier.
fn uuid_column(
    row: &QueryResult,
    backend: SupportedBackend,
    column: &str,
) -> Result<Uuid, ModuleOperatorError> {
    backend.uuid_from_row(row, column).map_err(database_error)
}

fn parse_descriptor(
    raw: &str,
    context: &str,
) -> Result<ModuleArtifactDescriptor, ModuleOperatorError> {
    serde_json::from_str(raw).map_err(|e| {
        ModuleOperatorError::Database(format!("stored descriptor for {context} is invalid: {e}"))
    })
}

/// Data-schema fingerprint of an admitted descriptor: the persistence
/// contract schema digest and the declared data schema digest. A change in
/// either means live data may require conversion.
fn data_schema_fingerprint(descriptor: &ModuleArtifactDescriptor) -> (Option<&str>, Option<&str>) {
    (
        descriptor
            .persistence_contract
            .as_ref()
            .map(|contract| contract.schema_digest.as_str()),
        descriptor.data_schema_digest.as_deref(),
    )
}

fn coordinate_digest(coordinate: &Option<ModuleReleaseCoordinate>) -> &str {
    match coordinate {
        Some(ModuleReleaseCoordinate::Dynamic { release_digest, .. }) => release_digest.as_str(),
        Some(ModuleReleaseCoordinate::Static {
            bundle_root_digest, ..
        }) => bundle_root_digest.as_str(),
        None => "none",
    }
}

/// One installation row joined with its admission status, newest first.
struct InstallationRow {
    installation_id: Uuid,
    version: String,
    manifest_digest: String,
    payload_digest: String,
    data_owner_id: Uuid,
    settings_instance_id: Uuid,
    descriptor: ModuleArtifactDescriptor,
    admission_status: String,
}

impl InstallationRow {
    fn from_row(row: &QueryResult, backend: SupportedBackend) -> Result<Self, ModuleOperatorError> {
        let descriptor_json: String = row.try_get("", "descriptor_json").map_err(database_error)?;
        let manifest_digest: String = row.try_get("", "manifest_digest").map_err(database_error)?;
        Ok(Self {
            installation_id: uuid_column(row, backend, "installation_id")?,
            version: row.try_get("", "version").map_err(database_error)?,
            descriptor: parse_descriptor(
                &descriptor_json,
                &format!("installation with manifest digest {manifest_digest}"),
            )?,
            manifest_digest,
            payload_digest: row.try_get("", "payload_digest").map_err(database_error)?,
            data_owner_id: uuid_column(row, backend, "data_owner_id")?,
            settings_instance_id: uuid_column(row, backend, "settings_instance_id")?,
            admission_status: row
                .try_get("", "admission_status")
                .map_err(database_error)?,
        })
    }
}

/// Read-only operator projection service over canonical module state.
#[derive(Clone)]
pub struct ModuleOperatorService {
    db: DatabaseConnection,
}

impl ModuleOperatorService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Validates version coordinate immutability against admitted releases:
    /// rejects when the same module slug and version were already admitted
    /// with different bytes. Namespace ownership admits a single publisher
    /// per slug, so identity is checked on `(slug, version)`; the reported
    /// publisher is the attested ingress publisher when one exists.
    pub async fn validate_version_identity_immutability(
        &self,
        scope: &ModuleInstallationScope,
        publisher: &str,
        slug: &str,
        version: &str,
        candidate_release_digest: &str,
    ) -> Result<(), ModuleOperatorError> {
        let backend =
            SupportedBackend::new(self.db.get_database_backend()).map_err(database_error)?;
        let (scope_kind, scope_tenant_key) = scope_key(scope);
        let tx = self.db.begin().await.map_err(database_error)?;
        crate::backend::set_installation_scope(&tx, scope)
            .await
            .map_err(database_error)?;
        let row = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT r.release_digest, e.publisher_identity \
                     FROM module_admitted_oci_releases r \
                     LEFT JOIN module_external_prebuilt_ingress e ON e.release_digest = r.release_digest \
                         AND e.scope_kind = r.scope_kind AND e.scope_tenant_key = r.scope_tenant_key \
                     WHERE r.slug = {} AND r.version = {} AND r.release_digest <> {} \
                       AND r.scope_kind = {} AND r.scope_tenant_key = {} \
                     LIMIT 1",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                    backend.placeholder(5),
                ),
                vec![
                    slug.to_string().into(),
                    version.to_string().into(),
                    candidate_release_digest.to_string().into(),
                    scope_kind.into(),
                    scope_tenant_key.into(),
                ],
            ))
            .await
            .map_err(database_error)?;

        if let Some(row) = row {
            let existing_digest: String =
                row.try_get("", "release_digest").map_err(database_error)?;
            let existing_publisher: Option<String> = row
                .try_get("", "publisher_identity")
                .map_err(database_error)?;
            return Err(ModuleOperatorError::VersionConflict {
                publisher: existing_publisher.unwrap_or_else(|| publisher.to_string()),
                slug: slug.to_string(),
                version: version.to_string(),
                existing_digest,
                new_digest: candidate_release_digest.to_string(),
            });
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
        let chain = self.load_installation_chain(slug, scope).await?;
        let current_index = chain
            .iter()
            .position(|row| row.admission_status == "active");
        let current_row = current_index.map(|index| &chain[index]);
        let predecessor_row = current_index.and_then(|index| chain.get(index + 1));

        let current_identity = match current_row {
            Some(row) => Some(self.dynamic_coordinate(slug, row, scope).await?),
            None => None,
        };
        let predecessor_identity = match predecessor_row {
            Some(row) => Some(self.dynamic_coordinate(slug, row, scope).await?),
            None => None,
        };

        // Schema-migration derivation. Static candidates are always treated
        // as schema migrations: native distribution migrations cannot be
        // introspected from admitted descriptors, so manual approval is the
        // policy. Dynamic candidates compare admitted-descriptor data-schema
        // fingerprints; when the active installation owns a data schema and
        // the candidate was never admitted, compatibility is unverifiable
        // and automatic mode fails closed.
        let mut has_schema_migration = false;
        let mut schema_unverifiable_digest: Option<&str> = None;
        match &candidate_coordinate {
            Some(ModuleReleaseCoordinate::Static { .. }) => {
                has_schema_migration = true;
            }
            Some(ModuleReleaseCoordinate::Dynamic { release_digest, .. }) => {
                if let Some(current) = current_row {
                    let current_fingerprint = data_schema_fingerprint(&current.descriptor);
                    match self.candidate_descriptor(release_digest, scope).await? {
                        Some(candidate) => {
                            has_schema_migration =
                                data_schema_fingerprint(&candidate) != current_fingerprint;
                        }
                        None => {
                            if current_fingerprint != (None, None) {
                                schema_unverifiable_digest = Some(release_digest.as_str());
                            }
                        }
                    }
                }
            }
            None => {}
        }

        let dependent_modules = self.dependent_modules(slug, scope).await?;
        let affected_nodes = self
            .affected_nodes(current_row.map(|row| row.installation_id))
            .await?;
        let blast_radius = ModuleBlastRadius {
            affected_scope: scope.clone(),
            affected_tenants_count: self.affected_tenants_count(slug, scope).await?,
            affected_nodes,
            dependent_modules,
            has_schema_migration,
            data_owner_id: current_row.map(|row| row.data_owner_id),
            settings_instance_id: current_row.map(|row| row.settings_instance_id),
        };

        let mut denial_reasons = Vec::new();
        // Exact transition rule: automatic mode is denied per exact transition
        // for data-schema migrations, cross-module impact, or when schema
        // compatibility cannot be verified from an admitted descriptor.
        if mode == UpdateMode::Automatic
            && (blast_radius.has_schema_migration || !blast_radius.dependent_modules.is_empty())
        {
            denial_reasons.push(
                "Automatic mode denied: transition requires manual approval due to data-schema migration or cross-module impact"
                    .to_string(),
            );
        }
        if mode == UpdateMode::Automatic
            && let Some(release_digest) = schema_unverifiable_digest
        {
            denial_reasons.push(format!(
                "Automatic mode denied: candidate release {release_digest} has no admitted descriptor; data-schema compatibility with the active installation cannot be verified"
            ));
        }
        let eligibility = TransitionEligibility {
            eligible: denial_reasons.is_empty(),
            denial_reasons,
        };

        let point_of_no_return = if blast_radius.has_schema_migration {
            Some(match &candidate_coordinate {
                Some(ModuleReleaseCoordinate::Static { .. }) => "PointOfNoReturn: static distribution updates may commit native schema migrations that cannot be introspected from admitted descriptors; manual approval is required".to_string(),
                _ => "PointOfNoReturn: candidate changes the admitted data-schema fingerprint of the active installation; live data conversion may be required".to_string(),
            })
        } else {
            None
        };

        let (_, tenant_id) = scope_parts(scope);
        let fence_state = ConflictFenceSet::derive_module_update_fences(
            slug,
            tenant_id,
            &blast_radius.affected_nodes,
        );

        let preview_seed = format!(
            "{operation_id}:{slug}:{mode:?}:{}:{}",
            coordinate_digest(&current_identity),
            coordinate_digest(&candidate_coordinate),
        );
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
            point_of_no_return,
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
        let chain = self.load_installation_chain(slug, scope).await?;
        let Some(latest) = chain.first() else {
            return Ok(ModuleStatusProjection {
                module_slug: slug.to_string(),
                scope: scope.clone(),
                installation_id: None,
                presentation_state: CanonicalPresentationState::Ready,
                display_label: CanonicalPresentationState::Ready
                    .display_label()
                    .to_string(),
                containment_outcome: None,
                current_identity: None,
                candidate_identity: None,
                predecessor_identity: None,
                work_generation: 0,
                retired: false,
                active_retention_holds_count: 0,
                diagnostics: vec!["No installation found".to_string()],
                recovery_action: Some("Install module".to_string()),
            });
        };

        // The serving release is the newest `active` installation; the
        // newest installation overall may instead be a staged candidate,
        // an inactive preparation, or a rolled-back candidate. Projecting
        // only the newest row mislabels a healthy serving predecessor
        // whenever a newer candidate exists, and any admission status
        // outside the real vocabulary (admitted, installed, inactive,
        // active, rolled_back) must surface as recovery-required rather
        // than a silent in-flight state.
        let serving_index = chain
            .iter()
            .position(|row| row.admission_status == "active");
        let basis = serving_index.map_or(latest, |index| &chain[index]);
        let (work_generation, retired) = self.work_generation(basis.installation_id).await?;

        let (
            presentation_state,
            containment_outcome,
            diagnostics,
            recovery_action,
            current_index,
            candidate_index,
        ) = if retired {
            (
                CanonicalPresentationState::Cancelled,
                Some(ContainmentOutcome::Stopped),
                vec!["Installation is retired".to_string()],
                Some("Reinstall or purge data".to_string()),
                None,
                None,
            )
        } else {
            match latest.admission_status.as_str() {
                "active" => (
                    CanonicalPresentationState::Accepted,
                    None,
                    Vec::new(),
                    None,
                    Some(0),
                    None,
                ),
                "inactive" => match serving_index {
                    Some(index) => (
                        CanonicalPresentationState::Accepted,
                        None,
                        vec!["Newer inactive installation prepared".to_string()],
                        None,
                        Some(index),
                        Some(0),
                    ),
                    None => (
                        CanonicalPresentationState::Ready,
                        None,
                        vec!["Inactive installation prepared".to_string()],
                        Some("Enable when ready".to_string()),
                        None,
                        Some(0),
                    ),
                },
                "admitted" | "installed" => (
                    CanonicalPresentationState::Running,
                    None,
                    Vec::new(),
                    None,
                    serving_index,
                    Some(0),
                ),
                "rolled_back" => match serving_index {
                    Some(index) => (
                        CanonicalPresentationState::Recovered,
                        None,
                        vec!["Candidate was rolled back; predecessor is serving".to_string()],
                        None,
                        Some(index),
                        None,
                    ),
                    None => (
                        CanonicalPresentationState::RecoveryRequired,
                        Some(ContainmentOutcome::Fenced),
                        vec![
                            "Installation was rolled back with no serving predecessor".to_string(),
                        ],
                        Some("Investigate diagnostics and trigger recovery".to_string()),
                        None,
                        None,
                    ),
                },
                other => (
                    CanonicalPresentationState::RecoveryRequired,
                    Some(ContainmentOutcome::Fenced),
                    vec![format!("Unknown admission status '{other}'")],
                    Some("Investigate diagnostics and trigger recovery".to_string()),
                    serving_index,
                    None,
                ),
            }
        };

        // A retired newest installation (uninstalled candidate) is not a
        // pending candidate even while an older release keeps serving.
        let candidate_index = match candidate_index {
            Some(0) if serving_index.is_some() => {
                let (_, latest_retired) = self.work_generation(latest.installation_id).await?;
                if latest_retired { None } else { Some(0) }
            }
            other => other,
        };

        let current_identity = match current_index {
            Some(index) => Some(self.dynamic_coordinate(slug, &chain[index], scope).await?),
            None => None,
        };
        let candidate_identity = match candidate_index {
            Some(index) => Some(self.dynamic_coordinate(slug, &chain[index], scope).await?),
            None => None,
        };
        let predecessor_index = match current_index {
            Some(index) => (index + 1 < chain.len()).then_some(index + 1),
            None => (chain.len() > 1).then_some(1),
        };
        let predecessor_identity = match predecessor_index {
            Some(index) => Some(self.dynamic_coordinate(slug, &chain[index], scope).await?),
            None => None,
        };
        let active_retention_holds_count = self.active_holds_for(basis).await?.len();

        Ok(ModuleStatusProjection {
            module_slug: slug.to_string(),
            scope: scope.clone(),
            installation_id: Some(latest.installation_id),
            presentation_state,
            display_label: presentation_state.display_label().to_string(),
            containment_outcome,
            current_identity,
            candidate_identity,
            predecessor_identity,
            work_generation,
            retired,
            active_retention_holds_count,
            diagnostics,
            recovery_action,
        })
    }

    /// Assembles a support bundle with redacted diagnostic evidence. The
    /// caller is responsible for authorizing access before exposing it.
    pub async fn generate_support_bundle(
        &self,
        slug: &str,
        scope: &ModuleInstallationScope,
    ) -> Result<ModuleSupportBundle, ModuleOperatorError> {
        let status = self.get_status(slug, scope).await?;
        let chain = self.load_installation_chain(slug, scope).await?;
        let active_retention_holds = match chain.first() {
            Some(row) => self.active_holds_for(row).await?,
            None => Vec::new(),
        };
        let operation_id = self.latest_operation_id(slug, scope).await?;

        Ok(ModuleSupportBundle {
            bundle_id: Uuid::new_v4(),
            module_slug: slug.to_string(),
            installation_id: status.installation_id,
            operation_id,
            generated_at: Utc::now(),
            presentation_state: status.presentation_state,
            containment_outcome: status.containment_outcome,
            diagnostics: status.diagnostics,
            recovery_action: status.recovery_action,
            active_retention_holds,
            work_generation: status.work_generation,
            retired: status.retired,
        })
    }

    /// Loads the installation chain for a slug in one scope, newest first.
    async fn load_installation_chain(
        &self,
        slug: &str,
        scope: &ModuleInstallationScope,
    ) -> Result<Vec<InstallationRow>, ModuleOperatorError> {
        let backend =
            SupportedBackend::new(self.db.get_database_backend()).map_err(database_error)?;
        let (scope_kind, tenant_id) = scope_parts(scope);
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT i.installation_id, i.version, i.manifest_digest, i.payload_digest, \
                            i.data_owner_id, i.settings_instance_id, \
                            CAST(i.descriptor AS TEXT) AS descriptor_json, \
                            a.status AS admission_status \
                     FROM module_artifact_installations i \
                     JOIN module_artifact_admissions a ON a.installation_id = i.installation_id \
                     WHERE i.slug = {} AND i.scope_kind = {} AND {} \
                     ORDER BY i.installed_at DESC, i.installation_id DESC",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    tenant_clause(backend, "i.tenant_id", 3),
                ),
                vec![
                    slug.to_string().into(),
                    scope_kind.into(),
                    backend.optional_uuid_value(tenant_id),
                ],
            ))
            .await
            .map_err(database_error)?;

        rows.iter()
            .map(|row| InstallationRow::from_row(row, backend))
            .collect()
    }

    /// Resolves the externally attested publisher of an admitted release, if
    /// the release arrived through external prebuilt ingress.
    async fn publisher_for_release(
        &self,
        release_digest: &str,
        scope: &ModuleInstallationScope,
    ) -> Result<Option<String>, ModuleOperatorError> {
        let backend =
            SupportedBackend::new(self.db.get_database_backend()).map_err(database_error)?;
        let (scope_kind, scope_tenant_key) = scope_key(scope);
        let tx = self.db.begin().await.map_err(database_error)?;
        crate::backend::set_installation_scope(&tx, scope)
            .await
            .map_err(database_error)?;
        let row = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT publisher_identity FROM module_external_prebuilt_ingress \
                     WHERE release_digest = {} AND scope_kind = {} AND scope_tenant_key = {}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                ),
                vec![
                    release_digest.to_string().into(),
                    scope_kind.into(),
                    scope_tenant_key.into(),
                ],
            ))
            .await
            .map_err(database_error)?;

        match row {
            Some(row) => row
                .try_get("", "publisher_identity")
                .map_err(database_error),
            None => Ok(None),
        }
    }

    async fn dynamic_coordinate(
        &self,
        slug: &str,
        row: &InstallationRow,
        scope: &ModuleInstallationScope,
    ) -> Result<ModuleReleaseCoordinate, ModuleOperatorError> {
        Ok(ModuleReleaseCoordinate::Dynamic {
            publisher_identity: self
                .publisher_for_release(&row.manifest_digest, scope)
                .await?,
            module_slug: slug.to_string(),
            version: row.version.clone(),
            release_digest: row.manifest_digest.clone(),
            payload_digest: row.payload_digest.clone(),
        })
    }

    /// Loads the admitted descriptor of a candidate release by digest.
    async fn candidate_descriptor(
        &self,
        release_digest: &str,
        scope: &ModuleInstallationScope,
    ) -> Result<Option<ModuleArtifactDescriptor>, ModuleOperatorError> {
        let backend =
            SupportedBackend::new(self.db.get_database_backend()).map_err(database_error)?;
        let (scope_kind, scope_tenant_key) = scope_key(scope);
        let tx = self.db.begin().await.map_err(database_error)?;
        crate::backend::set_installation_scope(&tx, scope)
            .await
            .map_err(database_error)?;
        let row = tx
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT CAST(descriptor_json AS TEXT) AS descriptor_json \
                     FROM module_admitted_oci_releases WHERE release_digest = {} AND scope_kind = {} AND scope_tenant_key = {}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                ),
                vec![release_digest.to_string().into(), scope_kind.into(), scope_tenant_key.into()],
            ))
            .await
            .map_err(database_error)?;

        match row {
            Some(row) => {
                let raw: String = row.try_get("", "descriptor_json").map_err(database_error)?;
                parse_descriptor(&raw, &format!("admitted release {release_digest}")).map(Some)
            }
            None => Ok(None),
        }
    }

    /// Slugs of other installations in the scope whose admitted descriptor
    /// declares a dependency on `slug`.
    async fn dependent_modules(
        &self,
        slug: &str,
        scope: &ModuleInstallationScope,
    ) -> Result<Vec<String>, ModuleOperatorError> {
        let backend =
            SupportedBackend::new(self.db.get_database_backend()).map_err(database_error)?;
        let (scope_kind, tenant_id) = scope_parts(scope);
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT slug, CAST(descriptor AS TEXT) AS descriptor_json \
                     FROM module_artifact_installations \
                     WHERE scope_kind = {} AND {} AND slug <> {}",
                    backend.placeholder(1),
                    tenant_clause(backend, "tenant_id", 2),
                    backend.placeholder(3),
                ),
                vec![
                    scope_kind.into(),
                    backend.optional_uuid_value(tenant_id),
                    slug.to_string().into(),
                ],
            ))
            .await
            .map_err(database_error)?;

        let mut dependents = Vec::new();
        for row in &rows {
            let dependent_slug: String = row.try_get("", "slug").map_err(database_error)?;
            let raw: String = row.try_get("", "descriptor_json").map_err(database_error)?;
            let descriptor = parse_descriptor(&raw, &format!("installation of {dependent_slug}"))?;
            if descriptor
                .dependencies
                .iter()
                .any(|dependency| dependency.slug == slug)
            {
                dependents.push(dependent_slug);
            }
        }
        dependents.sort();
        dependents.dedup();
        Ok(dependents)
    }

    /// Distinct nodes holding a reconciliation assignment for an installation.
    async fn affected_nodes(
        &self,
        installation_id: Option<Uuid>,
    ) -> Result<Vec<String>, ModuleOperatorError> {
        let Some(installation_id) = installation_id else {
            return Ok(Vec::new());
        };
        let backend =
            SupportedBackend::new(self.db.get_database_backend()).map_err(database_error)?;
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT DISTINCT node_id \
                     FROM module_artifact_node_reconciliation_assignments \
                     WHERE installation_id = {}",
                    backend.placeholder(1),
                ),
                vec![backend.uuid_value(installation_id)],
            ))
            .await
            .map_err(database_error)?;

        let mut nodes = Vec::with_capacity(rows.len());
        for row in &rows {
            nodes.push(uuid_column(row, backend, "node_id")?.to_string());
        }
        nodes.sort();
        Ok(nodes)
    }

    async fn affected_tenants_count(
        &self,
        slug: &str,
        scope: &ModuleInstallationScope,
    ) -> Result<u64, ModuleOperatorError> {
        match scope {
            // A tenant-scoped transition targets exactly one tenant.
            ModuleInstallationScope::Tenant { .. } => Ok(1),
            ModuleInstallationScope::Platform => {
                let backend = SupportedBackend::new(self.db.get_database_backend())
                    .map_err(database_error)?;
                let row = self
                    .db
                    .query_one_raw(Statement::from_sql_and_values(
                        backend.db_backend(),
                        format!(
                            "SELECT COUNT(DISTINCT tenant_id) AS affected \
                             FROM module_artifact_installations \
                             WHERE slug = {} AND scope_kind = 'tenant'",
                            backend.placeholder(1),
                        ),
                        vec![slug.to_string().into()],
                    ))
                    .await
                    .map_err(database_error)?
                    .ok_or_else(|| {
                        ModuleOperatorError::Database(
                            "tenant count query returned no row".to_string(),
                        )
                    })?;
                let count: i64 = row.try_get("", "affected").map_err(database_error)?;
                u64::try_from(count).map_err(database_error)
            }
        }
    }

    /// Work generation and retirement flag of an installation. A missing
    /// generation row means the installation predates generation tracking at
    /// admission time; canonically it is generation 1 and not retired.
    async fn work_generation(
        &self,
        installation_id: Uuid,
    ) -> Result<(u64, bool), ModuleOperatorError> {
        let backend =
            SupportedBackend::new(self.db.get_database_backend()).map_err(database_error)?;
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT work_generation, retired FROM module_artifact_work_generations \
                     WHERE installation_id = {}",
                    backend.placeholder(1),
                ),
                vec![backend.uuid_value(installation_id)],
            ))
            .await
            .map_err(database_error)?;

        let Some(row) = row else {
            return Ok((1, false));
        };
        let generation: i64 = match backend {
            SupportedBackend::Postgres => {
                row.try_get("", "work_generation").map_err(database_error)?
            }
            SupportedBackend::Sqlite => {
                let value: i32 = row.try_get("", "work_generation").map_err(database_error)?;
                i64::from(value)
            }
        };
        let retired = match backend {
            SupportedBackend::Postgres => {
                row.try_get::<bool>("", "retired").map_err(database_error)?
            }
            SupportedBackend::Sqlite => {
                row.try_get::<i32>("", "retired").map_err(database_error)? != 0
            }
        };
        Ok((u64::try_from(generation).map_err(database_error)?, retired))
    }

    /// Active retention holds targeting the installation's manifest or
    /// payload digests, from the canonical durable hold store.
    async fn active_holds_for(
        &self,
        row: &InstallationRow,
    ) -> Result<Vec<RetentionHoldRecord>, ModuleOperatorError> {
        let holds = RetentionHoldStore::list_active_holds(&self.db)
            .await
            .map_err(database_error)?;
        Ok(holds
            .into_iter()
            .filter(|hold| {
                let (target_type, target_identity) = hold.target.identity_key();
                (target_type == "oci_manifest" && target_identity == row.manifest_digest)
                    || (target_type == "payload_cas" && target_identity == row.payload_digest)
            })
            .collect())
    }

    /// Most recent transition operation for the slug in the scope, from the
    /// durable checkpoint ledger.
    async fn latest_operation_id(
        &self,
        slug: &str,
        scope: &ModuleInstallationScope,
    ) -> Result<Option<Uuid>, ModuleOperatorError> {
        let backend =
            SupportedBackend::new(self.db.get_database_backend()).map_err(database_error)?;
        let (_, tenant_id) = scope_parts(scope);
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT operation_id FROM module_transition_checkpoints \
                     WHERE module_slug = {} AND {} \
                     ORDER BY created_at DESC, operation_id DESC LIMIT 1",
                    backend.placeholder(1),
                    tenant_clause(backend, "tenant_id", 2),
                ),
                vec![
                    slug.to_string().into(),
                    backend.optional_uuid_value(tenant_id),
                ],
            ))
            .await
            .map_err(database_error)?;

        match row {
            Some(row) => Ok(Some(uuid_column(&row, backend, "operation_id")?)),
            None => Ok(None),
        }
    }
}
