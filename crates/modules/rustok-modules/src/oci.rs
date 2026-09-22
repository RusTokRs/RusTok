//! OCI Distribution adapter for immutable module artifacts.

use async_trait::async_trait;
use futures_util::StreamExt;
use oci_distribution::{
    manifest::{OCI_IMAGE_MEDIA_TYPE, OciDescriptor},
    secrets::RegistryAuth,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::{
    ALLOY_WORKSPACE_PUBLICATION_BUILD_TYPE, ALLOY_WORKSPACE_PUBLICATION_BUILDER_ID,
    ALLOY_WORKSPACE_PUBLICATION_SOURCE_REF, ALLOY_WORKSPACE_PUBLICATION_SOURCE_URI,
    ArtifactAdmissionLimits, ArtifactModuleKind, ArtifactPayloadKind, ArtifactPayloadSource,
    ArtifactRegistry, ControlPlaneInfrastructure, ModuleArtifactDescriptor, ModuleArtifactPackage,
    ModuleBuildOutcome, ModuleBuildRequest, ModuleBuildResult, ModuleInstallationError,
    OciArtifactReference,
    oci_transport::{Blob, OciRegistryTransport, RegistryReference},
};

/// Stable OCI config media type for a serialized immutable module descriptor.
pub const MODULE_ARTIFACT_DESCRIPTOR_MEDIA_TYPE: &str =
    "application/vnd.rustok.module.descriptor.v1+json";
/// Stable OCI referrer media type for bounded machine-readable test evidence.
pub const MODULE_ARTIFACT_TEST_EVIDENCE_MEDIA_TYPE: &str =
    "application/vnd.rustok.module.test-evidence.v1+json";
/// Stable OCI referrer media type for immutable release lineage evidence.
pub const MODULE_ARTIFACT_RELEASE_LINEAGE_MEDIA_TYPE: &str =
    "application/vnd.rustok.module.release-lineage.v1+json";
/// OCI 1.1 media type for the mandatory empty config of an evidence referrer.
pub const OCI_EMPTY_CONFIG_MEDIA_TYPE: &str = "application/vnd.oci.empty.v1+json";

const OCI_EMPTY_CONFIG_BYTES: &[u8] = b"{}";
const OCI_REGISTRY_MAX_CONCURRENT_REQUESTS: usize = 1;
const OCI_REGISTRY_ADMISSION_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// Leaves bounded time for the worker's subsequent Cosign invocation within
/// its deployment-owned fifteen-minute credential lease.
const OCI_REGISTRY_PUBLICATION_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Proxy handling mode for the registry egress boundary. The client always
/// ignores process and system proxy settings; an approved deployment boundary
/// may still provide transparent egress routing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OciRegistryProxyMode {
    Disabled,
    DeploymentBoundaryOnly,
}

/// Explicit registry transport and egress policy. The registry transport
/// validates this complete policy before it creates any HTTP client.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OciRegistryTransportPolicy {
    pub allow_redirects: bool,
    pub allow_cross_host_auth: bool,
    pub verify_tls: bool,
    pub proxy_mode: OciRegistryProxyMode,
    pub request_timeout_ms: u64,
    pub max_retries: u8,
    pub max_transfer_bytes: u64,
    pub max_decompressed_bytes: u64,
    pub max_concurrent_requests: usize,
}

impl OciRegistryTransportPolicy {
    pub const fn strict() -> Self {
        Self {
            allow_redirects: false,
            allow_cross_host_auth: false,
            verify_tls: true,
            proxy_mode: OciRegistryProxyMode::DeploymentBoundaryOnly,
            request_timeout_ms: 300_000,
            max_retries: 2,
            max_transfer_bytes: 64 * 1024 * 1024,
            max_decompressed_bytes: 64 * 1024 * 1024,
            max_concurrent_requests: OCI_REGISTRY_MAX_CONCURRENT_REQUESTS,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.allow_redirects
            || self.allow_cross_host_auth
            || !self.verify_tls
            || self.request_timeout_ms == 0
            || self.request_timeout_ms > 900_000
            || self.max_retries > 3
            || self.max_transfer_bytes == 0
            || self.max_decompressed_bytes == 0
            || self.max_decompressed_bytes > self.max_transfer_bytes
            || self.max_concurrent_requests == 0
            || self.max_concurrent_requests > 4
        {
            return Err("OCI registry transport policy is not fail-closed".to_string());
        }
        Ok(())
    }
}

/// Removes a private OCI staging file if its producer returns an error or is
/// cancelled, including by the outer admission deadline.
struct ArtifactStagingFile {
    path: Option<std::path::PathBuf>,
}

impl ArtifactStagingFile {
    fn new(stage_id: Uuid) -> Self {
        Self {
            path: Some(std::env::temp_dir().join(format!("rustok-artifact-stage-{stage_id}"))),
        }
    }

    fn path(&self) -> &std::path::Path {
        self.path
            .as_deref()
            .expect("staging path is available until it is persisted")
    }

    fn persist(mut self) -> std::path::PathBuf {
        self.path
            .take()
            .expect("staging path is available until it is persisted")
    }
}

impl Drop for ArtifactStagingFile {
    fn drop(&mut self) {
        if let Some(path) = self.path.as_ref() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Constructs the only OCI registry transport used by module artifact
/// publication and admission. Its policy is enforced by the client rather than
/// delegated to process environment or network defaults.
fn strict_oci_registry_transport() -> Result<OciRegistryTransport, String> {
    OciRegistryTransport::with_policy(OciRegistryTransportPolicy::strict())
}

/// Deployment-owned destination for an OCI publication. The publisher derives
/// deterministic write tags, but callers receive only digest-pinned identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OciArtifactPublicationTarget {
    pub registry: String,
    pub repository: String,
}

impl OciArtifactPublicationTarget {
    pub fn validate(&self) -> Result<(), OciArtifactPublicationError> {
        OciArtifactReference {
            registry: self.registry.clone(),
            repository: self.repository.clone(),
            digest: format!("sha256:{}", "0".repeat(64)),
        }
        .validate()
        .map_err(|error| OciArtifactPublicationError::InvalidTarget(error.to_string()))
    }

    fn tag_reference(&self, tag: String) -> Result<RegistryReference, OciArtifactPublicationError> {
        RegistryReference::new(self.registry.clone(), self.repository.clone(), tag)
            .map_err(OciArtifactPublicationError::InvalidTarget)
    }

