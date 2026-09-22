use rustok_api::manifest_hash::{canonical_manifest_snapshot_json, hash_manifest_snapshot};
use sea_orm::{DatabaseConnection, DatabaseTransaction, DbErr};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::modules::{
    InstalledManifestModule, ManifestDiff, ManifestError, ManifestManager, ModulesManifest,
};
use rustok_api::{ModuleCompositionSnapshotView, PortError, StaticInstalledModuleView};
use rustok_build::build::Model as Build;
use rustok_build::{BuildEventPublisher, BuildRequest, BuildService};
use rustok_modules::{
    ModuleCommandContext, ModuleCompositionBuildAdmission, ModuleCompositionBuildEnqueueResult,
    ModuleCompositionBuildEnqueuer, ModuleCompositionBuildLease, ModuleCompositionError,
    ModuleCompositionOperation, ModuleCompositionSnapshot, ModuleCompositionUpdate,
    ModuleControlPlane, ModuleDefinitionError, SeaOrmModuleCompositionService,
    SharedStaticInstalledModuleReader, StaticInstalledModuleReader,
    StaticInstalledModuleReaderError,
};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCompositionSnapshot {
    pub revision: i64,
    pub manifest_hash: String,
    pub manifest: ModulesManifest,
}

impl From<PlatformCompositionSnapshot> for ModuleCompositionSnapshotView {
    fn from(snapshot: PlatformCompositionSnapshot) -> Self {
        Self {
            revision: snapshot.revision,
        }
    }
}

#[derive(Debug, Error)]
pub enum PlatformCompositionError {
    #[error(transparent)]
    Owner(#[from] ModuleCompositionError),
    #[error(transparent)]
    Definition(#[from] ModuleDefinitionError),
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error("Failed to serialize platform manifest: {0}")]
    Serialize(String),
    #[error("Failed to deserialize platform manifest: {0}")]
    Deserialize(String),
    #[error("module effective-policy resolution failed: {0}")]
    EffectivePolicy(String),
    #[error("Platform manifest revision conflict: expected {expected}, current {current}")]
    RevisionConflict { expected: i64, current: i64 },
}

#[derive(Debug, Error)]
pub enum PlatformCompositionBuildError {
    #[error(transparent)]
    Composition(#[from] PlatformCompositionError),
    #[error("Failed to enqueue build: {0}")]
    Build(String),
}

pub struct PlatformCompositionBuildResult {
    pub snapshot: PlatformCompositionSnapshot,
    pub build: Build,
    pub replayed: bool,
}

/// Host-authenticated intent to change the active platform-native module set.
/// The composition adapter obtains the durable snapshot, applies the static
/// manifest adapter, and delegates the revision-guarded write to the owner.
#[derive(Debug, Clone, Serialize)]
pub enum PlatformCompositionModuleChange {
    Install {
        module_slug: String,
        version: String,
    },
    Uninstall {
        module_slug: String,
    },
    Upgrade {
        module_slug: String,
        version: String,
    },
}

/// Context supplied by an authenticated host transport. It intentionally
/// carries no manifest or digest: those are derived by the platform adapter
/// and composition owner respectively.
#[derive(Debug, Clone)]
pub struct PlatformCompositionModuleMutation {
    pub context: ModuleCommandContext,
    pub expected_revision: i64,
    pub change: PlatformCompositionModuleChange,
}

pub struct PlatformCompositionBuildService;

pub struct PlatformCompositionBuildCommand {
    pub context: ModuleCommandContext,
    pub expected_revision: i64,
    pub manifest: ModulesManifest,
    pub manifest_diff: ManifestDiff,
    pub reason: String,
}

#[derive(Serialize)]
struct PlatformCompositionBuildReceiptRequest<'a> {
    context: &'a ModuleCommandContext,
    expected_revision: i64,
    manifest: &'a ModulesManifest,
    manifest_changes: &'a [String],
    reason: &'a str,
}

struct ServerCompositionBuildEnqueuer {
    manifest: ModulesManifest,
    manifest_diff: ManifestDiff,
    requested_by: String,
    reason: String,
}

#[async_trait::async_trait]
impl ModuleCompositionBuildEnqueuer for ServerCompositionBuildEnqueuer {
    type Output = Build;

