//! Upgrade planning, application, retention, and metadata tests.
#![allow(unused_imports)]

use super::fixtures::*;
use super::super::*;

    #[tokio::test]
    async fn upgrade_planning_reads_before_transforming_and_never_writes() {
        let completed = Arc::new(AtomicBool::new(false));
        let tenant_id = Uuid::new_v4();
        let source = test_scope(tenant_id, "sample_module");
        let target = ArtifactDataScope {
            namespace_instance_id: Uuid::new_v4(),
            data_contract_revision: 2,
            policy_revision: 2,
            ..source.clone()
        };
        let planner = ArtifactDataUpgradePlanner::new(
            CompletedPageBroker {
                completed: Arc::clone(&completed),
            },
            UpgradeHook {
                read_completed: Arc::clone(&completed),
            },
            AcceptingSchemaValidator,
        );

        let plan = planner
            .plan(ArtifactDataUpgradeRequest {
                plan_id: Uuid::new_v4(),
                target_installation_id: Uuid::new_v4(),
                source,
                target,
                hook_binding_id: "upgrade.v2".to_string(),
                page: ArtifactDataPageRequest {
                    prefix: "state/".to_string(),
                    after_key: None,
                    limit: 10,
                },
            })
            .await
            .expect("upgrade plan");

        assert_eq!(plan.records.len(), 1);
        assert_eq!(plan.records[0].source_revision, 7);
        assert_eq!(plan.records[0].value, json!({ "version": 2 }));
        assert_eq!(plan.next_after_key.as_deref(), Some("state/current"));
    }

    #[tokio::test]
    async fn upgrade_hook_requires_a_dedicated_admitted_binding() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let executor = RecordingUpgradeBindingExecutor {
            calls: Arc::clone(&calls),
        };
        let release = ArtifactReleaseRef {
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            digest: "sha256:artifact".to_string(),
        };
        let mut binding = ModuleRuntimeBinding {
            id: "upgrade.v2".to_string(),
            kind: ModuleRuntimeBindingKind::Command,
            entrypoint: "upgrade.v2".to_string(),
            input_schema_digest: "sha256:input".to_string(),
            output_schema_digest: "sha256:output".to_string(),
            permission: "sample_module.data.upgrade".to_string(),
            idempotency: ModuleBindingIdempotency::Required,
            limit_profile: "data_upgrade".to_string(),
            capabilities: Vec::new(),
            event_topics: Vec::new(),
            schedule: None,
            http: None,
        };
        assert!(matches!(
            ArtifactBindingDataUpgradeHook::new(executor.clone(), release.clone(), binding.clone()),
            Err(ArtifactDataError::InvalidUpgrade)
        ));

        binding.kind = ModuleRuntimeBindingKind::DataUpgrade;
        let hook = ArtifactBindingDataUpgradeHook::new(executor, release, binding)
            .expect("dedicated upgrade hook");
        let tenant_id = Uuid::new_v4();
        let source = test_scope(tenant_id, "sample_module");
        let target = ArtifactDataScope {
            namespace_instance_id: Uuid::new_v4(),
            data_contract_revision: 2,
            policy_revision: 2,
            ..source.clone()
        };
        let transformed = hook
            .transform_data(
                "upgrade.v2",
                ArtifactDataUpgradeInput {
                    source: source.clone(),
                    target,
                    record: ArtifactDataRecord {
                        key: "state/current".to_string(),
                        value: json!({ "version": 1 }),
                        revision: 7,
                    },
                },
            )
            .await
            .expect("transformed value");

        assert_eq!(transformed, json!({ "version": 2 }));
        let calls = calls.lock().expect("calls lock");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "sample_module");
        assert_eq!(calls[0].1, "upgrade.v2");
        assert_eq!(calls[0].2, ExecutionPhase::Manual);
        assert_eq!(calls[0].3["source"], serde_json::to_value(source).unwrap());
        assert_eq!(calls[0].3["record"]["revision"], 7);
    }

    #[tokio::test]
    async fn upgrade_application_retries_by_plan_id_before_checkpointing() {
        let tenant_id = Uuid::new_v4();
        let source = test_scope(tenant_id, "sample_module");
        let target = ArtifactDataScope {
            namespace_instance_id: Uuid::new_v4(),
            data_contract_revision: 2,
            policy_revision: 2,
            ..source.clone()
        };
        let plan = ArtifactDataUpgradePlan {
            plan_id: Uuid::new_v4(),
            target_installation_id: Uuid::new_v4(),
            source,
            target,
            hook_binding_id: "upgrade.v2".to_string(),
            records: vec![ArtifactDataUpgradeRecord {
                key: "state/current".to_string(),
                value: json!({ "version": 2 }),
                source_revision: 7,
            }],
            next_after_key: Some("state/current".to_string()),
        };
        let data = UpgradeApplyBroker {
            source: ArtifactDataRecord {
                key: "state/current".to_string(),
                value: json!({ "version": 1 }),
                revision: 7,
            },
            target: Arc::new(Mutex::new(HashMap::new())),
        };
        let checkpoints = RecordingCheckpointStore::default();
        let applier = ArtifactDataUpgradeApplier::new(data.clone(), checkpoints.clone());
        let request = ArtifactDataUpgradeApplyRequest {
            plan,
            installation_scope: ModuleInstallationScope::Tenant { tenant_id },
            expected_installation_revision: 4,
            has_irreversible_migration: true,
            context: ModuleCommandContext {
                actor_id: Uuid::new_v4(),
                tenant_id: Some(tenant_id),
                trace_id: "test:data-upgrade".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
            reason: "apply approved upgrade plan".to_string(),
        };
        let checkpoint_actor_id = request.context.actor_id;
        let checkpoint_reason = request.reason.clone();
        let checkpoint_idempotency_key = request.context.idempotency_key;

        assert!(matches!(
            applier.apply(request.clone()).await,
            Err(ArtifactDataError::MigrationCheckpoint(_))
        ));
        assert_eq!(
            checkpoints.requests.lock().expect("checkpoint lock").len(),
            0
        );

        let retry = applier.apply(request).await.expect("idempotent retry");
        assert_eq!(retry.records[0].value, json!({ "version": 2 }));
        assert_eq!(retry.installation_revision, 5);
        let requests = checkpoints.requests.lock().expect("checkpoint lock");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].context.actor_id, checkpoint_actor_id);
        assert_eq!(requests[0].reason, checkpoint_reason);
        assert_eq!(
            requests[0].context.idempotency_key,
            checkpoint_idempotency_key
        );
    }

    #[test]
    fn object_metadata_never_accepts_a_physical_or_unbounded_identity() {
        let object = ArtifactDataObject {
            name: "exports/report.json".to_string(),
            content_type: "application/json".to_string(),
            size_bytes: 1024,
            digest_sha256: format!("sha256:{}", "a".repeat(64)),
            revision: 1,
        };
        assert!(object.validate().is_ok());

        let mut invalid = object;
        invalid.name = "../storage-key".to_string();
        assert_eq!(invalid.validate(), Err(ArtifactDataError::InvalidObject));
        invalid.name = "exports/report.json".to_string();
        invalid.digest_sha256 = "sha256:not-a-digest".to_string();
        assert_eq!(invalid.validate(), Err(ArtifactDataError::InvalidObject));
        invalid.digest_sha256 = format!("sha256:{}", "A".repeat(64));
        assert_eq!(invalid.validate(), Err(ArtifactDataError::InvalidObject));
        invalid.digest_sha256 = format!("sha256:{}", "a".repeat(64));
        invalid.content_type = " application/json".to_string();
        assert_eq!(invalid.validate(), Err(ArtifactDataError::InvalidObject));
    }

    #[test]
    fn object_upload_derives_owner_verified_metadata() {
        let upload = ArtifactDataObjectUpload {
            name: "exports/report.json".to_string(),
            content_type: "application/json".to_string(),
            data: Bytes::from_static(b"{}"),
            expected_revision: None,
            idempotency_key: Uuid::new_v4(),
        };
        let object = object_for_upload(&upload).expect("bounded object upload");
        assert_eq!(object.size_bytes, 2);
        assert_eq!(
            object.digest_sha256,
            "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
        );
        assert!(object.name.contains("report"));

        let mut invalid = upload;
        invalid.idempotency_key = Uuid::nil();
        assert_eq!(
            object_for_upload(&invalid),
            Err(ArtifactDataError::InvalidIdempotencyKey)
        );
    }

    #[tokio::test]
    async fn object_retention_snapshot_requires_explicit_eligible_rule() {
        let now = chrono::Utc::now();
        let scope = test_scope(Uuid::new_v4(), "sample_module");
        let storage_key = "module-artifact-data/retained";
        let policy = SnapshotArtifactDataObjectRetentionPolicy::new(now, HashMap::new());
        assert!(
            !policy
                .may_delete(&scope, storage_key)
                .await
                .expect("missing rule fails closed")
        );

        let policy = SnapshotArtifactDataObjectRetentionPolicy::new(
            now,
            HashMap::from([(
                storage_key.to_string(),
                ArtifactDataObjectRetentionRule {
                    delete_after: now,
                    legal_hold: false,
                    audit_hold: false,
                    rollback_hold: false,
                },
            )]),
        );
        assert!(
            policy
                .may_delete(&scope, storage_key)
                .await
                .expect("eligible rule")
        );
