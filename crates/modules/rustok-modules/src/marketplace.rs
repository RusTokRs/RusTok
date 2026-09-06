use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::MarketplaceRegistryFreshness;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ModuleGovernanceLifecycleSnapshot, ModuleSettingSpec};

pub const MODULE_MARKETPLACE_DEFAULT_LIMIT: u32 = 100;
pub const MODULE_MARKETPLACE_MAX_LIMIT: u32 = 100;
pub const MODULE_REGISTRY_ID_MAX_BYTES: usize = 96;
const MODULE_RELEASE_REFERENCE_MAX_BYTES: usize = 2_048;

/// Transport-neutral marketplace query accepted by the host-composed catalog
/// port. Filtering belongs to the catalog boundary, not to UI adapters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleMarketplaceQuery {
    pub search: Option<String>,
    pub category: Option<String>,
    pub tag: Option<String>,
    pub source: Option<String>,
    pub trust_level: Option<String>,
    pub only_compatible: bool,
    pub installed_only: bool,
    pub preferred_locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub limit: u32,
}

impl Default for ModuleMarketplaceQuery {
    fn default() -> Self {
        Self {
            search: None,
            category: None,
            tag: None,
            source: None,
            trust_level: None,
            only_compatible: false,
            installed_only: false,
            preferred_locale: None,
            fallback_locale: None,
            limit: MODULE_MARKETPLACE_DEFAULT_LIMIT,
        }
    }
}

/// Normalizes one module slug before it crosses a catalog-provider or URL
/// boundary. The returned grammar cannot alter a URL path, query, or fragment.
pub fn normalize_module_marketplace_slug(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || normalized.len() > 128
        || !normalized.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
        })
        || !normalized
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        || !normalized
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
    {
        return None;
    }
    Some(normalized)
}

