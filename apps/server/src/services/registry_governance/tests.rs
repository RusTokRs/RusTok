#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::registry_governance_event::{
        self as registry_governance_event, ActiveModel as RegistryGovernanceEventActiveModel,
    };
    use crate::models::registry_module_owner::{
        self as registry_module_owner, ActiveModel as RegistryModuleOwnerActiveModel,
    };
    use crate::models::registry_module_release::{
        Entity as RegistryModuleReleaseEntity, RegistryModuleReleaseStatus,
    };
    use crate::models::registry_publish_request::{
        self, ActiveModel as RegistryPublishRequestActiveModel, RegistryPublishRequestStatus,
    };
    use crate::models::registry_publish_request_translation::ActiveModel as RegistryPublishRequestTranslationActiveModel;
    use crate::models::registry_validation_job::{
        self as registry_validation_job, ActiveModel as RegistryValidationJobActiveModel,
        Entity as RegistryValidationJobEntity, RegistryValidationJobStatus,
    };
    use crate::models::registry_validation_stage::{
        self as registry_validation_stage, ActiveModel as RegistryValidationStageActiveModel,
        Entity as RegistryValidationStageEntity, RegistryValidationStageStatus,
    };
    use crate::services::registry_principal::{RegistryAuthority, RegistryPrincipalRef};
    use chrono::{Duration, Utc};
    use rustok_migrations::Migrator;
    use rustok_storage::{local::LocalStorage, StorageService};
    use rustok_test_utils::db::{setup_test_db, setup_test_db_with_migrations};
    use sea_orm::{
        ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
        QueryFilter, QueryOrder, Set, Statement,
    };
    use sha2::Digest;

    const SAMPLE_DEFAULT_LOCALE: &str = "en";
    const SAMPLE_MODULE_NAME: &str = "Blog";
    const SAMPLE_MODULE_DESCRIPTION: &str = "Blog module description long enough for validation.";

    fn principal_json(label: &str) -> serde_json::Value {
        actor_principal(label).to_json_value()
    }

    fn request_actor_label(request: &registry_publish_request::Model) -> String {
        principal_display_label(&request.requested_by)
    }

    fn authority_from_actor(actor: &str) -> RegistryAuthority {
        RegistryAuthority {
            principal: RegistryPrincipalRef::from_legacy_value(actor),
            can_manage_modules: false,
        }
    }

    fn temp_storage_service() -> StorageService {
        let base_dir = std::env::temp_dir().join(format!(
            "rustok-registry-storage-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&base_dir).unwrap();
        StorageService::new(LocalStorage::new(base_dir, "/media"))
    }

    async fn setup_registry_metadata_db() -> DatabaseConnection {
        let db = setup_test_db().await;
        db.execute(Statement::from_string(
            db.get_database_backend(),
            r#"
            CREATE TABLE registry_publish_request_translations (
                request_id TEXT NOT NULL,
                locale TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (request_id, locale)
            )
            "#
            .to_string(),
        ))
        .await
        .unwrap();
        db
    }

    #[tokio::test]
    async fn artifact_bundle_validation_accepts_matching_bundle() {
        let db = setup_registry_metadata_db().await;
        let request = sample_publish_request_model();
        insert_publish_request_translation(
            &db,
            &request.id,
            SAMPLE_DEFAULT_LOCALE,
            SAMPLE_MODULE_NAME,
            SAMPLE_MODULE_DESCRIPTION,
        )
        .await;
        let artifact = RegistryArtifactUpload {
            content_type: "application/json".to_string(),
            bytes: bytes::Bytes::from(sample_publish_artifact_json("blog", true).into_bytes()),
        };

        let validation = validate_registry_artifact(&db, &request, &artifact)
            .await
            .unwrap();

        assert!(validation.errors.is_empty(), "{:?}", validation.errors);
    }

    #[tokio::test]
    async fn alloy_artifact_validation_accepts_only_workspace_delivery() {
        let db = setup_registry_metadata_db().await;
        let mut request = sample_publish_request_model();
        request.artifact_origin = "alloy_authored".to_string();
        insert_publish_request_translation(
            &db,
            &request.id,
            SAMPLE_DEFAULT_LOCALE,
            SAMPLE_MODULE_NAME,
            SAMPLE_MODULE_DESCRIPTION,
        )
        .await;
        let artifact = RegistryArtifactUpload {
            content_type: rustok_modules::MODULE_ARTIFACT_RHAI_WORKSPACE_MEDIA_TYPE.to_string(),
            bytes: bytes::Bytes::from_static(
                br#"{"schema_version":1,"entrypoint":"src/main.rhai","files":[{"path":"src/main.rhai","kind":"source","contents":"40 + 2"}]}"#,
            ),
        };

        let validation = validate_registry_artifact(&db, &request, &artifact)
            .await
            .unwrap();

        assert!(validation.errors.is_empty(), "{:?}", validation.errors);
    }

    #[tokio::test]
    async fn artifact_bundle_validation_rejects_mismatched_slug() {
        let db = setup_registry_metadata_db().await;
        let request = sample_publish_request_model();
        insert_publish_request_translation(
            &db,
            &request.id,
            SAMPLE_DEFAULT_LOCALE,
            SAMPLE_MODULE_NAME,
            SAMPLE_MODULE_DESCRIPTION,
        )
        .await;
        let artifact = RegistryArtifactUpload {
            content_type: "application/json".to_string(),
            bytes: bytes::Bytes::from(sample_publish_artifact_json("forum", true).into_bytes()),
        };

        let validation = validate_registry_artifact(&db, &request, &artifact)
            .await
            .unwrap();

        assert!(
            validation
                .errors
                .iter()
                .any(|error| error.contains("module.slug")),
            "{:?}",
            validation.errors
        );
    }

    #[tokio::test]
    async fn artifact_bundle_validation_rejects_missing_requested_admin_manifest() {
        let db = setup_registry_metadata_db().await;
        let request = sample_publish_request_model();
        insert_publish_request_translation(
            &db,
            &request.id,
            SAMPLE_DEFAULT_LOCALE,
            SAMPLE_MODULE_NAME,
            SAMPLE_MODULE_DESCRIPTION,
        )
        .await;
        let artifact = RegistryArtifactUpload {
            content_type: "application/json".to_string(),
            bytes: bytes::Bytes::from(sample_publish_artifact_json("blog", false).into_bytes()),
        };

        let validation = validate_registry_artifact(&db, &request, &artifact)
            .await
            .unwrap();

        assert!(
            validation
                .errors
                .iter()
                .any(|error| error.contains("admin/Cargo.toml")),
            "{:?}",
            validation.errors
        );
    }

    #[tokio::test]
    async fn artifact_bundle_validation_rejects_oversized_bundle_before_parsing() {
        let db = setup_registry_metadata_db().await;
        let request = sample_publish_request_model();
        let artifact = RegistryArtifactUpload {
            content_type: "application/json".to_string(),
            bytes: bytes::Bytes::from(vec![b'x'; MODULE_PUBLISH_ARTIFACT_MAX_BYTES + 1]),
        };

        let validation = validate_registry_artifact(&db, &request, &artifact)
            .await
            .unwrap();

        assert_eq!(
            validation.errors,
            vec![format!(
                "Artifact bundle exceeds the {} byte validation limit.",
                MODULE_PUBLISH_ARTIFACT_MAX_BYTES
            )]
        );
    }

    #[tokio::test]
    async fn artifact_bundle_validation_keeps_unsupported_type_content_free() {
        let db = setup_registry_metadata_db().await;
        let request = sample_publish_request_model();
        insert_publish_request_translation(
            &db,
            &request.id,
            SAMPLE_DEFAULT_LOCALE,
            SAMPLE_MODULE_NAME,
            SAMPLE_MODULE_DESCRIPTION,
        )
        .await;
        let artifact = RegistryArtifactUpload {
            content_type: "application/json".to_string(),
            bytes: bytes::Bytes::from(sample_publish_artifact_json("blog", true).replace(
                rustok_modules::MODULE_PUBLISH_BUNDLE_TYPE,
                "untrusted-artifact-type",
            )),
        };

        let validation = validate_registry_artifact(&db, &request, &artifact)
            .await
            .unwrap();

        assert!(validation
            .errors
            .iter()
            .any(|error| error == "Artifact bundle type is unsupported."));
        assert!(validation
            .errors
            .iter()
            .all(|error| !error.contains("untrusted-artifact-type")));
    }

    #[test]
    fn rejected_publish_request_can_retry_after_validation_failure() {
        assert!(rejected_publish_request_can_retry(
            Some("validation_failed"),
            Some("Validation job failed before bundle checks: missing artifact"),
        ));
        assert!(rejected_publish_request_can_retry(
            None,
            Some("Validation job failed before bundle checks: missing artifact"),
        ));
    }

    #[test]
    fn rejected_publish_request_cannot_retry_after_manual_governance_reject() {
        assert!(!rejected_publish_request_can_retry(
            Some("request_rejected"),
            Some("Governance rejection reason: owner mismatch"),
        ));
    }

    #[test]
    fn normalize_required_reason_rejects_blank_values() {
        let error = normalize_required_reason("   ", "Registry publish reject")
            .expect_err("blank reason should be rejected");

        assert!(error
            .to_string()
            .contains("Registry publish reject requires a non-empty reason"));
    }

    #[test]
    fn normalize_required_reason_trims_non_empty_values() {
        let reason =
            normalize_required_reason("  Needs manual review  ", "Registry publish reject")
                .expect("non-empty reason should normalize");

        assert_eq!(reason, "Needs manual review");
    }

    #[test]
    fn derive_follow_up_gate_snapshots_reads_latest_gate_events() {
        let now = Utc::now();
        let mut request = sample_publish_request_model();
        request.status = RegistryPublishRequestStatus::Approved;
        request.validated_at = Some(now);
        let events = vec![
            registry_governance_event::Model {
                id: "rge_compile".to_string(),
                slug: "blog".to_string(),
                request_id: Some(request.id.clone()),
                release_id: None,
                event_type: "follow_up_gate_queued".to_string(),
                actor: principal_json("xtask:module-publish"),
                publisher: None,
                details: serde_json::json!({
                    "stage_key": "compile_smoke",
                    "status": "pending",
                    "detail": "Compile smoke still runs outside the current registry validator."
                }),
                created_at: now,
            },
            registry_governance_event::Model {
                id: "rge_tests".to_string(),
                slug: "blog".to_string(),
                request_id: Some(request.id.clone()),
                release_id: None,
                event_type: "follow_up_gate_failed".to_string(),
                actor: principal_json("governance:moderator"),
                publisher: None,
                details: serde_json::json!({
                    "stage_key": "targeted_tests",
                    "status": "failed",
                    "detail": "Targeted tests failed in CI."
                }),
                created_at: now,
            },
        ];

        let validation_stages = derive_validation_stage_snapshots(Some(&request), &events, &[]);
        let snapshots =
            derive_follow_up_gate_snapshots(Some(&request), &events, &validation_stages);

        assert_eq!(snapshots.len(), 3);
        assert_eq!(
            snapshots
                .iter()
                .find(|gate| gate.key == "compile_smoke")
                .map(|gate| gate.status.as_str()),
            Some("pending")
        );
        assert_eq!(
            snapshots
                .iter()
                .find(|gate| gate.key == "targeted_tests")
                .map(|gate| gate.status.as_str()),
            Some("failed")
        );
        assert_eq!(
            snapshots
                .iter()
                .find(|gate| gate.key == "security_policy_review")
                .map(|gate| gate.status.as_str()),
            Some("pending")
        );
    }

    #[tokio::test]
    async fn validate_publish_request_queues_single_active_validation_job() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let request = insert_publish_request(&db, RegistryPublishRequestStatus::Submitted).await;
        let service = RegistryGovernanceService::new(db.clone());
        let actor = request_actor_label(&request);
        let authority = authority_from_actor(&actor);

        let queued = service
            .validate_publish_request(&request.id, &authority)
            .await
            .unwrap();
        assert!(queued.queued);
        assert!(queued.validation_job_id.is_some());
        assert_eq!(
            queued.request.status,
            RegistryPublishRequestStatus::Validating
        );

        let jobs = RegistryValidationJobEntity::find()
            .filter(registry_validation_job::Column::RequestId.eq(request.id.clone()))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].status, RegistryValidationJobStatus::Queued);
        assert_eq!(jobs[0].attempt_number, 1);

        let second = service
            .validate_publish_request(&request.id, &authority)
            .await
            .unwrap();
        assert!(!second.queued);
        assert_eq!(
            second.request.status,
            RegistryPublishRequestStatus::Validating
        );
        assert_eq!(second.validation_job_id, queued.validation_job_id);

        let jobs = RegistryValidationJobEntity::find()
            .filter(registry_validation_job::Column::RequestId.eq(request.id))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(jobs.len(), 1);
    }

    #[tokio::test]
    async fn validate_publish_request_requeues_after_automated_failure_with_incremented_attempt() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let request = insert_publish_request(&db, RegistryPublishRequestStatus::Rejected).await;
        insert_failed_validation_job(&db, &request).await;
        insert_validation_failed_event(&db, &request).await;
        let service = RegistryGovernanceService::new(db.clone());
        let actor = request_actor_label(&request);
        let authority = authority_from_actor(&actor);

        let queued = service
            .validate_publish_request(&request.id, &authority)
            .await
            .unwrap();

        assert!(queued.queued);
        assert_eq!(
            queued.request.status,
            RegistryPublishRequestStatus::Validating
        );

        let jobs = RegistryValidationJobEntity::find()
            .filter(registry_validation_job::Column::RequestId.eq(request.id))
            .order_by_asc(registry_validation_job::Column::AttemptNumber)
            .all(&db)
            .await
            .unwrap();
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].status, RegistryValidationJobStatus::Failed);
        assert_eq!(jobs[0].attempt_number, 1);
        assert_eq!(jobs[1].status, RegistryValidationJobStatus::Queued);
        assert_eq!(jobs[1].attempt_number, 2);
        assert_eq!(jobs[1].queue_reason, "requeued_after_validation_failed");
    }

    #[tokio::test]
    async fn validate_publish_request_recovers_a_stale_running_job() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let request = insert_publish_request(&db, RegistryPublishRequestStatus::Validating).await;
        insert_stale_running_validation_job(&db, &request).await;
        let service = RegistryGovernanceService::new(db.clone());
        let actor = request_actor_label(&request);
        let authority = authority_from_actor(&actor);

        let queued = service
            .validate_publish_request(&request.id, &authority)
            .await
            .unwrap();

        assert!(queued.queued);
        let jobs = RegistryValidationJobEntity::find()
            .filter(registry_validation_job::Column::RequestId.eq(request.id))
            .order_by_asc(registry_validation_job::Column::AttemptNumber)
            .all(&db)
            .await
            .unwrap();
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].status, RegistryValidationJobStatus::Failed);
        assert_eq!(
            jobs[0].last_error.as_deref(),
            Some("validation_worker_lease_expired")
        );
        assert_eq!(jobs[1].status, RegistryValidationJobStatus::Queued);
        assert_eq!(jobs[1].attempt_number, 2);
        assert_eq!(
            jobs[1].queue_reason,
            "requeued_after_validation_lease_expired"
        );
    }

    #[tokio::test]
    async fn report_validation_stage_requeue_increments_attempt_number() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let request = insert_publish_request(&db, RegistryPublishRequestStatus::Approved).await;
        let actor = request_actor_label(&request);
        insert_registry_owner_binding(&db, &request.slug, &actor).await;
        insert_validation_stage(
            &db,
            &request,
            "compile_smoke",
            RegistryValidationStageStatus::Queued,
            1,
            "Compile smoke queued.",
        )
        .await;
        let service = RegistryGovernanceService::new(db.clone());
        let authority = authority_from_actor(&actor);

        let failed = service
            .report_validation_stage(
                &request.id,
                &authority,
                "compile_smoke",
                "failed",
                Some("Compile smoke failed in CI."),
                None,
                false,
            )
            .await
            .unwrap();
        assert_eq!(failed.stage.attempt_number, 1);
        assert_eq!(failed.stage.status, RegistryValidationStageStatus::Failed);

        let requeued = service
            .report_validation_stage(
                &request.id,
                &authority,
                "compile_smoke",
                "queued",
                Some("Compile smoke queued again after fixes."),
                None,
                true,
            )
            .await
            .unwrap();
        assert_eq!(requeued.stage.attempt_number, 2);
        assert_eq!(requeued.stage.status, RegistryValidationStageStatus::Queued);

        let stages = RegistryValidationStageEntity::find()
            .filter(registry_validation_stage::Column::RequestId.eq(request.id))
            .filter(registry_validation_stage::Column::StageKey.eq("compile_smoke"))
            .order_by_asc(registry_validation_stage::Column::AttemptNumber)
            .all(&db)
            .await
            .unwrap();
        assert_eq!(stages.len(), 2);
        assert_eq!(stages[0].status, RegistryValidationStageStatus::Failed);
        assert_eq!(stages[1].status, RegistryValidationStageStatus::Queued);
    }

    #[tokio::test]
    async fn requeue_expired_remote_validation_claims_blocks_current_attempt_and_queues_next() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let request = insert_publish_request(&db, RegistryPublishRequestStatus::Approved).await;
        let now = Utc::now();
        RegistryValidationStageActiveModel {
            id: Set(format!("rvs_{}", uuid::Uuid::new_v4().simple())),
            request_id: Set(request.id.clone()),
            slug: Set(request.slug.clone()),
            version: Set(request.version.clone()),
            stage_key: Set("compile_smoke".to_string()),
            status: Set(RegistryValidationStageStatus::Running),
            triggered_by: Set("remote-runner:worker-1".to_string()),
            queue_reason: Set("validation_passed".to_string()),
            attempt_number: Set(1),
            detail: Set("Remote runner is processing compile smoke.".to_string()),
            started_at: Set(Some(now)),
            finished_at: Set(None),
            last_error: Set(None),
            claim_id: Set(Some("rvc_test".to_string())),
            claimed_by: Set(Some("worker-1".to_string())),
            claim_expires_at: Set(Some(now - Duration::seconds(5))),
            last_heartbeat_at: Set(Some(now - Duration::seconds(10))),
            runner_kind: Set(Some("remote".to_string())),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&db)
        .await
        .unwrap();

        let requeued = rustok_modules::SeaOrmModuleGovernanceService::new(db.clone())
            .requeue_expired_remote_validation_claims()
            .await
            .unwrap();
        assert_eq!(requeued, 1);

        let stages = RegistryValidationStageEntity::find()
            .filter(registry_validation_stage::Column::RequestId.eq(request.id))
            .filter(registry_validation_stage::Column::StageKey.eq("compile_smoke"))
            .order_by_asc(registry_validation_stage::Column::AttemptNumber)
            .all(&db)
            .await
            .unwrap();
        assert_eq!(stages.len(), 2);
        assert_eq!(stages[0].status, RegistryValidationStageStatus::Blocked);
        assert_eq!(stages[1].status, RegistryValidationStageStatus::Queued);
        assert_eq!(stages[1].attempt_number, 2);
        assert!(stages[1].claim_id.is_none());
    }

    #[tokio::test]
    async fn complete_remote_validation_stage_rejects_expired_claim() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let request = insert_publish_request(&db, RegistryPublishRequestStatus::Approved).await;
        insert_remote_running_validation_stage(
            &db,
            &request,
            "compile_smoke",
            "rvc_expired",
            "worker-1",
            Utc::now() - Duration::seconds(5),
        )
        .await;
        let service = RegistryGovernanceService::new(db);

        let error = service
            .complete_remote_validation_stage(
                "rvc_expired",
                "worker-1",
                Some("Compile smoke passed."),
                Some("local_runner_passed"),
            )
            .await
            .expect_err("expired claim should be rejected");

        assert!(error
            .to_string()
            .contains("Remote validation claim 'rvc_expired' has expired"));
    }

    #[tokio::test]
    async fn complete_remote_validation_stage_rejects_duplicate_completion() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let request = insert_publish_request(&db, RegistryPublishRequestStatus::Approved).await;
        insert_remote_running_validation_stage(
            &db,
            &request,
            "compile_smoke",
            "rvc_duplicate",
            "worker-1",
            Utc::now() + Duration::minutes(5),
        )
        .await;
        let service = RegistryGovernanceService::new(db.clone());

        let completed = service
            .complete_remote_validation_stage(
                "rvc_duplicate",
                "worker-1",
                Some("Compile smoke passed."),
                Some("local_runner_passed"),
            )
            .await
            .expect("first completion should succeed");
        assert_eq!(
            completed.stage.status,
            RegistryValidationStageStatus::Passed
        );

        let error = service
            .complete_remote_validation_stage(
                "rvc_duplicate",
                "worker-1",
                Some("Compile smoke passed again."),
                Some("local_runner_passed"),
            )
            .await
            .expect_err("duplicate completion should be rejected");

        assert!(error
            .to_string()
            .contains("Remote validation claim 'rvc_duplicate' was not found"));
    }

    #[tokio::test]
    async fn review_governance_actor_cannot_yank_published_release() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let request = insert_publish_request(&db, RegistryPublishRequestStatus::Published).await;
        insert_registry_owner_binding(&db, &request.slug, "owner:blog").await;
        insert_active_release(&db, &request, "publisher:blog").await;
        let service = RegistryGovernanceService::new(db.clone());
        let authority = authority_from_actor("governance:moderator");

        let error = service
            .yank_release(
                &request.slug,
                &request.version,
                "Moderation review requested a rollback.",
                "rollback",
                &authority,
            )
            .await
            .expect_err("review governance actor should not manage published releases");

        assert!(error
            .to_string()
            .contains("is not allowed to yank published release"));

        let release = RegistryModuleReleaseEntity::find()
            .filter(registry_module_release::Column::Slug.eq(request.slug))
            .filter(registry_module_release::Column::Version.eq(request.version))
            .one(&db)
            .await
            .unwrap()
            .expect("release should still exist");
        assert_eq!(release.status, RegistryModuleReleaseStatus::Active);
        assert!(release.yanked_at.is_none());
    }

    #[tokio::test]
    async fn lifecycle_snapshot_prefers_persisted_validation_stages() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let request = insert_publish_request(&db, RegistryPublishRequestStatus::Approved).await;
        insert_validation_stage(
            &db,
            &request,
            "compile_smoke",
            RegistryValidationStageStatus::Blocked,
            1,
            "Compile smoke is blocked on an external runner.",
        )
        .await;
        insert_follow_up_gate_event(
            &db,
            &request,
            "compile_smoke",
            "follow_up_gate_passed",
            "passed",
            "Legacy event should be ignored once persisted stages exist.",
        )
        .await;
        let service = RegistryGovernanceService::new(db.clone());

        let snapshot = service
            .lifecycle_snapshot(&request.slug)
            .await
            .unwrap()
            .expect("lifecycle snapshot");

        assert_eq!(
            snapshot
                .validation_stages
                .iter()
                .find(|stage| stage.key == "compile_smoke")
                .map(|stage| stage.status.as_str()),
            Some("blocked")
        );
        assert_eq!(
            snapshot
                .follow_up_gates
                .iter()
                .find(|stage| stage.key == "compile_smoke")
                .map(|stage| stage.status.as_str()),
            Some("blocked")
        );
    }

    fn sample_publish_request_model() -> registry_publish_request::Model {
        registry_publish_request::Model {
            id: "rpr_test".to_string(),
            slug: "blog".to_string(),
            version: "0.1.0".to_string(),
            crate_name: "rustok-blog".to_string(),
            default_locale: SAMPLE_DEFAULT_LOCALE.to_string(),
            ownership: "first_party".to_string(),
            trust_level: "core".to_string(),
            license: "MIT".to_string(),
            entry_type: Some("backend".to_string()),
            artifact_origin: "platform_built".to_string(),
            marketplace: serde_json::json!({
                "category": "content",
                "tags": ["blog", "content"]
            }),
            ui_packages: serde_json::json!({
                "admin": { "crate_name": "rustok-blog-admin" },
                "storefront": { "crate_name": "rustok-blog-storefront" }
            }),
            status: RegistryPublishRequestStatus::Draft,
            requested_by: principal_json("user:00000000-0000-0000-0000-000000000111"),
            publisher_principal: Some(principal_json("user:00000000-0000-0000-0000-000000000111")),
            approved_by: None,
            rejected_by: None,
            rejection_reason: None,
            changes_requested_by: None,
            changes_requested_reason: None,
            changes_requested_reason_code: None,
            changes_requested_at: None,
            held_by: None,
            held_reason: None,
            held_reason_code: None,
            held_at: None,
            held_from_status: None,
            validation_warnings: serde_json::json!([]),
            validation_errors: serde_json::json!([]),
            artifact_storage_key: None,
            artifact_checksum_sha256: None,
            artifact_size: None,
            artifact_content_type: None,
            submitted_at: None,
            validated_at: None,
            approved_at: None,
            published_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    async fn insert_publish_request(
        db: &DatabaseConnection,
        status: RegistryPublishRequestStatus,
    ) -> registry_publish_request::Model {
        let now = Utc::now();
        let mut request = sample_publish_request_model();
        request.id = format!("rpr_{}", uuid::Uuid::new_v4().simple());
        request.status = status;
        request.created_at = now;
        request.updated_at = now;
        request.submitted_at = matches!(
            &request.status,
            RegistryPublishRequestStatus::Submitted
                | RegistryPublishRequestStatus::Validating
                | RegistryPublishRequestStatus::Approved
                | RegistryPublishRequestStatus::ChangesRequested
                | RegistryPublishRequestStatus::OnHold
                | RegistryPublishRequestStatus::Rejected
                | RegistryPublishRequestStatus::Published
        )
        .then_some(now);
        request.validated_at = matches!(
            &request.status,
            RegistryPublishRequestStatus::Approved
                | RegistryPublishRequestStatus::ChangesRequested
                | RegistryPublishRequestStatus::OnHold
                | RegistryPublishRequestStatus::Rejected
                | RegistryPublishRequestStatus::Published
        )
        .then_some(now);
        let active: RegistryPublishRequestActiveModel = request.clone().into();
        let request = active.insert(db).await.unwrap();
        insert_publish_request_translation(
            db,
            &request.id,
            SAMPLE_DEFAULT_LOCALE,
            SAMPLE_MODULE_NAME,
            SAMPLE_MODULE_DESCRIPTION,
        )
        .await;
        request
    }

    async fn insert_publish_request_with_artifact(
        db: &DatabaseConnection,
        storage: &StorageService,
        status: RegistryPublishRequestStatus,
        artifact_json: String,
    ) -> registry_publish_request::Model {
        let mut request = insert_publish_request(db, status).await;
        let artifact_storage_key =
            registry_artifact_storage_key(&request.id, &request.slug, &request.version);
        let artifact_bytes = bytes::Bytes::from(artifact_json.into_bytes());
        let artifact_checksum_sha256 = hex::encode(sha2::Sha256::digest(&artifact_bytes));
        let uploaded = storage
            .store(&artifact_storage_key, artifact_bytes, "application/json")
            .await
            .unwrap();
        let mut active: RegistryPublishRequestActiveModel = request.clone().into();
        active.artifact_storage_key = Set(Some(uploaded.path));
        active.artifact_checksum_sha256 = Set(Some(artifact_checksum_sha256));
        active.artifact_size = Set(Some(uploaded.size as i64));
        active.artifact_content_type = Set(Some("application/json".to_string()));
        active.updated_at = Set(Utc::now());
        request = active.update(db).await.unwrap();
        request
    }

    async fn insert_failed_validation_job(
        db: &DatabaseConnection,
        request: &registry_publish_request::Model,
    ) {
        let now = Utc::now();
        let active = RegistryValidationJobActiveModel {
            id: Set(format!("rvj_{}", uuid::Uuid::new_v4().simple())),
            request_id: Set(request.id.clone()),
            slug: Set(request.slug.clone()),
            version: Set(request.version.clone()),
            status: Set(RegistryValidationJobStatus::Failed),
            triggered_by: Set(request_actor_label(request)),
            queue_reason: Set("initial_validation".to_string()),
            attempt_number: Set(1),
            started_at: Set(Some(now)),
            finished_at: Set(Some(now)),
            last_error: Set(Some("Validation failed".to_string())),
            created_at: Set(now),
            updated_at: Set(now),
        };
        active.insert(db).await.unwrap();
    }

    async fn insert_stale_running_validation_job(
        db: &DatabaseConnection,
        request: &registry_publish_request::Model,
    ) {
        let now = Utc::now();
        let started_at = now - Duration::minutes(16);
        let active = RegistryValidationJobActiveModel {
            id: Set(format!("rvj_{}", uuid::Uuid::new_v4().simple())),
            request_id: Set(request.id.clone()),
            slug: Set(request.slug.clone()),
            version: Set(request.version.clone()),
            status: Set(RegistryValidationJobStatus::Running),
            triggered_by: Set(request_actor_label(request)),
            queue_reason: Set("initial_validation".to_string()),
            attempt_number: Set(1),
            started_at: Set(Some(started_at)),
            finished_at: Set(None),
            last_error: Set(None),
            created_at: Set(started_at),
            updated_at: Set(started_at),
        };
        active.insert(db).await.unwrap();
    }

    async fn insert_validation_failed_event(
        db: &DatabaseConnection,
        request: &registry_publish_request::Model,
    ) {
        let active = RegistryGovernanceEventActiveModel {
            id: Set(format!("rge_{}", uuid::Uuid::new_v4().simple())),
            slug: Set(request.slug.clone()),
            request_id: Set(Some(request.id.clone())),
            release_id: Set(None),
            event_type: Set("validation_failed".to_string()),
            actor: Set(request.requested_by.clone()),
            publisher: Set(None),
            details: Set(serde_json::json!({
                "version": request.version.clone(),
                "status": "rejected",
                "errors": ["Validation failed"],
            })),
            created_at: Set(Utc::now()),
        };
        active.insert(db).await.unwrap();
    }

    async fn insert_validation_stage(
        db: &DatabaseConnection,
        request: &registry_publish_request::Model,
        stage_key: &str,
        status: RegistryValidationStageStatus,
        attempt_number: i32,
        detail: &str,
    ) {
        let now = Utc::now();
        let active = RegistryValidationStageActiveModel {
            id: Set(format!("rvs_{}", uuid::Uuid::new_v4().simple())),
            request_id: Set(request.id.clone()),
            slug: Set(request.slug.clone()),
            version: Set(request.version.clone()),
            stage_key: Set(stage_key.to_string()),
            status: Set(status.clone()),
            triggered_by: Set(request_actor_label(request)),
            queue_reason: Set("test_setup".to_string()),
            attempt_number: Set(attempt_number),
            detail: Set(detail.to_string()),
            started_at: Set(None),
            finished_at: Set(matches!(
                status,
                RegistryValidationStageStatus::Passed
                    | RegistryValidationStageStatus::Failed
                    | RegistryValidationStageStatus::Blocked
            )
            .then_some(now)),
            last_error: Set(matches!(status, RegistryValidationStageStatus::Failed)
                .then_some(detail.to_string())),
            claim_id: Set(None),
            claimed_by: Set(None),
            claim_expires_at: Set(None),
            last_heartbeat_at: Set(None),
            runner_kind: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };
        active.insert(db).await.unwrap();
    }

    async fn insert_remote_running_validation_stage(
        db: &DatabaseConnection,
        request: &registry_publish_request::Model,
        stage_key: &str,
        claim_id: &str,
        runner_id: &str,
        claim_expires_at: chrono::DateTime<Utc>,
    ) {
        let now = Utc::now();
        let active = RegistryValidationStageActiveModel {
            id: Set(format!("rvs_{}", uuid::Uuid::new_v4().simple())),
            request_id: Set(request.id.clone()),
            slug: Set(request.slug.clone()),
            version: Set(request.version.clone()),
            stage_key: Set(stage_key.to_string()),
            status: Set(RegistryValidationStageStatus::Running),
            triggered_by: Set(format!("remote-runner:{runner_id}")),
            queue_reason: Set("test_setup".to_string()),
            attempt_number: Set(1),
            detail: Set("Remote validation is in progress.".to_string()),
            started_at: Set(Some(now)),
            finished_at: Set(None),
            last_error: Set(None),
            claim_id: Set(Some(claim_id.to_string())),
            claimed_by: Set(Some(runner_id.to_string())),
            claim_expires_at: Set(Some(claim_expires_at)),
            last_heartbeat_at: Set(Some(now)),
            runner_kind: Set(Some("remote".to_string())),
            created_at: Set(now),
            updated_at: Set(now),
        };
        active.insert(db).await.unwrap();
    }

    async fn insert_registry_owner_binding(
        db: &DatabaseConnection,
        slug: &str,
        owner_principal: &str,
    ) {
        let now = Utc::now();
        let active = RegistryModuleOwnerActiveModel {
            slug: Set(slug.to_string()),
            owner_principal: Set(principal_json(owner_principal)),
            bound_by: Set(principal_json("registry:admin")),
            bound_at: Set(now),
            updated_at: Set(now),
        };
        active.insert(db).await.unwrap();
    }

    async fn insert_active_release(
        db: &DatabaseConnection,
        request: &registry_publish_request::Model,
        publisher: &str,
    ) {
        let now = Utc::now();
        let active = RegistryModuleReleaseActiveModel {
            id: Set(format!("rrel_{}", uuid::Uuid::new_v4().simple())),
            request_id: Set(Some(request.id.clone())),
            slug: Set(request.slug.clone()),
            version: Set(request.version.clone()),
            crate_name: Set(request.crate_name.clone()),
            default_locale: Set(request.default_locale.clone()),
            ownership: Set(request.ownership.clone()),
            trust_level: Set(request.trust_level.clone()),
            license: Set(request.license.clone()),
            entry_type: Set(request.entry_type.clone()),
            marketplace: Set(request.marketplace.clone()),
            ui_packages: Set(request.ui_packages.clone()),
            status: Set(RegistryModuleReleaseStatus::Active),
            publisher: Set(principal_json(publisher)),
            artifact_storage_key: Set(Some("registry/artifacts/test/blog-0.1.0.crate".to_string())),
            checksum_sha256: Set(Some("checksum".to_string())),
            artifact_size: Set(Some(1024)),
            yanked_reason: Set(None),
            yanked_by: Set(None),
            yanked_at: Set(None),
            published_at: Set(now),
            created_at: Set(now),
            updated_at: Set(now),
        };
        let release = active.insert(db).await.unwrap();
        insert_release_translation(
            db,
            &release.id,
            SAMPLE_DEFAULT_LOCALE,
            SAMPLE_MODULE_NAME,
            SAMPLE_MODULE_DESCRIPTION,
        )
        .await;
    }

    async fn insert_follow_up_gate_event(
        db: &DatabaseConnection,
        request: &registry_publish_request::Model,
        gate: &str,
        event_type: &str,
        status: &str,
        detail: &str,
    ) {
        let active = RegistryGovernanceEventActiveModel {
            id: Set(format!("rge_{}", uuid::Uuid::new_v4().simple())),
            slug: Set(request.slug.clone()),
            request_id: Set(Some(request.id.clone())),
            release_id: Set(None),
            event_type: Set(event_type.to_string()),
            actor: Set(request.requested_by.clone()),
            publisher: Set(None),
            details: Set(serde_json::json!({
                "stage_key": gate,
                "status": status,
                "detail": detail,
            })),
            created_at: Set(Utc::now()),
        };
        active.insert(db).await.unwrap();
    }

    async fn insert_publish_request_translation(
        db: &DatabaseConnection,
        request_id: &str,
        locale: &str,
        name: &str,
        description: &str,
    ) {
        RegistryPublishRequestTranslationActiveModel {
            request_id: Set(request_id.to_string()),
            locale: Set(locale.to_string()),
            name: Set(name.to_string()),
            description: Set(description.to_string()),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        }
        .insert(db)
        .await
        .unwrap();
    }

    async fn insert_release_translation(
        db: &DatabaseConnection,
        release_id: &str,
        locale: &str,
        name: &str,
        description: &str,
    ) {
        RegistryModuleReleaseTranslationActiveModel {
            release_id: Set(release_id.to_string()),
            locale: Set(locale.to_string()),
            name: Set(name.to_string()),
            description: Set(description.to_string()),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
        }
        .insert(db)
        .await
        .unwrap();
    }

    fn sample_publish_artifact_json(slug: &str, include_admin_manifest: bool) -> String {
        let package_manifest = r#"
[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
description = "Blog module description long enough for validation."
ownership = "first_party"
trust_level = "core"

[marketplace]
category = "content"
tags = ["blog", "content"]

[crate]
entry_type = "backend"

[provides.admin_ui]
leptos_crate = "rustok-blog-admin"

[provides.storefront_ui]
leptos_crate = "rustok-blog-storefront"
"#;
        let crate_manifest = r#"
[package]
name = "rustok-blog"
version = "0.1.0"
license = "MIT"
"#;
        let admin_manifest = include_admin_manifest.then_some(
            r#"
[package]
name = "rustok-blog-admin"
version = "0.1.0"
"#,
        );
        let storefront_manifest = Some(
            r#"
[package]
name = "rustok-blog-storefront"
version = "0.1.0"
"#,
        );

        serde_json::json!({
            "schema_version": REGISTRY_MUTATION_SCHEMA_VERSION,
            "artifact_type": rustok_modules::MODULE_PUBLISH_BUNDLE_TYPE,
            "module": {
                "slug": slug,
                "version": "0.1.0",
                "crate_name": "rustok-blog",
                "module_name": "Blog",
                "module_description": "Blog module description long enough for validation.",
                "ownership": "first_party",
                "trust_level": "core",
                "license": "MIT",
                "module_entry_type": "backend",
                "marketplace": {
                    "category": "content",
                    "tags": ["blog", "content"]
                },
                "ui_packages": {
                    "admin": {
                        "crate_name": "rustok-blog-admin",
                        "manifest_path": "crates/rustok-blog/admin/Cargo.toml"
                    },
                    "storefront": {
                        "crate_name": "rustok-blog-storefront",
                        "manifest_path": "crates/rustok-blog/storefront/Cargo.toml"
                    }
                }
            },
            "files": {
                "rustok-module.toml": package_manifest,
                "Cargo.toml": crate_manifest,
                "admin/Cargo.toml": admin_manifest,
                "storefront/Cargo.toml": storefront_manifest
            }
        })
        .to_string()
    }
}