    fn digest_reference(&self, digest: String) -> OciArtifactReference {
        OciArtifactReference {
            registry: self.registry.clone(),
            repository: self.repository.clone(),
            digest,
        }
    }
}

/// Digest-verified predicate bytes for a fixed Cosign attestation class.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OciArtifactEvidence {
    pub digest: String,
    pub bytes: Vec<u8>,
}

/// Immutable owner facts carried into the signed provenance for one Rhai
/// workspace package. The publication worker derives this value exclusively
/// from the registry owner's Alloy stage receipt; it never accepts it from an
/// artifact upload or a mutable Alloy record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OciRhaiWorkspacePublicationProvenance {
    pub request_id: String,
    pub alloy_tenant_id: Uuid,
    pub alloy_script_id: Uuid,
    pub source_revision: u32,
    pub source_digest: String,
    pub review_digest: String,
    pub descriptor_digest: String,
}

impl OciRhaiWorkspacePublicationProvenance {
    fn validate_against(
        &self,
        descriptor: &ModuleArtifactDescriptor,
    ) -> Result<(), OciArtifactPublicationError> {
        if self.request_id.trim().is_empty()
            || self.request_id.len() > 256
            || self.request_id.chars().any(char::is_control)
            || self.alloy_tenant_id.is_nil()
            || self.alloy_script_id.is_nil()
            || self.source_revision == 0
            || self.source_digest != descriptor.artifact_digest
            || !is_sha256_digest(&self.source_digest)
            || !is_sha256_digest(&self.review_digest)
            || self.descriptor_digest != crate::canonical_artifact_descriptor_digest(descriptor)
        {
            return Err(OciArtifactPublicationError::InvalidBundle(
                "Alloy workspace provenance does not match the immutable owner receipt".to_string(),
            ));
        }
        Ok(())
    }
}

/// Complete, immutable publication input. A build worker or publication host
/// must construct this only from fixed verified output paths.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OciArtifactPublicationBundle {
    pub descriptor: ModuleArtifactDescriptor,
    /// Exact OCI layer media type for `payload`. A Rhai descriptor may use the
    /// canonical workspace representation instead of the legacy single-source
    /// layer type, so callers must not infer it from payload kind alone.
    pub payload_media_type: String,
    pub payload: Vec<u8>,
    pub sbom: OciArtifactEvidence,
    pub provenance: OciArtifactEvidence,
}

impl OciArtifactPublicationBundle {
    /// Binds a publication input to a successful immutable build result.
    pub fn from_verified_component(
        request: &ModuleBuildRequest,
        result: &ModuleBuildResult,
        descriptor: ModuleArtifactDescriptor,
        payload: Vec<u8>,
        sbom: OciArtifactEvidence,
        provenance: OciArtifactEvidence,
        limits: ArtifactAdmissionLimits,
    ) -> Result<Self, OciArtifactPublicationError> {
        request
            .validate()
            .and_then(|_| result.validate_against(request))
            .map_err(|error| OciArtifactPublicationError::InvalidBundle(error.to_string()))?;
        if !matches!(&result.outcome, ModuleBuildOutcome::Succeeded)
            || descriptor.slug != request.expected_module_slug
            || descriptor.version != request.expected_version
            || descriptor.runtime_abi != request.runtime_abi
            || descriptor.payload_kind != crate::ArtifactPayloadKind::WasmComponent
            || Some(descriptor.artifact_digest.as_str()) != result.component_digest.as_deref()
            || Some(sbom.digest.as_str()) != result.sbom_digest.as_deref()
            || Some(provenance.digest.as_str()) != result.provenance_digest.as_deref()
        {
            return Err(OciArtifactPublicationError::InvalidBundle(
                "publication input does not match the successful immutable build result"
                    .to_string(),
            ));
        }
        let bundle = Self {
            payload_media_type: descriptor.payload_kind.oci_layer_media_type().to_string(),
            descriptor,
            payload,
            sbom,
            provenance,
        };
        bundle.validate(limits)?;
        Ok(bundle)
    }

    /// Builds the only OCI publication input accepted for an Alloy-authored
    /// Rhai release. It re-parses and canonicalizes the upload, binds its
    /// descriptor and capability declaration, and derives the two signed
    /// evidence documents from owner-receipted metadata rather than caller
    /// supplied files.
    pub fn from_verified_rhai_workspace(
        descriptor: ModuleArtifactDescriptor,
        payload: Vec<u8>,
        license: &str,
        provenance: OciRhaiWorkspacePublicationProvenance,
        limits: ArtifactAdmissionLimits,
    ) -> Result<Self, OciArtifactPublicationError> {
        if descriptor.payload_kind != ArtifactPayloadKind::Rhai
            || descriptor.module_kind != ArtifactModuleKind::Optional
            || descriptor.runtime_abi != rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI
            || !descriptor
                .payload_kind
                .supports_media_type(rustok_sandbox::RHAI_WORKSPACE_MEDIA_TYPE)
            || license.trim().is_empty()
            || license.len() > 256
            || license.chars().any(char::is_control)
        {
            return Err(OciArtifactPublicationError::InvalidBundle(
                "Rhai workspace publication input has an invalid descriptor or license".to_string(),
            ));
        }
        let workspace: rustok_sandbox::RhaiWorkspace =
            serde_json::from_slice(&payload).map_err(|_| {
                OciArtifactPublicationError::InvalidBundle(
                    "Rhai workspace publication payload is not valid JSON".to_string(),
                )
            })?;
        let canonical_payload = workspace.canonical_bytes().map_err(|_| {
            OciArtifactPublicationError::InvalidBundle(
                "Rhai workspace publication payload is not canonical".to_string(),
            )
        })?;
        if canonical_payload != payload
            || workspace.digest().map_err(|_| {
                OciArtifactPublicationError::InvalidBundle(
                    "Rhai workspace publication payload digest is invalid".to_string(),
                )
            })? != descriptor.artifact_digest
            || workspace.entrypoint != descriptor.entrypoint
            || workspace
                .validate_declared_capabilities(&descriptor.capabilities)
                .is_err()
        {
            return Err(OciArtifactPublicationError::InvalidBundle(
                "Rhai workspace publication payload does not match its descriptor".to_string(),
            ));
        }
        provenance.validate_against(&descriptor)?;
        let sbom_bytes = canonical_alloy_workspace_sbom(&descriptor, license)?;
        let provenance_bytes = canonical_alloy_workspace_provenance(&descriptor, &provenance)?;
        let bundle = Self {
            descriptor,
            payload_media_type: rustok_sandbox::RHAI_WORKSPACE_MEDIA_TYPE.to_string(),
            payload,
            sbom: OciArtifactEvidence {
                digest: sha256_digest(&sbom_bytes),
                bytes: sbom_bytes,
            },
            provenance: OciArtifactEvidence {
                digest: sha256_digest(&provenance_bytes),
                bytes: provenance_bytes,
            },
        };
        bundle.validate(limits)?;
        Ok(bundle)
    }

