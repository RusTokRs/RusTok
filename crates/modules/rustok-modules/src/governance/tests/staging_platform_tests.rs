use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

use super::fixtures::*;
use super::*;
use crate::installation::{ArtifactVerificationEvidence, OciArtifactReference};
use crate::publication_evidence::{
    ModulePublicationTrustEvidence, ModulePublicationTrustReport,
    ModulePublicationTrustSubject, ModulePublicationTrustVerifier,
};
use crate::publish_validation::{
    ModulePublishValidationCheck, ModulePublishValidationDetails,
    ModulePublishValidationResultOutcome,
};
use crate::{ControlPlaneInfrastructure, ModuleCommandContext};


    #[test]
    fn platform_build_stage_command_requires_tenant_context_and_matching_actor() {
        let actor_id = Uuid::new_v4();
        let mut command = ModulePublishPlatformBuildStageCommand {
            request_id: "request-1".to_string(),
            expected_revision: 1,
            context: ModuleCommandContext {
                actor_id,
                tenant_id: Some(Uuid::new_v4()),
                trace_id: "test:platform-build-stage".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
            build_request_id: Uuid::new_v4(),
            actor_principal: serde_json::json!({ "kind": "user", "id": actor_id }),
            actor_can_manage_modules: false,
        };
        assert!(command.validate().is_ok());

        command.context.tenant_id = None;
        assert_eq!(
            command.validate(),
            Err(ModuleGovernanceError::InvalidPlatformBuildStageCommand)
        );

        command.context.tenant_id = Some(Uuid::new_v4());
        command.actor_principal = serde_json::json!({ "kind": "user", "id": Uuid::new_v4() });
        assert_eq!(
            command.validate(),
            Err(ModuleGovernanceError::InvalidPlatformBuildStageCommand)
        );
    }


            toolchain: ModuleBuildToolchain {
                rust_toolchain: "1.85.0".to_string(),
                component_target: MODULE_BUILD_COMPONENT_TARGET.to_string(),
            },
            authoring: ModuleBuildAuthoring {
                sdk_version: "1.0.0".to_string(),
                template_version: "1.0.0".to_string(),
            },
            dependency_policy: ModuleBuildDependencyPolicy {
                lock_digest: stage_digest('b'),
                allowed_registries: vec!["https://crates.io".to_string()],
                allow_git_dependencies: false,
                allow_build_scripts: false,
                allow_native_links: false,
            },
            limits: ModuleBuildLimits {
                cpu_cores: 2,
                memory_bytes: 512 * 1024 * 1024,
                disk_bytes: 2 * 1024 * 1024 * 1024,
                process_limit: 32,
                output_bytes: 1024 * 1024,
                wall_clock_ms: 300_000,
            },
            network_policy: ModuleBuildNetworkPolicy::Denied,
            validation_profiles: vec![
                ModuleBuildValidationProfile::Check,
                ModuleBuildValidationProfile::Test,
                ModuleBuildValidationProfile::DependencyPolicy,
                ModuleBuildValidationProfile::Vulnerability,
            ],
            attempt: 1,
        };
        let reference = |marker| OciArtifactReference {
            registry: "registry.example".to_string(),
            repository: "modules/sample_module".to_string(),
            digest: stage_digest(marker),
        };
        let result = ModuleBuildResult {
            protocol_version: request.protocol_version,
            request_id: request.request_id,
            tenant_id,
            attempt: request.attempt,
            outcome: ModuleBuildOutcome::Succeeded,
            source_digest: request.source.digest.clone(),
            dependency_lock_digest: request.dependency_policy.lock_digest.clone(),
            toolchain_digest: request.toolchain.protocol_digest(),
            wit_digest: request.wit.protocol_digest(),
            component_digest: Some(stage_digest('c')),
            sbom_digest: Some(stage_digest('d')),
            provenance_digest: Some(stage_digest('e')),
            component_interface: Some(ModuleBuildComponentInterface {
                exports: vec!["run".to_string()],
                imports: Vec::new(),
            }),
            evidence: ModuleBuildEvidence {
                log_reference: "cas://logs/platform-build".to_string(),
                policy_report_reference: "cas://reports/platform-build".to_string(),
                validation_results: request
                    .validation_profiles
                    .iter()
                    .copied()
                    .map(|profile| ModuleBuildValidationResult {
                        profile,
                        outcome: ModuleBuildValidationOutcome::Passed,
                    })
                    .collect(),
                scenario_comparison: Some(rustok_sandbox::LocalSandboxScenarioComparison {
                    scenario_digest: request.scenario.digest.clone(),
                    result: rustok_sandbox::LocalSandboxScenarioResult::Success,
                }),
                diagnostics: Vec::new(),
            },
            publication: Some(ModuleBuildPublicationReceipt {
                artifact: reference('f'),
                signature_manifest: reference('a'),
                signature_authority: ModuleBuildSignatureAuthority::BuildService,
            }),
            metrics: ModuleBuildMetrics {
                duration_ms: 1,
                peak_memory_bytes: 1,
                output_bytes: 1,
            },
            retryable: false,
            next_action: ModuleBuildNextAction::AdmitArtifact,
        };
        (request, result)
    }


    #[tokio::test]
    async fn platform_build_stage_receipt_rejects_changed_context_or_privilege() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE module_build_requests (\
                request_id TEXT PRIMARY KEY, request JSON NOT NULL, result JSON NOT NULL, \
                status TEXT NOT NULL, revision INTEGER NOT NULL\
             )",
            "CREATE TABLE registry_publish_requests (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, version TEXT NOT NULL, \
                revision INTEGER NOT NULL, status TEXT NOT NULL, artifact_origin TEXT NOT NULL, \
                artifact_checksum_sha256 TEXT NULL, requested_by_principal JSON NOT NULL, \
                publisher_principal JSON NULL, updated_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_publish_build_staging (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, expected_revision INTEGER NOT NULL, \
                tenant_id TEXT NOT NULL, build_request_id TEXT NOT NULL, source_reference TEXT NOT NULL, \
                source_digest TEXT NOT NULL, parent_release_slug TEXT NULL, \
                parent_release_version TEXT NULL, parent_release_digest TEXT NULL, component_digest TEXT NOT NULL, \
                artifact_manifest_digest TEXT NOT NULL, signature_manifest_digest TEXT NOT NULL, \
                staged_by_principal JSON NOT NULL, actor_id TEXT NOT NULL, trace_id TEXT NOT NULL, \
                correlation_id TEXT NOT NULL, actor_can_manage_modules BOOLEAN NOT NULL, \
                idempotency_key TEXT NOT NULL, staged_at TEXT NOT NULL, \
                UNIQUE (request_id, idempotency_key)\
             )",
            "CREATE TABLE registry_module_owners (slug TEXT PRIMARY KEY, owner_principal JSON NOT NULL)",
            "CREATE TABLE registry_module_releases (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, slug TEXT NOT NULL, version TEXT NOT NULL, \
                checksum_sha256 TEXT NOT NULL, status TEXT NOT NULL, artifact_origin TEXT NOT NULL\
             )",
            "CREATE TABLE registry_module_release_artifacts (release_id TEXT PRIMARY KEY)",
            "CREATE TABLE registry_publish_platform_admissions (\
                request_id TEXT PRIMARY KEY, runtime_kind TEXT NOT NULL\
             )",
            "CREATE TABLE registry_validation_stages (\
                id TEXT PRIMARY KEY, request_id TEXT NOT NULL, stage_key TEXT NOT NULL, \
                status TEXT NOT NULL, attempt_number INTEGER NOT NULL, queue_reason TEXT NOT NULL, \
                runner_kind TEXT NULL, created_at TEXT NOT NULL\
             )",
            "CREATE TABLE registry_governance_events (\
                id TEXT PRIMARY KEY, slug TEXT NOT NULL, request_id TEXT NULL, release_id TEXT NULL, \
                event_type TEXT NOT NULL, actor_principal JSON NOT NULL, publisher_principal JSON NULL, \
                details JSON NOT NULL, created_at TEXT NOT NULL\
             )",
        ] {
            database
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("stage fixture schema");
        }

        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let build_request_id = Uuid::new_v4();
        let (mut build_request, build_result) =
            completed_platform_build(tenant_id, build_request_id);
        build_request.expected_version = "1.1.0".to_string();
        build_request.parent_release = Some(ArtifactReleaseRef {
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            digest: stage_digest('b'),
        });
        build_request
            .validate()
            .expect("valid evolved build request");
        build_result
            .validate_against(&build_request)
            .expect("valid evolved build result");
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_build_requests (request_id, request, result, status, revision) \
                 VALUES (?1, ?2, ?3, 'completed', 3)"
                    .to_string(),
                vec![
                    build_request_id.to_string().into(),
                    Value::Json(Some(Box::new(
                        serde_json::to_value(&build_request).expect("build request JSON"),
                    ))),
                    Value::Json(Some(Box::new(
                        serde_json::to_value(&build_result).expect("build result JSON"),
                    ))),
                ],
            ))
            .await
            .expect("completed build fixture");
        let actor_principal = serde_json::json!({ "kind": "user", "id": actor_id });
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO registry_publish_requests (\
                    id, slug, version, revision, status, artifact_origin, artifact_checksum_sha256, \
                    requested_by_principal, publisher_principal, updated_at\
                 ) VALUES (?1, 'sample_module', '1.1.0', 1, 'submitted', 'platform_built', ?2, ?3, NULL, datetime('now'))"
                    .to_string(),
                vec![
                    "request-1".into(),
                    "a".repeat(64).into(),
                    Value::Json(Some(Box::new(actor_principal.clone()))),
                ],
            ))
            .await
            .expect("publish request fixture");
        for statement in [
            "INSERT INTO registry_module_releases \
             (id, request_id, slug, version, checksum_sha256, status, artifact_origin) VALUES \
             ('parent-release', 'parent-request', 'sample_module', '1.0.0', \
              'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', 'active', 'alloy_authored')",
            "INSERT INTO registry_module_release_artifacts (release_id) VALUES ('parent-release')",
            "INSERT INTO registry_publish_platform_admissions (request_id, runtime_kind) \
             VALUES ('parent-request', 'rhai')",
        ] {
            database
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("Rhai parent fixture");
        }

        let command = ModulePublishPlatformBuildStageCommand {
            request_id: "request-1".to_string(),
            expected_revision: 1,
            context: ModuleCommandContext {
                actor_id,
                tenant_id: Some(tenant_id),
                trace_id: "test:platform-build-stage-receipt".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
            build_request_id,
            actor_principal,
            actor_can_manage_modules: false,
        };
        let service = SeaOrmModuleGovernanceService::new(database.clone());
        let first = service
            .stage_platform_build(command.clone())
            .await
            .expect("first stage");
        assert!(first.created);
        let receipt = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT expected_revision, tenant_id, actor_id, trace_id, correlation_id, \
                 parent_release_slug, parent_release_version, parent_release_digest, \
                 actor_can_manage_modules, idempotency_key FROM registry_publish_build_staging"
                    .to_string(),
            ))
            .await
            .expect("stage receipt query")
            .expect("stage receipt");
        assert_eq!(
            receipt
                .try_get::<i64>("", "expected_revision")
                .expect("expected revision"),
            command.expected_revision
        );
        assert_eq!(
            receipt.try_get::<String>("", "tenant_id").expect("tenant"),
            tenant_id.to_string()
        );
        assert_eq!(
            receipt.try_get::<String>("", "actor_id").expect("actor"),
            actor_id.to_string()
        );
        assert_eq!(
            receipt.try_get::<String>("", "trace_id").expect("trace"),
            command.context.trace_id
        );
        assert_eq!(
            receipt
                .try_get::<String>("", "correlation_id")
                .expect("correlation"),
            command.context.correlation_id.to_string()
        );
        assert!(
            !receipt
                .try_get::<bool>("", "actor_can_manage_modules")
                .expect("privilege")
        );
        assert_eq!(
            receipt
                .try_get::<String>("", "idempotency_key")
                .expect("idempotency"),
            command.context.idempotency_key.to_string()
        );
        let parent = build_request.parent_release.expect("Rhai parent");
        assert_eq!(
            receipt
                .try_get::<String>("", "parent_release_slug")
                .expect("parent slug"),
            parent.slug
        );
        assert_eq!(
            receipt
                .try_get::<String>("", "parent_release_version")
                .expect("parent version"),
            parent.version
        );
        assert_eq!(
            receipt
                .try_get::<String>("", "parent_release_digest")
                .expect("parent digest"),
            parent.digest
        );

        let replay = service
            .stage_platform_build(command.clone())
            .await
            .expect("exact replay");
        assert!(!replay.created);
        assert_eq!(replay.staging_id, first.staging_id);

        let mut changed_trace = command.clone();
        changed_trace.context.trace_id = "test:changed-trace".to_string();
        assert_eq!(
            service.stage_platform_build(changed_trace).await,
            Err(ModuleGovernanceError::PlatformBuildStageIdempotencyConflict)
        );

        let mut changed_privilege = command;
        changed_privilege.actor_can_manage_modules = true;
        assert_eq!(
            service.stage_platform_build(changed_privilege).await,
            Err(ModuleGovernanceError::PlatformBuildStageIdempotencyConflict)
        );
    }


    #[test]
    fn platform_build_keeps_payload_and_oci_manifest_digests_distinct() {
        let reference = |marker: char| OciArtifactReference {
            registry: "registry.example".to_string(),
            repository: "modules/sample-module".to_string(),
            digest: format!("sha256:{}", marker.to_string().repeat(64)),
        };
        let receipt = ModuleBuildPublicationReceipt {
            artifact: reference('a'),
            signature_manifest: reference('d'),
            signature_authority: ModuleBuildSignatureAuthority::BuildService,
        };
        let component_digest = format!("sha256:{}", "e".repeat(64));

        assert_ne!(component_digest, receipt.artifact.digest);
        assert!(platform_build_artifact_identities_valid(
            &component_digest,
            &receipt
        ));
        assert!(!platform_build_artifact_identities_valid(
            "sha256:not-a-digest",
            &receipt
        ));
    }