    async fn enqueue(
        &self,
        transaction: &DatabaseTransaction,
        snapshot: &ModuleCompositionSnapshot,
    ) -> Result<Self::Output, String> {
        let (build, _created) = BuildService::request_build_on_connection(
            transaction,
            BuildRequest {
                manifest_ref: format!("platform_state:{}", snapshot.revision),
                manifest_revision: snapshot.revision,
                manifest_snapshot: snapshot.manifest.clone(),
                artifact_identity: snapshot.manifest_hash.clone(),
                requested_by: self.requested_by.clone(),
                reason: Some(self.reason.clone()),
                modules_delta: self.manifest_diff.summary(),
                modules: ManifestManager::build_modules(&self.manifest),
                profile: ManifestManager::deployment_profile(&self.manifest),
                execution_plan: ManifestManager::build_execution_plan(&self.manifest),
            },
        )
        .await
        .map_err(|error| error.to_string())?;
        Ok(build)
    }
}

pub struct PlatformCompositionService;

/// Server adapter that exposes the active static composition through the
/// owner-owned browser-safe installed-module port.
#[derive(Clone)]
pub struct ServerStaticInstalledModuleReader {
    db: DatabaseConnection,
}

impl ServerStaticInstalledModuleReader {
    pub fn shared(db: DatabaseConnection) -> SharedStaticInstalledModuleReader {
        SharedStaticInstalledModuleReader(Arc::new(Self { db }))
    }
}

#[async_trait::async_trait]
impl StaticInstalledModuleReader for ServerStaticInstalledModuleReader {
    async fn list(
        &self,
    ) -> Result<Vec<StaticInstalledModuleView>, StaticInstalledModuleReaderError> {
        PlatformCompositionService::installed_modules(&self.db)
            .await
            .map_err(|error| {
                tracing::error!(
                    error = %error,
                    "failed to resolve the static installed-module projection"
                );
                StaticInstalledModuleReaderError::Unavailable
            })
    }
}

impl PlatformCompositionService {
    pub async fn active_snapshot(
        db: &DatabaseConnection,
    ) -> Result<PlatformCompositionSnapshot, PlatformCompositionError> {
        let owner = ModuleControlPlane::new(db.clone()).composition();
        let snapshot = match owner.active_snapshot().await {
            Ok(snapshot) => snapshot,
            Err(ModuleCompositionError::MissingActiveComposition) => {
                let bootstrap = Self::bootstrap_manifest()?;
                let bootstrap_json = Self::manifest_snapshot_json(&bootstrap)?;
                owner
                    .ensure_active_snapshot(&bootstrap_json, "bootstrap")
                    .await?
            }
            Err(error) => return Err(error.into()),
        };
        Self::snapshot_from_owner(snapshot)
    }

    pub async fn active_manifest(
        db: &DatabaseConnection,
    ) -> Result<ModulesManifest, PlatformCompositionError> {
        Ok(Self::active_snapshot(db).await?.manifest)
    }

    /// Returns the bounded browser-safe composition projection after the host
    /// has ensured the owner snapshot is initialized.
    pub async fn active_snapshot_view(
        db: &DatabaseConnection,
    ) -> Result<ModuleCompositionSnapshotView, PlatformCompositionError> {
        Self::active_snapshot(db).await.map(Into::into)
    }

    /// Returns the installed platform-native module projection from the
    /// durable active composition. GraphQL and native transports must not
    /// inspect the manifest directly.
    pub async fn installed_modules(
        db: &DatabaseConnection,
    ) -> Result<Vec<StaticInstalledModuleView>, PlatformCompositionError> {
        let manifest = Self::active_manifest(db).await?;
        Ok(ManifestManager::installed_modules(&manifest)
            .into_iter()
            .map(static_installed_module_view)
            .collect())
    }

    pub fn manifest_snapshot_json(
        manifest: &ModulesManifest,
    ) -> Result<serde_json::Value, PlatformCompositionError> {
        canonical_manifest_snapshot_json(manifest)
            .map_err(|err| PlatformCompositionError::Serialize(err.to_string()))
    }

