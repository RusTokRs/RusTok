use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ArtifactAdmissionLimits, ArtifactPayloadKind, ArtifactPayloadSource,
    ArtifactVerificationEvidence, ControlPlaneInfrastructure, ModuleArtifactPackage,
    ModuleBuildServiceAttestationCommand, ModuleGovernanceError, ModulePlatformAdmissionCommand,
    ModulePlatformPublicationSource, ModulePublicationEvidenceResult, OciArtifactReference,
    SeaOrmModuleGovernanceService, TrustVerificationRequest, TrustVerifier,
    normalize_module_registry_id,
};

/// Deployment policy selected for one platform-built publication verification.
/// Registry credentials and trust roots remain inside the two adapters and are
/// never represented in this command or persisted by the modules owner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModulePlatformPublicationEvidenceCommand {
    pub request_id: String,
    pub registry_id: String,
    pub trust_policy_revision: u64,
    pub capability_policy_revision: u64,
    pub build_service_issuer_identity: String,
    pub build_service_policy_revision: String,
    pub actor_principal: serde_json::Value,
}

impl ModulePlatformPublicationEvidenceCommand {
    pub fn validate(&self) -> Result<(), ModulePlatformPublicationEvidenceError> {
        if self.request_id.trim().is_empty()
            || normalize_module_registry_id(&self.registry_id).as_deref()
                != Some(self.registry_id.as_str())
            || self.trust_policy_revision == 0
            || self.capability_policy_revision == 0
            || self.build_service_issuer_identity.trim().is_empty()
            || self.build_service_policy_revision.trim().is_empty()
            || !self.actor_principal.is_object()
        {
            return Err(ModulePlatformPublicationEvidenceError::InvalidCommand);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModulePlatformPublicationEvidenceResult {
    pub build_service_attestation: ModulePublicationEvidenceResult,
    pub platform_admission: ModulePublicationEvidenceResult,
}

#[derive(Debug, Error)]
pub enum ModulePlatformPublicationEvidenceError {
    #[error("platform publication-evidence command is invalid")]
    InvalidCommand,
    #[error("platform publication source is unavailable: {0}")]
    Source(#[source] ModuleGovernanceError),
    #[error("platform publication OCI registry is unavailable: {0}")]
    Registry(String),
    #[error("published OCI artifact does not match the owner-selected build")]
    ArtifactIdentityMismatch,
    #[error("isolated platform publication verification failed: {0}")]
    Verification(String),
    #[error("isolated platform publication verification did not admit the selected artifact")]
    VerificationRejected,
    #[error("platform publication evidence could not be recorded: {0}")]
    Record(#[source] ModuleGovernanceError),
}

/// Narrow owner port used by the production producer and focused tests. The
/// concrete implementation delegates to the modules governance aggregate, so
/// a worker cannot persist either reserved authority through a generic path.
#[async_trait]
pub trait ModulePlatformPublicationEvidenceOwner: Send + Sync {
    async fn load_source(
        &self,
        request_id: &str,
    ) -> Result<ModulePlatformPublicationSource, ModuleGovernanceError>;

    async fn record_build_service_attestation(
        &self,
        command: ModuleBuildServiceAttestationCommand,
    ) -> Result<ModulePublicationEvidenceResult, ModuleGovernanceError>;

    async fn record_platform_admission(
        &self,
        command: ModulePlatformAdmissionCommand,
    ) -> Result<ModulePublicationEvidenceResult, ModuleGovernanceError>;
}

#[async_trait]
impl ModulePlatformPublicationEvidenceOwner for SeaOrmModuleGovernanceService {
    async fn load_source(
        &self,
        request_id: &str,
    ) -> Result<ModulePlatformPublicationSource, ModuleGovernanceError> {
        self.load_platform_publication_source(request_id).await
    }

    async fn record_build_service_attestation(
        &self,
        command: ModuleBuildServiceAttestationCommand,
    ) -> Result<ModulePublicationEvidenceResult, ModuleGovernanceError> {
        SeaOrmModuleGovernanceService::record_build_service_attestation(self, command).await
    }

    async fn record_platform_admission(
        &self,
        command: ModulePlatformAdmissionCommand,
    ) -> Result<ModulePublicationEvidenceResult, ModuleGovernanceError> {
        SeaOrmModuleGovernanceService::record_platform_admission(self, command).await
    }
}

/// Credential-owning adapter that creates an exact digest-pinned registry
/// reader. Production implementations acquire only a short-lived lease for
/// the registry/repository carried by `reference`.
#[async_trait]
pub trait ModulePublicationArtifactRegistryProvider: Send + Sync {
    async fn registry_for(
        &self,
        reference: &OciArtifactReference,
    ) -> Result<Arc<dyn crate::ArtifactRegistry>, String>;
}

/// Canonical production orchestration for the two reserved supply-chain facts.
/// It reloads owner state, fetches and rehashes the exact OCI payload, invokes
/// the isolated verifier, and only then records build-service and platform
/// admission evidence. Partial persistence is safe to retry because both owner
/// operations are immutable and idempotent.
pub struct ModulePlatformPublicationEvidenceProducer {
    owner: Arc<dyn ModulePlatformPublicationEvidenceOwner>,
    registries: Arc<dyn ModulePublicationArtifactRegistryProvider>,
    verifier: Arc<dyn TrustVerifier>,
    limits: ArtifactAdmissionLimits,
    infrastructure: ControlPlaneInfrastructure,
}

impl ModulePlatformPublicationEvidenceProducer {
    pub fn new(
        owner: Arc<dyn ModulePlatformPublicationEvidenceOwner>,
        registries: Arc<dyn ModulePublicationArtifactRegistryProvider>,
        verifier: Arc<dyn TrustVerifier>,
    ) -> Self {
        Self {
            owner,
            registries,
            verifier,
            limits: ArtifactAdmissionLimits::default(),
            infrastructure: ControlPlaneInfrastructure::default(),
        }
    }

    pub fn with_limits(mut self, limits: ArtifactAdmissionLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn with_infrastructure(mut self, infrastructure: ControlPlaneInfrastructure) -> Self {
        self.infrastructure = infrastructure;
        self
    }

    pub async fn produce(
        &self,
        command: ModulePlatformPublicationEvidenceCommand,
    ) -> Result<ModulePlatformPublicationEvidenceResult, ModulePlatformPublicationEvidenceError>
    {
        command.validate()?;
        let source = self
            .owner
            .load_source(&command.request_id)
            .await
            .map_err(ModulePlatformPublicationEvidenceError::Source)?;
        if source.request_id != command.request_id {
            return Err(ModulePlatformPublicationEvidenceError::ArtifactIdentityMismatch);
        }
        let registry = self
            .registries
            .registry_for(&source.receipt.artifact)
            .await
            .map_err(ModulePlatformPublicationEvidenceError::Registry)?;
        let package = registry
            .fetch(&source.receipt.artifact, self.limits)
            .await
            .map_err(|error| ModulePlatformPublicationEvidenceError::Registry(error.to_string()))?;
        let temporary_payload = match &package.payload {
            ArtifactPayloadSource::TemporaryFile(path) => Some(path.clone()),
            ArtifactPayloadSource::Bytes(_) => None,
        };
        let result = self.verify_and_record(command, source, package).await;
        if let Some(path) = temporary_payload {
            let _ = tokio::fs::remove_file(path).await;
        }
        result
    }

    async fn verify_and_record(
        &self,
        command: ModulePlatformPublicationEvidenceCommand,
        source: ModulePlatformPublicationSource,
        package: ModuleArtifactPackage,
    ) -> Result<ModulePlatformPublicationEvidenceResult, ModulePlatformPublicationEvidenceError>
    {
        package
            .verify(self.limits)
            .await
            .map_err(|error| ModulePlatformPublicationEvidenceError::Registry(error.to_string()))?;
        if package.reference != source.receipt.artifact
            || package.descriptor.slug != source.slug
            || package.descriptor.version != source.version
            || package.descriptor.artifact_digest != source.component_digest
            || package.descriptor.payload_kind == ArtifactPayloadKind::StaticPromoted
        {
            return Err(ModulePlatformPublicationEvidenceError::ArtifactIdentityMismatch);
        }
        let verification_request = TrustVerificationRequest {
            reference: package.reference.clone(),
            descriptor: package.descriptor.clone(),
            trust_policy_revision: command.trust_policy_revision,
            capability_policy_revision: command.capability_policy_revision,
        };
        let decision = self
            .verifier
            .verify(verification_request.clone())
            .await
            .map_err(ModulePlatformPublicationEvidenceError::Verification)?;
        if decision.trust_policy_revision != verification_request.trust_policy_revision
            || decision.capability_policy_revision
                != verification_request.capability_policy_revision
            || decision.signer_identity.trim().is_empty()
            || !decision.admitted()
        {
            return Err(ModulePlatformPublicationEvidenceError::VerificationRejected);
        }
        let evidence = ArtifactVerificationEvidence {
            manifest_digest: package.reference.digest.clone(),
            payload_digest: package.descriptor.artifact_digest.clone(),
            media_type: package.media_type.clone(),
            signer_identity: decision.signer_identity,
            trust_policy_revision: decision.trust_policy_revision,
            capability_policy_revision: decision.capability_policy_revision,
            signature_verified: decision.signature_verified,
            provenance_verified: decision.provenance_verified,
            sbom_verified: decision.sbom_verified,
            license_policy_verified: decision.license_policy_verified,
            vulnerability_policy_verified: decision.vulnerability_policy_verified,
            evidence: decision.evidence,
            verified_at: self.infrastructure.now(),
        };
        let build_service_attestation = self
            .owner
            .record_build_service_attestation(ModuleBuildServiceAttestationCommand {
                request_id: source.request_id.clone(),
                expected_revision: source.request_revision,
                receipt: source.receipt,
                issuer_identity: command.build_service_issuer_identity,
                policy_revision: command.build_service_policy_revision,
                actor_principal: command.actor_principal.clone(),
            })
            .await
            .map_err(ModulePlatformPublicationEvidenceError::Record)?;
        let platform_admission = self
            .owner
            .record_platform_admission(ModulePlatformAdmissionCommand {
                request_id: source.request_id,
                expected_revision: build_service_attestation.request_revision,
                registry_id: command.registry_id,
                reference: package.reference,
                descriptor: package.descriptor,
                evidence,
                actor_principal: command.actor_principal,
            })
            .await
            .map_err(ModulePlatformPublicationEvidenceError::Record)?;
        Ok(ModulePlatformPublicationEvidenceResult {
            build_service_attestation,
            platform_admission,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;
    use sha2::{Digest, Sha256};

    use super::*;
    use crate::{
        ArtifactModuleKind, ArtifactRegistry, MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
        ModuleArtifactDescriptor, ModuleBuildPublicationReceipt, ModuleBuildSignatureAuthority,
        ModuleInstallationError, TrustEvidenceKind, TrustEvidenceReference,
        TrustVerificationDecision,
    };

    fn digest(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn reference(byte: char) -> OciArtifactReference {
        OciArtifactReference {
            registry: "registry.example".to_string(),
            repository: "modules/sample_module".to_string(),
            digest: digest(byte),
        }
    }

    fn descriptor(payload: &[u8]) -> ModuleArtifactDescriptor {
        ModuleArtifactDescriptor {
            schema_version: MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            payload_kind: ArtifactPayloadKind::WasmComponent,
            module_kind: ArtifactModuleKind::Optional,
            runtime_abi: "rustok:module/runtime@1".to_string(),
            platform_compatibility: "^0.1".to_string(),
            required_features: Vec::new(),
            artifact_digest: format!("sha256:{}", hex::encode(Sha256::digest(payload))),
            entrypoint: "main".to_string(),
            capabilities: Vec::new(),
            bindings: Vec::new(),
            dependencies: Vec::new(),
            permissions: Vec::new(),
            schema_documents: Vec::new(),
            settings_schema_digest: None,
            data_schema_digest: None,
            localization_catalogs: Vec::new(),
            ui_contributions: Vec::new(),
            persistence_contract: None,
        }
    }

    struct Owner {
        source: ModulePlatformPublicationSource,
        build: Mutex<Vec<ModuleBuildServiceAttestationCommand>>,
        admission: Mutex<Vec<ModulePlatformAdmissionCommand>>,
    }

    #[async_trait]
    impl ModulePlatformPublicationEvidenceOwner for Owner {
        async fn load_source(
            &self,
            _request_id: &str,
        ) -> Result<ModulePlatformPublicationSource, ModuleGovernanceError> {
            Ok(self.source.clone())
        }

        async fn record_build_service_attestation(
            &self,
            command: ModuleBuildServiceAttestationCommand,
        ) -> Result<ModulePublicationEvidenceResult, ModuleGovernanceError> {
            let request_revision = command.expected_revision + 1;
            self.build.lock().expect("build records").push(command);
            Ok(ModulePublicationEvidenceResult {
                evidence_id: "build-evidence".to_string(),
                recorded: true,
                request_revision,
            })
        }

        async fn record_platform_admission(
            &self,
            command: ModulePlatformAdmissionCommand,
        ) -> Result<ModulePublicationEvidenceResult, ModuleGovernanceError> {
            let request_revision = command.expected_revision + 1;
            self.admission
                .lock()
                .expect("admission records")
                .push(command);
            Ok(ModulePublicationEvidenceResult {
                evidence_id: "platform-evidence".to_string(),
                recorded: true,
                request_revision,
            })
        }
    }

    struct RegistryProvider {
        package: ModuleArtifactPackage,
    }

    struct Registry {
        package: ModuleArtifactPackage,
    }

    #[async_trait]
    impl ArtifactRegistry for Registry {
        async fn fetch(
            &self,
            _reference: &OciArtifactReference,
            _limits: ArtifactAdmissionLimits,
        ) -> Result<ModuleArtifactPackage, ModuleInstallationError> {
            Ok(self.package.clone())
        }
    }

    #[async_trait]
    impl ModulePublicationArtifactRegistryProvider for RegistryProvider {
        async fn registry_for(
            &self,
            _reference: &OciArtifactReference,
        ) -> Result<Arc<dyn ArtifactRegistry>, String> {
            Ok(Arc::new(Registry {
                package: self.package.clone(),
            }))
        }
    }

    struct Verifier;

    #[async_trait]
    impl TrustVerifier for Verifier {
        async fn verify(
            &self,
            request: TrustVerificationRequest,
        ) -> Result<TrustVerificationDecision, String> {
            Ok(TrustVerificationDecision {
                signer_identity: "build-service:production".to_string(),
                trust_policy_revision: request.trust_policy_revision,
                capability_policy_revision: request.capability_policy_revision,
                signature_verified: true,
                provenance_verified: true,
                sbom_verified: true,
                license_policy_verified: true,
                vulnerability_policy_verified: true,
                evidence: [
                    TrustEvidenceKind::Signature,
                    TrustEvidenceKind::Provenance,
                    TrustEvidenceKind::Sbom,
                ]
                .into_iter()
                .map(|kind| TrustEvidenceReference {
                    kind,
                    reference: format!("oci://{}#{kind:?}", request.reference.canonical()),
                    digest: digest(match kind {
                        TrustEvidenceKind::Signature => '5',
                        TrustEvidenceKind::Provenance => '6',
                        TrustEvidenceKind::Sbom => '7',
                    }),
                })
                .collect(),
            })
        }
    }

    fn command() -> ModulePlatformPublicationEvidenceCommand {
        ModulePlatformPublicationEvidenceCommand {
            request_id: "request-1".to_string(),
            registry_id: "local".to_string(),
            trust_policy_revision: 7,
            capability_policy_revision: 9,
            build_service_issuer_identity: "build-service:production".to_string(),
            build_service_policy_revision: "build-policy-12".to_string(),
            actor_principal: serde_json::json!({"kind":"service","id":"publication-evidence"}),
        }
    }

    #[tokio::test]
    async fn producer_binds_owner_source_oci_payload_and_verifier_decision() {
        let payload = b"component".to_vec();
        let descriptor = descriptor(&payload);
        let receipt = ModuleBuildPublicationReceipt {
            artifact: reference('1'),
            sbom_referrer: reference('2'),
            provenance_referrer: reference('3'),
            signature_manifest: reference('4'),
            signature_authority: ModuleBuildSignatureAuthority::BuildService,
        };
        let owner = Arc::new(Owner {
            source: ModulePlatformPublicationSource {
                request_id: "request-1".to_string(),
                request_revision: 1,
                tenant_id: uuid::Uuid::new_v4(),
                build_request_id: uuid::Uuid::new_v4(),
                slug: descriptor.slug.clone(),
                version: descriptor.version.clone(),
                component_digest: descriptor.artifact_digest.clone(),
                receipt: receipt.clone(),
            },
            build: Mutex::new(Vec::new()),
            admission: Mutex::new(Vec::new()),
        });
        let producer = ModulePlatformPublicationEvidenceProducer::new(
            owner.clone(),
            Arc::new(RegistryProvider {
                package: ModuleArtifactPackage {
                    reference: receipt.artifact.clone(),
                    descriptor: descriptor.clone(),
                    media_type: descriptor.payload_kind.oci_layer_media_type().to_string(),
                    payload: ArtifactPayloadSource::Bytes(payload),
                },
            }),
            Arc::new(Verifier),
        );

        let result = producer.produce(command()).await.expect("produce evidence");

        assert!(result.build_service_attestation.recorded);
        assert!(result.platform_admission.recorded);
        let build = owner.build.lock().expect("build records");
        assert_eq!(build.len(), 1);
        assert_eq!(build[0].receipt, receipt);
        assert_eq!(build[0].expected_revision, 1);
        let admission = owner.admission.lock().expect("admission records");
        assert_eq!(admission.len(), 1);
        assert_eq!(admission[0].descriptor, descriptor);
        assert_eq!(admission[0].reference, receipt.artifact);
        assert_eq!(admission[0].expected_revision, 2);
        assert_eq!(admission[0].evidence.trust_policy_revision, 7);
        assert_eq!(admission[0].evidence.capability_policy_revision, 9);
    }

    #[tokio::test]
    async fn producer_rejects_a_descriptor_that_does_not_match_the_staged_component() {
        let payload = b"component".to_vec();
        let descriptor = descriptor(&payload);
        let receipt = ModuleBuildPublicationReceipt {
            artifact: reference('1'),
            sbom_referrer: reference('2'),
            provenance_referrer: reference('3'),
            signature_manifest: reference('4'),
            signature_authority: ModuleBuildSignatureAuthority::BuildService,
        };
        let owner = Arc::new(Owner {
            source: ModulePlatformPublicationSource {
                request_id: "request-1".to_string(),
                request_revision: 1,
                tenant_id: uuid::Uuid::new_v4(),
                build_request_id: uuid::Uuid::new_v4(),
                slug: descriptor.slug.clone(),
                version: descriptor.version.clone(),
                component_digest: digest('f'),
                receipt: receipt.clone(),
            },
            build: Mutex::new(Vec::new()),
            admission: Mutex::new(Vec::new()),
        });
        let producer = ModulePlatformPublicationEvidenceProducer::new(
            owner.clone(),
            Arc::new(RegistryProvider {
                package: ModuleArtifactPackage {
                    reference: receipt.artifact,
                    descriptor,
                    media_type: ArtifactPayloadKind::WasmComponent
                        .oci_layer_media_type()
                        .to_string(),
                    payload: ArtifactPayloadSource::Bytes(payload),
                },
            }),
            Arc::new(Verifier),
        );

        let error = producer
            .produce(command())
            .await
            .expect_err("mismatched component must fail");

        assert!(matches!(
            error,
            ModulePlatformPublicationEvidenceError::ArtifactIdentityMismatch
        ));
        assert!(owner.build.lock().expect("build records").is_empty());
        assert!(
            owner
                .admission
                .lock()
                .expect("admission records")
                .is_empty()
        );
    }
}
