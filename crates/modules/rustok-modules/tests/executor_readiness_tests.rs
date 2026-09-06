//! Integration tests for verified payload caching, authenticated prefetch/readiness across
//! executor pools and generations, owner-selected placement enforcement (zero in-process fallback),
//! and dual candidate/predecessor smoke readiness for automatic mode.

use std::{
    collections::HashSet,
    sync::Arc,
};

use rustok_api::ArtifactPermissionLocalization;
use rustok_core::MigrationSource;
use rustok_modules::{
    ArtifactBlobStore, ArtifactModuleKind, ArtifactPayloadKind,
    ArtifactPermissionDescriptor, ArtifactSchemaDocument,
    EvaluateReadinessCommand, ExecutorPoolIdentity,
    ExecutorReadinessError, ExecutorReadinessService, InMemoryArtifactBlobStore,
    MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION, ModuleArtifactDescriptor,
    ModuleBindingIdempotency, ModuleRuntimeBinding, ModuleRuntimeBindingKind,
    ModulesModule, OwnerPlacementPolicy, ReleaseReadinessTarget, RuntimeFingerprint,
    VerifiedPayloadCache,
};
use rustok_sandbox::{CapabilityName, RHAI_SANDBOX_RUNTIME_ABI, SandboxExecutorPlacement};
use sea_orm::Database;
use sea_orm_migration::{MigrationTrait, SchemaManager};
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

async fn setup_test_db() -> sea_orm::DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory db connects");
    let schema_manager = SchemaManager::new(&db);
    rustok_outbox::SysEventsMigration
        .up(&schema_manager)
        .await
        .expect("outbox migration");
    for migration in ModulesModule.migrations() {
        migration
            .up(&schema_manager)
            .await
            .expect("migration succeeds");
    }
    db
}

fn sample_runtime_fingerprint() -> RuntimeFingerprint {
    RuntimeFingerprint {
        executor_binary_digest: sha256_digest(b"executor-binary-v1.0.0-linux-x86_64"),
        engine_build_digest: sha256_digest(b"rhai-engine-v1.20.0-release-build"),
        engine_config_revision: "cfg-rev-42".to_string(),
        isolated_worker_image_digest: None,
        target_cpu_contract: "x86_64-unknown-linux-gnu".to_string(),
        runtime_abi: RHAI_SANDBOX_RUNTIME_ABI.to_string(),
    }
}

fn sample_isolated_runtime_fingerprint() -> RuntimeFingerprint {
    RuntimeFingerprint {
        executor_binary_digest: sha256_digest(b"executor-binary-v1.0.0-linux-x86_64"),
        engine_build_digest: sha256_digest(b"wasm-engine-v2.0.0-isolated-build"),
        engine_config_revision: "cfg-rev-42".to_string(),
        isolated_worker_image_digest: Some(sha256_digest(b"ghcr.io/rustok/sandbox-worker:v1.0.0")),
        target_cpu_contract: "x86_64-unknown-linux-gnu".to_string(),
        runtime_abi: "wasm-component:v1".to_string(),
    }
}

fn build_sample_target(
    slug: &str,
    version: &str,
    payload_bytes: &[u8],
    capabilities: Vec<CapabilityName>,
) -> ReleaseReadinessTarget {
    let payload_digest = sha256_digest(payload_bytes);
    let descriptor = ModuleArtifactDescriptor {
        schema_version: MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
        slug: slug.to_string(),
        version: version.to_string(),
        module_kind: ArtifactModuleKind::Optional,
        payload_kind: ArtifactPayloadKind::Rhai,
        artifact_digest: payload_digest.clone(),
        runtime_abi: RHAI_SANDBOX_RUNTIME_ABI.to_string(),
        platform_compatibility: "^0.1".to_string(),
        required_features: Vec::new(),
        entrypoint: "src/main.rhai".to_string(),
        capabilities: capabilities.clone(),
        bindings: vec![ModuleRuntimeBinding {
            id: format!("{slug}.hook"),
            kind: ModuleRuntimeBindingKind::PreEnable,
            entrypoint: "src/main.rhai".to_string(),
            input_schema_digest: sha256_digest(b"{}"),
            output_schema_digest: sha256_digest(b"{}"),
            permission: format!("{slug}.read"),
            idempotency: ModuleBindingIdempotency::Required,
            limit_profile: "standard".to_string(),
            capabilities,
            event_topics: Vec::new(),
            schedule: None,
            http: None,
        }],
        dependencies: Vec::new(),
        permissions: vec![ArtifactPermissionDescriptor {
            key: format!("{slug}.read"),
            localizations: vec![ArtifactPermissionLocalization {
                locale: "en".to_string(),
                label: "Read access".to_string(),
                description: "Permission to read data".to_string(),
            }],
        }],
        schema_documents: vec![ArtifactSchemaDocument {
            digest: sha256_digest(b"{}"),
            document: serde_json::json!({}),
        }],
        settings_schema_digest: None,
        data_schema_digest: None,
        localization_catalogs: Vec::new(),
        ui_contributions: Vec::new(),
        persistence_contract: None,
    };

    let manifest_bytes = serde_json::to_vec(&descriptor).unwrap();
    let release_digest = sha256_digest(&manifest_bytes);

    ReleaseReadinessTarget {
        release_digest,
        descriptor,
        payload_digest,
        installed_artifact: None,
    }
}

