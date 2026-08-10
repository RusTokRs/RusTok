use rustok_api::manifest_hash::{
    canonical_manifest_snapshot_json, hash_manifest, hash_manifest_snapshot,
};
use rustok_core::ModuleRegistry;
use sea_orm::{DatabaseConnection, DatabaseTransaction, DbErr};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::modules::{
    InstalledManifestModule, ManifestDiff, ManifestError, ManifestManager, ModulesManifest,
};
use rustok_build::build::Model as Build;
use rustok_build::{BuildEventPublisher, BuildRequest, BuildService};
use rustok_modules::{
    ModuleCompositionBuildEnqueuer, ModuleCompositionError, ModuleCompositionSnapshot,
    ModuleCompositionUpdate, ModuleControlPlane, ModuleDefinitionError,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCompositionSnapshot {
    pub revision: i64,
    pub manifest_hash: String,
    pub manifest: ModulesManifest,
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
}

/// Host-authenticated intent to change the active platform-native module set.
/// The composition adapter obtains the durable snapshot, applies the static
/// manifest adapter, and delegates the revision-guarded write to the owner.
#[derive(Debug, Clone)]
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
    pub expected_revision: Option<i64>,
    pub requested_by: String,
    pub change: PlatformCompositionModuleChange,
}

pub struct PlatformCompositionBuildService;

pub struct PlatformCompositionBuildCommand {
    pub expected_revision: Option<i64>,
    pub manifest: ModulesManifest,
    pub manifest_diff: ManifestDiff,
    pub requested_by: String,
    pub reason: String,
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

    /// Returns the installed platform-native module projection from the
    /// durable active composition. GraphQL and native transports must not
    /// inspect the manifest directly.
    pub async fn installed_modules(
        db: &DatabaseConnection,
    ) -> Result<Vec<InstalledManifestModule>, PlatformCompositionError> {
        let manifest = Self::active_manifest(db).await?;
        Ok(ManifestManager::installed_modules(&manifest))
    }

    pub async fn update_manifest(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        expected_revision: Option<i64>,
        manifest: ModulesManifest,
        updated_by: Option<String>,
    ) -> Result<PlatformCompositionSnapshot, PlatformCompositionError> {
        ManifestManager::validate_with_registry(&manifest, registry)?;

        let manifest_json = Self::manifest_snapshot_json(&manifest)?;
        let snapshot = ModuleControlPlane::new(db.clone())
            .composition()
            .replace_active_snapshot(ModuleCompositionUpdate {
                expected_revision,
                manifest: manifest_json,
                updated_by,
            })
            .await?;
        Self::snapshot_from_owner(snapshot)
    }

    pub fn manifest_snapshot_json(
        manifest: &ModulesManifest,
    ) -> Result<serde_json::Value, PlatformCompositionError> {
        canonical_manifest_snapshot_json(manifest)
            .map_err(|err| PlatformCompositionError::Serialize(err.to_string()))
    }

    pub fn manifest_hash(manifest: &ModulesManifest) -> String {
        hash_manifest(manifest).unwrap_or_else(|_| hash_manifest_snapshot(&serde_json::Value::Null))
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
        let command = PlatformCompositionBuildCommand {
            expected_revision: Some(mutation.expected_revision.unwrap_or(snapshot.revision)),
            manifest,
            manifest_diff,
            requested_by: mutation.requested_by,
            reason,
        };
        Self::update_manifest_and_request_build(db, event_publisher, registry, command).await
    }

    pub async fn update_manifest_and_request_build(
        db: &DatabaseConnection,
        event_publisher: std::sync::Arc<dyn BuildEventPublisher>,
        registry: &rustok_core::ModuleRegistry,
        command: PlatformCompositionBuildCommand,
    ) -> Result<PlatformCompositionBuildResult, PlatformCompositionBuildError> {
        let PlatformCompositionBuildCommand {
            expected_revision,
            manifest,
            manifest_diff,
            requested_by,
            reason,
        } = command;
        ManifestManager::validate_with_registry(&manifest, registry)
            .map_err(PlatformCompositionError::from)?;
        let manifest_json = PlatformCompositionService::manifest_snapshot_json(&manifest)?;
        let enqueuer = ServerCompositionBuildEnqueuer {
            manifest,
            manifest_diff,
            requested_by: requested_by.clone(),
            reason,
        };
        let (owner_snapshot, build) = ModuleControlPlane::new(db.clone())
            .composition()
            .replace_active_snapshot_and_enqueue(
                ModuleCompositionUpdate {
                    expected_revision,
                    manifest: manifest_json,
                    updated_by: Some(requested_by),
                },
                &enqueuer,
            )
            .await
            .map_err(PlatformCompositionError::from)?;
        let result = PlatformCompositionBuildResult {
            snapshot: PlatformCompositionService::snapshot_from_owner(owner_snapshot)?,
            build,
        };
        event_publisher
            .publish(rustok_build::BuildEvent::BuildRequested {
                build_id: result.build.id,
                requested_by: result.build.requested_by.clone(),
            })
            .await
            .map_err(|error| PlatformCompositionBuildError::Build(error.to_string()))?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
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
}