    fn validate(&self, limits: ArtifactAdmissionLimits) -> Result<(), OciArtifactPublicationError> {
        self.descriptor
            .validate()
            .map_err(|error| OciArtifactPublicationError::InvalidBundle(error.to_string()))?;
        if self.payload_media_type.trim().is_empty()
            || self.payload_media_type.len() > 255
            || self.payload_media_type.chars().any(char::is_control)
            || !self
                .descriptor
                .payload_kind
                .supports_media_type(&self.payload_media_type)
        {
            return Err(OciArtifactPublicationError::InvalidBundle(
                "payload media type is not valid for the immutable descriptor".to_string(),
            ));
        }
        let descriptor_bytes = serde_json::to_vec(&self.descriptor)
            .map_err(|error| OciArtifactPublicationError::InvalidBundle(error.to_string()))?;
        limits
            .validate_descriptor_size(descriptor_bytes.len() as u64)
            .map_err(|error| OciArtifactPublicationError::InvalidBundle(error.to_string()))?;
        validate_publication_bytes(
            "payload",
            &self.payload,
            &self.descriptor.artifact_digest,
            limits.max_payload_bytes,
        )?;
        validate_publication_bytes(
            "SBOM",
            &self.sbom.bytes,
            &self.sbom.digest,
            limits.max_payload_bytes,
        )?;
        validate_publication_bytes(
            "provenance",
            &self.provenance.bytes,
            &self.provenance.digest,
            limits.max_payload_bytes,
        )?;
        validate_cyclonedx_predicate(&self.sbom.bytes)?;
        validate_slsa_statement(&self.provenance.bytes, &self.descriptor.artifact_digest)
    }
}

/// One bounded digest-verified blob used by the generic build-publication
/// primitive. Domain publishers own the media type and byte contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OciBuildPublicationBlob {
    pub media_type: String,
    pub digest: String,
    pub bytes: Vec<u8>,
}

/// Generic OCI artifact input for trusted build publishers that do not use the
/// sandbox-module descriptor contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OciBuildPublicationArtifact {
    pub config: OciBuildPublicationBlob,
    pub layer: OciBuildPublicationBlob,
}

/// Publication port for the descriptor-configured executable package. Evidence
/// is signed as Cosign attestations by the credential-owning publication
/// boundary after this port has returned the digest-pinned subject.
#[async_trait]
pub trait OciArtifactPublisher: Send + Sync {
    async fn publish(
        &self,
        target: OciArtifactPublicationTarget,
        bundle: OciArtifactPublicationBundle,
        limits: ArtifactAdmissionLimits,
    ) -> Result<OciArtifactReference, OciArtifactPublicationError>;
}

/// Terminal publication error for immutable module artifacts and evidence.
#[derive(Debug, Error)]
pub enum OciArtifactPublicationError {
    #[error("OCI publication target is invalid: {0}")]
    InvalidTarget(String),
    #[error("OCI publication input is invalid: {0}")]
    InvalidBundle(String),
    #[error("OCI publication failed: {0}")]
    Registry(String),
    #[error("OCI registry returned manifest digest `{received}`, expected `{expected}")]
    ManifestDigestMismatch { expected: String, received: String },
}

/// OCI Distribution publisher for immutable module packages. It uploads one
/// descriptor-configured executable layer. The shared signed-publication
/// boundary subsequently creates the SBOM and provenance attestations through
/// Cosign for this exact digest-pinned subject.
#[derive(Clone)]
pub struct OciDistributionArtifactPublisher {
    client: OciRegistryTransport,
    auth: RegistryAuth,
}

impl OciDistributionArtifactPublisher {
    /// Creates a publisher with the mandatory client-enforced registry policy.
    pub fn strict(auth: RegistryAuth) -> Result<Self, OciArtifactPublicationError> {
        Ok(Self {
            client: strict_oci_registry_transport()
                .map_err(OciArtifactPublicationError::Registry)?,
            auth,
        })
    }
}

#[async_trait]
impl OciArtifactPublisher for OciDistributionArtifactPublisher {
    async fn publish(
        &self,
        target: OciArtifactPublicationTarget,
        bundle: OciArtifactPublicationBundle,
        limits: ArtifactAdmissionLimits,
    ) -> Result<OciArtifactReference, OciArtifactPublicationError> {
        let publication = async {
            target.validate()?;
            bundle.validate(limits)?;
            let descriptor_bytes = serde_json::to_vec(&bundle.descriptor)
                .map_err(|error| OciArtifactPublicationError::InvalidBundle(error.to_string()))?;
            let primary_tag = derived_tag("artifact", &[&sha256_digest(&descriptor_bytes)]);
            let primary_write_reference = target.tag_reference(primary_tag)?;
            let descriptor_digest = sha256_digest(&descriptor_bytes);
            let layers = [Blob {
                media_type: &bundle.payload_media_type,
                digest: &bundle.descriptor.artifact_digest,
                bytes: &bundle.payload,
            }];
            self.client
                .push_artifact(
                    &primary_write_reference,
                    &self.auth,
                    Blob {
                        media_type: MODULE_ARTIFACT_DESCRIPTOR_MEDIA_TYPE,
                        digest: &descriptor_digest,
                        bytes: &descriptor_bytes,
                    },
                    &layers,
                    &bundle.payload_media_type,
                )
                .await
                .map_err(|error| OciArtifactPublicationError::Registry(error.to_string()))?;
            let artifact = self
                .resolve_published_reference(&target, &primary_write_reference)
                .await?;
            Ok(artifact)
        };
        tokio::time::timeout(OCI_REGISTRY_PUBLICATION_TIMEOUT, publication)
            .await
            .map_err(|_| {
                OciArtifactPublicationError::Registry(
                    "OCI artifact publication exceeded the 600 second deadline".to_string(),
                )
            })?
    }
}

