//! Host-independent queued build execution.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, anyhow, bail};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    BuildCommandSpec, BuildEventPublisher, BuildExecutionReport, BuildService,
    build::{BuildStage, BuildStatus},
    build_manifest_snapshot_path, parse_execution_plan, run_build_command,
};

pub struct BuildExecutionService {
    build_service: BuildService,
    workspace_root: PathBuf,
}

impl std::fmt::Debug for BuildExecutionService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuildExecutionService")
            .field("build_service", &self.build_service)
            .field("workspace_root", &self.workspace_root)
            .finish()
    }
}

impl BuildExecutionService {
    pub fn new(
        db: DatabaseConnection,
        event_publisher: Arc<dyn BuildEventPublisher>,
        workspace_root: PathBuf,
    ) -> Self {
        Self {
            build_service: BuildService::with_runtime(db, event_publisher),
            workspace_root,
        }
    }

    pub async fn execute_next_queued_build(
        &self,
        dry_run: bool,
    ) -> anyhow::Result<Option<BuildExecutionReport>> {
        if let Some(running) = self.build_service.running_build().await? {
            bail!("build {} is already running", running.id);
        }

        // A preview reads an immutable queued plan but must never reserve it.
        // Another executor may claim it after this read; a preview is not a lease.
        let build = if dry_run {
            self.build_service.next_queued_build().await?
        } else {
            self.build_service.claim_next_queued_build().await?
        };
        let Some(build) = build else {
            return Ok(None);
        };

        if dry_run {
            self.execute_build_plan(build, true).await.map(Some)
        } else {
            self.execute_claimed_build(build).await.map(Some)
        }
    }

    pub async fn execute_build(
        &self,
        build_id: Uuid,
        dry_run: bool,
    ) -> anyhow::Result<BuildExecutionReport> {
        let existing = self
            .build_service
            .get_build(build_id)
            .await?
            .ok_or_else(|| anyhow!("Build not found"))?;

        if existing.is_final() {
            bail!(
                "build {} is already final and cannot be executed again",
                existing.id
            );
        }
        if existing.status == BuildStatus::Running {
            bail!("build {} is already running", existing.id);
        }
        if let Some(running) = self
            .build_service
            .running_build()
            .await?
            .filter(|running| running.id != build_id)
        {
            bail!("build {} is already running", running.id);
        }

        if dry_run {
            return self.execute_build_plan(existing, true).await;
        }

        let Some(build) = self.build_service.claim_queued_build(build_id).await? else {
            bail!("build {} could not be claimed for execution", build_id);
        };

        self.execute_claimed_build(build).await
    }

    async fn execute_claimed_build(
        &self,
        build: crate::build::Model,
    ) -> anyhow::Result<BuildExecutionReport> {
        let build_id = build.id;
        match self.execute_build_plan(build, false).await {
            Ok(report) => Ok(report),
            Err(error) => {
                // Record all observed errors after claim, including preparation
                // and manifest errors, without overwriting terminal states.
                if let Err(record_error) = self
                    .build_service
                    .fail_build(build_id, "Build execution failed".to_string())
                    .await
                {
                    return Err(error.context(format!(
                        "could not reliably record or notify build {build_id} failure: {record_error:#}"
                    )));
                }
                Err(error)
            }
        }
    }