#[tokio::test]
async fn test_verified_payload_caching_and_cas_rehash() {
    let db = setup_test_db().await;
    let blobs = Arc::new(InMemoryArtifactBlobStore::default());
    let cache = Arc::new(VerifiedPayloadCache::new());

    let script = b"fn main() { return 100; }";
    let payload_digest = sha256_digest(script);
    blobs.put_verified(&payload_digest, script).await.unwrap();

    let mut routes = HashSet::new();
    routes.insert("platform.http".to_string());

    let service = ExecutorReadinessService::new(db.clone(), blobs.clone())
        .with_cache(cache.clone())
        .with_routes(routes);

    let fp = sample_runtime_fingerprint();
    let fp_digest = fp.compute_digest();

    assert_eq!(cache.len(), 0);

    // Initial fetch reads CAS, verifies sha256, and caches
    let fetched = service
        .fetch_and_verify_payload(&payload_digest, &fp)
        .await
        .expect("fetch and verify succeeds");
    assert_eq!(fetched.as_slice(), script);
    assert_eq!(cache.len(), 1);

    // Second fetch hits in-memory cache directly
    let cached = cache.get(&payload_digest, &fp_digest).expect("cached entry exists");
    assert_eq!(cached.payload_bytes.as_slice(), script);
    assert_eq!(cached.runtime_fingerprint, fp_digest);
}

#[tokio::test]
async fn test_engine_change_invalidates_readiness_receipts() {
    let db = setup_test_db().await;
    let blobs = Arc::new(InMemoryArtifactBlobStore::default());

    let script = b"fn main() { return 200; }";
    let target = build_sample_target("order_processor", "1.0.0", script, vec![]);
    blobs.put_verified(&target.payload_digest, script).await.unwrap();

    let service = ExecutorReadinessService::new(db.clone(), blobs.clone());

    let pool_initial = ExecutorPoolIdentity {
        pool_id: "pool-workers-01".to_string(),
        pool_generation: 1,
        fingerprint: sample_runtime_fingerprint(),
        placement: SandboxExecutorPlacement::InProcess,
        placement_attestation: None,
    };

    let policy = OwnerPlacementPolicy {
        policy_revision: 1,
        required_placement: SandboxExecutorPlacement::InProcess,
        allow_in_process_fallback: false,
    };

    let command = EvaluateReadinessCommand {
        operation_id: Uuid::new_v4(),
        installation_id: Uuid::new_v4(),
        target: target.clone(),
        pool: pool_initial.clone(),
        policy: policy.clone(),
        smoke_test_passed: true,
    };

    // 1. Initial readiness evaluation succeeds on fingerprint 1
    let receipt1 = service
        .evaluate_readiness(command)
        .await
        .expect("initial readiness succeeds");
    assert!(receipt1.is_valid_for(&pool_initial));

    // 2. Engine changes (e.g. new engine configuration or engine build)
    let mut modified_fingerprint = sample_runtime_fingerprint();
    modified_fingerprint.engine_config_revision = "cfg-rev-99-updated".to_string();

    let pool_updated_engine = ExecutorPoolIdentity {
        pool_id: "pool-workers-01".to_string(),
        pool_generation: 1,
        fingerprint: modified_fingerprint,
        placement: SandboxExecutorPlacement::InProcess,
        placement_attestation: None,
    };

    // The old receipt is INVALID for the updated engine fingerprint!
    assert!(
        !receipt1.is_valid_for(&pool_updated_engine),
        "Engine change MUST invalidate previous readiness receipt"
    );

    // Automatic mode is DENIED because current engine has no receipt
    let auto_err = service
        .check_automatic_mode_eligibility(&target.release_digest, None, &[pool_updated_engine.clone()])
        .await
        .expect_err("automatic mode must be denied on changed engine");
    assert!(matches!(auto_err, ExecutorReadinessError::AutomaticModeDenied(_)));

    // 3. Repeating readiness on updated engine re-authorizes execution
    let command2 = EvaluateReadinessCommand {
        operation_id: Uuid::new_v4(),
        installation_id: Uuid::new_v4(),
        target: target.clone(),
        pool: pool_updated_engine.clone(),
        policy: policy.clone(),
        smoke_test_passed: true,
    };
    let receipt2 = service
        .evaluate_readiness(command2)
        .await
        .expect("readiness on new engine succeeds");
    assert!(receipt2.is_valid_for(&pool_updated_engine));
    assert_ne!(receipt1.runtime_fingerprint, receipt2.runtime_fingerprint);

    // Automatic mode is now allowed
    service
        .check_automatic_mode_eligibility(&target.release_digest, None, &[pool_updated_engine])
        .await
        .expect("automatic mode succeeds after new engine readiness evaluation");
}