impl OciDistributionArtifactPublisher {
    /// Resolves the standard Cosign OCI signature manifest after Cosign has
    /// signed a digest-pinned artifact. The standard tag is used only for the
    /// registry lookup; callers receive the resulting manifest digest.
    pub async fn resolve_cosign_signature(
        &self,
        target: &OciArtifactPublicationTarget,
        artifact: &OciArtifactReference,
    ) -> Result<OciArtifactReference, OciArtifactPublicationError> {
        target.validate()?;
        artifact
            .validate()
            .map_err(|error| OciArtifactPublicationError::InvalidBundle(error.to_string()))?;
        if artifact.registry != target.registry || artifact.repository != target.repository {
            return Err(OciArtifactPublicationError::InvalidBundle(
                "Cosign signature subject does not belong to the publication target".to_string(),
            ));
        }
        let signature_tag = cosign_signature_tag(&artifact.digest)?;
        let write_reference = target.tag_reference(signature_tag)?;
        self.resolve_published_reference(target, &write_reference)
            .await
    }

    /// Publishes one generic digest-verified build artifact using the current
    /// domain-owned config and layer media types. The derived write tag is an
    /// implementation detail; only the resolved manifest digest is returned.
    pub async fn publish_build_artifact(
        &self,
        target: &OciArtifactPublicationTarget,
        artifact: OciBuildPublicationArtifact,
        maximum_blob_bytes: u64,
    ) -> Result<OciArtifactReference, OciArtifactPublicationError> {
        target.validate()?;
        validate_build_blob("config", &artifact.config, maximum_blob_bytes)?;
        validate_build_blob("layer", &artifact.layer, maximum_blob_bytes)?;
        let write_reference = target.tag_reference(derived_current_tag(
            "build",
            &[
                &artifact.config.digest,
                &artifact.config.media_type,
                &artifact.layer.digest,
                &artifact.layer.media_type,
            ],
        ))?;
        let layers = [Blob {
            media_type: &artifact.layer.media_type,
            digest: &artifact.layer.digest,
            bytes: &artifact.layer.bytes,
        }];
        self.client
            .push_artifact(
                &write_reference,
                &self.auth,
                Blob {
                    media_type: &artifact.config.media_type,
                    digest: &artifact.config.digest,
                    bytes: &artifact.config.bytes,
                },
                &layers,
                &artifact.layer.media_type,
            )
            .await
            .map_err(|error| OciArtifactPublicationError::Registry(error.to_string()))?;
        self.resolve_published_reference(target, &write_reference)
            .await
    }

    /// Publishes one generic evidence referrer for an exact digest-pinned
    /// subject. Evidence type and bytes are validated by the domain publisher
    /// before this registry primitive is called and rechecked here.
    pub async fn publish_build_referrer(
        &self,
        target: &OciArtifactPublicationTarget,
        subject: &OciArtifactReference,
        evidence: OciBuildPublicationBlob,
        maximum_blob_bytes: u64,
    ) -> Result<OciArtifactReference, OciArtifactPublicationError> {
        target.validate()?;
        subject
            .validate()
            .map_err(|error| OciArtifactPublicationError::InvalidBundle(error.to_string()))?;
        if subject.registry != target.registry || subject.repository != target.repository {
            return Err(OciArtifactPublicationError::InvalidBundle(
                "build evidence subject does not belong to the publication target".to_string(),
            ));
        }
        validate_build_blob("evidence", &evidence, maximum_blob_bytes)?;
        let write_reference = target.tag_reference(derived_current_tag(
            "evidence",
            &[&subject.digest, &evidence.media_type, &evidence.digest],
        ))?;
        let empty_config_digest = sha256_digest(OCI_EMPTY_CONFIG_BYTES);
        self.client
            .push_blob(
                &write_reference,
                &self.auth,
                OCI_EMPTY_CONFIG_BYTES,
                &empty_config_digest,
            )
            .await
            .map_err(|error| OciArtifactPublicationError::Registry(error.to_string()))?;
        self.client
            .push_blob(
                &write_reference,
                &self.auth,
                &evidence.bytes,
                &evidence.digest,
            )
            .await
            .map_err(|error| OciArtifactPublicationError::Registry(error.to_string()))?;
        let manifest = OciReferrerManifest {
            schema_version: 2,
            media_type: OCI_IMAGE_MEDIA_TYPE.to_string(),
            artifact_type: evidence.media_type.clone(),
            config: OciDescriptor {
                media_type: OCI_EMPTY_CONFIG_MEDIA_TYPE.to_string(),
                digest: empty_config_digest,
                size: OCI_EMPTY_CONFIG_BYTES.len() as i64,
                urls: None,
                annotations: None,
            },
            layers: vec![OciDescriptor {
                media_type: evidence.media_type,
                digest: evidence.digest,
                size: evidence.bytes.len() as i64,
                urls: None,
                annotations: None,
            }],
            subject: OciDescriptor {
                media_type: OCI_IMAGE_MEDIA_TYPE.to_string(),
                digest: subject.digest.clone(),
                size: self.published_manifest_size(subject).await?,
                urls: None,
                annotations: None,
            },
        };
        let body = serde_json::to_vec(&manifest)
            .map_err(|error| OciArtifactPublicationError::InvalidBundle(error.to_string()))?;
        self.client
            .push_manifest(&write_reference, &self.auth, body, OCI_IMAGE_MEDIA_TYPE)
            .await
            .map_err(|error| OciArtifactPublicationError::Registry(error.to_string()))?;
        self.resolve_published_reference(target, &write_reference)
            .await
    }

    async fn published_manifest_size(
        &self,
        reference: &OciArtifactReference,
    ) -> Result<i64, OciArtifactPublicationError> {
        let image = RegistryReference::new(
            reference.registry.clone(),
            reference.repository.clone(),
            reference.digest.clone(),
        )
        .map_err(OciArtifactPublicationError::Registry)?;
        let (body, digest) = self
            .client
            .pull_manifest(&image, &self.auth)
            .await
            .map_err(|error| OciArtifactPublicationError::Registry(error.to_string()))?;
        let expected = sha256_digest(&body);
        if digest != expected || digest != reference.digest {
            return Err(OciArtifactPublicationError::ManifestDigestMismatch {
                expected: reference.digest.clone(),
                received: digest,
            });
        }
        i64::try_from(body.len()).map_err(|_| {
            OciArtifactPublicationError::InvalidBundle(
                "published OCI manifest exceeds signed descriptor size range".to_string(),
            )
        })
    }

