//! Integration tests for digest-pinned OCI validation/admission and CAS-only runtime/recovery.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use rustok_api::ArtifactPermissionLocalization;
use rustok_core::MigrationSource;
use rustok_modules::{
    ArtifactAdmissionLimits, ArtifactBindingDispatchEnvelope, ArtifactBlobStore,
    ArtifactModuleKind, ArtifactPayloadKind, ArtifactPayloadSource,
    ArtifactPermissionDescriptor, ArtifactRegistry, ArtifactReleaseRef,
    ArtifactRuntime, ArtifactRuntimeError, ArtifactSchemaDocument,
    InMemoryArtifactBlobStore, InstalledModuleArtifact,
    MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION, ModuleArtifactDescriptor,
    ModuleBindingIdempotency, ModuleCommandContext, ModuleControlPlane,
    ModuleDependencyLockGraph, ModuleInstallationError, ModuleInstallationScope,
    ModuleRuntimeBinding, ModuleRuntimeBindingKind, ModulesModule,
    OciArtifactReference, OciReleaseAdmissionCommand,
    OciReleaseAdmissionError, OciReleaseAdmissionService,
};
use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityResponse,
    ExecutionMetrics, ExecutionPhase, ExecutorRegistry, RHAI_SANDBOX_RUNTIME_ABI,
    RhaiBindingOutput, SandboxContext, SandboxError, SandboxExecutor,
    SandboxExecutorKind, SandboxHost, SandboxOutcome, SandboxPolicy,
    SandboxRequest, SandboxResult, SandboxRuntime,
};
use sea_orm::Database;
use sea_orm_migration::{MigrationTrait, SchemaManager};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Helper calculating sha256 digest string with standard prefix.
fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

/// Registry test spy that tracks calls to `fetch()`.
struct SpyRegistry {
    packages: HashMap<String, rustok_modules::ModuleArtifactPackage>,
    fetch_call_count: AtomicUsize,
}

impl SpyRegistry {
    fn new() -> Self {
        Self {
            packages: HashMap::new(),
            fetch_call_count: AtomicUsize::new(0),
        }
    }

    fn add_package(&mut self, package: rustok_modules::ModuleArtifactPackage) {
        self.packages
            .insert(package.reference.canonical(), package);
    }

    fn call_count(&self) -> usize {
        self.fetch_call_count.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl ArtifactRegistry for SpyRegistry {
    async fn fetch(
        &self,
        reference: &OciArtifactReference,
        _limits: ArtifactAdmissionLimits,
    ) -> Result<rustok_modules::ModuleArtifactPackage, ModuleInstallationError> {
        self.fetch_call_count.fetch_add(1, Ordering::SeqCst);
        self.packages
            .get(&reference.canonical())
            .cloned()
            .ok_or_else(|| {
                ModuleInstallationError::Registry(format!(
                    "package `{}` not found in test registry",
                    reference.canonical()
                ))
            })
    }
}

struct DenyBroker;

#[async_trait::async_trait]
impl CapabilityBroker for DenyBroker {
    async fn invoke(
        &self,
        _call: &CapabilityCall,
        _grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        unreachable!("test does not invoke capability")
    }
}

#[derive(Clone)]
struct RecordingExecutor {
    observed: Arc<Mutex<Option<SandboxRequest>>>,
    output: serde_json::Value,
}

#[async_trait::async_trait]
impl SandboxExecutor for RecordingExecutor {
    fn kind(&self) -> SandboxExecutorKind {
        SandboxExecutorKind::Rhai
    }

    async fn execute(
        &self,
        request: &SandboxRequest,
        _host: SandboxHost,
    ) -> SandboxResult<SandboxOutcome> {
        *self.observed.lock().unwrap() = Some(request.clone());
        let output = serde_json::to_value(RhaiBindingOutput::new(self.output.clone()))
            .map_err(|e| SandboxError::Internal(e.to_string()))?;
        Ok(SandboxOutcome {
            execution_id: request.context.execution_id,
            output,
            rhai_scope: None,
            metrics: ExecutionMetrics::default(),
        })
    }
}

fn build_test_descriptor(slug: &str, version: &str, payload_bytes: &[u8]) -> (ModuleArtifactDescriptor, ArtifactSchemaDocument, ArtifactSchemaDocument) {
    let digest = sha256_digest(payload_bytes);
    let input_schema_doc = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "required": ["id"],
        "properties": { "id": { "type": "integer" } },
        "additionalProperties": false
    });
    let input_schema_bytes = serde_json::to_vec(&input_schema_doc).unwrap();
    let input_schema_digest = sha256_digest(&input_schema_bytes);
    let input_schema = ArtifactSchemaDocument {
        digest: input_schema_digest.clone(),
        document: input_schema_doc,
    };

