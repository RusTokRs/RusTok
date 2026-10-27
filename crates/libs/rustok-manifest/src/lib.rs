//! # RusToK Manifest
//!
//! Canonical manifest models, schema validation, and topological dependency graph
//! resolution for RusToK platform composition (`modules.toml`) and module packages
//! (`rustok-module.toml`).
//!
//! `rustok-manifest` is a zero-runtime, pure data and validation library. It is consumed
//! by the server compilation pipeline, `xtask` scaffolding, and platform verification gates.
//!
//! ## Core Architecture & Modules
//!
//! - **Data Models**: [`types`] defines typed representations of root composition manifests
//!   ([`ModulesManifest`]), module specs ([`ModuleSpec`]), and package contracts ([`ModulePackageManifest`]).
//! - **Schema Validation**: [`validation`] provides cycle-free Kahn topological sorting
//!   ([`validate_modules_manifest`]), package invariant verification ([`validate_module_package_manifest`]),
//!   and canonical slug rules ([`is_valid_module_slug`]).
//! - **Parsing & Loading**: [`parse_modules_manifest`], [`load_modules_manifest`],
//!   [`parse_module_package_manifest`], and [`load_module_package_manifest`].
//! - **Snapshot Integrity**: [`compute_modules_manifest_hash`] computes deterministic SHA-256
//!   hashes of resolved module configurations under the `snapshot-hash` feature.

pub mod error;
pub mod types;
pub mod validation;

#[cfg(test)]
mod contract_tests;

pub use error::{ManifestError, Result};
pub use types::{
    AdminBuildConfig, BuildConfig, Manifest, ModuleGraphqlProvides, ModuleHttpProvides,
    ModulePackageAdminUiChildPage, ModulePackageAdminUiProvides, ModulePackageCliProvides,
    ModulePackageCrateContract, ModulePackageDependency, ModulePackageGraphqlProvides,
    ModulePackageHttpProvides, ModulePackageI18nProvides, ModulePackageManifest,
    ModulePackageMarketplaceMetadata, ModulePackageMetadata, ModulePackageProvides,
    ModulePackageStorefrontUiComponent, ModulePackageStorefrontUiProvides, ModuleSpec,
    ModuleUiComponentProvides, ModuleUiI18nProvides, ModuleUiProvides, ModulesManifest,
    ServerBuildConfig, Settings, StorefrontBuildConfig,
};
pub use validation::{
    is_valid_module_slug, is_valid_ownership, is_valid_trust_level, is_valid_ui_classification,
    validate_module_package_manifest, validate_modules_manifest,
};

use std::path::Path;

/// Parse `modules.toml` from a string slice without validation.
pub fn parse_modules_manifest(content: &str) -> Result<ModulesManifest> {
    toml::from_str(content).map_err(|e| ManifestError::TomlParse("inline string".to_string(), e))
}

/// Load and parse `modules.toml` from a file path without validation.
pub fn load_modules_manifest(path: impl AsRef<Path>) -> Result<ModulesManifest> {
    let path_ref = path.as_ref();
    let content = std::fs::read_to_string(path_ref)
        .map_err(|e| ManifestError::Io(path_ref.display().to_string(), e))?;
    toml::from_str(&content)
        .map_err(|e| ManifestError::TomlParse(path_ref.display().to_string(), e))
}

/// Parse `rustok-module.toml` from a string slice without validation.
pub fn parse_module_package_manifest(content: &str) -> Result<ModulePackageManifest> {
    toml::from_str(content).map_err(|e| ManifestError::TomlParse("inline string".to_string(), e))
}

/// Load and parse `rustok-module.toml` from a file path without validation.
pub fn load_module_package_manifest(path: impl AsRef<Path>) -> Result<ModulePackageManifest> {
    let path_ref = path.as_ref();
    let content = std::fs::read_to_string(path_ref)
        .map_err(|e| ManifestError::Io(path_ref.display().to_string(), e))?;
    toml::from_str(&content)
        .map_err(|e| ManifestError::TomlParse(path_ref.display().to_string(), e))
}

/// Compute deterministic SHA-256 hash of a ModulesManifest.
#[cfg(feature = "snapshot-hash")]
pub fn compute_modules_manifest_hash(manifest: &ModulesManifest) -> Result<String> {
    use sha2::{Digest, Sha256};
    let json = serde_json::to_vec(manifest).map_err(|e| {
        ManifestError::Validation(format!("JSON serialization error for hash: {e}"))
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&json);
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_and_validates_root_modules_toml() {
        let root_manifest = include_str!("../../../../modules.toml");
        let manifest = parse_modules_manifest(root_manifest).unwrap();
        validate_modules_manifest(&manifest).unwrap();
        assert_eq!(manifest.schema, 2);
        assert!(manifest.modules.contains_key("content"));
    }

    #[test]
    fn loads_and_validates_blog_module_toml() {
        let blog_manifest = include_str!("../../../modules/rustok-blog/rustok-module.toml");
        let manifest = parse_module_package_manifest(blog_manifest).unwrap();
        validate_module_package_manifest(&manifest).unwrap();
        assert_eq!(manifest.module.slug, "blog");
        assert_eq!(manifest.module.ui_classification, "dual_surface");
    }
}
