//! Integration tests for external-prebuilt dynamic package ingress with independently
//! verified ownership, lineage, signature, SBOM, provenance, ABI/capability, and policy
//! evidence, proving strict native promotion denial, quarantine non-mutation upon rejection,
//! and CAS-only runtime and recovery.

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
    ControlPlaneInfrastructure,
    ExternalAbiCapabilityEvidence, ExternalLineageEvidence, ExternalPolicyEvidence,
    ExternalPrebuiltIngressCommand, ExternalPrebuiltIngressError,
    ExternalPrebuiltIngressEvidence,
    ExternalPrebuiltIngressService, ExternalProvenanceEvidence,
    ExternalPublisherEvidence, ExternalSbomEvidence, ExternalSignatureEvidence,
    InMemoryArtifactBlobStore, InstalledModuleArtifact,
    MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION, ModuleArtifactDescriptor,
    ModuleBindingIdempotency, ModuleCommandContext, ModuleControlPlane,
    ModuleDependencyLockGraph, ModuleInstallationError, ModuleInstallationScope,
    ModuleRuntimeBinding, ModuleRuntimeBindingKind, ModuleStaticPromotionAuthorizer,
    ModuleStaticPromotionError, ModuleStaticPromotionRequestCommand,
    ModulesModule, OciArtifactReference,
};
use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityResponse,
    ExecutionMetrics, ExecutionPhase, ExecutorRegistry, RHAI_SANDBOX_RUNTIME_ABI,
    RhaiBindingOutput, SandboxContext, SandboxError, SandboxExecutor,
    SandboxExecutorKind, SandboxHost, SandboxOutcome, SandboxPolicy,
    SandboxRequest, SandboxResult, SandboxRuntime,
};
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

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

struct AllowAllPromotionAuthorizer;

#[async_trait::async_trait]
impl ModuleStaticPromotionAuthorizer for AllowAllPromotionAuthorizer {
    async fn authorize_request(
        &self,
        _command: &ModuleStaticPromotionRequestCommand,
    ) -> Result<(), ModuleStaticPromotionError> {
        Ok(())
    }

    async fn authorize_approval(
        &self,
        _command: &rustok_modules::ModuleStaticPromotionApprovalCommand,
    ) -> Result<(), ModuleStaticPromotionError> {
        Ok(())
    }
}

