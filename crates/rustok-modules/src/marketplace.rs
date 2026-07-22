use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ModuleGovernanceLifecycleSnapshot, ModuleSettingSpec};

pub const MODULE_MARKETPLACE_DEFAULT_LIMIT: u32 = 100;
pub const MODULE_MARKETPLACE_MAX_LIMIT: u32 = 100;

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleMarketplaceVersion {
    pub version: String,
    pub changelog: Option<String>,
    pub yanked: bool,
    pub published_at: Option<String>,
    pub checksum_sha256: Option<String>,
    pub signature_present: bool,
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
