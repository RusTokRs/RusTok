use std::sync::Arc;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
};
use tracing::{error, info};
use uuid::Uuid;

use crate::build::{
    ActiveModel as BuildActiveModel, BuildStage, BuildStatus, Entity as BuildEntity, Model as Build,
};
use crate::{BuildEvent, BuildEventPublisher, BuildRequest, NoopBuildEventPublisher};
use rustok_api::manifest_hash::hash_manifest_snapshot;

const MAX_HISTORY_PAGE_SIZE: u64 = 100;
const MAX_HISTORY_OFFSET: u64 = 1_000_000;

pub struct BuildService {
    db: DatabaseConnection,
    event_publisher: Arc<dyn BuildEventPublisher>,
}

impl BuildService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            event_publisher: Arc::new(NoopBuildEventPublisher),
        }
    }

    pub fn with_event_publisher(
        db: DatabaseConnection,
        event_publisher: Arc<dyn BuildEventPublisher>,
    ) -> Self {
        Self {
            db,
            event_publisher,
        }
    }

    pub fn with_runtime(
        db: DatabaseConnection,
        event_publisher: Arc<dyn BuildEventPublisher>,
    ) -> Self {
        Self {
            db,
            event_publisher,
        }
    }

    pub async fn request_build(&self, request: BuildRequest) -> anyhow::Result<Build> {
        let (build, created) = Self::request_build_on_connection(&self.db, request).await?;

        if created {
            info!(build_id = %build.id, "Build requested");
            self.event_publisher
                .publish(BuildEvent::BuildRequested {
                    build_id: build.id,
                    requested_by: build.requested_by.clone(),
                })
                .await?;
        }

        Ok(build)
    }

    pub async fn request_build_on_connection<C>(
        db: &C,
        request: BuildRequest,
    ) -> anyhow::Result<(Build, bool)>
    where
        C: sea_orm::ConnectionTrait,
    {
        let manifest_hash = compute_build_request_hash(&request);

        if let Some(existing) = Self::find_build_by_hash_on(db, &manifest_hash)
            .await?
            .filter(|existing| existing.status == BuildStatus::Success)
        {
            info!(
                build_id = %existing.id,
                "Build with same immutable execution plan already exists, returning existing build"
            );
            return Ok((existing, false));
        }

        let build = Build::new(
            request.manifest_ref,
            manifest_hash,
            request.manifest_revision,
            request.manifest_snapshot.clone(),
            request.requested_by,
            request.profile,
        );

        let modules_delta = serde_json::json!({
            "summary": request.modules_delta,
            "modules": request.modules,
            "execution_plan": request.execution_plan,
        });

        let active_model = BuildActiveModel {
            id: Set(build.id),
            status: Set(build.status.clone()),
            stage: Set(build.stage.clone()),
            progress: Set(build.progress),
            profile: Set(build.profile.clone()),
            manifest_ref: Set(build.manifest_ref.clone()),
            manifest_hash: Set(build.manifest_hash.clone()),
            manifest_revision: Set(build.manifest_revision),
            manifest_snapshot: Set(build.manifest_snapshot.clone()),
            modules_delta: Set(Some(modules_delta)),
            requested_by: Set(build.requested_by.clone()),
            reason: Set(request.reason),
            logs_url: Set(None),
            error_message: Set(None),
            started_at: Set(None),
            finished_at: Set(None),
            created_at: Set(build.created_at),
            updated_at: Set(build.updated_at),
        };

        active_model.insert(db).await?;

        Ok((build, true))
    }

    pub async fn get_build(&self, build_id: Uuid) -> anyhow::Result<Option<Build>> {
        Ok(BuildEntity::find_by_id(build_id).one(&self.db).await?)
    }

    pub async fn active_build(&self) -> anyhow::Result<Option<Build>> {
        Ok(BuildEntity::find()
            .filter(crate::build::Column::Status.is_in([BuildStatus::Queued, BuildStatus::Running]))
            .order_by_desc(crate::build::Column::CreatedAt)
            .one(&self.db)
            .await?)
    }

    pub async fn running_build(&self) -> anyhow::Result<Option<Build>> {
        Ok(BuildEntity::find()
            .filter(crate::build::Column::Status.eq(BuildStatus::Running))
            .order_by_desc(crate::build::Column::CreatedAt)
            .one(&self.db)
            .await?)
    }

    pub async fn next_queued_build(&self) -> anyhow::Result<Option<Build>> {
        Ok(BuildEntity::find()
            .filter(crate::build::Column::Status.eq(BuildStatus::Queued))
            .order_by_asc(crate::build::Column::CreatedAt)
            .one(&self.db)
            .await?)
    }

    pub async fn list_builds_page(&self, limit: u64, offset: u64) -> anyhow::Result<Vec<Build>> {
        validate_history_page(limit, offset)?;
        let builds = BuildEntity::find()
            .order_by_desc(crate::build::Column::CreatedAt)
            .offset(offset)
            .limit(limit)
            .all(&self.db)
            .await?;
        Ok(builds)
    }

    async fn find_build_by_hash_on<C>(db: &C, hash: &str) -> anyhow::Result<Option<Build>>
    where
        C: sea_orm::ConnectionTrait,
    {
        Ok(BuildEntity::find()
            .filter(crate::build::Column::ManifestHash.eq(hash))
            .one(db)
            .await?)
    }

    pub async fn update_build_status(
        &self,
        build_id: Uuid,
        status: BuildStatus,
        stage: Option<BuildStage>,
        progress: Option<i32>,
    ) -> anyhow::Result<()> {
        let updated = self
            .db
            .transaction::<_, Option<(BuildStatus, Build)>, sea_orm::DbErr>(|txn| {
                let status = status.clone();
                let stage = stage.clone();
                Box::pin(async move {
                    let build = BuildEntity::find_by_id(build_id).one(txn).await?;
                    let Some(build) = build else {
                        return Ok(None);
                    };

                    if build.is_final() {
                        return Ok(None);
                    }

                    let now = Utc::now();
                    let previous_status = build.status.clone();
                    let started_at_is_none = build.started_at.is_none();
                    let mut active_model: BuildActiveModel = build.into();
                    active_model.status = Set(status.clone());

                    if let Some(stage) = stage {
                        active_model.stage = Set(stage);
                    }
                    if let Some(progress) = progress {
                        active_model.progress = Set(progress);
                    }

                    active_model.updated_at = Set(now);

                    if status == BuildStatus::Running && started_at_is_none {
                        active_model.started_at = Set(Some(now));
                    }

                    if status.is_final() {
                        active_model.finished_at = Set(Some(now));
                    }

                    let updated = active_model.update(txn).await?;
                    Ok(Some((previous_status, updated)))
                })
            })
            .await
            .map_err(|e| anyhow::anyhow!("Failed to update build status: {e}"))?;

        if let Some((previous_status, updated)) = updated {
            let event = match status {
                BuildStatus::Running if previous_status != BuildStatus::Running => {
                    BuildEvent::BuildStarted {
                        build_id,
                        stage: updated.stage.clone(),
                        progress: updated.progress,
                    }
                }
                BuildStatus::Running => BuildEvent::BuildProgress {
                    build_id,
                    stage: updated.stage.clone(),
                    progress: updated.progress,
                },
                BuildStatus::Success => BuildEvent::BuildCompleted { build_id },
                BuildStatus::Cancelled => BuildEvent::BuildCancelled {
                    build_id,
                    stage: updated.stage.clone(),
                    progress: updated.progress,
                },
                BuildStatus::Queued | BuildStatus::Failed => return Ok(()),
            };

            self.event_publisher.publish(event).await?;
        }

        Ok(())
    }

    pub async fn fail_build(&self, build_id: Uuid, err_msg: String) -> anyhow::Result<()> {
        let updated = self
            .db
            .transaction::<_, Option<Build>, sea_orm::DbErr>(|txn| {
                let err_msg = err_msg.clone();
                Box::pin(async move {
                    let build = BuildEntity::find_by_id(build_id).one(txn).await?;
                    let Some(build) = build else {
                        return Ok(None);
                    };

                    if build.is_final() {
                        return Ok(None);
                    }

                    let now = Utc::now();
                    let mut active_model: BuildActiveModel = build.into();
                    active_model.status = Set(BuildStatus::Failed);
                    active_model.error_message = Set(Some(err_msg));
                    active_model.finished_at = Set(Some(now));
                    active_model.updated_at = Set(now);
                    let updated = active_model.update(txn).await?;
                    Ok(Some(updated))
                })
            })
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fail build: {e}"))?;

        if let Some(updated) = updated {
            self.event_publisher
                .publish(BuildEvent::BuildFailed {
                    build_id,
                    stage: updated.stage.clone(),
                    progress: updated.progress,
                    error: err_msg,
                })
                .await?;
        }

        error!(build_id = %build_id, "Build failed");
        Ok(())
    }
}
fn validate_history_page(limit: u64, offset: u64) -> anyhow::Result<()> {
    if limit == 0 || limit > MAX_HISTORY_PAGE_SIZE || offset > MAX_HISTORY_OFFSET {
        anyhow::bail!(
            "history query requires a limit between 1 and {MAX_HISTORY_PAGE_SIZE} and an offset not greater than {MAX_HISTORY_OFFSET}"
        );
    }
    Ok(())
}