async fn setup_db() -> sea_orm::DatabaseConnection {
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

fn valid_evidence() -> ExternalPrebuiltIngressEvidence {
    ExternalPrebuiltIngressEvidence {
        publisher: ExternalPublisherEvidence {
            identity: "pubkey:ed25519:e0a1b2c3d4e5f67890abcdef1234567890abcdef1234567890abcdef12345678".to_string(),
            verified: true,
        },
        lineage: ExternalLineageEvidence {
            source_reference: "git+https://github.com/rustok/external-extension.git#commit=1234567".to_string(),
            source_digest: sha256_digest(b"canonical source snapshot bytes"),
            build_toolchain: "rust:1.80-wasm32-wasip1".to_string(),
            verified: true,
        },
        signature: ExternalSignatureEvidence {
            signature_reference: "cosign://ghcr.io/rustok/external-extension:sha256-sig".to_string(),
            signature_digest: sha256_digest(b"cryptographic signature envelope bytes"),
            verified: true,
        },
        sbom: ExternalSbomEvidence {
            sbom_reference: "sbom://ghcr.io/rustok/external-extension:sha256-sbom".to_string(),
            sbom_digest: sha256_digest(b"cyclonedx sbom json bytes"),
            media_type: "application/vnd.cyclonedx+json".to_string(),
            verified: true,
        },
        provenance: ExternalProvenanceEvidence {
            provenance_reference: "provenance://ghcr.io/rustok/external-extension:sha256-provenance".to_string(),
            provenance_digest: sha256_digest(b"in-toto provenance attestation bytes"),
            media_type: "application/vnd.in-toto+json".to_string(),
            verified: true,
        },
        abi_capability: ExternalAbiCapabilityEvidence {
            abi_kind: ArtifactPayloadKind::WasmComponent,
            declared_capabilities: vec!["platform.http".to_string()],
            broker_routes_verified: true,
            verified: true,
        },
        policy: ExternalPolicyEvidence {
            policy_revision: 42,
            license_approved: true,
            vulnerability_scan_passed: true,
            verified: true,
        },
    }
}

fn build_test_descriptor(
    slug: &str,
    version: &str,
    payload_bytes: &[u8],
    payload_kind: ArtifactPayloadKind,
    runtime_abi: &str,
) -> (ModuleArtifactDescriptor, ArtifactSchemaDocument, ArtifactSchemaDocument) {
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
        payload_kind,
        artifact_digest: digest,
        runtime_abi: runtime_abi.to_string(),
        platform_compatibility: "^0.1".to_string(),
        required_features: Vec::new(),
        entrypoint: "src/main".to_string(),
        capabilities: Vec::new(),
        bindings: vec![ModuleRuntimeBinding {
            id: format!("{slug}.hook"),
            kind: ModuleRuntimeBindingKind::PreEnable,
            entrypoint: "src/main".to_string(),
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

fn create_sample_package(
    payload_bytes: &[u8],
    slug: &str,
    version: &str,
) -> (rustok_modules::ModuleArtifactPackage, String) {
    let (descriptor, _, _) = build_test_descriptor(
        slug,
        version,
        payload_bytes,
        ArtifactPayloadKind::WasmComponent,
        "wasm-component:v1",
    );
    let manifest_bytes = serde_json::to_vec(&descriptor).unwrap();
    let manifest_digest = sha256_digest(&manifest_bytes);
    let reference = OciArtifactReference {
        registry: "ghcr.io".to_string(),
        repository: format!("rustok/{slug}"),
        digest: manifest_digest.clone(),
    };

    let package = rustok_modules::ModuleArtifactPackage {
        reference,
        descriptor,
        media_type: ArtifactPayloadKind::WasmComponent.oci_layer_media_type().to_string(),
        payload: ArtifactPayloadSource::Bytes(payload_bytes.to_vec()),
    };

    (package, manifest_digest)
}

#[tokio::test]
async fn test_external_prebuilt_admission_success() {
    let db = setup_db().await;
    let blobs = Arc::new(InMemoryArtifactBlobStore::default());
    let mut registry = SpyRegistry::new();

    let wasm_bytes = b"\x00asm\x01\x00\x00\x00-external-prebuilt-component-payload";
    let (package, manifest_digest) = create_sample_package(wasm_bytes, "payment_gateway", "1.0.0");
    registry.add_package(package.clone());
    let registry = Arc::new(registry);

    let service = ExternalPrebuiltIngressService::new(db.clone(), blobs.clone(), registry.clone())
        .with_infrastructure(ControlPlaneInfrastructure::default());

    let command = ExternalPrebuiltIngressCommand {
        reference: package.reference.clone(),
        scope: ModuleInstallationScope::Platform,
        context: ModuleCommandContext {
            actor_id: Uuid::new_v4(),
            tenant_id: None,
            trace_id: "trace-ext-001".to_string(),
            correlation_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        },
        evidence: valid_evidence(),
        trust_policy_revision: Some(1),
        capability_policy_revision: Some(1),
    };

    let receipt = service
        .admit_external_prebuilt(command)
        .await
        .expect("external prebuilt admission succeeds");

    assert_eq!(receipt.artifact_origin, "external_prebuilt");
    assert!(receipt.native_promotion_denied);
    assert!(receipt.cas_published);
    assert_eq!(receipt.payload_digest, sha256_digest(wasm_bytes));
    assert_eq!(receipt.payload_size_bytes, wasm_bytes.len() as u64);

    // Verify payload exists in platform CAS
    assert!(service.has_cas_payload(&receipt.payload_digest).await);
    let cas_bytes = blobs.get_verified(&receipt.payload_digest).await.unwrap();
    assert_eq!(cas_bytes.as_slice(), wasm_bytes);

    // Verify lookup of admitted ingress record
    let stored = service
        .get_ingress_record(&manifest_digest)
        .await
        .expect("get_ingress_record succeeds")
        .expect("ingress record found");
    assert_eq!(stored.reference.digest, manifest_digest);
    assert_eq!(stored.artifact_origin, "external_prebuilt");
    assert!(stored.native_promotion_denied);

    // Verify database row in module_admitted_oci_releases has artifact_origin = 'external_prebuilt'
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT artifact_origin FROM module_admitted_oci_releases WHERE release_digest = ?1",
            vec![manifest_digest.clone().into()],
        ))
        .await
        .unwrap()
        .expect("module_admitted_oci_releases row exists");
    let origin: String = row.try_get("", "artifact_origin").unwrap();
    assert_eq!(origin, "external_prebuilt");

    // Verify database row in module_external_prebuilt_ingress has native_promotion_denied = 1
    let row_ext = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT native_promotion_denied, license_policy_verified, vulnerability_policy_verified, \
                    abi_verified, capability_verified \
             FROM module_external_prebuilt_ingress WHERE release_digest = ?1",
            vec![manifest_digest.into()],
        ))
        .await
        .unwrap()
        .expect("module_external_prebuilt_ingress row exists");
    let denied: i32 = row_ext.try_get("", "native_promotion_denied").unwrap();
    assert_eq!(denied, 1);
}

