use rustok_core::ModuleRegistry;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr,
    EntityTrait, QueryFilter, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::models::build::Model as Build;
use crate::models::platform_state::{
    ActiveModel as PlatformStateActiveModel, Column as PlatformStateColumn,
    Entity as PlatformStateEntity, Model as PlatformStateModel,
};
use crate::modules::{ManifestDiff, ManifestError, ManifestManager, ModulesManifest};
use crate::services::build_service::{BuildEventPublisher, BuildRequest, BuildService};

pub const ACTIVE_PLATFORM_STATE_ID: &str = "active";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCompositionSnapshot {
    pub revision: i64,
    pub manifest_hash: String,
    pub manifest: ModulesManifest,
}

#[derive(Debug, Error)]
pub enum PlatformCompositionError {
    #[error(transparent)]
    Database(#[from] DbErr),
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error("Failed to serialize platform manifest: {0}")]
    Serialize(String),
    #[error("Failed to deserialize platform manifest: {0}")]
    Deserialize(String),
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

pub struct PlatformCompositionBuildService;

pub struct PlatformCompositionService;

impl PlatformCompositionService {
    pub async fn active_snapshot(
        db: &DatabaseConnection,
    ) -> Result<PlatformCompositionSnapshot, PlatformCompositionError> {
        let state = Self::active_state(db).await?;
        Self::snapshot_from_state(state)
    }

    pub async fn active_manifest(
        db: &DatabaseConnection,
    ) -> Result<ModulesManifest, PlatformCompositionError> {
        Ok(Self::active_snapshot(db).await?.manifest)
    }

    pub async fn active_state(
        db: &DatabaseConnection,
    ) -> Result<PlatformStateModel, PlatformCompositionError> {
        Self::active_state_on(db).await
    }

    pub async fn active_state_on<C>(db: &C) -> Result<PlatformStateModel, PlatformCompositionError>
    where
        C: ConnectionTrait,
    {
        if let Some(state) = PlatformStateEntity::find_by_id(ACTIVE_PLATFORM_STATE_ID.to_string())
            .one(db)
            .await?
        {
            return Ok(state);
        }

        let manifest = Self::bootstrap_manifest()?;
        let manifest_json = serde_json::to_value(&manifest)
            .map_err(|err| PlatformCompositionError::Serialize(err.to_string()))?;
        let manifest_hash = Self::manifest_hash(&manifest);
        let now = chrono::Utc::now().into();
        let active = PlatformStateActiveModel {
            id: Set(ACTIVE_PLATFORM_STATE_ID.to_string()),
            revision: Set(1),
            manifest_json: Set(manifest_json),
            manifest_hash: Set(manifest_hash),
            active_release_id: Set(None),
            updated_by: Set(Some("bootstrap".to_string())),
            created_at: Set(now),
            updated_at: Set(now),
        };

        match active.insert(db).await {
            Ok(state) => Ok(state),
            Err(_) => PlatformStateEntity::find_by_id(ACTIVE_PLATFORM_STATE_ID.to_string())
                .one(db)
                .await?
                .ok_or_else(|| {
                    PlatformCompositionError::Database(DbErr::RecordNotFound(
                        "platform_state.active".to_string(),
                    ))
                }),
        }
    }

    pub async fn update_manifest(
        db: &DatabaseConnection,
        registry: &ModuleRegistry,
        expected_revision: Option<i64>,
        manifest: ModulesManifest,
        updated_by: Option<String>,
    ) -> Result<PlatformCompositionSnapshot, PlatformCompositionError> {
        ManifestManager::validate_with_registry(&manifest, registry)?;

        let current = Self::active_state(db).await?;
        if let Some(expected) = expected_revision {
            if expected != current.revision {
                return Err(PlatformCompositionError::RevisionConflict {
                    expected,
                    current: current.revision,
                });
            }
        }

        let next_revision = current.revision + 1;
        let manifest_json = serde_json::to_value(&manifest)
            .map_err(|err| PlatformCompositionError::Serialize(err.to_string()))?;
        let manifest_hash = Self::manifest_hash(&manifest);
        let result = PlatformStateEntity::update_many()
            .filter(PlatformStateColumn::Id.eq(ACTIVE_PLATFORM_STATE_ID))
            .filter(PlatformStateColumn::Revision.eq(current.revision))
            .col_expr(PlatformStateColumn::Revision, Expr::value(next_revision))
            .col_expr(
                PlatformStateColumn::ManifestJson,
                Expr::value(manifest_json.clone()),
            )
            .col_expr(
                PlatformStateColumn::ManifestHash,
                Expr::value(manifest_hash.clone()),
            )
            .col_expr(
                PlatformStateColumn::UpdatedBy,
                Expr::value(updated_by.clone()),
            )
            .col_expr(
                PlatformStateColumn::UpdatedAt,
                Expr::value(chrono::Utc::now()),
            )
            .exec(db)
            .await?;

        if result.rows_affected != 1 {
            let refreshed = Self::active_state(db).await?;
            return Err(PlatformCompositionError::RevisionConflict {
                expected: current.revision,
                current: refreshed.revision,
            });
        }

        Ok(PlatformCompositionSnapshot {
            revision: next_revision,
            manifest_hash,
            manifest,
        })
    }

    pub fn manifest_snapshot_json(
        manifest: &ModulesManifest,
    ) -> Result<serde_json::Value, PlatformCompositionError> {
        serde_json::to_value(manifest)
            .map_err(|err| PlatformCompositionError::Serialize(err.to_string()))
    }

