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
use crate::release::{
    ActiveModel as ReleaseActiveModel, Entity as ReleaseEntity, Model as Release, ReleaseStatus,
};
use crate::{
    BuildEvent, BuildEventPublisher, BuildRequest, NoopBuildEventPublisher, ReleaseActivationHook,
    ReleaseArtifactBundle,
};
use rustok_api::manifest_hash::hash_manifest_snapshot;

pub struct BuildService {
    db: DatabaseConnection,
    event_publisher: Arc<dyn BuildEventPublisher>,
    activation_hook: Arc<dyn ReleaseActivationHook>,
}

impl BuildService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            event_publisher: Arc::new(NoopBuildEventPublisher),
            activation_hook: Arc::new(crate::NoopReleaseActivationHook),
        }
    }

    pub fn with_event_publisher(
        db: DatabaseConnection,
        event_publisher: Arc<dyn BuildEventPublisher>,
    ) -> Self {
        Self {
            db,
            event_publisher,
            activation_hook: Arc::new(crate::NoopReleaseActivationHook),
        }
    }

    pub fn with_runtime(
        db: DatabaseConnection,
        event_publisher: Arc<dyn BuildEventPublisher>,
        activation_hook: Arc<dyn ReleaseActivationHook>,
    ) -> Self {
        Self {
            db,
            event_publisher,
            activation_hook,
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

        if let Some(existing) = Self::find_build_by_hash_on(db, &manifest_hash).await? {
            if existing.status == BuildStatus::Success {
                info!(
                    build_id = %existing.id,
                    "Build with same immutable execution plan already exists, returning existing build"
                );
                return Ok((existing, false));
            }
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
            release_id: Set(None),
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

    pub async fn list_builds(&self, limit: u64) -> anyhow::Result<Vec<Build>> {
        let builds = BuildEntity::find()
            .order_by_desc(crate::build::Column::CreatedAt)
            .limit(limit)
            .all(&self.db)
            .await?;
        Ok(builds)
    }

    #[allow(dead_code)]
    async fn find_build_by_hash(&self, hash: &str) -> anyhow::Result<Option<Build>> {
        Self::find_build_by_hash_on(&self.db, hash).await
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
                BuildStatus::Success => BuildEvent::BuildCompleted {
                    build_id,
                    release_id: updated.release_id.clone(),
                },
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

    pub async fn create_release(
        &self,
        build_id: Uuid,
        environment: String,
        modules: Vec<String>,
    ) -> anyhow::Result<Release> {
        let build = self
            .get_build(build_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Build not found"))?;

        let mut release = Release::new(
            build_id,
            environment,
            build.manifest_hash.clone(),
            build.manifest_revision,
            build.manifest_snapshot.clone(),
            modules,
        );

        if let Some(prev) = self.get_active_release().await? {
            release.previous_release_id = Some(prev.id);
        }

        let active_model = ReleaseActiveModel {
            id: Set(release.id.clone()),
            status: Set(release.status.clone()),
            build_id: Set(release.build_id),
            environment: Set(release.environment.clone()),
            container_image: Set(None),
            server_artifact_url: Set(None),
            admin_artifact_url: Set(None),
            storefront_artifact_url: Set(None),
            manifest_hash: Set(release.manifest_hash.clone()),
            manifest_revision: Set(release.manifest_revision),
            manifest_snapshot: Set(release.manifest_snapshot.clone()),
            modules: Set(release.modules.clone()),
            previous_release_id: Set(release.previous_release_id.clone()),
            deployed_at: Set(None),
            rolled_back_at: Set(None),
            created_at: Set(release.created_at),
            updated_at: Set(release.updated_at),
        };

        active_model.insert(&self.db).await?;

        let mut build_model: BuildActiveModel = build.into();
        build_model.release_id = Set(Some(release.id.clone()));
        build_model.update(&self.db).await?;

        self.event_publisher
            .publish(BuildEvent::BuildCompleted {
                build_id,
                release_id: Some(release.id.clone()),
            })
            .await?;

        info!(release_id = %release.id, "Release created");

        Ok(release)
    }

    pub async fn get_release(&self, release_id: &str) -> anyhow::Result<Option<Release>> {
        Ok(ReleaseEntity::find_by_id(release_id).one(&self.db).await?)
    }

    pub async fn activate_release(&self, release_id: &str) -> anyhow::Result<Release> {
        let updated = self
            .db
            .transaction::<_, Release, sea_orm::DbErr>(|txn| {
                let release_id = release_id.to_string();
                Box::pin(async move {
                    let target = ReleaseEntity::find_by_id(&release_id)
                        .one(txn)
                        .await?
                        .ok_or_else(|| sea_orm::DbErr::Custom("Release not found".to_string()))?;

                    let now = Utc::now();

                    if let Some(current) = ReleaseEntity::find()
                        .filter(crate::release::Column::Status.eq(ReleaseStatus::Active))
                        .one(txn)
                        .await?
                    {
                        if current.id != target.id {
                            let mut current_model: ReleaseActiveModel = current.into();
                            current_model.status = Set(ReleaseStatus::RolledBack);
                            current_model.rolled_back_at = Set(Some(now));
                            current_model.updated_at = Set(now);
                            current_model.update(txn).await?;
                        }
                    }

                    let mut target_model: ReleaseActiveModel = target.into();
                    target_model.status = Set(ReleaseStatus::Active);
                    target_model.deployed_at = Set(Some(now));
                    target_model.updated_at = Set(now);
                    target_model.update(txn).await
                })
            })
            .await
            .map_err(|error| anyhow::anyhow!("Failed to activate release: {error}"))?;

        self.activation_hook
            .after_release_activated(&updated)
            .await?;

        Ok(updated)
    }

    pub async fn mark_release_deploying(&self, release_id: &str) -> anyhow::Result<Release> {
        let updated = self
            .db
            .transaction::<_, Release, sea_orm::DbErr>(|txn| {
                let release_id = release_id.to_string();
                Box::pin(async move {
                    let release = ReleaseEntity::find_by_id(&release_id)
                        .one(txn)
                        .await?
                        .ok_or_else(|| sea_orm::DbErr::Custom("Release not found".to_string()))?;

                    let mut active_model: ReleaseActiveModel = release.into();
                    active_model.status = Set(ReleaseStatus::Deploying);
                    active_model.updated_at = Set(Utc::now());
                    active_model.update(txn).await
                })
            })
            .await
            .map_err(|error| anyhow::anyhow!("Failed to mark release deploying: {error}"))?;

        Ok(updated)
    }

    pub async fn attach_release_artifacts(
        &self,
        release_id: &str,
        artifacts: ReleaseArtifactBundle,
    ) -> anyhow::Result<Release> {
        let updated = self
            .db
            .transaction::<_, Release, sea_orm::DbErr>(|txn| {
                let release_id = release_id.to_string();
                let artifacts = artifacts.clone();
                Box::pin(async move {
                    let release = ReleaseEntity::find_by_id(&release_id)
                        .one(txn)
                        .await?
                        .ok_or_else(|| sea_orm::DbErr::Custom("Release not found".to_string()))?;

                    let mut active_model: ReleaseActiveModel = release.into();
                    active_model.container_image = Set(artifacts.container_image);
                    active_model.server_artifact_url = Set(artifacts.server_artifact_url);
                    active_model.admin_artifact_url = Set(artifacts.admin_artifact_url);
                    active_model.storefront_artifact_url = Set(artifacts.storefront_artifact_url);
                    active_model.updated_at = Set(Utc::now());
                    active_model.update(txn).await
                })
            })
            .await
            .map_err(|error| anyhow::anyhow!("Failed to attach release artifacts: {error}"))?;

        Ok(updated)
    }

    pub async fn fail_release(&self, release_id: &str) -> anyhow::Result<Release> {
        let updated = self
            .db
            .transaction::<_, Release, sea_orm::DbErr>(|txn| {
                let release_id = release_id.to_string();
                Box::pin(async move {
                    let release = ReleaseEntity::find_by_id(&release_id)
                        .one(txn)
                        .await?
                        .ok_or_else(|| sea_orm::DbErr::Custom("Release not found".to_string()))?;

                    let mut active_model: ReleaseActiveModel = release.into();
                    active_model.status = Set(ReleaseStatus::Failed);
                    active_model.updated_at = Set(Utc::now());
                    active_model.update(txn).await
                })
            })
            .await
            .map_err(|error| anyhow::anyhow!("Failed to mark release failed: {error}"))?;

        Ok(updated)
    }

    async fn get_active_release(&self) -> anyhow::Result<Option<Release>> {
        Ok(ReleaseEntity::find()
            .filter(crate::release::Column::Status.eq(ReleaseStatus::Active))
            .one(&self.db)
            .await?)
    }

    pub async fn list_releases(&self, limit: u64) -> anyhow::Result<Vec<Release>> {
        let releases = ReleaseEntity::find()
            .order_by_desc(crate::release::Column::CreatedAt)
            .limit(limit)
            .all(&self.db)
            .await?;
        Ok(releases)
    }

    pub async fn rollback(&self, release_id: &str) -> anyhow::Result<Release> {
        let current = self
            .get_release(release_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Release not found"))?;

        let previous_id = current
            .previous_release_id
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No previous release to rollback to"))?;

        let previous = self
            .get_release(&previous_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Previous release not found"))?;

        let mut current_model: ReleaseActiveModel = current.into();
        current_model.status = Set(ReleaseStatus::RolledBack);
        current_model.rolled_back_at = Set(Some(Utc::now()));
        current_model.updated_at = Set(Utc::now());
        current_model.update(&self.db).await?;

        let mut prev_model: ReleaseActiveModel = previous.clone().into();
        prev_model.status = Set(ReleaseStatus::Active);
        prev_model.deployed_at = Set(Some(Utc::now()));
        prev_model.updated_at = Set(Utc::now());
        prev_model.update(&self.db).await?;

        info!(
            from_release = %release_id,
            to_release = %previous_id,
            "Rollback completed"
        );

        self.event_publisher
            .publish(BuildEvent::BuildCompleted {
                build_id: previous.build_id,
                release_id: Some(previous.id.clone()),
            })
            .await?;

        Ok(previous)
    }
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

    use super::compute_build_request_hash;
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
}