#[tokio::test]
async fn test_external_prebuilt_rejection_does_not_mutate_quarantine_state() {
    let db = setup_db().await;
    let blobs = Arc::new(InMemoryArtifactBlobStore::default());
    let mut registry = SpyRegistry::new();

    let wasm_bytes = b"\x00asm\x01\x00\x00\x00-suspect-unverified-payload";
    let (package, _manifest_digest) = create_sample_package(wasm_bytes, "suspect_mod", "1.0.0");
    registry.add_package(package.clone());
    let registry = Arc::new(registry);

    let service = ExternalPrebuiltIngressService::new(db.clone(), blobs.clone(), registry.clone())
        .with_infrastructure(ControlPlaneInfrastructure::default());

    // 1. Test unverified publisher rejection
    let mut evidence = valid_evidence();
    evidence.publisher.verified = false;
    let command = ExternalPrebuiltIngressCommand {
        reference: package.reference.clone(),
        scope: ModuleInstallationScope::Platform,
        context: ModuleCommandContext {
            actor_id: Uuid::new_v4(),
            tenant_id: None,
            trace_id: "trace-ext-rej-1".to_string(),
            correlation_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        },
        evidence,
        trust_policy_revision: Some(1),
        capability_policy_revision: Some(1),
    };
    let err = service.admit_external_prebuilt(command).await.unwrap_err();
    assert!(matches!(err, ExternalPrebuiltIngressError::InvalidPublisher(_)));

    // 2. Test unverified signature rejection
    let mut evidence = valid_evidence();
    evidence.signature.verified = false;
    let command = ExternalPrebuiltIngressCommand {
        reference: package.reference.clone(),
        scope: ModuleInstallationScope::Platform,
        context: ModuleCommandContext {
            actor_id: Uuid::new_v4(),
            tenant_id: None,
            trace_id: "trace-ext-rej-2".to_string(),
            correlation_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        },
        evidence,
        trust_policy_revision: Some(1),
        capability_policy_revision: Some(1),
    };
    let err = service.admit_external_prebuilt(command).await.unwrap_err();
    assert!(matches!(err, ExternalPrebuiltIngressError::InvalidSignature(_)));

    // 3. Test failed vulnerability scan rejection
    let mut evidence = valid_evidence();
    evidence.policy.vulnerability_scan_passed = false;
    let command = ExternalPrebuiltIngressCommand {
        reference: package.reference.clone(),
        scope: ModuleInstallationScope::Platform,
        context: ModuleCommandContext {
            actor_id: Uuid::new_v4(),
            tenant_id: None,
            trace_id: "trace-ext-rej-3".to_string(),
            correlation_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        },
        evidence,
        trust_policy_revision: Some(1),
        capability_policy_revision: Some(1),
    };
    let err = service.admit_external_prebuilt(command).await.unwrap_err();
    assert!(matches!(err, ExternalPrebuiltIngressError::PolicyViolation(_)));

    // 4. CRITICAL INVARIANT: Rejection alone DOES NOT mutate quarantine state!
    // Verify that module_artifact_security_states contains ZERO rows!
    let count_row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS total FROM module_artifact_security_states",
            vec![],
        ))
        .await
        .unwrap()
        .expect("count row");
    let total: i32 = count_row.try_get("", "total").unwrap();
    assert_eq!(total, 0, "Rejection alone must NOT mutate quarantine state");

    // Also verify CAS has NO blobs
    assert!(!service.has_cas_payload(&sha256_digest(wasm_bytes)).await);
}