    async fn resolve_published_reference(
        &self,
        target: &OciArtifactPublicationTarget,
        write_reference: &RegistryReference,
    ) -> Result<OciArtifactReference, OciArtifactPublicationError> {
        let (body, received) = self
            .client
            .pull_manifest(write_reference, &self.auth)
            .await
            .map_err(|error| OciArtifactPublicationError::Registry(error.to_string()))?;
        let expected = sha256_digest(&body);
        if received != expected {
            return Err(OciArtifactPublicationError::ManifestDigestMismatch { expected, received });
        }
        let reference = target.digest_reference(expected);
        reference
            .validate()
            .map_err(|error| OciArtifactPublicationError::Registry(error.to_string()))?;
        Ok(reference)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OciReferrerManifest {
    schema_version: u8,
    media_type: String,
    artifact_type: String,
    config: OciDescriptor,
    layers: Vec<OciDescriptor>,
    subject: OciDescriptor,
}

fn validate_publication_bytes(
    kind: &str,
    bytes: &[u8],
    expected_digest: &str,
    maximum_bytes: u64,
) -> Result<(), OciArtifactPublicationError> {
    if bytes.is_empty() || bytes.len() as u64 > maximum_bytes {
        return Err(OciArtifactPublicationError::InvalidBundle(format!(
            "{kind} bytes are empty or exceed the configured publication limit"
        )));
    }
    let actual_digest = sha256_digest(bytes);
    if actual_digest != expected_digest {
        return Err(OciArtifactPublicationError::InvalidBundle(format!(
            "{kind} digest mismatch: expected `{expected_digest}`, received `{actual_digest}`"
        )));
    }
    Ok(())
}

/// Validates the source-side CycloneDX document before the credential-owning
/// Cosign boundary uses it as an attestation predicate. Detailed license and
/// vulnerability policy checks remain the isolated verifier's responsibility.
fn validate_cyclonedx_predicate(bytes: &[u8]) -> Result<(), OciArtifactPublicationError> {
    let document: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| {
        OciArtifactPublicationError::InvalidBundle(
            "SBOM evidence is not a CycloneDX JSON document".to_string(),
        )
    })?;
    if document
        .get("bomFormat")
        .and_then(serde_json::Value::as_str)
        != Some("CycloneDX")
        || document
            .get("specVersion")
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty)
        || document
            .pointer("/metadata/component")
            .and_then(serde_json::Value::as_object)
            .is_none()
    {
        return Err(OciArtifactPublicationError::InvalidBundle(
            "SBOM evidence is not a bounded CycloneDX predicate".to_string(),
        ));
    }
    Ok(())
}

/// The pre-publication SLSA document records the source builder's payload
/// subject. Cosign later signs only its predicate and creates a new envelope
/// whose subject is the immutable OCI manifest. Requiring this original
/// payload binding here prevents a build from smuggling unrelated provenance
/// into that final manifest-bound attestation.
fn validate_slsa_statement(
    bytes: &[u8],
    expected_payload_digest: &str,
) -> Result<(), OciArtifactPublicationError> {
    let document: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| {
        OciArtifactPublicationError::InvalidBundle(
            "provenance evidence is not an in-toto SLSA statement".to_string(),
        )
    })?;
    let expected_digest = expected_payload_digest
        .strip_prefix("sha256:")
        .ok_or_else(|| {
            OciArtifactPublicationError::InvalidBundle(
                "descriptor payload digest is not sha256".to_string(),
            )
        })?;
    let subject_matches = document
        .get("subject")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|subjects| {
            subjects.iter().any(|subject| {
                subject
                    .pointer("/digest/sha256")
                    .and_then(serde_json::Value::as_str)
                    == Some(expected_digest)
            })
        });
    if document.get("_type").and_then(serde_json::Value::as_str)
        != Some("https://in-toto.io/Statement/v1")
        || document
            .get("predicateType")
            .and_then(serde_json::Value::as_str)
            != Some("https://slsa.dev/provenance/v1")
        || document
            .get("predicate")
            .filter(|value| value.is_object())
            .is_none()
        || !subject_matches
    {
        return Err(OciArtifactPublicationError::InvalidBundle(
            "provenance evidence is not a payload-bound SLSA v1 statement".to_string(),
        ));
    }
    Ok(())
}

fn canonical_alloy_workspace_sbom(
    descriptor: &ModuleArtifactDescriptor,
    license: &str,
) -> Result<Vec<u8>, OciArtifactPublicationError> {
    serde_json::to_vec(&serde_json::json!({
        "bomFormat": "CycloneDX",
        "specVersion": "1.6",
        "version": 1,
        "metadata": {
            "component": {
                "type": "file",
                "name": descriptor.slug.as_str(),
                "version": descriptor.version.as_str(),
                "hashes": [{
                    "alg": "SHA-256",
                    "content": descriptor.artifact_digest.strip_prefix("sha256:").expect("validated descriptor digest")
                }],
                "licenses": [{ "license": { "id": license } }],
                "properties": [{
                    "name": "rustok:payload-media-type",
                    "value": rustok_sandbox::RHAI_WORKSPACE_MEDIA_TYPE
                }]
            }
        }
    }))
    .map_err(|error| OciArtifactPublicationError::InvalidBundle(error.to_string()))
}

fn canonical_alloy_workspace_provenance(
    descriptor: &ModuleArtifactDescriptor,
    provenance: &OciRhaiWorkspacePublicationProvenance,
) -> Result<Vec<u8>, OciArtifactPublicationError> {
    serde_json::to_vec(&serde_json::json!({
        "_type": "https://in-toto.io/Statement/v1",
        "subject": [{
            "name": format!("{}@{}", descriptor.slug, descriptor.version),
            "digest": {
                "sha256": descriptor.artifact_digest.strip_prefix("sha256:").expect("validated descriptor digest")
            }
        }],
        "predicateType": "https://slsa.dev/provenance/v1",
        "predicate": {
            "buildDefinition": {
                "buildType": ALLOY_WORKSPACE_PUBLICATION_BUILD_TYPE,
                "externalParameters": {
                    "source": {
                        "uri": ALLOY_WORKSPACE_PUBLICATION_SOURCE_URI,
                        "ref": ALLOY_WORKSPACE_PUBLICATION_SOURCE_REF
                    },
                    "rustok": {
                        "artifactOrigin": "alloy_authored",
                        "requestId": provenance.request_id.as_str(),
                        "alloyTenantId": provenance.alloy_tenant_id,
                        "alloyScriptId": provenance.alloy_script_id,
                        "sourceRevision": provenance.source_revision,
                        "sourceDigest": provenance.source_digest.as_str(),
                        "reviewDigest": provenance.review_digest.as_str(),
                        "descriptorDigest": provenance.descriptor_digest.as_str(),
                        "workspaceEntrypoint": descriptor.entrypoint.as_str()
                    }
                },
                "internalParameters": {},
                "resolvedDependencies": []
            },
            "runDetails": {
                "builder": { "id": ALLOY_WORKSPACE_PUBLICATION_BUILDER_ID },
                "metadata": {
                    "invocationId": provenance.descriptor_digest.as_str()
                }
            }
        }
    }))
    .map_err(|error| OciArtifactPublicationError::InvalidBundle(error.to_string()))
}