    // With dry_run=false the caller must first claim this build. A dry run
    // deliberately operates only on the read snapshot and performs no writes.
    async fn execute_build_plan(
        &self,
        build: crate::build::Model,
        dry_run: bool,
    ) -> anyhow::Result<BuildExecutionReport> {
        let plan = parse_execution_plan(build.id, build.modules_delta.as_ref())?;
        let manifest_path = build_manifest_snapshot_path(build.id);
        let server_spec =
            BuildCommandSpec::from_server_plan(&plan, &self.workspace_root, manifest_path.clone());
        let admin_spec = plan
            .admin_build
            .as_ref()
            .map(|plan| {
                BuildCommandSpec::from_frontend_plan(
                    plan,
                    &self.workspace_root,
                    manifest_path.clone(),
                )
            })
            .transpose()?;
        let storefront_spec = plan
            .storefront_build
            .as_ref()
            .map(|plan| {
                BuildCommandSpec::from_frontend_plan(
                    plan,
                    &self.workspace_root,
                    manifest_path.clone(),
                )
            })
            .transpose()?;

        if dry_run {
            return Ok(report_for(
                build.id,
                "dry-run",
                &server_spec,
                admin_spec.as_ref(),
                storefront_spec.as_ref(),
            ));
        }

        self.build_service
            .update_build_status(
                build.id,
                BuildStatus::Running,
                Some(BuildStage::Checkout),
                Some(5),
            )
            .await?;
        materialize_manifest_snapshot(build.id, &build.manifest_snapshot, &manifest_path).await?;

        let mut specs = vec![("server", server_spec.clone())];
        if let Some(spec) = admin_spec.clone() {
            specs.push(("admin", spec));
        }
        if let Some(spec) = storefront_spec.clone() {
            specs.push(("storefront", spec));
        }
        let total_steps = specs.len().max(1);
        for (index, (label, spec)) in specs.iter().enumerate() {
            let progress = 15 + (((index + 1) * 75) / total_steps) as i32;
            self.build_service
                .update_build_status(
                    build.id,
                    BuildStatus::Running,
                    Some(BuildStage::Build),
                    Some(progress),
                )
                .await?;
            run_build_command(spec).await.with_context(|| {
                format!(
                    "failed to execute {label} build command for build {}",
                    build.id
                )
            })?;
        }
        self.build_service
            .update_build_status(
                build.id,
                BuildStatus::Success,
                Some(BuildStage::Complete),
                Some(100),
            )
            .await?;
        Ok(report_for(
            build.id,
            "success",
            &server_spec,
            admin_spec.as_ref(),
            storefront_spec.as_ref(),
        ))
    }
}

fn report_for(
    build_id: Uuid,
    status: &str,
    server: &BuildCommandSpec,
    admin: Option<&BuildCommandSpec>,
    storefront: Option<&BuildCommandSpec>,
) -> BuildExecutionReport {
    BuildExecutionReport {
        build_id,
        status: status.to_string(),
        cargo_command: server.render(),
        admin_command: admin.map(BuildCommandSpec::render),
        storefront_command: storefront.map(BuildCommandSpec::render),
    }
}

async fn materialize_manifest_snapshot(
    build_id: Uuid,
    snapshot: &serde_json::Value,
    manifest_path: &Path,
) -> anyhow::Result<()> {
    let mut snapshot = snapshot.clone();
    remove_null_values(&mut snapshot);
    let manifest_toml = toml::to_string_pretty(&snapshot)
        .with_context(|| format!("failed to encode manifest snapshot for build {build_id}"))?;
    if let Some(parent) = manifest_path.parent() {
        tokio::fs::create_dir_all(parent).await.with_context(|| {
            format!(
                "failed to create build manifest directory {}",
                parent.display()
            )
        })?;
    }
    tokio::fs::write(manifest_path, manifest_toml)
        .await
        .with_context(|| {
            format!(
                "failed to write build manifest snapshot {}",
                manifest_path.display()
            )
        })
}

