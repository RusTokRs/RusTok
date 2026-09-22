use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ArtifactAdmissionLimits, ArtifactPayloadKind, ArtifactPayloadSource,
    ArtifactVerificationEvidence, ControlPlaneInfrastructure, ModuleAlloyPublicationSource,
    ModuleArtifactPackage, ModuleBuildServiceAttestationCommand, ModuleGovernanceError,
    ModulePlatformAdmissionCommand, ModulePlatformPublicationSource,
    ModulePublicationEvidenceResult, OciArtifactReference, SeaOrmModuleGovernanceService,
    TrustVerificationRequest, TrustVerifier, normalize_module_registry_id,
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

/// Deployment-owned policy selected when an owner-receipted Alloy workspace
/// has already passed delivery validation and is ready for its deterministic
/// OCI package publication. Unlike the platform-built command, this contains
/// no build-service issuer because Alloy must never manufacture a build-worker
/// attestation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleAlloyPublicationEvidenceCommand {
    pub request_id: String,
    pub registry_id: String,
    pub artifact: OciArtifactReference,
    pub trust_policy_revision: u64,
    pub capability_policy_revision: u64,
    pub actor_principal: serde_json::Value,
}

impl ModuleAlloyPublicationEvidenceCommand {
    pub fn validate(&self) -> Result<(), ModuleAlloyPublicationEvidenceError> {
        if self.request_id.trim().is_empty()
            || normalize_module_registry_id(&self.registry_id).as_deref()
                != Some(self.registry_id.as_str())
            || self.trust_policy_revision == 0
            || self.capability_policy_revision == 0
            || !self.actor_principal.is_object()
        {
            return Err(ModuleAlloyPublicationEvidenceError::InvalidCommand);
        }
        self.artifact
            .validate()
            .map_err(|_| ModuleAlloyPublicationEvidenceError::InvalidCommand)
    }
}