/// Normalizes the stable logical identity of a federated module registry.
/// Endpoint URLs are deliberately excluded: moving a registry must not change
/// the identity of its published releases.
pub fn normalize_module_registry_id(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || normalized.len() > MODULE_REGISTRY_ID_MAX_BYTES
        || !normalized.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
        || !normalized
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        || !normalized
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
    {
        return None;
    }
    Some(normalized)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleMarketplaceArtifactOrigin {
    PlatformBuilt,
    ExternalPrebuilt,
    AlloyAuthored,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleMarketplaceRuntimeKind {
    Rhai,
    WasmComponent,
    Sidecar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleMarketplaceEvidenceKind {
    AuthorSignature,
    BuildServiceAttestation,
    Sbom,
    Provenance,
    PlatformAdmission,
    MarketplaceApproval,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleMarketplaceEvidenceReference {
    pub kind: ModuleMarketplaceEvidenceKind,
    pub reference: String,
    pub digest: String,
}

/// Immutable installable identity for one artifact marketplace release.
/// Human metadata and source-host URLs cannot substitute for these facts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleMarketplaceArtifactRelease {
    pub registry_id: String,
    pub repository: String,
    pub origin: ModuleMarketplaceArtifactOrigin,
    pub runtime_kind: ModuleMarketplaceRuntimeKind,
    pub oci_manifest_digest: String,
    pub payload_digest: String,
    pub descriptor_digest: String,
    pub source_reference: String,
    pub source_digest: String,
    pub evidence: Vec<ModuleMarketplaceEvidenceReference>,
}

impl ModuleMarketplaceArtifactRelease {
    pub fn validate(&self) -> Result<(), ModuleMarketplaceArtifactReleaseError> {
        if normalize_module_registry_id(&self.registry_id).as_deref()
            != Some(self.registry_id.as_str())
        {
            return Err(ModuleMarketplaceArtifactReleaseError::InvalidRegistryId);
        }
        if !valid_release_reference(&self.repository)
            || !valid_release_reference(&self.source_reference)
        {
            return Err(ModuleMarketplaceArtifactReleaseError::InvalidReference);
        }
        for digest in [
            &self.oci_manifest_digest,
            &self.payload_digest,
            &self.descriptor_digest,
            &self.source_digest,
        ] {
            if !canonical_sha256_digest(digest) {
                return Err(ModuleMarketplaceArtifactReleaseError::InvalidDigest);
            }
        }

        let mut kinds = std::collections::BTreeSet::new();
        for evidence in &self.evidence {
            if !kinds.insert(evidence.kind) {
                return Err(ModuleMarketplaceArtifactReleaseError::DuplicateEvidence);
            }
            if !valid_release_reference(&evidence.reference)
                || !canonical_sha256_digest(&evidence.digest)
            {
                return Err(ModuleMarketplaceArtifactReleaseError::InvalidEvidence);
            }
        }
        for required in [
            ModuleMarketplaceEvidenceKind::AuthorSignature,
            ModuleMarketplaceEvidenceKind::Sbom,
            ModuleMarketplaceEvidenceKind::Provenance,
            ModuleMarketplaceEvidenceKind::PlatformAdmission,
            ModuleMarketplaceEvidenceKind::MarketplaceApproval,
        ] {
            if !kinds.contains(&required) {
                return Err(ModuleMarketplaceArtifactReleaseError::MissingEvidence(
                    required,
                ));
            }
        }
        if self.origin == ModuleMarketplaceArtifactOrigin::PlatformBuilt
            && !kinds.contains(&ModuleMarketplaceEvidenceKind::BuildServiceAttestation)
        {
            return Err(ModuleMarketplaceArtifactReleaseError::MissingEvidence(
                ModuleMarketplaceEvidenceKind::BuildServiceAttestation,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ModuleMarketplaceArtifactReleaseError {
    #[error("module registry identity is invalid")]
    InvalidRegistryId,
    #[error("module artifact release reference is invalid")]
    InvalidReference,
    #[error("module artifact release digest is invalid")]
    InvalidDigest,
    #[error("module artifact release evidence is invalid")]
    InvalidEvidence,
    #[error("module artifact release evidence kind is duplicated")]
    DuplicateEvidence,
    #[error("module artifact release is missing {0:?} evidence")]
    MissingEvidence(ModuleMarketplaceEvidenceKind),
}

fn valid_release_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MODULE_RELEASE_REFERENCE_MAX_BYTES
        && value
            .chars()
            .all(|character| !character.is_control() && !character.is_whitespace())
}

fn canonical_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleMarketplaceVersion {
    pub version: String,
    pub changelog: Option<String>,
    pub yanked: bool,
    pub published_at: Option<String>,
    pub checksum_sha256: Option<String>,
    pub signature_present: bool,
    #[serde(default)]
    pub artifact: Option<ModuleMarketplaceArtifactRelease>,
}

/// Complete marketplace projection consumed identically by GraphQL and native
/// admin transports. It contains no server, HTTP, filesystem, or UI types.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModuleMarketplaceEntry {
    pub slug: String,
    pub name: String,
    pub latest_version: String,
    pub description: String,
    pub source: String,
    pub kind: String,
    pub category: String,
    pub tags: Vec<String>,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub screenshots: Vec<String>,
    pub crate_name: String,
    pub dependencies: Vec<String>,
    pub ownership: String,
    pub trust_level: String,
    pub rustok_min_version: Option<String>,
    pub rustok_max_version: Option<String>,
    pub publisher: Option<String>,
    pub checksum_sha256: Option<String>,
    pub signature_present: bool,
    pub versions: Vec<ModuleMarketplaceVersion>,
    pub has_admin_ui: bool,
    pub has_storefront_ui: bool,
    pub ui_classification: String,
    pub registry_lifecycle: Option<ModuleGovernanceLifecycleSnapshot>,
    pub compatible: bool,
    pub recommended_admin_surfaces: Vec<String>,
    pub showcase_admin_surfaces: Vec<String>,
    pub settings_schema: BTreeMap<String, ModuleSettingSpec>,
    pub installed: bool,
    pub installed_version: Option<String>,
    pub update_available: bool,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ModuleMarketplaceError {
    #[error("module marketplace query is invalid")]
    InvalidQuery,
    #[error("module marketplace catalog is unavailable")]
    Unavailable,
    #[error("module marketplace catalog returned an invalid contract")]
    InvalidContract,
}

#[async_trait]
pub trait ModuleMarketplaceCatalog: Send + Sync {
    async fn list(
        &self,
        query: ModuleMarketplaceQuery,
    ) -> Result<Vec<ModuleMarketplaceEntry>, ModuleMarketplaceError>;

    async fn get(
        &self,
        slug: &str,
        preferred_locale: Option<String>,
        fallback_locale: Option<String>,
    ) -> Result<Option<ModuleMarketplaceEntry>, ModuleMarketplaceError>;

    /// Returns one snapshot per explicitly configured federated registry.
    /// Local compiled-manifest composition is intentionally not represented as
    /// a remote registry and an empty result means no registries are configured.
    fn registry_freshness(&self) -> Vec<MarketplaceRegistryFreshness>;
}

/// Typed host-runtime handle for the selected local/remote marketplace
/// composition. Absence is a configuration error; callers never fall back to
/// workspace scanning.
#[derive(Clone)]
pub struct SharedModuleMarketplaceCatalog(pub Arc<dyn ModuleMarketplaceCatalog>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_query_uses_the_bounded_catalog_page() {
        assert_eq!(
            ModuleMarketplaceQuery::default().limit,
            MODULE_MARKETPLACE_DEFAULT_LIMIT
        );
        assert_eq!(
            MODULE_MARKETPLACE_DEFAULT_LIMIT,
            MODULE_MARKETPLACE_MAX_LIMIT
        );
    }

    fn evidence(
        kind: ModuleMarketplaceEvidenceKind,
        marker: char,
    ) -> ModuleMarketplaceEvidenceReference {
        ModuleMarketplaceEvidenceReference {
            kind,
            reference: format!("oci://registry.example/evidence/{kind:?}"),
            digest: format!("sha256:{}", marker.to_string().repeat(64)),
        }
    }

    fn artifact_release() -> ModuleMarketplaceArtifactRelease {
        ModuleMarketplaceArtifactRelease {
            registry_id: "community.eu".to_string(),
            repository: "modules/sample".to_string(),
            origin: ModuleMarketplaceArtifactOrigin::ExternalPrebuilt,
            runtime_kind: ModuleMarketplaceRuntimeKind::WasmComponent,
            oci_manifest_digest: format!("sha256:{}", "a".repeat(64)),
            payload_digest: format!("sha256:{}", "b".repeat(64)),
            descriptor_digest: format!("sha256:{}", "c".repeat(64)),
            source_reference: "https://source.example/sample?rev=exact".to_string(),
            source_digest: format!("sha256:{}", "d".repeat(64)),
            evidence: vec![
                evidence(ModuleMarketplaceEvidenceKind::AuthorSignature, 'e'),
                evidence(ModuleMarketplaceEvidenceKind::Sbom, 'f'),
                evidence(ModuleMarketplaceEvidenceKind::Provenance, '1'),
                evidence(ModuleMarketplaceEvidenceKind::PlatformAdmission, '2'),
                evidence(ModuleMarketplaceEvidenceKind::MarketplaceApproval, '3'),
            ],
        }
    }

    #[test]
    fn artifact_release_requires_canonical_identity_and_evidence() {
        artifact_release().validate().expect("artifact release");

        let mut invalid = artifact_release();
        invalid.registry_id = "endpoint/derived".to_string();
        assert_eq!(
            invalid.validate(),
            Err(ModuleMarketplaceArtifactReleaseError::InvalidRegistryId)
        );

        let mut missing = artifact_release();
        missing
            .evidence
            .retain(|value| value.kind != ModuleMarketplaceEvidenceKind::Sbom);
        assert_eq!(
            missing.validate(),
            Err(ModuleMarketplaceArtifactReleaseError::MissingEvidence(
                ModuleMarketplaceEvidenceKind::Sbom
            ))
        );
    }

    #[test]
    fn provider_slug_normalization_rejects_path_and_query_injection() {
        assert_eq!(
            normalize_module_marketplace_slug("  Page-Builder_2  ").as_deref(),
            Some("page-builder_2")
        );
        for invalid in [
            "../modules",
            "module/child",
            "module?tenant=other",
            "module#fragment",
            "-module",
            "module_",
            "",
        ] {
            assert_eq!(normalize_module_marketplace_slug(invalid), None);
        }
    }
}
