//! OCI Distribution adapter for immutable module artifacts.

use async_trait::async_trait;
use futures_util::StreamExt;
use http::HeaderValue;
use oci_distribution::{
    Client, Reference,
    client::{ClientConfig, ClientProtocol, Config, ImageLayer},
    manifest::{OCI_IMAGE_MEDIA_TYPE, OciDescriptor, OciImageManifest},
    secrets::RegistryAuth,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::{
    ArtifactAdmissionLimits, ArtifactPayloadSource, ArtifactRegistry, ControlPlaneInfrastructure,
    ModuleArtifactDescriptor, ModuleArtifactPackage, ModuleBuildOutcome, ModuleBuildRequest,
    ModuleBuildResult, ModuleInstallationError, OciArtifactReference,
};

/// Stable OCI config media type for a serialized immutable module descriptor.
pub const MODULE_ARTIFACT_DESCRIPTOR_MEDIA_TYPE: &str =
    "application/vnd.rustok.module.descriptor.v1+json";
/// Stable OCI referrer media type for CycloneDX JSON evidence.
pub const MODULE_ARTIFACT_SBOM_MEDIA_TYPE: &str = "application/vnd.cyclonedx+json";
/// Stable OCI referrer media type for in-toto provenance evidence.
pub const MODULE_ARTIFACT_PROVENANCE_MEDIA_TYPE: &str = "application/vnd.in-toto+json";
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

/// Proxy handling mode for the registry egress boundary. The OCI client does
/// not expose proxy hooks, so production deployments must enforce this mode
/// at the dedicated egress boundary rather than inheriting process settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OciRegistryProxyMode {
    Disabled,
    DeploymentBoundaryOnly,
}

/// Explicit registry transport and egress policy. Client-enforced fields are
/// applied by `strict_oci_distribution_client_with_policy`; fields without
/// upstream client hooks are deployment obligations and remain validated here
/// so a weaker policy cannot be constructed accidentally.
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

/// Constructs the strict subset of registry transport policy that the current
/// OCI Distribution client can enforce itself. Redirect/proxy enforcement is
/// intentionally left to the deployment egress boundary until the client is
/// replaced by a transport with explicit hooks for those policies.
pub fn strict_oci_distribution_client() -> Result<Client, String> {
    strict_oci_distribution_client_with_policy(OciRegistryTransportPolicy::strict())
}

/// Constructs the OCI client after validating the complete transport policy.
/// The upstream client currently exposes only a subset of these controls;
/// timeout, redirect, proxy, retry, and decompression enforcement remains at
/// the deployment-owned egress boundary.
pub fn strict_oci_distribution_client_with_policy(
    policy: OciRegistryTransportPolicy,
) -> Result<Client, String> {
    policy.validate()?;
    let mut config = ClientConfig::default();
    config.protocol = ClientProtocol::Https;
    config.accept_invalid_certificates = !policy.verify_tls;
    config.platform_resolver = None;
    config.max_concurrent_upload = policy.max_concurrent_requests;
    config.max_concurrent_download = policy.max_concurrent_requests;
    Client::try_from(config).map_err(|error| error.to_string())
}

/// Referrer evidence classes admitted by the publication and trust pipelines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OciArtifactEvidenceKind {
    Sbom,
    Provenance,
    TestEvidence,
    ReleaseLineage,
}

impl OciArtifactEvidenceKind {
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Sbom => MODULE_ARTIFACT_SBOM_MEDIA_TYPE,
            Self::Provenance => MODULE_ARTIFACT_PROVENANCE_MEDIA_TYPE,
            Self::TestEvidence => MODULE_ARTIFACT_TEST_EVIDENCE_MEDIA_TYPE,
            Self::ReleaseLineage => MODULE_ARTIFACT_RELEASE_LINEAGE_MEDIA_TYPE,
        }
    }
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

    fn tag_reference(&self, tag: String) -> Reference {
        Reference::with_tag(self.registry.clone(), self.repository.clone(), tag)
    }

    fn digest_reference(&self, digest: String) -> OciArtifactReference {
        OciArtifactReference {
            registry: self.registry.clone(),
            repository: self.repository.clone(),
            digest,
        }
    }
}

