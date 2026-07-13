//! OCI Distribution adapter for immutable module artifacts.

use async_trait::async_trait;
use futures_util::StreamExt;
use oci_distribution::{secrets::RegistryAuth, Client, Reference};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::{
    ArtifactAdmissionLimits, ArtifactRegistry, ModuleArtifactPackage, ModuleInstallationError,
    OciArtifactReference,
};

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
}

impl OciDistributionArtifactRegistry {
    pub fn new(client: Client, auth: RegistryAuth) -> Self {
        Self { client, auth }
    }

    pub fn anonymous() -> Self {
        Self::new(Client::default(), RegistryAuth::Anonymous)
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
        let image = Self::image_reference(reference)?;
        let (manifest, manifest_digest, config) = self
            .client
            .pull_manifest_and_config(&image, &self.auth)
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
        limits.validate_descriptor_size(config.len() as u64)?;
        let descriptor = serde_json::from_str(&config).map_err(|error| {
            ModuleInstallationError::Registry(format!(
                "OCI artifact config is not a module descriptor: {error}"
            ))
        })?;
        let expected_media_type = media_type_for_descriptor(&descriptor);
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
            .pull_payload_to_temporary_storage(&image, layer, &descriptor.artifact_digest, limits)
            .await?;
        let package = ModuleArtifactPackage {
            reference: reference.clone(),
            media_type: layer.media_type.clone(),
            descriptor,
            payload,
        };
        package.verify(limits)?;
        Ok(package)
    }
}

impl OciDistributionArtifactRegistry {
    /// Streams a registry layer through a bounded private staging file. The
    /// current object-storage port still accepts a bounded buffer after this
    /// check; this method deliberately avoids an unbounded network `Vec<u8>`.
    async fn pull_payload_to_temporary_storage(
        &self,
        image: &Reference,
        layer: &oci_distribution::manifest::OciDescriptor,
        expected_digest: &str,
        limits: ArtifactAdmissionLimits,
    ) -> Result<Vec<u8>, ModuleInstallationError> {
        let path = std::env::temp_dir().join(format!("rustok-artifact-stage-{}", Uuid::new_v4()));
        let result = async {
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .await
                .map_err(|error| ModuleInstallationError::Registry(error.to_string()))?;
            let mut stream = self
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
                received = received.checked_add(chunk.len() as u64).ok_or_else(|| {
                    ModuleInstallationError::ArtifactTooLarge {
                        kind: "payload",
                        limit: limits.max_payload_bytes,
                        actual: u64::MAX,
                    }
                })?;
                limits.validate_payload_size(received)?;
                hasher.update(&chunk);
                file.write_all(&chunk)
                    .await
                    .map_err(|error| ModuleInstallationError::Registry(error.to_string()))?;
            }
            file.flush()
                .await
                .map_err(|error| ModuleInstallationError::Registry(error.to_string()))?;
            let actual_digest = format!("sha256:{}", hex::encode(hasher.finalize()));
            if actual_digest != expected_digest {
                return Err(ModuleInstallationError::PayloadDigestMismatch {
                    expected: expected_digest.to_string(),
                    actual: actual_digest,
                });
            }
            tokio::fs::read(&path)
                .await
                .map_err(|error| ModuleInstallationError::Registry(error.to_string()))
        }
        .await;
        let _ = tokio::fs::remove_file(&path).await;
        result
    }
}

fn media_type_for_descriptor(descriptor: &crate::ModuleArtifactDescriptor) -> &'static str {
    match descriptor.payload_kind {
        crate::ArtifactPayloadKind::Rhai => "application/vnd.rustok.rhai.source.v1",
        crate::ArtifactPayloadKind::WasmComponent => "application/wasm",
        crate::ArtifactPayloadKind::Sidecar => "application/vnd.rustok.sidecar.v1",
        crate::ArtifactPayloadKind::StaticPromoted => "application/vnd.rustok.static-promotion.v1",
    }
}

#[cfg(test)]
mod tests {
    use crate::OciArtifactReference;

    use super::OciDistributionArtifactRegistry;

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
}