#[tokio::test]
async fn test_monotonic_pool_generation_gating() {
    let db = setup_test_db().await;
    let blobs = Arc::new(InMemoryArtifactBlobStore::default());

    let script = b"fn main() { return 300; }";
    let target = build_sample_target("notification_hub", "1.0.0", script, vec![]);
    blobs.put_verified(&target.payload_digest, script).await.unwrap();

    let service = ExecutorReadinessService::new(db.clone(), blobs.clone());

    let pool_gen1 = ExecutorPoolIdentity {
        pool_id: "pool-nodes-alpha".to_string(),
        pool_generation: 1,
        fingerprint: sample_runtime_fingerprint(),
        placement: SandboxExecutorPlacement::InProcess,
        placement_attestation: None,
    };

    let policy = OwnerPlacementPolicy {
        policy_revision: 1,
        required_placement: SandboxExecutorPlacement::InProcess,
        allow_in_process_fallback: false,
    };

    // Evaluate readiness on generation 1
    let receipt_gen1 = service
        .evaluate_readiness(EvaluateReadinessCommand {
            operation_id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
            target: target.clone(),
            pool: pool_gen1.clone(),
            policy: policy.clone(),
            smoke_test_passed: true,
        })
        .await
        .expect("readiness on gen 1 succeeds");
    assert_eq!(receipt_gen1.pool_generation, 1);

    // Node restarts or pool scales up: monotonic pool_generation increments to 2
    let mut pool_gen2 = pool_gen1.clone();
    pool_gen2.pool_generation = 2;

    // Earlier generation 1 receipt CANNOT authorize generation 2!
    assert!(
        !receipt_gen1.is_valid_for(&pool_gen2),
        "Stale pool generation receipt cannot be valid for new pool generation"
    );

    // Automatic mode is denied on new pool generation until smoke readiness is repeated
    let auto_denied = service
        .check_automatic_mode_eligibility(&target.release_digest, None, &[pool_gen2.clone()])
        .await
        .expect_err("automatic mode denied on un-smoked generation");
    assert!(matches!(auto_denied, ExecutorReadinessError::AutomaticModeDenied(_)));

    // Re-evaluate readiness on generation 2
    let receipt_gen2 = service
        .evaluate_readiness(EvaluateReadinessCommand {
            operation_id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
            target: target.clone(),
            pool: pool_gen2.clone(),
            policy: policy.clone(),
            smoke_test_passed: true,
        })
        .await
        .expect("readiness on gen 2 succeeds");
    assert_eq!(receipt_gen2.pool_generation, 2);

    service
        .check_automatic_mode_eligibility(&target.release_digest, None, &[pool_gen2])
        .await
        .expect("automatic mode approved after generation 2 readiness");
}

