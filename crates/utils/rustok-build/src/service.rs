use std::sync::Arc;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    DbBackend, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set, Statement, TransactionTrait,
};
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

use crate::build::{
    ActiveModel as BuildActiveModel, BuildStage, BuildStatus, Entity as BuildEntity, Model as Build,
};
use crate::{BuildEvent, BuildEventPublisher, BuildRequest, NoopBuildEventPublisher};
use rustok_api::manifest_hash::hash_manifest_snapshot;

const MAX_HISTORY_PAGE_SIZE: u64 = 100;
const MAX_HISTORY_OFFSET: u64 = 1_000_000;
const MAX_PROGRESS_UPDATE_PAGE_SIZE: u64 = 100;
const MAX_ACTIVE_BUILD_PROGRESS_SNAPSHOT: u64 = 1_000;
const MAX_DURABLE_BUILD_PROGRESS_SEQUENCE: i64 = i64::MAX;

pub struct BuildService {
    db: DatabaseConnection,
    event_publisher: Arc<dyn BuildEventPublisher>,
}

impl std::fmt::Debug for BuildService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuildService").finish()
    }
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
        let txn = self.db.begin().await?;
        let (build, created) = Self::request_build_on_connection(&txn, request).await?;
        txn.commit().await?;

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

    pub async fn request_build_on_connection(
        db: &DatabaseTransaction,
        request: BuildRequest,
    ) -> anyhow::Result<(Build, bool)> {
        let manifest_hash = compute_build_request_hash(&request);

        if let Some(existing) = Self::find_build_by_hash_on(db, &manifest_hash)
            .await?
            .filter(|existing| existing.status == BuildStatus::Success)
        {
            validate_build_sequence(&existing)?;
            info!(
                build_id = %existing.id,
                "Build with same immutable execution plan already exists, returning existing build"
            );
            return Ok((existing, false));
        }

        let event_sequence = reserve_build_progress_sequence(db).await?;
        let mut build = Build::new(
            request.manifest_ref,
            manifest_hash,
            request.manifest_revision,
            request.manifest_snapshot.clone(),
            request.requested_by,
            request.profile,
        );
        build.event_sequence = event_sequence;

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
            event_sequence: Set(build.event_sequence),
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

        // Return the persisted model, including the execution metadata and
        // reason populated above. Composition receipts and transport snapshots
        // serialize this result, so the pre-insert placeholder is incomplete.
        let build = active_model.insert(db).await?;

        Ok((build, true))
    }

    pub async fn get_build(&self, build_id: Uuid) -> anyhow::Result<Option<Build>> {
        Ok(BuildEntity::find_by_id(build_id).one(&self.db).await?)
    }

    pub async fn active_build(&self) -> anyhow::Result<Option<Build>> {
        Ok(BuildEntity::find()
            .filter(crate::build::Column::Status.is_in([BuildStatus::Queued, BuildStatus::Running]))
            .order_by_desc(crate::build::Column::CreatedAt)
            .order_by_desc(crate::build::Column::Id)
            .one(&self.db)
            .await?)
    }

    pub async fn active_build_progress(&self) -> anyhow::Result<Vec<Build>> {
        let builds = BuildEntity::find()
            .filter(crate::build::Column::Status.is_in([BuildStatus::Queued, BuildStatus::Running]))
            .order_by_asc(crate::build::Column::CreatedAt)
            .order_by_asc(crate::build::Column::Id)
            .limit(MAX_ACTIVE_BUILD_PROGRESS_SNAPSHOT + 1)
            .all(&self.db)
            .await?;
        if builds.len() as u64 > MAX_ACTIVE_BUILD_PROGRESS_SNAPSHOT {
            anyhow::bail!(
                "active build snapshot exceeds the supported limit of {MAX_ACTIVE_BUILD_PROGRESS_SNAPSHOT}; subscribe to a specific build"
            );
        }
        validate_build_sequences(&builds)?;
        Ok(builds)
    }

    pub async fn build_progress(&self, build_id: Uuid) -> anyhow::Result<Option<Build>> {
        let build = self.get_build(build_id).await?;
        if let Some(build) = build.as_ref() {
            validate_build_sequence(build)?;
        }
        Ok(build)
    }

    pub async fn current_progress_sequence(&self) -> anyhow::Result<i64> {
        read_build_progress_sequence(&self.db).await
    }

    pub async fn build_progress_after(
        &self,
        sequence: i64,
        limit: u64,
    ) -> anyhow::Result<Vec<Build>> {
        validate_progress_page(sequence, limit)?;
        let builds = BuildEntity::find()
            .filter(crate::build::Column::EventSequence.gt(sequence))
            .order_by_asc(crate::build::Column::EventSequence)
            .order_by_asc(crate::build::Column::Id)
            .limit(limit)
            .all(&self.db)
            .await?;
        validate_build_sequences(&builds)?;
        Ok(builds)
    }

    pub async fn running_build(&self) -> anyhow::Result<Option<Build>> {
        Ok(BuildEntity::find()
            .filter(crate::build::Column::Status.eq(BuildStatus::Running))
            .order_by_desc(crate::build::Column::CreatedAt)
            .order_by_desc(crate::build::Column::Id)
            .one(&self.db)
            .await?)
    }

    pub async fn next_queued_build(&self) -> anyhow::Result<Option<Build>> {
        Ok(BuildEntity::find()
            .filter(crate::build::Column::Status.eq(BuildStatus::Queued))
            .order_by_asc(crate::build::Column::CreatedAt)
            .order_by_asc(crate::build::Column::Id)
            .one(&self.db)
            .await?)
    }

    /// Atomically claims a specific queued build for execution.
    ///
    /// The status predicate makes concurrent workers mutually exclusive for the same build:
    /// exactly one caller can transition queued to running.
    pub async fn claim_queued_build(&self, build_id: Uuid) -> anyhow::Result<Option<Build>> {
        let claimed = self
            .db
            .transaction::<_, Option<Build>, sea_orm::DbErr>(|txn| {
                Box::pin(async move {
                    let Some(candidate) = BuildEntity::find_by_id(build_id).one(txn).await? else {
                        return Ok(None);
                    };
                    if candidate.status != BuildStatus::Queued {
                        return Ok(None);
                    }
                    if candidate.event_sequence < 0 {
                        return Err(sea_orm::DbErr::Custom(format!(
                            "build {build_id} has a corrupt negative progress sequence"
                        )));
                    }

                    let now = Utc::now();
                    let event_sequence = reserve_build_progress_sequence(txn)
                        .await
                        .map_err(|error| sea_orm::DbErr::Custom(error.to_string()))?;
                    let updated = BuildEntity::update_many()
                        .col_expr(
                            crate::build::Column::Status,
                            sea_orm::sea_query::Expr::value(BuildStatus::Running),
                        )
                        .col_expr(
                            crate::build::Column::StartedAt,
                            sea_orm::sea_query::Expr::value(Some(now)),
                        )
                        .col_expr(
                            crate::build::Column::UpdatedAt,
                            sea_orm::sea_query::Expr::value(now),
                        )
                        .col_expr(
                            crate::build::Column::EventSequence,
                            sea_orm::sea_query::Expr::value(event_sequence),
                        )
                        .filter(crate::build::Column::Id.eq(build_id))
                        .filter(crate::build::Column::Status.eq(BuildStatus::Queued))
                        .filter(crate::build::Column::EventSequence.eq(candidate.event_sequence))
                        .exec(txn)
                        .await?;

                    if updated.rows_affected != 1 {
                        return Ok(None);
                    }

                    BuildEntity::find_by_id(build_id).one(txn).await
                })
            })
            .await
            .map_err(|e| anyhow::anyhow!(format!("Failed to claim build {build_id}: {e}")))?;

        let Some(claimed) = claimed else {
            return Ok(None);
        };

        if let Err(error) = self
            .event_publisher
            .publish(BuildEvent::BuildStarted {
                build_id: claimed.id,
                stage: claimed.stage.clone(),
                progress: claimed.progress,
            })
            .await
        {
            // The database row is authoritative. A lost wake-up is repaired by
            // the durable progress poll and must not fail a committed claim.
            warn!(
                build_id = %build_id,
                error = %error,
                "Build start notification failed; retaining committed claim"
            );
        }

        // A cancellation may win after the claim commits but before the worker
        // begins side effects. Re-read the canonical row after notification.
        let current = self.get_build(build_id).await?;
        if let Some(current) = current.as_ref() {
            validate_build_sequence(current)?;
        }
        Ok(current.filter(|build| build.status == BuildStatus::Running))
    }

    /// Claims the oldest queued build atomically.
    ///
    /// Concurrent workers may observe the same candidate, but only one can satisfy
    /// the queued-status compare-and-swap. The losing worker receives None.
    pub async fn claim_next_queued_build(&self) -> anyhow::Result<Option<Build>> {
        let candidate = self.next_queued_build().await?;
        let Some(candidate) = candidate else {
            return Ok(None);
        };

        self.claim_queued_build(candidate.id).await
    }

    pub async fn list_builds_page(&self, limit: u64, offset: u64) -> anyhow::Result<Vec<Build>> {
        validate_history_page(limit, offset)?;
        let builds = BuildEntity::find()
            .order_by_desc(crate::build::Column::CreatedAt)
            .order_by_desc(crate::build::Column::Id)
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
            .order_by_desc(crate::build::Column::CreatedAt)
            .order_by_desc(crate::build::Column::Id)
            .one(db)
            .await?)
    }

    #[instrument(skip(self))]
    pub async fn update_build_status(
        &self,
        build_id: Uuid,
        status: BuildStatus,
        stage: Option<BuildStage>,
        progress: Option<i32>,
    ) -> anyhow::Result<()> {
        let updated = self
            .db
            .transaction::<_, (BuildStatus, Build), sea_orm::DbErr>(|txn| {
                let status = status.clone();
                let stage = stage.clone();
                Box::pin(async move {
                    let build = BuildEntity::find_by_id(build_id)
                        .one(txn)
                        .await?
                        .ok_or_else(|| {
                            sea_orm::DbErr::Custom(format!("build {build_id} not found"))
                        })?;
                    if build.is_final() {
                        return Err(sea_orm::DbErr::Custom(format!(
                            "build {build_id} is already final"
                        )));
                    }
                    if build.event_sequence < 0 {
                        return Err(sea_orm::DbErr::Custom(format!(
                            "build {build_id} has a corrupt negative progress sequence"
                        )));
                    }
                    if !matches!(
                        (&build.status, &status),
                        (
                            BuildStatus::Queued,
                            BuildStatus::Running | BuildStatus::Cancelled
                        ) | (
                            BuildStatus::Running,
                            BuildStatus::Running | BuildStatus::Success | BuildStatus::Cancelled
                        )
                    ) {
                        return Err(sea_orm::DbErr::Custom(format!(
                            "invalid status transition for build {build_id}"
                        )));
                    }

                    let previous_status = build.status.clone();
                    let next_stage = stage.unwrap_or_else(|| build.stage.clone());
                    let next_progress = progress.unwrap_or(build.progress);
                    if status == BuildStatus::Success
                        && (next_stage != BuildStage::Complete || next_progress != 100)
                    {
                        return Err(sea_orm::DbErr::Custom(format!(
                            "successful build {build_id} must finish at Complete/100"
                        )));
                    }
                    if !(0..=100).contains(&next_progress) {
                        return Err(sea_orm::DbErr::Custom(format!(
                            "build {build_id} progress must be between 0 and 100"
                        )));
                    }
                    if next_progress < build.progress {
                        return Err(sea_orm::DbErr::Custom(format!(
                            "build {build_id} progress cannot decrease"
                        )));
                    }
                    if build_stage_rank(&next_stage) < build_stage_rank(&build.stage) {
                        return Err(sea_orm::DbErr::Custom(format!(
                            "build {build_id} stage cannot move backwards"
                        )));
                    }
                    if status == BuildStatus::Running && next_stage == BuildStage::Complete {
                        return Err(sea_orm::DbErr::Custom(format!(
                            "running build {build_id} cannot enter the Complete stage"
                        )));
                    }

                    let now = Utc::now();
                    let event_sequence = reserve_build_progress_sequence(txn)
                        .await
                        .map_err(|error| sea_orm::DbErr::Custom(error.to_string()))?;
                    let mut update = BuildEntity::update_many()
                        .col_expr(
                            crate::build::Column::Status,
                            sea_orm::sea_query::Expr::value(status.clone()),
                        )
                        .col_expr(
                            crate::build::Column::Stage,
                            sea_orm::sea_query::Expr::value(next_stage),
                        )
                        .col_expr(
                            crate::build::Column::Progress,
                            sea_orm::sea_query::Expr::value(next_progress),
                        )
                        .col_expr(
                            crate::build::Column::UpdatedAt,
                            sea_orm::sea_query::Expr::value(now),
                        )
                        .col_expr(
                            crate::build::Column::EventSequence,
                            sea_orm::sea_query::Expr::value(event_sequence),
                        )
                        .filter(crate::build::Column::Id.eq(build_id))
                        // Status alone is not a version fence: Running -> Running
                        // writers must also compare the observed progress watermark.
                        .filter(crate::build::Column::Status.eq(previous_status.clone()))
                        .filter(crate::build::Column::EventSequence.eq(build.event_sequence));
                    if status == BuildStatus::Running && build.started_at.is_none() {
                        update = update.col_expr(
                            crate::build::Column::StartedAt,
                            sea_orm::sea_query::Expr::value(Some(now)),
                        );
                    }
                    if status.is_final() {
                        update = update.col_expr(
                            crate::build::Column::FinishedAt,
                            sea_orm::sea_query::Expr::value(Some(now)),
                        );
                    }
                    if update.exec(txn).await?.rows_affected != 1 {
                        return Err(sea_orm::DbErr::Custom(format!(
                            "build {build_id} changed during status update"
                        )));
                    }
                    let updated = BuildEntity::find_by_id(build_id)
                        .one(txn)
                        .await?
                        .ok_or_else(|| {
                            sea_orm::DbErr::Custom(format!(
                                "build {build_id} disappeared during status update"
                            ))
                        })?;
                    Ok((previous_status, updated))
                })
            })
            .await
            .map_err(|e| anyhow::anyhow!("Failed to update build status: {e}"))?;

        let (previous_status, updated) = updated;
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

        if let Err(error) = self.event_publisher.publish(event).await {
            // State and sequence committed together. The notification is only a
            // local wake-up hint; replicas recover the snapshot from the database.
            warn!(
                build_id = %build_id,
                error = %error,
                "Build progress notification failed; retaining committed state"
            );
        }

        Ok(())
    }

    pub async fn fail_build(&self, build_id: Uuid, err_msg: String) -> anyhow::Result<()> {
        let updated = self
            .db
            .transaction::<_, Option<Build>, sea_orm::DbErr>(|txn| {
                let err_msg = err_msg.clone();
                Box::pin(async move {
                    let build = BuildEntity::find_by_id(build_id)
                        .one(txn)
                        .await?
                        .ok_or_else(|| {
                            sea_orm::DbErr::Custom(format!("build {build_id} not found"))
                        })?;

                    if build.is_final() {
                        return Ok(None);
                    }
                    if build.event_sequence < 0 {
                        return Err(sea_orm::DbErr::Custom(format!(
                            "build {build_id} has a corrupt negative progress sequence"
                        )));
                    }

                    let now = Utc::now();
                    let event_sequence = reserve_build_progress_sequence(txn)
                        .await
                        .map_err(|error| sea_orm::DbErr::Custom(error.to_string()))?;
                    let update = BuildEntity::update_many()
                        .col_expr(
                            crate::build::Column::Status,
                            sea_orm::sea_query::Expr::value(BuildStatus::Failed),
                        )
                        .col_expr(
                            crate::build::Column::ErrorMessage,
                            sea_orm::sea_query::Expr::value(Some(err_msg)),
                        )
                        .col_expr(
                            crate::build::Column::FinishedAt,
                            sea_orm::sea_query::Expr::value(Some(now)),
                        )
                        .col_expr(
                            crate::build::Column::UpdatedAt,
                            sea_orm::sea_query::Expr::value(now),
                        )
                        .col_expr(
                            crate::build::Column::EventSequence,
                            sea_orm::sea_query::Expr::value(event_sequence),
                        )
                        .filter(crate::build::Column::Id.eq(build_id))
                        .filter(crate::build::Column::Status.eq(build.status))
                        .filter(crate::build::Column::EventSequence.eq(build.event_sequence));
                    if update.exec(txn).await?.rows_affected != 1 {
                        return Err(sea_orm::DbErr::Custom(format!(
                            "build {build_id} changed during failure recording"
                        )));
                    }
                    let updated = BuildEntity::find_by_id(build_id)
                        .one(txn)
                        .await?
                        .ok_or_else(|| {
                            sea_orm::DbErr::Custom(format!(
                                "build {build_id} disappeared during failure recording"
                            ))
                        })?;
                    Ok(Some(updated))
                })
            })
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fail build: {e}"))?;

        if let Some(updated) = updated {
            if let Err(error) = self
                .event_publisher
                .publish(BuildEvent::BuildFailed {
                    build_id,
                    stage: updated.stage.clone(),
                    progress: updated.progress,
                    error: err_msg,
                })
                .await
            {
                warn!(
                    build_id = %build_id,
                    error = %error,
                    "Build failure notification failed; retaining committed failure state"
                );
            }
            error!(build_id = %build_id, "Build failed");
        }

        Ok(())
    }
}
async fn reserve_build_progress_sequence(
    db: &DatabaseTransaction,
) -> anyhow::Result<i64> {
    let backend = db.get_database_backend();
    let storage_guard = match backend {
        DbBackend::Postgres => "",
        DbBackend::Sqlite => " AND typeof(sequence) = 'integer'",
        _ => anyhow::bail!("durable build progress requires PostgreSQL or SQLite"),
    };
    let statement = Statement::from_string(
        backend,
        format!(
            "UPDATE build_event_clock \
             SET sequence = sequence + 1 \
             WHERE id = 1 AND sequence >= 0 AND sequence < {MAX_DURABLE_BUILD_PROGRESS_SEQUENCE}{storage_guard} \
             RETURNING sequence"
        ),
    );
    let row = db
        .query_one_raw(statement)
        .await?
        .ok_or_else(|| anyhow::anyhow!("build progress sequence clock is missing, corrupt, or exhausted"))?;
    let sequence = row.try_get::<i64>("", "sequence")?;
    if sequence <= 0 {
        anyhow::bail!("build progress sequence clock returned an invalid value");
    }
    Ok(sequence)
}