fn is_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn validate_build_blob(
    kind: &str,
    blob: &OciBuildPublicationBlob,
    maximum_bytes: u64,
) -> Result<(), OciArtifactPublicationError> {
    if maximum_bytes == 0
        || blob.media_type.is_empty()
        || blob.media_type.len() > 255
        || !blob.media_type.contains('/')
        || blob.media_type.chars().any(char::is_control)
    {
        return Err(OciArtifactPublicationError::InvalidBundle(format!(
            "{kind} media type or publication limit is invalid"
        )));
    }
    validate_publication_bytes(kind, &blob.bytes, &blob.digest, maximum_bytes)
}

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn derived_tag(kind: &str, fields: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"rustok.oci.publication.v1");
    hasher.update(kind.as_bytes());
    for field in fields {
        hasher.update([0]);
        hasher.update(field.as_bytes());
    }
    format!("rustok-{kind}-{}", hex::encode(hasher.finalize()))
}

fn derived_current_tag(kind: &str, fields: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"rustok.oci.publication");
    hasher.update(kind.as_bytes());
    for field in fields {
        hasher.update([0]);
        hasher.update(field.as_bytes());
    }
    format!("rustok-{kind}-{}", hex::encode(hasher.finalize()))
}

fn cosign_signature_tag(digest: &str) -> Result<String, OciArtifactPublicationError> {
    Ok(format!("sha256-{}.sig", cosign_subject_digest_hex(digest)?))
}

fn cosign_subject_digest_hex(digest: &str) -> Result<&str, OciArtifactPublicationError> {
    let hex = digest.strip_prefix("sha256:").ok_or_else(|| {
        OciArtifactPublicationError::InvalidBundle(
            "Cosign subject must use a sha256 digest".to_string(),
        )
    })?;
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(OciArtifactPublicationError::InvalidBundle(
            "Cosign subject must use a valid sha256 digest".to_string(),
        ));
    }
    Ok(hex)
}

/// Resolves a module artifact from an OCI Distribution registry.
///
/// The OCI manifest config is the canonical `ModuleArtifactDescriptor` JSON.
/// Exactly one layer must match both its digest and payload media type. The
/// registry client verifies registry transport semantics; `ModuleArtifactPackage`
/// verifies descriptor identity and the downloaded payload bytes.
#[derive(Clone)]
pub struct OciDistributionArtifactRegistry {
    client: OciRegistryTransport,
    auth: RegistryAuth,
    infrastructure: ControlPlaneInfrastructure,
}

impl OciDistributionArtifactRegistry {
    /// Creates an authenticated registry adapter with the mandatory
    /// client-enforced registry policy.
    pub fn strict(auth: RegistryAuth) -> Result<Self, ModuleInstallationError> {
        Self::strict_with_infrastructure(auth, ControlPlaneInfrastructure::default())
    }