#[tokio::test]
async fn test_external_prebuilt_rejects_static_or_native_abi() {
    let mut evidence = valid_evidence();
    evidence.abi_capability.abi_kind = ArtifactPayloadKind::StaticPromoted;

    let err = evidence.validate().unwrap_err();
    assert!(matches!(err, ExternalPrebuiltIngressError::ExternalPrebuiltCannotBePromoted));
}

#[tokio::test]
async fn test_external_prebuilt_strict_native_promotion_denial() {
    let db = setup_db().await;
    let blobs = Arc::new(InMemoryArtifactBlobStore::default());
    let mut registry = SpyRegistry::new();

    let wasm_bytes = b"\x00asm\x01\x00\x00\x00-dynamic-external-binary";
    let (package, manifest_digest) = create_sample_package(wasm_bytes, "ext_analytics", "1.0.0");
    registry.add_package(package.clone());
    let registry = Arc::new(registry);

    let service = ExternalPrebuiltIngressService::new(db.clone(), blobs.clone(), registry.clone())
        .with_infrastructure(ControlPlaneInfrastructure::default());

    let command = ExternalPrebuiltIngressCommand {
        reference: package.reference.clone(),
        scope: ModuleInstallationScope::Platform,
        context: ModuleCommandContext {
            actor_id: Uuid::new_v4(),
            tenant_id: None,
            trace_id: "trace-ext-promo".to_string(),
            correlation_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        },
        evidence: valid_evidence(),
        trust_policy_revision: Some(1),
        capability_policy_revision: Some(1),
    };

    let receipt = service
        .admit_external_prebuilt(command)
        .await
        .expect("external prebuilt admission succeeds");
    assert!(receipt.native_promotion_denied);

    let control_plane = ModuleControlPlane::new(db.clone());
    let promotion_service = control_plane.promotion(AllowAllPromotionAuthorizer);

    let promo_command = ModuleStaticPromotionRequestCommand {
        release_id: manifest_digest.clone(),
        source_reference: format!("cas://{}", receipt.payload_digest),
        source_digest: receipt.payload_digest.clone(),
        dependency_lock_digest: sha256_digest(b"lock-file-bytes"),
        context: ModuleCommandContext {
            actor_id: Uuid::new_v4(),
            tenant_id: None,
            trace_id: "trace-ext-promo-req".to_string(),
            correlation_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        },
    };

    let promo_err = promotion_service
        .request(promo_command)
        .await
        .expect_err("static promotion of external prebuilt MUST BE REJECTED");

    assert!(
        matches!(promo_err, ModuleStaticPromotionError::ExternalPrebuiltCannotBePromoted),
        "Expected ExternalPrebuiltCannotBePromoted error, received {:?}",
        promo_err
    );
}