async fn read_build_progress_sequence(db: &DatabaseConnection) -> anyhow::Result<i64> {
    let backend = db.get_database_backend();
    let storage_guard = match backend {
        DbBackend::Postgres => "",
        DbBackend::Sqlite => " AND typeof(sequence) = 'integer'",
        _ => anyhow::bail!("durable build progress requires PostgreSQL or SQLite"),
    };
    let statement = Statement::from_string(
        backend,
        format!(
            "SELECT sequence FROM build_event_clock \
             WHERE id = 1 AND sequence >= 0{storage_guard}"
        ),
    );
    let row = db
        .query_one_raw(statement)
        .await?
        .ok_or_else(|| anyhow::anyhow!("build progress sequence clock is missing, corrupt, or exhausted"))?;
    let sequence = row.try_get::<i64>("", "sequence")?;
    if sequence < 0 {
        anyhow::bail!("build progress sequence clock returned a negative value");
    }
    Ok(sequence)
}

fn validate_progress_page(sequence: i64, limit: u64) -> anyhow::Result<()> {
    if sequence < 0 || limit == 0 || limit > MAX_PROGRESS_UPDATE_PAGE_SIZE {
        anyhow::bail!(
            "build progress query requires a non-negative cursor and a limit between 1 and {MAX_PROGRESS_UPDATE_PAGE_SIZE}"
        );
    }
    Ok(())
}