/// Verified evidence bytes for a fixed OCI referrer class.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OciArtifactEvidence {
    pub digest: String,
    pub bytes: Vec<u8>,
}

/// Complete, immutable publication input. A build worker or publication host
/// must construct this only from fixed verified output paths.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OciArtifactPublicationBundle {
    pub descriptor: ModuleArtifactDescriptor,
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
            descriptor,
            payload,
            sbom,
            provenance,
        };
        bundle.validate(limits)?;
        Ok(bundle)
    }

    fn validate(&self, limits: ArtifactAdmissionLimits) -> Result<(), OciArtifactPublicationError> {
        self.descriptor
            .validate()
            .map_err(|error| OciArtifactPublicationError::InvalidBundle(error.to_string()))?;
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
        )
    }
}

/// Digest-pinned OCI identities emitted after publishing the artifact and its
/// two required OCI 1.1 evidence referrers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OciArtifactPublicationReceipt {
    pub artifact: OciArtifactReference,
    pub sbom_referrer: OciArtifactReference,
    pub provenance_referrer: OciArtifactReference,
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

/// Publication port. Implementations must never return write tags as artifact
/// identity; consumers resolve only the receipt's digest references.
#[async_trait]
pub trait OciArtifactPublisher: Send + Sync {
    async fn publish(
        &self,
        target: OciArtifactPublicationTarget,
        bundle: OciArtifactPublicationBundle,
        limits: ArtifactAdmissionLimits,
    ) -> Result<OciArtifactPublicationReceipt, OciArtifactPublicationError>;
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
/// descriptor-configured executable layer, then OCI 1.1 SBOM and provenance
/// referrers with an exact subject descriptor.
#[derive(Clone)]
pub struct OciDistributionArtifactPublisher {
    client: Client,
    auth: RegistryAuth,
}

impl OciDistributionArtifactPublisher {
    /// Creates a publisher that uses the mandatory strict registry transport
    /// subset. Callers cannot supply a client with weaker TLS settings.
    pub fn strict(auth: RegistryAuth) -> Result<Self, OciArtifactPublicationError> {
        Ok(Self {
            client: strict_oci_distribution_client()
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
    ) -> Result<OciArtifactPublicationReceipt, OciArtifactPublicationError> {
        let publication = async {
            target.validate()?;
            bundle.validate(limits)?;
            let descriptor_bytes = serde_json::to_vec(&bundle.descriptor)
                .map_err(|error| OciArtifactPublicationError::InvalidBundle(error.to_string()))?;
            let primary_tag = derived_tag("artifact", &[&sha256_digest(&descriptor_bytes)]);
            let primary_write_reference = target.tag_reference(primary_tag);
            let layer = ImageLayer::new(
                bundle.payload,
                bundle
                    .descriptor
                    .payload_kind
                    .oci_layer_media_type()
                    .to_string(),
                None,
            );
            let config = Config::new(
                descriptor_bytes,
                MODULE_ARTIFACT_DESCRIPTOR_MEDIA_TYPE.to_string(),
                None,
            );
            let mut manifest = OciImageManifest::build(&[layer.clone()], &config, None);
            manifest.media_type = Some(OCI_IMAGE_MEDIA_TYPE.to_string());
            manifest.artifact_type = Some(layer.media_type.clone());
            self.client
                .push(
                    &primary_write_reference,
                    &[layer],
                    config,
                    &self.auth,
                    Some(manifest),
                )
                .await
                .map_err(|error| OciArtifactPublicationError::Registry(error.to_string()))?;
            let artifact = self
                .resolve_published_reference(&target, &primary_write_reference)
                .await?;
            let sbom_referrer = self
                .publish_referrer(
                    &target,
                    &artifact,
                    OciArtifactEvidenceKind::Sbom,
                    bundle.sbom,
                )
                .await?;
            let provenance_referrer = self
                .publish_referrer(
                    &target,
                    &artifact,
                    OciArtifactEvidenceKind::Provenance,
                    bundle.provenance,
                )
                .await?;
            Ok(OciArtifactPublicationReceipt {
                artifact,
                sbom_referrer,
                provenance_referrer,
            })
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
        let write_reference = target.tag_reference(signature_tag);
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
        ));
        let layer = ImageLayer::new(
            artifact.layer.bytes,
            artifact.layer.media_type.clone(),
            None,
        );
        let config = Config::new(artifact.config.bytes, artifact.config.media_type, None);
        let mut manifest = OciImageManifest::build(&[layer.clone()], &config, None);
        manifest.media_type = Some(OCI_IMAGE_MEDIA_TYPE.to_string());
        manifest.artifact_type = Some(artifact.layer.media_type);
        self.client
            .push(
                &write_reference,
                &[layer],
                config,
                &self.auth,
                Some(manifest),
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
        ));
        let empty_config_digest = sha256_digest(OCI_EMPTY_CONFIG_BYTES);
        self.client
            .push_blob(
                &write_reference,
                OCI_EMPTY_CONFIG_BYTES,
                &empty_config_digest,
            )
            .await
            .map_err(|error| OciArtifactPublicationError::Registry(error.to_string()))?;
        self.client
            .push_blob(&write_reference, &evidence.bytes, &evidence.digest)
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
            .push_manifest_raw(
                &write_reference,
                body,
                HeaderValue::from_static(OCI_IMAGE_MEDIA_TYPE),
            )
            .await
            .map_err(|error| OciArtifactPublicationError::Registry(error.to_string()))?;
        self.resolve_published_reference(target, &write_reference)
            .await
    }