    pub fn manifest_hash(manifest: &ModulesManifest) -> String {
        let snapshot = Self::manifest_snapshot_json(manifest).unwrap_or(serde_json::Value::Null);
        hash_manifest_snapshot(&snapshot)
    }

    fn snapshot_from_state(
        state: PlatformStateModel,
    ) -> Result<PlatformCompositionSnapshot, PlatformCompositionError> {
        let manifest = serde_json::from_value(state.manifest_json)
            .map_err(|err| PlatformCompositionError::Deserialize(err.to_string()))?;
        Ok(PlatformCompositionSnapshot {
            revision: state.revision,
            manifest_hash: state.manifest_hash,
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

pub fn hash_manifest_snapshot(snapshot: &serde_json::Value) -> String {
    use sha2::Digest;

    let canonical_snapshot = canonicalize_json_value(snapshot);
    let canonical = serde_json::to_string(&canonical_snapshot).unwrap_or_default();
    let mut hasher = sha2::Sha256::new();
    sha2::Digest::update(&mut hasher, canonical.as_bytes());
    hex::encode(sha2::Digest::finalize(hasher))
}

fn canonicalize_json_value(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut entries = map.iter().collect::<Vec<_>>();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            let canonical = entries
                .into_iter()
                .map(|(key, nested)| (key.clone(), canonicalize_json_value(nested)))
                .collect::<serde_json::Map<String, serde_json::Value>>();
            serde_json::Value::Object(canonical)
        }
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .iter()
                .map(canonicalize_json_value)
                .collect::<Vec<serde_json::Value>>(),
        ),
        _ => value.clone(),
    }
}

impl PlatformCompositionBuildService {
    #[allow(clippy::too_many_arguments)]
    pub async fn update_manifest_and_request_build(
        db: &DatabaseConnection,
        event_publisher: std::sync::Arc<dyn BuildEventPublisher>,
        registry: &rustok_core::ModuleRegistry,
        expected_revision: Option<i64>,
        manifest: ModulesManifest,
        manifest_diff: ManifestDiff,
        requested_by: String,
        reason: String,
    ) -> Result<PlatformCompositionBuildResult, PlatformCompositionBuildError> {
        ManifestManager::validate_with_registry(&manifest, registry)
            .map_err(PlatformCompositionError::from)?;

        let txn = db.begin().await.map_err(PlatformCompositionError::from)?;
        let result = async {
            let current = PlatformCompositionService::active_state_on(&txn).await?;
            if let Some(expected) = expected_revision {
                if expected != current.revision {
                    return Err(PlatformCompositionBuildError::Composition(
                        PlatformCompositionError::RevisionConflict {
                            expected,
                            current: current.revision,
                        },
                    ));
                }
            }

            let next_revision = current.revision + 1;
            let manifest_json = PlatformCompositionService::manifest_snapshot_json(&manifest)?;
            let manifest_hash = PlatformCompositionService::manifest_hash(&manifest);
            let result = PlatformStateEntity::update_many()
                .filter(PlatformStateColumn::Id.eq(ACTIVE_PLATFORM_STATE_ID))
                .filter(PlatformStateColumn::Revision.eq(current.revision))
                .col_expr(PlatformStateColumn::Revision, Expr::value(next_revision))
                .col_expr(
                    PlatformStateColumn::ManifestJson,
                    Expr::value(manifest_json.clone()),
                )
                .col_expr(
                    PlatformStateColumn::ManifestHash,
                    Expr::value(manifest_hash.clone()),
                )
                .col_expr(
                    PlatformStateColumn::UpdatedBy,
                    Expr::value(Some(requested_by.clone())),
                )
                .col_expr(
                    PlatformStateColumn::UpdatedAt,
                    Expr::value(chrono::Utc::now()),
                )
                .exec(&txn)
                .await
                .map_err(PlatformCompositionError::from)?;

            if result.rows_affected != 1 {
                let refreshed = PlatformCompositionService::active_state_on(&txn).await?;
                return Err(PlatformCompositionBuildError::Composition(
                    PlatformCompositionError::RevisionConflict {
                        expected: current.revision,
                        current: refreshed.revision,
                    },
                ));
            }

            let snapshot = PlatformCompositionSnapshot {
                revision: next_revision,
                manifest_hash,
                manifest,
            };
            let (build, _created) = BuildService::request_build_on_connection(
                &txn,
                BuildRequest {
                    manifest_ref: format!("platform_state:{}", snapshot.revision),
                    manifest_revision: snapshot.revision,
                    manifest_snapshot: manifest_json,
                    requested_by: requested_by.clone(),
                    reason: Some(reason),
                    modules_delta: manifest_diff.summary(),
                    modules: ManifestManager::build_modules(&snapshot.manifest),
                    profile: ManifestManager::deployment_profile(&snapshot.manifest),
                    execution_plan: ManifestManager::build_execution_plan(&snapshot.manifest),
                },
            )
            .await
            .map_err(|err| PlatformCompositionBuildError::Build(err.to_string()))?;

            Ok(PlatformCompositionBuildResult { snapshot, build })
        }
        .await;

        match result {
            Ok(result) => {
                txn.commit().await.map_err(PlatformCompositionError::from)?;
                event_publisher
                    .publish(crate::services::build_service::BuildEvent::BuildRequested {
                        build_id: result.build.id,
                        requested_by: result.build.requested_by.clone(),
                    })
                    .await
                    .map_err(|err| PlatformCompositionBuildError::Build(err.to_string()))?;
                Ok(result)
            }
            Err(error) => {
                let _ = txn.rollback().await;
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::hash_manifest_snapshot;

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