    pub fn strict_with_infrastructure(
        auth: RegistryAuth,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Result<Self, ModuleInstallationError> {
        Ok(Self {
            client: strict_oci_registry_transport().map_err(ModuleInstallationError::Registry)?,
            auth,
            infrastructure,
        })
    }

    /// Creates an anonymous registry adapter with the strict transport subset
    /// used by production artifact distribution.
    pub fn strict_anonymous() -> Result<Self, ModuleInstallationError> {
        Self::strict(RegistryAuth::Anonymous)
    }

    pub fn strict_anonymous_with_infrastructure(
        infrastructure: ControlPlaneInfrastructure,
    ) -> Result<Self, ModuleInstallationError> {
        Self::strict_with_infrastructure(RegistryAuth::Anonymous, infrastructure)
    }

    fn image_reference(
        reference: &OciArtifactReference,
    ) -> Result<RegistryReference, ModuleInstallationError> {
        reference.validate()?;
        RegistryReference::new(
            reference.registry.clone(),
            reference.repository.clone(),
            reference.digest.clone(),
        )
        .map_err(ModuleInstallationError::Registry)
    }
}

#[async_trait]
impl ArtifactRegistry for OciDistributionArtifactRegistry {
    async fn fetch(
        &self,
        reference: &OciArtifactReference,
        limits: ArtifactAdmissionLimits,
    ) -> Result<ModuleArtifactPackage, ModuleInstallationError> {
        let admission = async {
            let image = Self::image_reference(reference)?;
            let (manifest, manifest_digest) = self
                .client
                .pull_image_manifest(&image, &self.auth)
                .await
                .map_err(|error| ModuleInstallationError::Registry(error.to_string()))?;
            if manifest_digest != reference.digest {
                return Err(ModuleInstallationError::RegistryIdentityMismatch {
                    requested: reference.canonical(),
                    received: format!(
                        "{}/{}@{manifest_digest}",
                        reference.registry, reference.repository
                    ),
                });
            }
            if manifest.config.media_type != MODULE_ARTIFACT_DESCRIPTOR_MEDIA_TYPE {
                return Err(ModuleInstallationError::Registry(format!(
                    "OCI artifact config media type must be `{MODULE_ARTIFACT_DESCRIPTOR_MEDIA_TYPE}`, received `{}`",
                    manifest.config.media_type
                )));
            }
            let config = self
                .pull_config_to_memory(&image, &manifest.config, limits)
                .await?;
            let _config_size = config.len() as u64;
            let config_digest = format!("sha256:{}", hex::encode(Sha256::digest(&config)));
            if manifest.config.digest != config_digest {
                return Err(ModuleInstallationError::Registry(format!(
                    "OCI artifact config digest mismatch: manifest declares `{}`, received `{config_digest}`",
                    manifest.config.digest
                )));
            }
            let descriptor: ModuleArtifactDescriptor =
                serde_json::from_slice(&config).map_err(|error| {
                    ModuleInstallationError::Registry(format!(
                        "OCI artifact config is not a module descriptor: {error}"
                    ))
                })?;
            let layers = manifest
                .layers
                .iter()
                .filter(|layer| {
                    descriptor_payload_layer_matches(&descriptor, &layer.digest, &layer.media_type)
                })
                .collect::<Vec<_>>();
            let [layer] = layers.as_slice() else {
                return Err(ModuleInstallationError::Registry(format!(
                    "OCI artifact must contain exactly one descriptor-valid payload layer with digest `{}`",
                    descriptor.artifact_digest
                )));
            };
            let layer_size = u64::try_from(layer.size).map_err(|_| {
                ModuleInstallationError::Registry("OCI layer declares a negative size".to_string())
            })?;
            limits.validate_payload_size(layer_size)?;
            let payload = self
                .pull_payload_to_temporary_storage(
                    &image,
                    layer,
                    &descriptor.artifact_digest,
                    limits,
                )
                .await?;
            let package = ModuleArtifactPackage {
                reference: reference.clone(),
                media_type: layer.media_type.clone(),
                descriptor,
                payload: ArtifactPayloadSource::TemporaryFile(payload),
            };
            package.verify(limits).await?;
            Ok(package)
        };
        tokio::time::timeout(OCI_REGISTRY_ADMISSION_TIMEOUT, admission)
            .await
            .map_err(|_| {
                ModuleInstallationError::Registry(
                    "OCI artifact admission exceeded the 300 second deadline".to_string(),
                )
            })?
    }
}

/// An OCI manifest retains the exact payload media type, while a Rhai
/// descriptor deliberately permits both its single-source and canonical
/// workspace representations. Admission accepts only one layer that matches
/// the descriptor payload digest and one of that kind's approved media types;
/// the selected type is preserved in the resulting package for runtime
/// resolution.
fn descriptor_payload_layer_matches(
    descriptor: &ModuleArtifactDescriptor,
    layer_digest: &str,
    layer_media_type: &str,
) -> bool {
    layer_digest == descriptor.artifact_digest
        && descriptor
            .payload_kind
            .supports_media_type(layer_media_type)
}

impl OciDistributionArtifactRegistry {
    /// Streams the descriptor config only after its declared size passes the
    /// admission bound. The upstream client otherwise buffers this blob before
    /// callers can validate it.
    async fn pull_config_to_memory(
        &self,
        image: &RegistryReference,
        config: &oci_distribution::manifest::OciDescriptor,
        limits: ArtifactAdmissionLimits,
    ) -> Result<Vec<u8>, ModuleInstallationError> {
        let declared_size = u64::try_from(config.size).map_err(|_| {
            ModuleInstallationError::Registry(
                "OCI artifact config declares a negative size".to_string(),
            )
        })?;
        limits.validate_descriptor_size(declared_size)?;
        let capacity = usize::try_from(declared_size).map_err(|_| {
            ModuleInstallationError::Registry(
                "OCI artifact config size cannot be represented by this platform".to_string(),
            )
        })?;
        let stream = self
            .client
            .pull_blob_stream(image, &config.digest, &self.auth)
            .await
            .map_err(|error| ModuleInstallationError::Registry(error.to_string()))?;
        futures_util::pin_mut!(stream);
        let mut bytes = Vec::with_capacity(capacity);
        let mut received = 0_u64;
        while let Some(chunk) = stream.next().await {
            let chunk =
                chunk.map_err(|error| ModuleInstallationError::Registry(error.to_string()))?;
            received = received.checked_add(chunk.len() as u64).ok_or({
                ModuleInstallationError::ArtifactTooLarge {
                    kind: "descriptor",
                    limit: limits.max_descriptor_bytes,
                    actual: u64::MAX,
                }
            })?;
            if received > declared_size {
                return Err(ModuleInstallationError::Registry(format!(
                    "OCI artifact config size mismatch: manifest declares `{declared_size}`, received more bytes"
                )));
            }
            limits.validate_descriptor_size(received)?;
            bytes.extend_from_slice(&chunk);
        }
        if received != declared_size {
            return Err(ModuleInstallationError::Registry(format!(
                "OCI artifact config size mismatch: manifest declares `{declared_size}`, received `{received}`"
            )));
        }
        Ok(bytes)
    }

    /// Streams a registry layer through a bounded private staging file. The
    /// current object-storage port still accepts a bounded buffer after this
    /// check; this method deliberately avoids an unbounded network `Vec<u8>`.
    async fn pull_payload_to_temporary_storage(
        &self,
        image: &RegistryReference,
        layer: &oci_distribution::manifest::OciDescriptor,
        expected_digest: &str,
        limits: ArtifactAdmissionLimits,
    ) -> Result<std::path::PathBuf, ModuleInstallationError> {
        let declared_size = u64::try_from(layer.size).map_err(|_| {
            ModuleInstallationError::Registry("OCI layer declares a negative size".to_string())
        })?;
        limits.validate_payload_size(declared_size)?;
        let staging_file = ArtifactStagingFile::new(self.infrastructure.new_id());
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(staging_file.path())
            .await
            .map_err(|error| ModuleInstallationError::Registry(error.to_string()))?;
        let stream = self
            .client
            .pull_blob_stream(image, &layer.digest, &self.auth)
            .await
            .map_err(|error| ModuleInstallationError::Registry(error.to_string()))?;
        futures_util::pin_mut!(stream);
        let mut received = 0_u64;
        let mut hasher = Sha256::new();
        while let Some(chunk) = stream.next().await {
            let chunk =
                chunk.map_err(|error| ModuleInstallationError::Registry(error.to_string()))?;
            received = received.checked_add(chunk.len() as u64).ok_or({
                ModuleInstallationError::ArtifactTooLarge {
                    kind: "payload",
                    limit: limits.max_payload_bytes,
                    actual: u64::MAX,
                }
            })?;
            if received > declared_size {
                return Err(ModuleInstallationError::Registry(format!(
                    "OCI layer size mismatch: manifest declares `{declared_size}`, received more bytes"
                )));
            }
            limits.validate_payload_size(received)?;
            hasher.update(&chunk);
            file.write_all(&chunk)
                .await
                .map_err(|error| ModuleInstallationError::Registry(error.to_string()))?;
        }
        file.flush()
            .await
            .map_err(|error| ModuleInstallationError::Registry(error.to_string()))?;
        if received != declared_size {
            return Err(ModuleInstallationError::Registry(format!(
                "OCI layer size mismatch: manifest declares `{declared_size}`, received `{received}`"
            )));
        }
        let actual_digest = format!("sha256:{}", hex::encode(hasher.finalize()));
        if actual_digest != expected_digest {
            return Err(ModuleInstallationError::PayloadDigestMismatch {
                expected: expected_digest.to_string(),
                actual: actual_digest,
            });
        }
        Ok(staging_file.persist())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ArtifactAdmissionLimits, ArtifactModuleKind, ArtifactPayloadKind,
        MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION, MODULE_ARTIFACT_WASM_COMPONENT_MEDIA_TYPE,
        ModuleArtifactDescriptor, OciArtifactReference,
    };