    async fn publish_referrer(
        &self,
        target: &OciArtifactPublicationTarget,
        subject: &OciArtifactReference,
        kind: OciArtifactEvidenceKind,
        evidence: OciArtifactEvidence,
    ) -> Result<OciArtifactReference, OciArtifactPublicationError> {
        let write_reference = target.tag_reference(derived_tag(
            "referrer",
            &[&subject.digest, kind.media_type(), &evidence.digest],
        ));
        let empty_config_digest = sha256_digest(OCI_EMPTY_CONFIG_BYTES);
        self.client
            .push_blob(
                &write_reference,
                OCI_EMPTY_CONFIG_BYTES,
                &empty_config_digest,
            )
            .await
            .map_err(|error| OciArtifactPublicationError::Registry(error.to_string()))?;
        self.client
            .push_blob(&write_reference, &evidence.bytes, &evidence.digest)
            .await
            .map_err(|error| OciArtifactPublicationError::Registry(error.to_string()))?;
        let manifest = OciReferrerManifest {
            schema_version: 2,
            media_type: OCI_IMAGE_MEDIA_TYPE.to_string(),
            artifact_type: kind.media_type().to_string(),
            config: OciDescriptor {
                media_type: OCI_EMPTY_CONFIG_MEDIA_TYPE.to_string(),
                digest: empty_config_digest,
                size: OCI_EMPTY_CONFIG_BYTES.len() as i64,
                urls: None,
                annotations: None,
            },
            layers: vec![OciDescriptor {
                media_type: kind.media_type().to_string(),
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
            .push_manifest_raw(
                &write_reference,
                body,
                HeaderValue::from_static(OCI_IMAGE_MEDIA_TYPE),
            )
            .await
            .map_err(|error| OciArtifactPublicationError::Registry(error.to_string()))?;
        self.resolve_published_reference(target, &write_reference)
            .await
    }

    async fn published_manifest_size(
        &self,
        reference: &OciArtifactReference,
    ) -> Result<i64, OciArtifactPublicationError> {
        let image = Reference::with_digest(
            reference.registry.clone(),
            reference.repository.clone(),
            reference.digest.clone(),
        );
        let (body, digest) = self
            .client
            .pull_manifest_raw(&image, &self.auth, &[])
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
        write_reference: &Reference,
    ) -> Result<OciArtifactReference, OciArtifactPublicationError> {
        let (body, received) = self
            .client
            .pull_manifest_raw(write_reference, &self.auth, &[])
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
    let hex = digest.strip_prefix("sha256:").ok_or_else(|| {
        OciArtifactPublicationError::InvalidBundle(
            "Cosign signature subject must use a sha256 digest".to_string(),
        )
    })?;
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(OciArtifactPublicationError::InvalidBundle(
            "Cosign signature subject must use a valid sha256 digest".to_string(),
        ));
    }
    Ok(format!("sha256-{hex}.sig"))
}

/// Resolves a module artifact from an OCI Distribution registry.
///
/// The OCI manifest config is the canonical `ModuleArtifactDescriptor` JSON.
/// Exactly one layer must match both its digest and payload media type. The
/// registry client verifies registry transport semantics; `ModuleArtifactPackage`
/// verifies descriptor identity and the downloaded payload bytes.
#[derive(Clone)]
pub struct OciDistributionArtifactRegistry {
    client: Client,
    auth: RegistryAuth,
    infrastructure: ControlPlaneInfrastructure,
}

impl OciDistributionArtifactRegistry {
    fn with_client(
        client: Client,
        auth: RegistryAuth,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self {
            client,
            auth,
            infrastructure,
        }
    }

    /// Creates an authenticated registry adapter with the mandatory strict
    /// transport subset. Callers cannot supply a client with weaker TLS
    /// settings.
    pub fn strict(auth: RegistryAuth) -> Result<Self, ModuleInstallationError> {
        Self::strict_with_infrastructure(auth, ControlPlaneInfrastructure::default())
    }

    pub fn strict_with_infrastructure(
        auth: RegistryAuth,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Result<Self, ModuleInstallationError> {
        Ok(Self::with_client(
            strict_oci_distribution_client().map_err(ModuleInstallationError::Registry)?,
            auth,
            infrastructure,
        ))
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
    ) -> Result<Reference, ModuleInstallationError> {
        reference.validate()?;
        Reference::try_from(reference.canonical().as_str()).map_err(|error| {
            ModuleInstallationError::Registry(format!(
                "invalid OCI distribution reference `{}`: {error}",
                reference.canonical()
            ))
        })
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
            let expected_media_type = descriptor.payload_kind.oci_layer_media_type();
            let layers = manifest
                .layers
                .iter()
                .filter(|layer| {
                    layer.digest == descriptor.artifact_digest
                        && layer.media_type == expected_media_type
                })
                .collect::<Vec<_>>();
            let [layer] = layers.as_slice() else {
                return Err(ModuleInstallationError::Registry(format!(
                    "OCI artifact must contain exactly one `{expected_media_type}` layer with digest `{}`",
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

impl OciDistributionArtifactRegistry {
    /// Streams the descriptor config only after its declared size passes the
    /// admission bound. The upstream client otherwise buffers this blob before
    /// callers can validate it.
    async fn pull_config_to_memory(
        &self,
        image: &Reference,
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
            .pull_blob_stream(image, config)
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
        image: &Reference,
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
            .pull_blob_stream(image, layer)
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
        ArtifactPayloadKind, MODULE_ARTIFACT_WASM_COMPONENT_MEDIA_TYPE, OciArtifactReference,
    };

    use uuid::Uuid;

    use super::{
        ArtifactStagingFile, MODULE_ARTIFACT_PROVENANCE_MEDIA_TYPE, OciArtifactEvidenceKind,
        OciDistributionArtifactRegistry, OciRegistryTransportPolicy, cosign_signature_tag,
    };

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

        assert_eq!(image.to_string(), reference.canonical());
    }

    #[test]
    fn payload_and_referrer_media_types_are_frozen_by_contract() {
        assert_eq!(
            ArtifactPayloadKind::WasmComponent.oci_layer_media_type(),
            MODULE_ARTIFACT_WASM_COMPONENT_MEDIA_TYPE
        );
        assert_eq!(
            OciArtifactEvidenceKind::Provenance.media_type(),
            MODULE_ARTIFACT_PROVENANCE_MEDIA_TYPE
        );
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
}