fn compute_build_request_hash(request: &BuildRequest) -> String {
    hash_manifest_snapshot(&serde_json::json!({
        "manifest_snapshot": &request.manifest_snapshot,
        "artifact_identity": &request.artifact_identity,
        "profile": &request.profile,
        "execution_plan": &request.execution_plan,
    }))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{compute_build_request_hash, validate_history_page};
    use crate::{BuildExecutionPlan, BuildRequest, BuildRuntimeMode, DeploymentProfile};

    #[test]
    fn build_request_hash_changes_for_each_runtime_mode() {
        let snapshot = serde_json::json!({
            "modules": {"catalog": {"version": "1.0.0"}},
            "profile": "default"
        });
        let request = |runtime_mode| BuildRequest {
            manifest_ref: "platform_state:1".to_string(),
            manifest_revision: 1,
            manifest_snapshot: snapshot.clone(),
            artifact_identity: "distribution_hash".to_string(),
            requested_by: "test".to_string(),
            reason: None,
            modules_delta: "test".to_string(),
            modules: HashMap::new(),
            profile: DeploymentProfile::HeadlessApi,
            execution_plan: BuildExecutionPlan {
                runtime_mode,
                cargo_package: "rustok-server".to_string(),
                cargo_profile: "release".to_string(),
                cargo_target: None,
                cargo_features: Vec::new(),
                cargo_command: "cargo build -p rustok-server --release".to_string(),
                admin_build: None,
                storefront_build: None,
            },
        };

        assert_ne!(
            compute_build_request_hash(&request(BuildRuntimeMode::Api)),
            compute_build_request_hash(&request(BuildRuntimeMode::Worker)),
        );
    }

    #[test]
    fn history_page_rejects_unbounded_queries() {
        assert!(validate_history_page(1, 0).is_ok());
        assert!(validate_history_page(100, 1_000_000).is_ok());
        assert!(validate_history_page(0, 0).is_err());
        assert!(validate_history_page(101, 0).is_err());
        assert!(validate_history_page(1, 1_000_001).is_err());
    }
}