#[derive(Debug, Error)]
pub enum ModuleAlloyPublicationEvidenceError {
    #[error("Alloy publication-evidence command is invalid")]
    InvalidCommand,
    #[error("Alloy publication source is unavailable: {0}")]
    Source(#[source] ModuleGovernanceError),
    #[error("Alloy publication evidence could not be recorded: {0}")]
    Record(#[source] ModuleGovernanceError),
    #[error("Alloy publication verification failed: {0}")]
    Verification(String),
    #[error("published Alloy OCI artifact does not match the immutable owner receipt")]
    ArtifactIdentityMismatch,
    #[error("isolated Alloy publication verification did not admit the selected artifact")]
    VerificationRejected,
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

/// Narrow owner port for an Alloy OCI admission. It intentionally lacks the
/// build-service write available to `ModulePlatformPublicationEvidenceOwner`:
/// an Alloy workspace receives only the independently verified platform
/// admission fact.
#[async_trait]
pub trait ModuleAlloyPublicationEvidenceOwner: Send + Sync {
    async fn load_alloy_source(
        &self,
        request_id: &str,
    ) -> Result<ModuleAlloyPublicationSource, ModuleGovernanceError>;

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

#[async_trait]
impl ModuleAlloyPublicationEvidenceOwner for SeaOrmModuleGovernanceService {
    async fn load_alloy_source(
        &self,
        request_id: &str,
    ) -> Result<ModuleAlloyPublicationSource, ModuleGovernanceError> {
        self.load_alloy_publication_source(request_id).await
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

#[derive(Debug)]
enum PublishedArtifactVerificationError {
    Registry(String),
    IdentityMismatch,
    Verification(String),
    Rejected,
}

struct PublishedArtifactVerificationInput<'a> {
    reference: &'a OciArtifactReference,
    expected_slug: &'a str,
    expected_version: &'a str,
    expected_payload_digest: &'a str,
    expected_descriptor: Option<&'a crate::ModuleArtifactDescriptor>,
    trust_policy_revision: u64,
    capability_policy_revision: u64,
    expected_media_type: Option<&'a str>,
    expected_alloy_workspace_provenance: Option<&'a crate::TrustAlloyWorkspaceProvenance>,
}

struct VerifiedPublishedArtifact {
    reference: OciArtifactReference,
    descriptor: crate::ModuleArtifactDescriptor,
    evidence: ArtifactVerificationEvidence,
}

/// Fetches one digest-pinned package through a credential-owning adapter and
/// reduces a successful isolated verification decision to the owner-safe
/// admission evidence. Platform builds and Alloy workspaces share this exact
/// re-fetch boundary; only their owner source and the evidence they may record
/// differ.
async fn verify_published_artifact(
    registries: &dyn ModulePublicationArtifactRegistryProvider,
    verifier: &dyn TrustVerifier,
    limits: ArtifactAdmissionLimits,
    infrastructure: &ControlPlaneInfrastructure,
    input: PublishedArtifactVerificationInput<'_>,
) -> Result<VerifiedPublishedArtifact, PublishedArtifactVerificationError> {
    let registry = registries
        .registry_for(input.reference)
        .await
        .map_err(PublishedArtifactVerificationError::Registry)?;
    let package = registry
        .fetch(input.reference, limits)
        .await
        .map_err(|error| PublishedArtifactVerificationError::Registry(error.to_string()))?;
    let temporary_payload = match &package.payload {
        ArtifactPayloadSource::TemporaryFile(path) => Some(path.clone()),
        ArtifactPayloadSource::Bytes(_) => None,
    };
    let result =
        verify_fetched_published_artifact(verifier, limits, infrastructure, input, package).await;
    if let Some(path) = temporary_payload {
        let _ = tokio::fs::remove_file(path).await;
    }
    result
}

async fn verify_fetched_published_artifact(
    verifier: &dyn TrustVerifier,
    limits: ArtifactAdmissionLimits,
    infrastructure: &ControlPlaneInfrastructure,
    input: PublishedArtifactVerificationInput<'_>,
    package: ModuleArtifactPackage,
) -> Result<VerifiedPublishedArtifact, PublishedArtifactVerificationError> {
    package
        .verify(limits)
        .await
        .map_err(|error| PublishedArtifactVerificationError::Registry(error.to_string()))?;
    if package.reference != *input.reference
        || package.descriptor.slug != input.expected_slug
        || package.descriptor.version != input.expected_version
        || package.descriptor.artifact_digest != input.expected_payload_digest
        || input
            .expected_descriptor
            .is_some_and(|descriptor| package.descriptor != *descriptor)
        || package.descriptor.payload_kind == ArtifactPayloadKind::StaticPromoted
        || input
            .expected_media_type
            .is_some_and(|media_type| package.media_type != media_type)
    {
        return Err(PublishedArtifactVerificationError::IdentityMismatch);
    }
    let verification_request = TrustVerificationRequest {
        reference: package.reference.clone(),
        descriptor: package.descriptor.clone(),
        trust_policy_revision: input.trust_policy_revision,
        capability_policy_revision: input.capability_policy_revision,
        expected_alloy_workspace_provenance: input.expected_alloy_workspace_provenance.cloned(),
    };
    let decision = verifier
        .verify(verification_request.clone())
        .await
        .map_err(PublishedArtifactVerificationError::Verification)?;
    if decision.trust_policy_revision != verification_request.trust_policy_revision
        || decision.capability_policy_revision != verification_request.capability_policy_revision
        || decision.signer_identity.trim().is_empty()
        || !decision.admitted()
    {
        return Err(PublishedArtifactVerificationError::Rejected);
    }
    Ok(VerifiedPublishedArtifact {
        reference: package.reference.clone(),
        descriptor: package.descriptor.clone(),
        evidence: ArtifactVerificationEvidence {
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
            verified_at: infrastructure.now(),
        },
    })
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
        let verified = verify_published_artifact(
            self.registries.as_ref(),
            self.verifier.as_ref(),
            self.limits,
            &self.infrastructure,
            PublishedArtifactVerificationInput {
                reference: &source.receipt.artifact,
                expected_slug: &source.slug,
                expected_version: &source.version,
                expected_payload_digest: &source.component_digest,
                expected_descriptor: None,
                trust_policy_revision: command.trust_policy_revision,
                capability_policy_revision: command.capability_policy_revision,
                expected_media_type: None,
                expected_alloy_workspace_provenance: None,
            },
        )
        .await
        .map_err(map_platform_publication_verification_error)?;
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
                reference: verified.reference,
                descriptor: verified.descriptor,
                evidence: verified.evidence,
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

fn map_platform_publication_verification_error(
    error: PublishedArtifactVerificationError,
) -> ModulePlatformPublicationEvidenceError {
    match error {
        PublishedArtifactVerificationError::Registry(error) => {
            ModulePlatformPublicationEvidenceError::Registry(error)
        }
        PublishedArtifactVerificationError::IdentityMismatch => {
            ModulePlatformPublicationEvidenceError::ArtifactIdentityMismatch
        }
        PublishedArtifactVerificationError::Verification(error) => {
            ModulePlatformPublicationEvidenceError::Verification(error)
        }
        PublishedArtifactVerificationError::Rejected => {
            ModulePlatformPublicationEvidenceError::VerificationRejected
        }
    }
}

/// Canonical admission orchestrator for a deterministic Rhai workspace OCI
/// package. The registry-validation worker publishes and signs the package;
/// this producer reloads the owner receipt, fetches the digest-pinned result,
/// verifies its signature/SLSA/SBOM in isolation, and records only platform
/// admission. It deliberately has no path to a build-service attestation.
pub struct ModuleAlloyPublicationEvidenceProducer {
    owner: Arc<dyn ModuleAlloyPublicationEvidenceOwner>,
    registries: Arc<dyn ModulePublicationArtifactRegistryProvider>,
    verifier: Arc<dyn TrustVerifier>,
    limits: ArtifactAdmissionLimits,
    infrastructure: ControlPlaneInfrastructure,
}

impl ModuleAlloyPublicationEvidenceProducer {
    pub fn new(
        owner: Arc<dyn ModuleAlloyPublicationEvidenceOwner>,
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
        command: ModuleAlloyPublicationEvidenceCommand,
    ) -> Result<ModulePublicationEvidenceResult, ModuleAlloyPublicationEvidenceError> {
        command.validate()?;
        let source = self
            .owner
            .load_alloy_source(&command.request_id)
            .await
            .map_err(ModuleAlloyPublicationEvidenceError::Source)?;
        if source.request_id != command.request_id {
            return Err(ModuleAlloyPublicationEvidenceError::ArtifactIdentityMismatch);
        }
        let expected_provenance = source.trust_provenance();
        let verified = verify_published_artifact(
            self.registries.as_ref(),
            self.verifier.as_ref(),
            self.limits,
            &self.infrastructure,
            PublishedArtifactVerificationInput {
                reference: &command.artifact,
                expected_slug: &source.slug,
                expected_version: &source.version,
                expected_payload_digest: &source.source_digest,
                expected_descriptor: Some(&source.descriptor),
                trust_policy_revision: command.trust_policy_revision,
                capability_policy_revision: command.capability_policy_revision,
                expected_media_type: Some(rustok_sandbox::RHAI_WORKSPACE_MEDIA_TYPE),
                expected_alloy_workspace_provenance: Some(&expected_provenance),
            },
        )
        .await
        .map_err(map_alloy_publication_verification_error)?;
        self.owner
            .record_platform_admission(ModulePlatformAdmissionCommand {
                request_id: source.request_id,
                expected_revision: source.request_revision,
                registry_id: command.registry_id,
                reference: verified.reference,
                descriptor: verified.descriptor,
                evidence: verified.evidence,
                actor_principal: command.actor_principal,
            })
            .await
            .map_err(ModuleAlloyPublicationEvidenceError::Record)
    }
}

fn map_alloy_publication_verification_error(
    error: PublishedArtifactVerificationError,
) -> ModuleAlloyPublicationEvidenceError {
    match error {
        PublishedArtifactVerificationError::Registry(error)
        | PublishedArtifactVerificationError::Verification(error) => {
            ModuleAlloyPublicationEvidenceError::Verification(error)
        }
        PublishedArtifactVerificationError::IdentityMismatch => {
            ModuleAlloyPublicationEvidenceError::ArtifactIdentityMismatch
        }
        PublishedArtifactVerificationError::Rejected => {
            ModuleAlloyPublicationEvidenceError::VerificationRejected
        }
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
        ModuleAlloyPublicationSource, ModuleArtifactDescriptor, ModuleBuildPublicationReceipt,
        ModuleBuildSignatureAuthority, ModuleInstallationError, TrustEvidenceKind,
        TrustEvidenceReference, TrustVerificationDecision, canonical_artifact_descriptor_digest,
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

    fn rhai_descriptor(workspace: &rustok_sandbox::RhaiWorkspace) -> ModuleArtifactDescriptor {
        ModuleArtifactDescriptor {
            schema_version: MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            payload_kind: ArtifactPayloadKind::Rhai,
            module_kind: ArtifactModuleKind::Optional,
            runtime_abi: rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI.to_string(),
            platform_compatibility: "^0.1".to_string(),
            required_features: Vec::new(),
            artifact_digest: workspace.digest().expect("workspace digest"),
            entrypoint: workspace.entrypoint.clone(),
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

    struct AlloyOwner {
        source: ModuleAlloyPublicationSource,
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

    #[async_trait]
    impl ModuleAlloyPublicationEvidenceOwner for AlloyOwner {
        async fn load_alloy_source(
            &self,
            _request_id: &str,
        ) -> Result<ModuleAlloyPublicationSource, ModuleGovernanceError> {
            Ok(self.source.clone())
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
                evidence_id: "alloy-platform-evidence".to_string(),
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

    fn admitted_decision(request: &TrustVerificationRequest) -> TrustVerificationDecision {
        TrustVerificationDecision {
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
        }
    }

    struct Verifier;

    #[async_trait]
    impl TrustVerifier for Verifier {
        async fn verify(
            &self,
            request: TrustVerificationRequest,
        ) -> Result<TrustVerificationDecision, String> {
            Ok(admitted_decision(&request))
        }
    }

    struct AlloyVerifier {
        requests: Mutex<Vec<TrustVerificationRequest>>,
    }

    #[async_trait]
    impl TrustVerifier for AlloyVerifier {
        async fn verify(
            &self,
            request: TrustVerificationRequest,
        ) -> Result<TrustVerificationDecision, String> {
            let decision = admitted_decision(&request);
            self.requests
                .lock()
                .expect("verification requests")
                .push(request);
            Ok(decision)
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

    fn alloy_command(artifact: OciArtifactReference) -> ModuleAlloyPublicationEvidenceCommand {
        ModuleAlloyPublicationEvidenceCommand {
            request_id: "request-1".to_string(),
            registry_id: "local".to_string(),
            artifact,
            trust_policy_revision: 7,
            capability_policy_revision: 9,
            actor_principal: serde_json::json!({"kind":"service","id":"publication-evidence"}),
        }
    }

    #[tokio::test]
    async fn producer_binds_owner_source_oci_payload_and_verifier_decision() {
        let payload = b"component".to_vec();
        let descriptor = descriptor(&payload);
        let receipt = ModuleBuildPublicationReceipt {
            artifact: reference('1'),
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

    #[tokio::test]
    async fn alloy_producer_binds_exact_owner_workspace_provenance_without_a_build_attestation() {
        let workspace = rustok_sandbox::RhaiWorkspace::single_source("40 + 2");
        let payload = workspace.canonical_bytes().expect("canonical workspace");
        let descriptor = rhai_descriptor(&workspace);
        let artifact = reference('8');
        let source = ModuleAlloyPublicationSource {
            request_id: "request-1".to_string(),
            request_revision: 3,
            slug: descriptor.slug.clone(),
            version: descriptor.version.clone(),
            license: "MIT".to_string(),
            alloy_tenant_id: uuid::Uuid::new_v4(),
            alloy_script_id: uuid::Uuid::new_v4(),
            source_revision: 7,
            source_digest: descriptor.artifact_digest.clone(),
            review_digest: digest('b'),
            descriptor_digest: canonical_artifact_descriptor_digest(&descriptor),
            descriptor: descriptor.clone(),
        };
        let owner = Arc::new(AlloyOwner {
            source: source.clone(),
            admission: Mutex::new(Vec::new()),
        });
        let verifier = Arc::new(AlloyVerifier {
            requests: Mutex::new(Vec::new()),
        });
        let producer = ModuleAlloyPublicationEvidenceProducer::new(
            owner.clone(),
            Arc::new(RegistryProvider {
                package: ModuleArtifactPackage {
                    reference: artifact.clone(),
                    descriptor: descriptor.clone(),
                    media_type: rustok_sandbox::RHAI_WORKSPACE_MEDIA_TYPE.to_string(),
                    payload: ArtifactPayloadSource::Bytes(payload),
                },
            }),
            verifier.clone(),
        );

        let result = producer
            .produce(alloy_command(artifact.clone()))
            .await
            .expect("Alloy platform admission");

        assert!(result.recorded);
        let admission = owner.admission.lock().expect("admission records");
        assert_eq!(admission.len(), 1);
        assert_eq!(admission[0].expected_revision, source.request_revision);
        assert_eq!(admission[0].reference, artifact);
        assert_eq!(admission[0].descriptor, descriptor);
        let requests = verifier.requests.lock().expect("verification requests");
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].expected_alloy_workspace_provenance,
            Some(source.trust_provenance())
        );
    }
}