#[tokio::test]
async fn test_capability_route_checks_fail_closed_on_missing_route() {
    let db = setup_test_db().await;
    let blobs = Arc::new(InMemoryArtifactBlobStore::default());

    let script = b"fn main() { return 400; }";
    // Target requires "platform.http" capability
    let target = build_sample_target(
        "payment_webhook",
        "1.0.0",
        script,
        vec![CapabilityName::new("platform.http").unwrap()],
    );
    blobs.put_verified(&target.payload_digest, script).await.unwrap();

    let pool = ExecutorPoolIdentity {
        pool_id: "pool-edge".to_string(),
        pool_generation: 1,
        fingerprint: sample_runtime_fingerprint(),
        placement: SandboxExecutorPlacement::InProcess,
        placement_attestation: None,
    };

    let policy = OwnerPlacementPolicy {
        policy_revision: 1,
        required_placement: SandboxExecutorPlacement::InProcess,
        allow_in_process_fallback: false,
    };

    // 1. Service with EMPTY routes fails closed
    let service_no_routes = ExecutorReadinessService::new(db.clone(), blobs.clone());
    let err = service_no_routes
        .evaluate_readiness(EvaluateReadinessCommand {
            operation_id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
            target: target.clone(),
            pool: pool.clone(),
            policy: policy.clone(),
            smoke_test_passed: true,
        })
        .await
        .expect_err("readiness must fail closed when declared capability route is missing");

    assert!(
        matches!(err, ExecutorReadinessError::MissingCapabilityRoute(_)),
        "Expected MissingCapabilityRoute, received {err:?}"
    );

    // 2. Service with "platform.http" route succeeds
    let mut routes = HashSet::new();
    routes.insert("platform.http".to_string());
    let service_with_routes = ExecutorReadinessService::new(db.clone(), blobs.clone())
        .with_routes(routes);

    let receipt = service_with_routes
        .evaluate_readiness(EvaluateReadinessCommand {
            operation_id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
            target: target.clone(),
            pool: pool.clone(),
            policy: policy.clone(),
            smoke_test_passed: true,
        })
        .await
        .expect("readiness succeeds when capability route is available");
    assert!(receipt.capability_routes_verified);
}

#[tokio::test]
async fn test_owner_selected_placement_enforcement_zero_in_process_fallback() {
    let db = setup_test_db().await;
    let blobs = Arc::new(InMemoryArtifactBlobStore::default());

    let script = b"fn main() { return 500; }";
    let target = build_sample_target("tenant_isolated_logic", "1.0.0", script, vec![]);
    blobs.put_verified(&target.payload_digest, script).await.unwrap();

    let service = ExecutorReadinessService::new(db.clone(), blobs.clone());

    // Owner policy requires IsolatedWorker!
    let policy_isolated = OwnerPlacementPolicy {
        policy_revision: 5,
        required_placement: SandboxExecutorPlacement::IsolatedWorker,
        allow_in_process_fallback: false,
    };

    // 1. In-process pool attempted when isolated worker required -> STRICT FAILURE (no fallback!)
    let in_process_pool = ExecutorPoolIdentity {
        pool_id: "pool-local-shared".to_string(),
        pool_generation: 1,
        fingerprint: sample_runtime_fingerprint(),
        placement: SandboxExecutorPlacement::InProcess,
        placement_attestation: None,
    };

    let fallback_err = service
        .evaluate_readiness(EvaluateReadinessCommand {
            operation_id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
            target: target.clone(),
            pool: in_process_pool,
            policy: policy_isolated.clone(),
            smoke_test_passed: true,
        })
        .await
        .expect_err("in-process execution MUST be strictly prohibited when isolated worker required");

    assert!(
        matches!(fallback_err, ExecutorReadinessError::IsolatedWorkerRequired),
        "Expected IsolatedWorkerRequired, received {fallback_err:?}"
    );

    // 2. Isolated worker pool without attestation -> FAILS
    let unauthenticated_isolated_pool = ExecutorPoolIdentity {
        pool_id: "pool-workers-unattested".to_string(),
        pool_generation: 1,
        fingerprint: sample_isolated_runtime_fingerprint(),
        placement: SandboxExecutorPlacement::IsolatedWorker,
        placement_attestation: None,
    };

    let attestation_err = service
        .evaluate_readiness(EvaluateReadinessCommand {
            operation_id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
            target: target.clone(),
            pool: unauthenticated_isolated_pool,
            policy: policy_isolated.clone(),
            smoke_test_passed: true,
        })
        .await
        .expect_err("isolated pool without attestation must fail");

    assert!(
        matches!(attestation_err, ExecutorReadinessError::MissingWorkerAttestation),
        "Expected MissingWorkerAttestation, received {attestation_err:?}"
    );

    // 3. Fully attested isolated worker pool -> SUCCEEDS
    let attested_isolated_pool = ExecutorPoolIdentity {
        pool_id: "pool-workers-attested".to_string(),
        pool_generation: 1,
        fingerprint: sample_isolated_runtime_fingerprint(),
        placement: SandboxExecutorPlacement::IsolatedWorker,
        placement_attestation: Some("jwt-worker-node-attestation-valid-token".to_string()),
    };

    let receipt = service
        .evaluate_readiness(EvaluateReadinessCommand {
            operation_id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
            target: target.clone(),
            pool: attested_isolated_pool.clone(),
            policy: policy_isolated.clone(),
            smoke_test_passed: true,
        })
        .await
        .expect("readiness succeeds for authenticated isolated worker pool");

    assert_eq!(receipt.placement, SandboxExecutorPlacement::IsolatedWorker);
}