    pub fn manifest_hash(manifest: &ModulesManifest) -> Result<String, PlatformCompositionError> {
        Ok(hash_manifest_snapshot(&Self::manifest_snapshot_json(
            manifest,
        )?))
    }

    fn snapshot_from_owner(
        snapshot: ModuleCompositionSnapshot,
    ) -> Result<PlatformCompositionSnapshot, PlatformCompositionError> {
        let manifest = serde_json::from_value(snapshot.manifest)
            .map_err(|err| PlatformCompositionError::Deserialize(err.to_string()))?;
        Ok(PlatformCompositionSnapshot {
            revision: snapshot.revision,
            manifest_hash: snapshot.manifest_hash,
            manifest,
        })
    }

    fn bootstrap_manifest() -> Result<ModulesManifest, PlatformCompositionError> {
        if let Ok(manifest) = ManifestManager::load() {
            return Ok(manifest);
        }

        let raw = include_str!("../../../../modules.toml");
        toml::from_str(raw).map_err(|err| {
            PlatformCompositionError::Manifest(ManifestError::Parse {
                path: "embedded modules.toml".to_string(),
                error: err.to_string(),
            })
        })
    }
}

fn static_installed_module_view(module: InstalledManifestModule) -> StaticInstalledModuleView {
    StaticInstalledModuleView {
        slug: module.slug,
        source: module.source,
        crate_name: module.crate_name,
        version: module.version,
        required: module.required,
        dependencies: module.depends_on,
    }
}

impl PlatformCompositionBuildService {
    /// Applies one static module mutation to the active composition and
    /// enqueues its build through the owner-controlled transaction. GraphQL
    /// and native transports provide only authenticated mutation intent.
    pub async fn apply_module_mutation_and_request_build(
        db: &DatabaseConnection,
        event_publisher: std::sync::Arc<dyn BuildEventPublisher>,
        registry: &rustok_core::ModuleRegistry,
        mutation: PlatformCompositionModuleMutation,
    ) -> Result<PlatformCompositionBuildResult, PlatformCompositionBuildError> {
        let operation = ModuleCompositionOperation {
            context: mutation.context.clone(),
            expected_revision: mutation.expected_revision,
        };
        let owner = ModuleControlPlane::new(db.clone()).composition();
        let lease = match owner
            .admit_build_operation::<Build, _>(&operation, &mutation.change)
            .await
            .map_err(PlatformCompositionError::from)?
        {
            ModuleCompositionBuildAdmission::Replay(result) => {
                return Self::finalize_build_request(event_publisher, result).await;
            }
            ModuleCompositionBuildAdmission::Run(lease) => lease,
        };

        let command = match Self::adapt_module_mutation(db, mutation).await {
            Ok(command) => command,
            Err(error) => return Self::fail_admitted_operation(&owner, lease, error).await,
        };
        Self::run_admitted_build(db, event_publisher, registry, &owner, lease, command).await
    }

    pub async fn update_manifest_and_request_build(
        db: &DatabaseConnection,
        event_publisher: std::sync::Arc<dyn BuildEventPublisher>,
        registry: &rustok_core::ModuleRegistry,
        command: PlatformCompositionBuildCommand,
    ) -> Result<PlatformCompositionBuildResult, PlatformCompositionBuildError> {
        let operation = ModuleCompositionOperation {
            context: command.context.clone(),
            expected_revision: command.expected_revision,
        };
        let receipt_request = PlatformCompositionBuildReceiptRequest {
            context: &command.context,
            expected_revision: command.expected_revision,
            manifest: &command.manifest,
            manifest_changes: &command.manifest_diff.changes,
            reason: &command.reason,
        };
        let owner = ModuleControlPlane::new(db.clone()).composition();
        let lease = match owner
            .admit_build_operation::<Build, _>(&operation, &receipt_request)
            .await
            .map_err(PlatformCompositionError::from)?
        {
            ModuleCompositionBuildAdmission::Replay(result) => {
                return Self::finalize_build_request(event_publisher, result).await;
            }
            ModuleCompositionBuildAdmission::Run(lease) => lease,
        };
        Self::run_admitted_build(db, event_publisher, registry, &owner, lease, command).await
    }

