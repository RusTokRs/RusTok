//! OCI Distribution adapter for immutable module artifacts.

use async_trait::async_trait;
use oci_distribution::{secrets::RegistryAuth, Client, Reference};

use crate::{
    ArtifactRegistry, ModuleArtifactPackage, ModuleInstallationError, OciArtifactReference,
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
        let mut payload = Vec::new();
        self.client
            .pull_blob(&image, layer, &mut payload)
            .await
            .map_err(|error| ModuleInstallationError::Registry(error.to_string()))?;
        let package = ModuleArtifactPackage {
            reference: reference.clone(),
            media_type: layer.media_type.clone(),
            descriptor,
            payload,
        };
        package.verify()?;
        Ok(package)
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