fn remove_null_values(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => values.iter_mut().for_each(remove_null_values),
        serde_json::Value::Object(values) => {
            values.retain(|_, value| !value.is_null());
            values.values_mut().for_each(remove_null_values);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use sea_orm::{
        ActiveModelTrait, ConnectionTrait, Database, EntityTrait, IntoActiveModel, Schema, Set,
    };

    use super::BuildExecutionService;
    use crate::{
        BuildEvent, BuildEventPublisher, BuildExecutionPlan, BuildRuntimeMode, BuildService,
        BuildStatus, DeploymentProfile, build,
    };

    #[derive(Default)]
    struct CountingPublisher(AtomicUsize);

    #[async_trait::async_trait]
    impl BuildEventPublisher for CountingPublisher {
        async fn publish(&self, _event: BuildEvent) -> anyhow::Result<()> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    async fn database() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");
        let backend = db.get_database_backend();
        let schema = Schema::new(backend);
        db.execute_raw(backend.build(&schema.create_table_from_entity(build::Entity)))
            .await
            .expect("create builds table");
        db
    }

    async fn queued_build(db: &sea_orm::DatabaseConnection, valid_plan: bool) -> build::Model {
        let mut build = build::Model::new(
            "platform_state:1".to_string(),
            "test-hash".to_string(),
            1,
            serde_json::json!({"modules": {}}),
            "test-operator".to_string(),
            DeploymentProfile::HeadlessApi,
        );
        if valid_plan {
            let plan = BuildExecutionPlan {
                runtime_mode: BuildRuntimeMode::Api,
                cargo_package: "rustok-server".to_string(),
                cargo_profile: "release".to_string(),
                cargo_target: None,
                cargo_features: vec![],
                cargo_command: "cargo build -p rustok-server --release".to_string(),
                admin_build: None,
                storefront_build: None,
            };
            build.modules_delta = Some(serde_json::json!({ "execution_plan": plan }));
        }
        build
            .into_active_model()
            .insert(db)
            .await
            .expect("insert build")
    }

    #[tokio::test]
    async fn dry_run_by_id_leaves_build_unchanged_and_claimable() {
        let db = database().await;
        let build = queued_build(&db, true).await;
        let publisher = Arc::new(CountingPublisher::default());
        let executor = BuildExecutionService::new(db.clone(), publisher.clone(), ".".into());
        let manifest_path = crate::build_manifest_snapshot_path(build.id);
        assert!(!manifest_path.exists());

        let report = executor
            .execute_build(build.id, true)
            .await
            .expect("preview");
        assert_eq!(report.build_id, build.id);
        assert_eq!(report.status, "dry-run");
        assert!(report.cargo_command.contains("rustok-server"));
        assert!(
            !manifest_path.exists(),
            "dry run must not materialize a manifest"
        );
        assert_eq!(
            build::Entity::find_by_id(build.id).one(&db).await.unwrap(),
            Some(build.clone())
        );
        assert_eq!(publisher.0.load(Ordering::SeqCst), 0);

        let service = BuildService::with_event_publisher(db.clone(), publisher.clone());
        let claimed = service
            .claim_queued_build(build.id)
            .await
            .expect("claim")
            .expect("still queued");
        assert_eq!(claimed.status, BuildStatus::Running);
        assert_eq!(publisher.0.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn next_dry_run_can_be_repeated_without_claim_or_events() {
        let db = database().await;
        let publisher = Arc::new(CountingPublisher::default());
        let executor = BuildExecutionService::new(db.clone(), publisher.clone(), ".".into());
        assert!(
            executor
                .execute_next_queued_build(true)
                .await
                .unwrap()
                .is_none()
        );
        let build = queued_build(&db, true).await;
        for _ in 0..2 {
            let report = executor
                .execute_next_queued_build(true)
                .await
                .unwrap()
                .expect("preview");
            assert_eq!(report.build_id, build.id);
            assert_eq!(report.status, "dry-run");
        }
        assert_eq!(
            build::Entity::find_by_id(build.id).one(&db).await.unwrap(),
            Some(build)
        );
        assert_eq!(publisher.0.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn dry_run_does_not_preview_running_or_terminal_builds() {
        for status in [
            BuildStatus::Running,
            BuildStatus::Success,
            BuildStatus::Failed,
            BuildStatus::Cancelled,
        ] {
            let db = database().await;
            let build = queued_build(&db, true).await;
            let mut active = build.into_active_model();
            active.status = Set(status);
            let before = active.update(&db).await.expect("set test status");
            let publisher = Arc::new(CountingPublisher::default());
            let executor = BuildExecutionService::new(db.clone(), publisher.clone(), ".".into());
            assert!(executor.execute_build(before.id, true).await.is_err());
            assert_eq!(
                build::Entity::find_by_id(before.id).one(&db).await.unwrap(),
                Some(before)
            );
            assert_eq!(publisher.0.load(Ordering::SeqCst), 0);
        }
    }

    #[derive(Default)]
    struct StartFailurePublisher {
        fail_failed_event: bool,
        failed_attempts: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl BuildEventPublisher for StartFailurePublisher {
        async fn publish(&self, event: BuildEvent) -> anyhow::Result<()> {
            match event {
                BuildEvent::BuildStarted { .. } => anyhow::bail!("start delivery failed"),
                BuildEvent::BuildFailed { .. } => {
                    self.failed_attempts.fetch_add(1, Ordering::SeqCst);
                    if self.fail_failed_event {
                        anyhow::bail!("failure delivery failed");
                    }
                    Ok(())
                }
                _ => Ok(()),
            }
        }
    }

    #[tokio::test]
    async fn claim_notification_failure_marks_build_failed_before_returning() {
        for fail_failed_event in [false, true] {
            let db = database().await;
            let build = queued_build(&db, true).await;
            let publisher = Arc::new(StartFailurePublisher {
                fail_failed_event,
                ..Default::default()
            });
            let executor = BuildExecutionService::new(db.clone(), publisher.clone(), ".".into());
            let error = executor
                .execute_build(build.id, false)
                .await
                .expect_err("notification failure");
            let diagnostic = format!("{error:#}");
            assert!(diagnostic.contains("start delivery failed"));
            if fail_failed_event {
                assert!(diagnostic.contains("failure delivery failed"));
            }
            let saved = build::Entity::find_by_id(build.id)
                .one(&db)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(saved.status, BuildStatus::Failed);
            assert_eq!(
                saved.error_message.as_deref(),
                Some("Build start notification failed")
            );
            assert!(saved.finished_at.is_some());
            assert_eq!(publisher.failed_attempts.load(Ordering::SeqCst), 1);
        }
    }

    struct CancelThenFailPublisher(sea_orm::DatabaseConnection);

    #[async_trait::async_trait]
    impl BuildEventPublisher for CancelThenFailPublisher {
        async fn publish(&self, event: BuildEvent) -> anyhow::Result<()> {
            if let BuildEvent::BuildStarted { build_id, .. } = event {
                let mut active = build::Entity::find_by_id(build_id)
                    .one(&self.0)
                    .await?
                    .expect("claimed build")
                    .into_active_model();
                active.status = Set(BuildStatus::Cancelled);
                active.update(&self.0).await?;
                anyhow::bail!("start delivery failed after concurrent cancellation");
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn claim_failure_does_not_overwrite_a_terminal_transition() {
        let db = database().await;
        let build = queued_build(&db, true).await;
        let executor = BuildExecutionService::new(
            db.clone(),
            Arc::new(CancelThenFailPublisher(db.clone())),
            ".".into(),
        );
        assert!(executor.execute_build(build.id, false).await.is_err());
        let saved = build::Entity::find_by_id(build.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.status, BuildStatus::Cancelled);
        assert_eq!(saved.error_message, None);
    }

    struct CompletionFailurePublisher;

    #[async_trait::async_trait]
    impl BuildEventPublisher for CompletionFailurePublisher {
        async fn publish(&self, event: BuildEvent) -> anyhow::Result<()> {
            if matches!(event, BuildEvent::BuildCompleted { .. }) {
                anyhow::bail!("completion delivery failed");
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn failed_completion_notification_does_not_undo_committed_success() {
        let db = database().await;
        let build = queued_build(&db, true).await;
        let service =
            BuildService::with_event_publisher(db.clone(), Arc::new(CompletionFailurePublisher));
        service.claim_queued_build(build.id).await.unwrap().unwrap();
        assert!(
            service
                .update_build_status(build.id, BuildStatus::Success, None, Some(100))
                .await
                .is_err()
        );
        service
            .fail_build(build.id, "notification failed".to_string())
            .await
            .unwrap();
        let saved = build::Entity::find_by_id(build.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.status, BuildStatus::Success);
        assert_eq!(saved.progress, 100);
        assert_eq!(saved.error_message, None);
    }

    #[tokio::test]
    async fn invalid_plan_after_claim_is_recorded_as_failure() {
        for by_id in [false, true] {
            let db = database().await;
            let build = queued_build(&db, false).await;
            let publisher = Arc::new(CountingPublisher::default());
            let executor = BuildExecutionService::new(db.clone(), publisher.clone(), ".".into());
            let error = if by_id {
                executor
                    .execute_build(build.id, false)
                    .await
                    .expect_err("invalid plan")
            } else {
                executor
                    .execute_next_queued_build(false)
                    .await
                    .expect_err("invalid plan")
            };
            assert!(format!("{error:#}").contains("execution metadata"));
            let saved = build::Entity::find_by_id(build.id)
                .one(&db)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(saved.status, BuildStatus::Failed);
            assert_eq!(
                saved.error_message.as_deref(),
                Some("Build execution failed")
            );
            assert!(saved.finished_at.is_some());
            assert_eq!(publisher.0.load(Ordering::SeqCst), 2);
        }
    }

    #[tokio::test]
    async fn manifest_serialization_failure_after_claim_is_recorded() {
        let db = database().await;
        let build = queued_build(&db, true).await;
        let mut active = build.into_active_model();
        active.manifest_snapshot = Set(serde_json::json!("not a toml table"));
        let build = active.update(&db).await.unwrap();
        let publisher = Arc::new(CountingPublisher::default());
        let executor = BuildExecutionService::new(db.clone(), publisher.clone(), ".".into());
        let error = executor
            .execute_build(build.id, false)
            .await
            .expect_err("invalid manifest");
        assert!(format!("{error:#}").contains("encode manifest snapshot"));
        let saved = build::Entity::find_by_id(build.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.status, BuildStatus::Failed);
        assert_eq!(
            saved.error_message.as_deref(),
            Some("Build execution failed")
        );
        assert_eq!(publisher.0.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn command_spawn_failure_after_claim_is_recorded() {
        let db = database().await;
        let build = queued_build(&db, true).await;
        let publisher = Arc::new(CountingPublisher::default());
        let missing_workspace =
            std::env::temp_dir().join(format!("rustok-missing-workspace-{}", build.id));
        assert!(!missing_workspace.exists());
        let executor = BuildExecutionService::new(db.clone(), publisher.clone(), missing_workspace);
        let outcome = executor.execute_build(build.id, false).await;
        let manifest_path = crate::build_manifest_snapshot_path(build.id);
        if let Some(parent) = manifest_path.parent() {
            tokio::fs::remove_dir_all(parent)
                .await
                .expect("remove test manifest");
        }
        let error = outcome.expect_err("missing workspace must fail");
        assert!(format!("{error:#}").contains("failed to execute server build command"));
        let saved = build::Entity::find_by_id(build.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.status, BuildStatus::Failed);
        assert_eq!(
            saved.error_message.as_deref(),
            Some("Build execution failed")
        );
        assert_eq!(publisher.0.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn status_updates_reject_terminal_or_invalid_transitions() {
        let db = database().await;
        let build = queued_build(&db, true).await;
        let publisher = Arc::new(CountingPublisher::default());
        let service = BuildService::with_event_publisher(db.clone(), publisher.clone());
        assert!(
            service
                .update_build_status(uuid::Uuid::new_v4(), BuildStatus::Running, None, None)
                .await
                .is_err()
        );
        assert!(
            service
                .update_build_status(build.id, BuildStatus::Success, None, None)
                .await
                .is_err()
        );
        assert_eq!(publisher.0.load(Ordering::SeqCst), 0);
        service
            .update_build_status(build.id, BuildStatus::Cancelled, None, None)
            .await
            .unwrap();
        let cancelled = build::Entity::find_by_id(build.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert!(
            service
                .update_build_status(build.id, BuildStatus::Running, None, None)
                .await
                .is_err()
        );
        service
            .fail_build(build.id, "must not replace cancelled".to_string())
            .await
            .unwrap();
        assert_eq!(
            build::Entity::find_by_id(build.id).one(&db).await.unwrap(),
            Some(cancelled)
        );
        assert_eq!(publisher.0.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn invalid_dry_run_plan_does_not_change_or_publish_build() {
        let db = database().await;
        let build = queued_build(&db, false).await;
        let publisher = Arc::new(CountingPublisher::default());
        let executor = BuildExecutionService::new(db.clone(), publisher.clone(), ".".into());
        assert!(executor.execute_build(build.id, true).await.is_err());
        assert!(executor.execute_next_queued_build(true).await.is_err());
        assert_eq!(
            build::Entity::find_by_id(build.id).one(&db).await.unwrap(),
            Some(build)
        );
        assert_eq!(publisher.0.load(Ordering::SeqCst), 0);
    }
}