    async fn adapt_module_mutation(
        db: &DatabaseConnection,
        mutation: PlatformCompositionModuleMutation,
    ) -> Result<PlatformCompositionBuildCommand, PlatformCompositionBuildError> {
        let snapshot = PlatformCompositionService::active_snapshot(db).await?;
        let mut manifest = snapshot.manifest;
        let (manifest_diff, reason) = match mutation.change {
            PlatformCompositionModuleChange::Install {
                module_slug,
                version,
            } => (
                ManifestManager::install_builtin_module(&mut manifest, &module_slug, Some(version))
                    .map_err(PlatformCompositionError::from)?,
                format!("install module {module_slug}"),
            ),
            PlatformCompositionModuleChange::Uninstall { module_slug } => (
                ManifestManager::uninstall_module(&mut manifest, &module_slug)
                    .map_err(PlatformCompositionError::from)?,
                format!("uninstall module {module_slug}"),
            ),
            PlatformCompositionModuleChange::Upgrade {
                module_slug,
                version,
            } => (
                ManifestManager::upgrade_module(&mut manifest, &module_slug, version)
                    .map_err(PlatformCompositionError::from)?,
                format!("upgrade module {module_slug}"),
            ),
        };
        Ok(PlatformCompositionBuildCommand {
            context: mutation.context,
            expected_revision: mutation.expected_revision,
            manifest,
            manifest_diff,
            reason,
        })
    }

    async fn run_admitted_build(
        _db: &DatabaseConnection,
        event_publisher: std::sync::Arc<dyn BuildEventPublisher>,
        registry: &rustok_core::ModuleRegistry,
        owner: &SeaOrmModuleCompositionService,
        lease: ModuleCompositionBuildLease,
        command: PlatformCompositionBuildCommand,
    ) -> Result<PlatformCompositionBuildResult, PlatformCompositionBuildError> {
        let PlatformCompositionBuildCommand {
            context,
            expected_revision,
            manifest,
            manifest_diff,
            reason,
        } = command;
        let prepared = (|| -> Result<serde_json::Value, PlatformCompositionError> {
            ManifestManager::validate(&manifest)?;
            ManifestManager::validate_deployment_selection(&manifest)?;
            ManifestManager::validate_with_registry(&manifest, registry)?;
            PlatformCompositionService::manifest_snapshot_json(&manifest)
        })();
        let manifest_json = match prepared {
            Ok(manifest_json) => manifest_json,
            Err(error) => {
                return Self::fail_admitted_operation(owner, lease, error.into()).await;
            }
        };
        let enqueuer = ServerCompositionBuildEnqueuer {
            manifest,
            manifest_diff,
            requested_by: context.actor_id.to_string(),
            reason,
        };
        let owner_result = owner
            .replace_active_snapshot_and_enqueue(
                ModuleCompositionUpdate {
                    operation: ModuleCompositionOperation {
                        context,
                        expected_revision,
                    },
                    manifest: manifest_json,
                },
                &enqueuer,
                lease,
            )
            .await
            .map_err(PlatformCompositionError::from)?;
        Self::finalize_build_request(event_publisher, owner_result).await
    }

    async fn finalize_build_request(
        event_publisher: std::sync::Arc<dyn BuildEventPublisher>,
        owner_result: ModuleCompositionBuildEnqueueResult<Build>,
    ) -> Result<PlatformCompositionBuildResult, PlatformCompositionBuildError> {
        let result = PlatformCompositionBuildResult {
            snapshot: PlatformCompositionService::snapshot_from_owner(owner_result.snapshot)?,
            build: owner_result.output,
            replayed: owner_result.replayed,
        };
        // Build persistence is owner-transactional, while hub and bus delivery
        // are intentionally at-least-once notifications. Re-emitting on a
        // terminal replay repairs a prior post-commit delivery failure without
        // queueing another immutable build record.
        event_publisher
            .publish(rustok_build::BuildEvent::BuildRequested {
                build_id: result.build.id,
                requested_by: result.build.requested_by.clone(),
            })
            .await
            .map_err(|error| PlatformCompositionBuildError::Build(error.to_string()))?;
        Ok(result)
    }