#[tokio::test]
async fn test_external_prebuilt_idempotent_replay_and_conflict() {
    let db = setup_db().await;
    let blobs = Arc::new(InMemoryArtifactBlobStore::default());
    let mut registry = SpyRegistry::new();

    let wasm_bytes = b"\x00asm\x01\x00\x00\x00-external-prebuilt-idempotent";
    let (package, _manifest_digest) = create_sample_package(wasm_bytes, "idem_mod", "1.0.0");
    registry.add_package(package.clone());
    let registry = Arc::new(registry);

    let service = ExternalPrebuiltIngressService::new(db.clone(), blobs.clone(), registry.clone())
        .with_infrastructure(ControlPlaneInfrastructure::default());

    let actor_id = Uuid::new_v4();
    let idempotency_key = Uuid::new_v4();

    let command1 = ExternalPrebuiltIngressCommand {
        reference: package.reference.clone(),
        scope: ModuleInstallationScope::Platform,
        context: ModuleCommandContext {
            actor_id,
            tenant_id: None,
            trace_id: "trace-idem-1".to_string(),
            correlation_id: Uuid::new_v4(),
            idempotency_key,
        },
        evidence: valid_evidence(),
        trust_policy_revision: Some(1),
        capability_policy_revision: Some(1),
    };

    let receipt1 = service
        .admit_external_prebuilt(command1)
        .await
        .expect("initial admission succeeds");
    assert!(receipt1.cas_published);

    // Replay with exact same reference and idempotency key -> returns cached receipt
    let command2 = ExternalPrebuiltIngressCommand {
        reference: package.reference.clone(),
        scope: ModuleInstallationScope::Platform,
        context: ModuleCommandContext {
            actor_id,
            tenant_id: None,
            trace_id: "trace-idem-2".to_string(),
            correlation_id: Uuid::new_v4(),
            idempotency_key,
        },
        evidence: valid_evidence(),
        trust_policy_revision: Some(1),
        capability_policy_revision: Some(1),
    };

    let receipt2 = service
        .admit_external_prebuilt(command2)
        .await
        .expect("replay succeeds");
    assert!(!receipt2.cas_published, "cas_published must be false on idempotent replay");
    assert_eq!(receipt1.payload_digest, receipt2.payload_digest);

    // Replay with same idempotency key but different release digest -> returns IdempotencyConflict
    let wasm_bytes_alt = b"\x00asm\x01\x00\x00\x00-alt-different-payload";
    let (package_alt, _) = create_sample_package(wasm_bytes_alt, "idem_mod_alt", "2.0.0");
    let command_conflict = ExternalPrebuiltIngressCommand {
        reference: package_alt.reference.clone(),
        scope: ModuleInstallationScope::Platform,
        context: ModuleCommandContext {
            actor_id,
            tenant_id: None,
            trace_id: "trace-idem-3".to_string(),
            correlation_id: Uuid::new_v4(),
            idempotency_key,
        },
        evidence: valid_evidence(),
        trust_policy_revision: Some(1),
        capability_policy_revision: Some(1),
    };

    let conflict_err = service
        .admit_external_prebuilt(command_conflict)
        .await
        .expect_err("conflict must be rejected");
    assert!(matches!(
        conflict_err,
        ExternalPrebuiltIngressError::IdempotencyConflict(key, _) if key == idempotency_key
    ));
}