fn validate_build_sequence(build: &Build) -> anyhow::Result<()> {
    if build.event_sequence < 0 {
        anyhow::bail!("build {} has a corrupt negative progress sequence", build.id);
    }
    Ok(())
}

fn validate_build_sequences(builds: &[Build]) -> anyhow::Result<()> {
    for build in builds {
        validate_build_sequence(build)?;
    }
    Ok(())
}

fn build_stage_rank(stage: &BuildStage) -> u8 {
    match stage {
        BuildStage::Pending => 0,
        BuildStage::Checkout => 1,
        BuildStage::Build => 2,
        BuildStage::Test => 3,
        BuildStage::Deploy => 4,
        BuildStage::Complete => 5,
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

    #[tokio::test]
    async fn new_build_result_matches_the_persisted_model_and_projection() {
        use sea_orm::{ConnectionTrait, Database, EntityTrait, Schema, TransactionTrait};

        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");
        let backend = db.get_database_backend();
        let schema = Schema::new(backend);
        db.execute_raw(backend.build(&schema.create_table_from_entity(crate::build::Entity)))
            .await
            .expect("create builds table");
        db.execute_unprepared(
            "CREATE TABLE build_event_clock (id INTEGER PRIMARY KEY CHECK (id = 1), sequence INTEGER NOT NULL CHECK (typeof(sequence) = 'integer' AND sequence >= 0))",
        )
        .await
        .expect("create build progress clock");
        db.execute_unprepared(
            "INSERT INTO build_event_clock (id, sequence) VALUES (1, 0)",
        )
        .await
        .expect("initialize build progress clock");
        let request = BuildRequest {
            manifest_ref: "platform_state:7".to_string(),
            manifest_revision: 7,
            manifest_snapshot: serde_json::json!({"modules": {}}),
            artifact_identity: "distribution".to_string(),
            requested_by: "operator".to_string(),
            reason: Some("install module".to_string()),
            modules_delta: "add catalog".to_string(),
            modules: HashMap::new(),
            profile: DeploymentProfile::HeadlessApi,
            execution_plan: BuildExecutionPlan {
                runtime_mode: BuildRuntimeMode::Api,
                cargo_package: "rustok-server".to_string(),
                cargo_profile: "release".to_string(),
                cargo_target: Some("x86_64-unknown-linux-gnu".to_string()),
                cargo_features: vec!["mod-catalog".to_string()],
                cargo_command: "cargo build -p rustok-server --release".to_string(),
                admin_build: None,
                storefront_build: None,
            },
        };
        let txn = db.begin().await.expect("begin build request transaction");
        let (created, is_new) =
            super::BuildService::request_build_on_connection(&txn, request.clone())
                .await
                .expect("enqueue build");
        txn.commit().await.expect("commit build request");
        assert!(is_new);
        assert_eq!(created.event_sequence, 1);
        assert_eq!(
            super::BuildService::new(db.clone())
                .current_progress_sequence()
                .await
                .expect("read progress sequence"),
            1
        );
        let stored = crate::build::Entity::find_by_id(created.id)
            .one(&db)
            .await
            .expect("read build")
            .expect("build exists");
        assert_eq!(created, stored);
        assert_eq!(created.reason, request.reason);
        let plan = crate::parse_execution_plan(created.id, created.modules_delta.as_ref())
            .expect("returned build has its execution plan");
        assert_eq!(plan, request.execution_plan);
        let snapshot = crate::build_snapshot(&created);
        assert_eq!(snapshot.reason.as_deref(), Some("install module"));
        assert_eq!(snapshot.modules_delta, "add catalog");
        assert_eq!(
            snapshot.build_command.as_deref(),
            Some("cargo build -p rustok-server --release")
        );
        assert_eq!(snapshot.build_features, vec!["mod-catalog".to_string()]);
        assert_eq!(
            snapshot.build_target.as_deref(),
            Some("x86_64-unknown-linux-gnu")
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