    async fn fail_admitted_operation(
        owner: &SeaOrmModuleCompositionService,
        lease: ModuleCompositionBuildLease,
        error: PlatformCompositionBuildError,
    ) -> Result<PlatformCompositionBuildResult, PlatformCompositionBuildError> {
        let terminal_error = composition_operation_failure(&error);
        owner
            .fail_build_operation(lease, &terminal_error)
            .await
            .map_err(PlatformCompositionError::from)?;
        Err(error)
    }
}

fn composition_operation_failure(error: &PlatformCompositionBuildError) -> PortError {
    match error {
        PlatformCompositionBuildError::Composition(PlatformCompositionError::Manifest(_))
        | PlatformCompositionBuildError::Composition(PlatformCompositionError::Definition(_)) => {
            PortError::validation("modules.composition_invalid_mutation", error.to_string())
        }
        PlatformCompositionBuildError::Composition(
            PlatformCompositionError::RevisionConflict { .. },
        ) => PortError::conflict("modules.composition_revision_conflict", error.to_string()),
        PlatformCompositionBuildError::Build(_)
        | PlatformCompositionBuildError::Composition(PlatformCompositionError::Database(_))
        | PlatformCompositionBuildError::Composition(PlatformCompositionError::EffectivePolicy(
            _,
        ))
        | PlatformCompositionBuildError::Composition(PlatformCompositionError::Owner(_)) => {
            PortError::unavailable("modules.composition_unavailable", error.to_string())
        }
        PlatformCompositionBuildError::Composition(PlatformCompositionError::Serialize(_))
        | PlatformCompositionBuildError::Composition(PlatformCompositionError::Deserialize(_)) => {
            PortError::invariant_violation("modules.composition_invariant", error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::static_installed_module_view;
    use crate::modules::InstalledManifestModule;
    use rustok_api::manifest_hash::hash_manifest_snapshot;

    #[test]
    fn manifest_snapshot_hash_is_sha256_hex() {
        let hash = hash_manifest_snapshot(&serde_json::json!({
            "modules": {"catalog": {"enabled": true}}
        }));
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn manifest_snapshot_hash_changes_when_snapshot_changes() {
        let left = hash_manifest_snapshot(&serde_json::json!({"a": 1}));
        let right = hash_manifest_snapshot(&serde_json::json!({"a": 2}));
        assert_ne!(left, right);
    }

    #[test]
    fn manifest_snapshot_hash_is_stable_for_different_object_key_order() {
        let left = hash_manifest_snapshot(&serde_json::json!({
            "modules": {"catalog": {"enabled": true}, "pricing": {"enabled": false}},
            "profile": "default",
            "settings": {"b": 1, "a": 2}
        }));
        let right = hash_manifest_snapshot(&serde_json::json!({
            "settings": {"a": 2, "b": 1},
            "profile": "default",
            "modules": {"pricing": {"enabled": false}, "catalog": {"enabled": true}}
        }));
        assert_eq!(left, right);
    }

    #[test]
    fn installed_module_view_redacts_private_manifest_locators() {
        let view = static_installed_module_view(InstalledManifestModule {
            slug: "catalog".to_string(),
            source: "git".to_string(),
            crate_name: "rustok-catalog".to_string(),
            version: Some("1.2.3".to_string()),
            git: Some("https://example.invalid/catalog.git".to_string()),
            rev: Some("deadbeef".to_string()),
            path: Some("crates/modules/rustok-catalog".to_string()),
            required: false,
            depends_on: vec!["content".to_string()],
        });

        assert_eq!(
            serde_json::to_value(view).expect("installed module view serializes"),
            serde_json::json!({
                "slug": "catalog",
                "source": "git",
                "crateName": "rustok-catalog",
                "version": "1.2.3",
                "required": false,
                "dependencies": ["content"],
            })
        );
    }
}