    use uuid::Uuid;

    use super::{
        ALLOY_WORKSPACE_PUBLICATION_SOURCE_URI, ArtifactStagingFile, OciArtifactPublicationBundle,
        OciDistributionArtifactRegistry, OciRegistryTransportPolicy,
        OciRhaiWorkspacePublicationProvenance, canonical_alloy_workspace_provenance,
        cosign_signature_tag, descriptor_payload_layer_matches,
    };

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

    fn rhai_provenance(
        descriptor: &ModuleArtifactDescriptor,
    ) -> OciRhaiWorkspacePublicationProvenance {
        OciRhaiWorkspacePublicationProvenance {
            request_id: "request-1".to_string(),
            alloy_tenant_id: Uuid::new_v4(),
            alloy_script_id: Uuid::new_v4(),
            source_revision: 7,
            source_digest: descriptor.artifact_digest.clone(),
            review_digest: format!("sha256:{}", "b".repeat(64)),
            descriptor_digest: crate::canonical_artifact_descriptor_digest(descriptor),
        }
    }

    #[test]
    fn staging_file_is_deleted_when_a_download_is_cancelled_or_fails() {
        let staging_file = ArtifactStagingFile::new(Uuid::new_v4());
        let path = staging_file.path().to_path_buf();
        std::fs::write(&path, b"partial artifact").expect("stage partial artifact");

        drop(staging_file);

        assert!(!path.exists());
    }

    #[test]
    fn persisted_staging_file_is_retained_for_admission_consumption() {
        let staging_file = ArtifactStagingFile::new(Uuid::new_v4());
        let path = staging_file.path().to_path_buf();
        std::fs::write(&path, b"verified artifact").expect("stage verified artifact");
        let persisted = staging_file.persist();

        assert_eq!(persisted, path);
        assert!(persisted.exists());
        std::fs::remove_file(persisted).expect("remove persisted test artifact");
    }

    #[test]
    fn parser_uses_a_digest_pinned_reference_without_a_tag() {
        let reference = OciArtifactReference {
            registry: "registry.example".to_string(),
            repository: "modules/sample_module".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
        };

        let image = OciDistributionArtifactRegistry::image_reference(&reference)
            .expect("digest-pinned reference");

        assert_eq!(image.canonical(), reference.canonical());
    }

    #[test]
    fn payload_media_type_is_frozen_by_contract() {
        assert_eq!(
            ArtifactPayloadKind::WasmComponent.oci_layer_media_type(),
            MODULE_ARTIFACT_WASM_COMPONENT_MEDIA_TYPE
        );
    }

    #[test]
    fn descriptor_payload_selection_retains_the_canonical_rhai_workspace_media_type() {
        let workspace = rustok_sandbox::RhaiWorkspace::single_source("40 + 2");
        let descriptor = rhai_descriptor(&workspace);

        assert!(descriptor_payload_layer_matches(
            &descriptor,
            &descriptor.artifact_digest,
            rustok_sandbox::RHAI_WORKSPACE_MEDIA_TYPE,
        ));
        assert!(!descriptor_payload_layer_matches(
            &descriptor,
            &format!("sha256:{}", "f".repeat(64)),
            rustok_sandbox::RHAI_WORKSPACE_MEDIA_TYPE,
        ));
        assert!(!descriptor_payload_layer_matches(
            &descriptor,
            &descriptor.artifact_digest,
            MODULE_ARTIFACT_WASM_COMPONENT_MEDIA_TYPE,
        ));
    }

    #[test]
    fn cosign_signature_tag_is_derived_only_from_a_sha256_subject() {
        assert_eq!(
            cosign_signature_tag(&format!("sha256:{}", "a".repeat(64))).expect("sha256 subject"),
            format!("sha256-{}.sig", "a".repeat(64))
        );
        assert!(cosign_signature_tag("sha512:abc").is_err());
        assert!(cosign_signature_tag(&format!("sha256:{}", "A".repeat(64))).is_err());
    }

    #[test]
    fn strict_registry_transport_policy_rejects_weaker_egress_controls() {
        let mut policy = OciRegistryTransportPolicy::strict();
        assert!(policy.validate().is_ok());

        policy.allow_redirects = true;
        assert!(policy.validate().is_err());

        policy = OciRegistryTransportPolicy::strict();
        policy.max_decompressed_bytes = policy.max_transfer_bytes + 1;
        assert!(policy.validate().is_err());
    }

    #[test]
    fn rhai_workspace_publication_is_canonical_receipt_bound_and_workspace_typed() {
        let workspace = rustok_sandbox::RhaiWorkspace::single_source("40 + 2");
        let descriptor = rhai_descriptor(&workspace);
        let provenance = rhai_provenance(&descriptor);
        let payload = workspace.canonical_bytes().expect("canonical workspace");
        let bundle = OciArtifactPublicationBundle::from_verified_rhai_workspace(
            descriptor.clone(),
            payload.clone(),
            "MIT",
            provenance.clone(),
            ArtifactAdmissionLimits::default(),
        )
        .expect("receipt-bound Rhai OCI bundle");

        assert_eq!(
            bundle.payload_media_type,
            rustok_sandbox::RHAI_WORKSPACE_MEDIA_TYPE
        );
        assert_eq!(bundle.payload, payload);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bundle.sbom.bytes)
                .expect("CycloneDX JSON")["metadata"]["component"]["licenses"][0]["license"]["id"],
            "MIT"
        );
        let statement: serde_json::Value = serde_json::from_slice(
            &canonical_alloy_workspace_provenance(&descriptor, &provenance)
                .expect("provenance JSON"),
        )
        .expect("SLSA JSON");
        assert_eq!(
            statement["predicate"]["buildDefinition"]["externalParameters"]["source"]["uri"],
            ALLOY_WORKSPACE_PUBLICATION_SOURCE_URI
        );
        assert_eq!(
            statement["predicate"]["buildDefinition"]["externalParameters"]["rustok"]["descriptorDigest"],
            provenance.descriptor_digest
        );

        let mut non_canonical = payload;
        non_canonical.push(b' ');
        assert!(
            OciArtifactPublicationBundle::from_verified_rhai_workspace(
                descriptor,
                non_canonical,
                "MIT",
                provenance,
                ArtifactAdmissionLimits::default(),
            )
            .is_err()
        );
    }
}