#[tokio::test]
async fn test_dual_candidate_and_predecessor_smoke_readiness_for_automatic_mode() {
    let db = setup_test_db().await;
    let blobs = Arc::new(InMemoryArtifactBlobStore::default());

    let candidate_script = b"fn main() { return 601; }";
    let predecessor_script = b"fn main() { return 600; }";

    let candidate = build_sample_target("commerce_cart", "2.0.0", candidate_script, vec![]);
    let predecessor = build_sample_target("commerce_cart", "1.9.0", predecessor_script, vec![]);

    blobs.put_verified(&candidate.payload_digest, candidate_script).await.unwrap();
    blobs.put_verified(&predecessor.payload_digest, predecessor_script).await.unwrap();

    let service = ExecutorReadinessService::new(db.clone(), blobs.clone());

    let pool = ExecutorPoolIdentity {
        pool_id: "pool-prod-serving".to_string(),
        pool_generation: 1,
        fingerprint: sample_runtime_fingerprint(),
        placement: SandboxExecutorPlacement::InProcess,
        placement_attestation: None,
    };

    let policy = OwnerPlacementPolicy {
        policy_revision: 1,
        required_placement: SandboxExecutorPlacement::InProcess,
        allow_in_process_fallback: false,
    };

    // 1. Predecessor only has readiness, candidate does not -> AUTOMATIC MODE DENIED
    service
        .evaluate_readiness(EvaluateReadinessCommand {
            operation_id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
            target: predecessor.clone(),
            pool: pool.clone(),
            policy: policy.clone(),
            smoke_test_passed: true,
        })
        .await
        .expect("predecessor readiness succeeds");

    let err_missing_candidate = service
        .check_automatic_mode_eligibility(
            &candidate.release_digest,
            Some(&predecessor.release_digest),
            &[pool.clone()],
        )
        .await
        .expect_err("must deny automatic mode when candidate lacks receipt");
    assert!(matches!(err_missing_candidate, ExecutorReadinessError::AutomaticModeDenied(_)));

    // 2. Candidate evaluated with FAILED smoke test -> AUTOMATIC MODE DENIED
    let err_smoke = service
        .evaluate_readiness(EvaluateReadinessCommand {
            operation_id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
            target: candidate.clone(),
            pool: pool.clone(),
            policy: policy.clone(),
            smoke_test_passed: false,
        })
        .await
        .expect_err("smoke failure must fail closed");
    assert!(matches!(err_smoke, ExecutorReadinessError::SmokeExecutionFailed(_)));

    // 3. Candidate evaluated with PASSED smoke test -> BOTH CANDIDATE AND PREDECESSOR READY
    service
        .evaluate_readiness(EvaluateReadinessCommand {
            operation_id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
            target: candidate.clone(),
            pool: pool.clone(),
            policy: policy.clone(),
            smoke_test_passed: true,
        })
        .await
        .expect("candidate readiness succeeds");

    // Both candidate and predecessor now hold valid receipts on active pool generation -> APPROVED
    service
        .check_automatic_mode_eligibility(
            &candidate.release_digest,
            Some(&predecessor.release_digest),
            &[pool.clone()],
        )
        .await
        .expect("automatic mode must be permitted when both candidate and predecessor pass smoke readiness");
}