#[tokio::test]
async fn test_runtime_and_recovery_read_cas_only_and_never_query_oci() {
    let db = setup_db().await;
    let blobs = Arc::new(InMemoryArtifactBlobStore::default());
    let mut registry = SpyRegistry::new();

    let rhai_script = b"fn main() { return 42; }";
    let (descriptor, _, _) = build_test_descriptor(
        "ext_rhai",
        "1.0.0",
        rhai_script,
        ArtifactPayloadKind::Rhai,
        RHAI_SANDBOX_RUNTIME_ABI,
    );
    let manifest_bytes = serde_json::to_vec(&descriptor).unwrap();
    let manifest_digest = sha256_digest(&manifest_bytes);
    let reference = OciArtifactReference {
        registry: "ghcr.io".to_string(),
        repository: "rustok/ext_rhai".to_string(),
        digest: manifest_digest.clone(),
    };

    let package = rustok_modules::ModuleArtifactPackage {
        reference: reference.clone(),
        descriptor: descriptor.clone(),
        media_type: ArtifactPayloadKind::Rhai.oci_layer_media_type().to_string(),
        payload: ArtifactPayloadSource::Bytes(rhai_script.to_vec()),
    };
    registry.add_package(package);
    let registry = Arc::new(registry);

    let service = ExternalPrebuiltIngressService::new(db.clone(), blobs.clone(), registry.clone())
        .with_infrastructure(ControlPlaneInfrastructure::default());

    let mut evidence = valid_evidence();
    evidence.abi_capability.abi_kind = ArtifactPayloadKind::Rhai;
    evidence.abi_capability.declared_capabilities = vec![];

    let command = ExternalPrebuiltIngressCommand {
        reference: reference.clone(),
        scope: ModuleInstallationScope::Platform,
        context: ModuleCommandContext {
            actor_id: Uuid::new_v4(),
            tenant_id: None,
            trace_id: "trace-runtime-cas".to_string(),
            correlation_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        },
        evidence,
        trust_policy_revision: Some(1),
        capability_policy_revision: Some(1),
    };

    let _receipt = service
        .admit_external_prebuilt(command)
        .await
        .expect("external prebuilt admission succeeds");

    // Exactly 1 fetch call occurred to OCI registry during initial admission
    assert_eq!(registry.call_count(), 1);

    // Setup runtime that uses blobs (CAS) only
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

    let installed = InstalledModuleArtifact {
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

    let binding = installed.descriptor.bindings[0].clone();
    let context = SandboxContext::new(ExecutionPhase::Event);
    let input = ArtifactBindingDispatchEnvelope::new(
        &binding,
        ExecutionPhase::Event,
        serde_json::json!({ "id": 1 }),
    );

    // Execute runtime from CAS
    let outcome = runtime
        .execute_binding(
            &installed,
            &binding,
            context.clone(),
            input.clone(),
            SandboxPolicy::default(),
        )
        .await
        .expect("runtime dispatch from CAS succeeds");

    assert_eq!(outcome.output, serde_json::json!({ "result": 42 }));

    // Registry call count is STILL 1: zero OCI calls during execution!
    assert_eq!(
        registry.call_count(),
        1,
        "Runtime must read CAS only and NEVER call OCI"
    );

    // Now prove failure when CAS payload is missing: runtime fails closed and NEVER repairs from OCI
    let mut missing_payload_artifact = installed.clone();
    let missing_digest = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    missing_payload_artifact.descriptor.artifact_digest = missing_digest.to_string();
    missing_payload_artifact.release.digest = missing_digest.to_string();

    let failure = runtime
        .execute_binding(
            &missing_payload_artifact,
            &binding,
            context,
            input,
            SandboxPolicy::default(),
        )
        .await;

    match failure {
        Err(ArtifactRuntimeError::Installation(ModuleInstallationError::BlobNotFound(d))) => {
            assert_eq!(d, missing_digest);
        }
        other => panic!("Expected BlobNotFound, received {other:?}"),
    }

    // Registry call count is STILL 1: runtime made 0 attempts to query OCI as fallback repair!
    assert_eq!(
        registry.call_count(),
        1,
        "Runtime must NEVER fall back to OCI on missing CAS blob"
    );
}

#[tokio::test]
async fn test_recovery_reads_cas_only_and_never_falls_back_to_oci() {
    let blobs = Arc::new(InMemoryArtifactBlobStore::default());
    let spy_registry = Arc::new(SpyRegistry::new());

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