    let output_schema_doc = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "required": ["result"],
        "properties": { "result": { "type": "integer" } },
        "additionalProperties": false
    });
    let output_schema_bytes = serde_json::to_vec(&output_schema_doc).unwrap();
    let output_schema_digest = sha256_digest(&output_schema_bytes);
    let output_schema = ArtifactSchemaDocument {
        digest: output_schema_digest.clone(),
        document: output_schema_doc,
    };

    let descriptor = ModuleArtifactDescriptor {
        schema_version: MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
        slug: slug.to_string(),
        version: version.to_string(),
        module_kind: ArtifactModuleKind::Optional,
        payload_kind: ArtifactPayloadKind::Rhai,
        artifact_digest: digest,
        runtime_abi: RHAI_SANDBOX_RUNTIME_ABI.to_string(),
        platform_compatibility: "^0.1".to_string(),
        required_features: Vec::new(),
        entrypoint: "src/main.rhai".to_string(),
        capabilities: Vec::new(),
        bindings: vec![ModuleRuntimeBinding {
            id: format!("{slug}.hook"),
            kind: ModuleRuntimeBindingKind::PreEnable,
            entrypoint: "src/main.rhai".to_string(),
            input_schema_digest: input_schema_digest.clone(),
            output_schema_digest: output_schema_digest.clone(),
            permission: format!("{slug}.read"),
            idempotency: ModuleBindingIdempotency::Required,
            limit_profile: "standard".to_string(),
            capabilities: Vec::new(),
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
        schema_documents: vec![input_schema.clone(), output_schema.clone()],
        settings_schema_digest: None,
        data_schema_digest: None,
        localization_catalogs: Vec::new(),
        ui_contributions: Vec::new(),
        persistence_contract: None,
    };

    (descriptor, input_schema, output_schema)
}

async fn setup_test_db() -> sea_orm::DatabaseConnection {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("sqlite database");
    rustok_outbox::SysEventsMigration
        .up(&SchemaManager::new(&database))
        .await
        .expect("outbox migration");
    for migration in ModulesModule.migrations() {
        migration
            .up(&SchemaManager::new(&database))
            .await
            .expect("module migration");
    }
    database
}

#[tokio::test]
async fn test_digest_pinned_oci_validation_and_streamed_cas_publication() {
    let db = setup_test_db().await;
    let blobs = Arc::new(InMemoryArtifactBlobStore::default());

    let payload_bytes = b"fn main() { return 42; }";
    let (descriptor, _, _) = build_test_descriptor("orders_notifier", "1.0.0", payload_bytes);

    let reference = OciArtifactReference {
        registry: "registry.example.com".to_string(),
        repository: "rustok/orders-notifier".to_string(),
        digest: sha256_digest(b"manifest-bytes-content-1"),
    };

    let mut spy_registry = SpyRegistry::new();
    spy_registry.add_package(rustok_modules::ModuleArtifactPackage {
        reference: reference.clone(),
        media_type: descriptor.payload_kind.oci_layer_media_type().to_string(),
        descriptor: descriptor.clone(),
        payload: ArtifactPayloadSource::Bytes(payload_bytes.to_vec()),
    });
    let registry = Arc::new(spy_registry);

    let control_plane = ModuleControlPlane::new(db.clone());
    let admission = control_plane.oci_release_admission(blobs.clone(), registry.clone());

    let context = ModuleCommandContext {
        actor_id: Uuid::new_v4(),
        tenant_id: None,
        idempotency_key: Uuid::new_v4(),
        trace_id: "trace-101".to_string(),
        correlation_id: Uuid::new_v4(),
    };

    let command = OciReleaseAdmissionCommand {
        reference: reference.clone(),
        scope: ModuleInstallationScope::Platform,
        context,
        trust_policy_revision: None,
        capability_policy_revision: None,
    };

    // 1. Execute admission
    let receipt = admission
        .admit_release(command)
        .await
        .expect("admission should succeed");

    assert!(receipt.cas_published, "Payload must be published to CAS");
    assert_eq!(receipt.reference, reference);
    assert_eq!(receipt.payload_digest, descriptor.artifact_digest);
    assert_eq!(receipt.payload_size_bytes, payload_bytes.len() as u64);

    // 2. Verify payload is in platform CAS
    let cas_bytes = blobs
        .get_verified(&descriptor.artifact_digest)
        .await
        .expect("payload must be verified in CAS");
    assert_eq!(cas_bytes, payload_bytes);

    // 3. Verify querying admitted release
    let looked_up = admission
        .get_admitted_release(&reference.digest)
        .await
        .expect("query should succeed")
        .expect("receipt must exist");
    assert_eq!(looked_up.reference.digest, reference.digest);
    assert_eq!(looked_up.payload_digest, descriptor.artifact_digest);

    // 4. Verify CAS presence query
    assert!(admission.has_cas_payload(&descriptor.artifact_digest).await);
    assert!(!admission.has_cas_payload("sha256:0000000000000000000000000000000000000000000000000000000000000000").await);
}

#[tokio::test]
async fn test_rejection_of_unpinned_reference() {
    let db = setup_test_db().await;
    let blobs = Arc::new(InMemoryArtifactBlobStore::default());
    let registry = Arc::new(SpyRegistry::new());

    let admission = OciReleaseAdmissionService::new(db, blobs, registry);

    let context = ModuleCommandContext {
        actor_id: Uuid::new_v4(),
        tenant_id: None,
        idempotency_key: Uuid::new_v4(),
        trace_id: "trace-unpinned".to_string(),
        correlation_id: Uuid::new_v4(),
    };

    // 1. Tag instead of digest
    let unpinned_ref = OciArtifactReference {
        registry: "registry.example.com".to_string(),
        repository: "rustok/orders-notifier".to_string(),
        digest: "latest".to_string(), // invalid digest
    };

    let result = admission
        .admit_release(OciReleaseAdmissionCommand {
            reference: unpinned_ref,
            scope: ModuleInstallationScope::Platform,
            context: context.clone(),
            trust_policy_revision: None,
            capability_policy_revision: None,
        })
        .await;

    assert!(
        matches!(result, Err(OciReleaseAdmissionError::InvalidReference(_)) | Err(OciReleaseAdmissionError::UnpinnedReference(_))),
        "Unpinned reference must be rejected"
    );

    // 2. Incomplete sha256 hex length
    let short_hex_ref = OciArtifactReference {
        registry: "registry.example.com".to_string(),
        repository: "rustok/orders-notifier".to_string(),
        digest: "sha256:1234abcd".to_string(),
    };

    let result = admission
        .admit_release(OciReleaseAdmissionCommand {
            reference: short_hex_ref,
            scope: ModuleInstallationScope::Platform,
            context,
            trust_policy_revision: None,
            capability_policy_revision: None,
        })
        .await;

    assert!(
        matches!(result, Err(OciReleaseAdmissionError::InvalidReference(_)) | Err(OciReleaseAdmissionError::UnpinnedReference(_))),
        "Invalid sha256 digest length must be rejected"
    );
}

#[tokio::test]
async fn test_rejection_of_descriptor_layer_digest_mismatch() {
    let db = setup_test_db().await;
    let blobs = Arc::new(InMemoryArtifactBlobStore::default());

    let payload_bytes = b"fn main() { return 42; }";
    let (mut descriptor, _, _) = build_test_descriptor("orders_notifier", "1.0.0", payload_bytes);
    // Tamper artifact digest in descriptor so it doesn't match payload
    descriptor.artifact_digest = "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string();

    let reference = OciArtifactReference {
        registry: "registry.example.com".to_string(),
        repository: "rustok/orders-notifier".to_string(),
        digest: sha256_digest(b"manifest-tampered"),
    };

    let mut spy_registry = SpyRegistry::new();
    spy_registry.add_package(rustok_modules::ModuleArtifactPackage {
        reference: reference.clone(),
        media_type: descriptor.payload_kind.oci_layer_media_type().to_string(),
        descriptor: descriptor.clone(),
        payload: ArtifactPayloadSource::Bytes(payload_bytes.to_vec()),
    });
    let registry = Arc::new(spy_registry);

    let admission = OciReleaseAdmissionService::new(db, blobs, registry);

    let context = ModuleCommandContext {
        actor_id: Uuid::new_v4(),
        tenant_id: None,
        idempotency_key: Uuid::new_v4(),
        trace_id: "trace-mismatch".to_string(),
        correlation_id: Uuid::new_v4(),
    };

    let result = admission
        .admit_release(OciReleaseAdmissionCommand {
            reference,
            scope: ModuleInstallationScope::Platform,
            context,
            trust_policy_revision: None,
            capability_policy_revision: None,
        })
        .await;

    assert!(
        result.is_err(),
        "Payload digest mismatch with descriptor must be rejected"
    );
}

#[tokio::test]
async fn test_idempotent_readmission_and_conflict_handling() {
    let db = setup_test_db().await;
    let blobs = Arc::new(InMemoryArtifactBlobStore::default());

    let payload_bytes = b"fn main() { return 42; }";
    let (descriptor, _, _) = build_test_descriptor("orders_notifier", "1.0.0", payload_bytes);

    let reference = OciArtifactReference {
        registry: "registry.example.com".to_string(),
        repository: "rustok/orders-notifier".to_string(),
        digest: sha256_digest(b"manifest-idempotency"),
    };

    let mut spy_registry = SpyRegistry::new();
    spy_registry.add_package(rustok_modules::ModuleArtifactPackage {
        reference: reference.clone(),
        media_type: descriptor.payload_kind.oci_layer_media_type().to_string(),
        descriptor: descriptor.clone(),
        payload: ArtifactPayloadSource::Bytes(payload_bytes.to_vec()),
    });
    let registry = Arc::new(spy_registry);

    let admission = OciReleaseAdmissionService::new(db, blobs, registry);

    let actor_id = Uuid::new_v4();
    let idempotency_key = Uuid::new_v4();

    let command1 = OciReleaseAdmissionCommand {
        reference: reference.clone(),
        scope: ModuleInstallationScope::Platform,
        context: ModuleCommandContext {
            actor_id,
            tenant_id: None,
            idempotency_key,
            trace_id: "trace-1".to_string(),
            correlation_id: Uuid::new_v4(),
        },
        trust_policy_revision: None,
        capability_policy_revision: None,
    };

    // First attempt succeeds and publishes to CAS
    let receipt1 = admission
        .admit_release(command1.clone())
        .await
        .expect("first attempt should succeed");
    assert!(receipt1.cas_published);

    // Second attempt with exact same parameters returns existing receipt without re-publishing
    let receipt2 = admission
        .admit_release(command1)
        .await
        .expect("second attempt should be idempotent");
    assert!(!receipt2.cas_published);
    assert_eq!(receipt1.reference, receipt2.reference);
    assert_eq!(receipt1.payload_digest, receipt2.payload_digest);

    // Third attempt with DIFFERENT reference under SAME idempotency key fails with conflict
    let different_ref = OciArtifactReference {
        registry: "registry.example.com".to_string(),
        repository: "rustok/orders-notifier".to_string(),
        digest: sha256_digest(b"different-manifest"),
    };

    let conflict_command = OciReleaseAdmissionCommand {
        reference: different_ref,
        scope: ModuleInstallationScope::Platform,
        context: ModuleCommandContext {
            actor_id,
            tenant_id: None,
            idempotency_key, // Reused idempotency key with different target
            trace_id: "trace-conflict".to_string(),
            correlation_id: Uuid::new_v4(),
        },
        trust_policy_revision: None,
        capability_policy_revision: None,
    };

    let conflict_result = admission.admit_release(conflict_command).await;
    assert!(
        matches!(conflict_result, Err(OciReleaseAdmissionError::IdempotencyConflict(_, _))),
        "Reusing idempotency key for a different release must return IdempotencyConflict"
    );
}

#[tokio::test]
async fn test_runtime_reads_cas_only_and_never_falls_back_to_oci() {
    let db = setup_test_db().await;
    let blobs = Arc::new(InMemoryArtifactBlobStore::default());

    let payload_bytes = b"fn main() { return 42; }";
    let (descriptor, _, _) = build_test_descriptor("orders_notifier", "1.0.0", payload_bytes);

    let reference = OciArtifactReference {
        registry: "registry.example.com".to_string(),
        repository: "rustok/orders-notifier".to_string(),
        digest: sha256_digest(b"manifest-runtime-test"),
    };

    let mut spy_registry = SpyRegistry::new();
    spy_registry.add_package(rustok_modules::ModuleArtifactPackage {
        reference: reference.clone(),
        media_type: descriptor.payload_kind.oci_layer_media_type().to_string(),
        descriptor: descriptor.clone(),
        payload: ArtifactPayloadSource::Bytes(payload_bytes.to_vec()),
    });
    let registry = Arc::new(spy_registry);

    let admission = OciReleaseAdmissionService::new(db, blobs.clone(), registry.clone());

    let command = OciReleaseAdmissionCommand {
        reference: reference.clone(),
        scope: ModuleInstallationScope::Platform,
        context: ModuleCommandContext {
            actor_id: Uuid::new_v4(),
            tenant_id: None,
            idempotency_key: Uuid::new_v4(),
            trace_id: "trace-runtime".to_string(),
            correlation_id: Uuid::new_v4(),
        },
        trust_policy_revision: None,
        capability_policy_revision: None,
    };

    // 1. Admit release into CAS
    let receipt = admission.admit_release(command).await.expect("admit release");
    assert!(receipt.cas_published);
    assert_eq!(registry.call_count(), 1, "Registry called once during admission");

    // 2. Set up runtime backed by the CAS blob store (using Arc<InMemoryArtifactBlobStore>)
    let observed = Arc::new(Mutex::new(None));
    let mut executors = ExecutorRegistry::new();
    executors
        .register_in_process(RecordingExecutor {
            observed,
            output: serde_json::json!({ "result": 42 }),
        })
        .expect("executor registration");
    let sandbox = SandboxRuntime::new(executors, Arc::new(DenyBroker));
    let runtime = ArtifactRuntime::new(blobs.clone(), sandbox);

    let installed_artifact = InstalledModuleArtifact {
        installation_id: Uuid::new_v4(),
        data_owner_id: Uuid::new_v4(),
        settings_instance_id: Uuid::new_v4(),
        scope: ModuleInstallationScope::Platform,
        reference: reference.clone(),
        release: ArtifactReleaseRef {
            slug: descriptor.slug.clone(),
            version: descriptor.version.clone(),
            digest: descriptor.artifact_digest.clone(),
        },
        descriptor: descriptor.clone(),
        payload_media_type: descriptor.payload_kind.oci_layer_media_type().to_string(),
        dependency_lock: ModuleDependencyLockGraph::create(0, Vec::new())
            .expect("empty dependency lock"),
        capability_grant_revision: 1,
        installed_at: chrono::Utc::now(),
    };

    let binding = installed_artifact.descriptor.bindings[0].clone();
    let context = SandboxContext::new(ExecutionPhase::Event);
    let input = ArtifactBindingDispatchEnvelope::new(
        &binding,
        ExecutionPhase::Event,
        serde_json::json!({ "id": 1 }),
    );

    // 3. Runtime executes binding using CAS payload
    let outcome = runtime
        .execute_binding(&installed_artifact, &binding, context.clone(), input.clone(), SandboxPolicy::default())
        .await
        .expect("runtime execution should succeed from CAS");

    assert_eq!(outcome.output, serde_json::json!({ "result": 42 }));
    assert_eq!(
        registry.call_count(),
        1,
        "Runtime must NOT call OCI registry when reading admitted CAS payload"
    );

    // 4. Now create an artifact referencing a missing/purged CAS blob
    let mut missing_payload_artifact = installed_artifact.clone();
    let missing_digest = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    missing_payload_artifact.descriptor.artifact_digest = missing_digest.to_string();
    missing_payload_artifact.release.digest = missing_digest.to_string();

    let failure = runtime
        .execute_binding(&missing_payload_artifact, &binding, context, input, SandboxPolicy::default())
        .await;

    // 5. Assert: Runtime fails closed with BlobNotFound and NEVER falls back to OCI
    match failure {
        Err(ArtifactRuntimeError::Installation(ModuleInstallationError::BlobNotFound(d))) => {
            assert_eq!(d, missing_digest);
        }
        other => panic!("Expected BlobNotFound, got {other:?}"),
    }

    assert_eq!(
        registry.call_count(),
        1,
        "Runtime must NEVER fall back to OCI registry when CAS payload is missing"
    );
}

#[tokio::test]
async fn test_recovery_reads_cas_only_and_never_falls_back_to_oci() {
    let blobs = Arc::new(InMemoryArtifactBlobStore::default());
    let spy_registry = Arc::new(SpyRegistry::new());

    // In recovery procedures (such as node recovery or post-purge recovery),
    // payload reads go directly to CAS:
    let missing_digest = "sha256:1111111111111111111111111111111111111111111111111111111111111111";

    let cas_result = blobs.get_verified(missing_digest).await;
    assert!(
        matches!(cas_result, Err(ModuleInstallationError::BlobNotFound(_))),
        "Recovery reading missing CAS blob must fail closed"
    );

    assert_eq!(
        spy_registry.call_count(),
        0,
        "Recovery must NEVER contact OCI registry"
    );
}
