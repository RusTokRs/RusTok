use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Canonical module slug grammar shared by manifests, artifact declarations,
/// build requests, and author tooling.
pub fn is_valid_module_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 48
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
        && !value.starts_with('_')
        && !value.ends_with('_')
}

/// Browser-safe static registry projection shared by internal transports.
///
/// The host resolves active-composition, lifecycle, and catalog metadata before
/// constructing this view. Consumers must not reconstruct it from a manifest,
/// build-time code generation, or direct lifecycle rows.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct StaticModuleRegistryView {
    #[serde(rename = "moduleSlug")]
    pub module_slug: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub kind: String,
    pub dependencies: Vec<String>,
    pub enabled: bool,
    #[serde(rename = "lifecycleRevision")]
    pub lifecycle_revision: i64,
    pub ownership: String,
    #[serde(rename = "trustLevel")]
    pub trust_level: String,
    #[serde(rename = "hasAdminUi")]
    pub has_admin_ui: bool,
    #[serde(rename = "hasStorefrontUi")]
    pub has_storefront_ui: bool,
    #[serde(rename = "uiClassification")]
    pub ui_classification: String,
    #[serde(rename = "recommendedAdminSurfaces")]
    pub recommended_admin_surfaces: Vec<String>,
    #[serde(rename = "showcaseAdminSurfaces")]
    pub showcase_admin_surfaces: Vec<String>,
}

impl StaticModuleRegistryView {
    pub fn is_core(&self) -> bool {
        self.kind == "core"
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestModuleContract {
    pub slug: String,
    pub required: bool,
    pub dependencies: BTreeSet<String>,
    pub has_runtime_entry: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryModuleContract {
    pub slug: String,
    pub core: bool,
    pub dependencies: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ModuleRegistryContractError {
    #[error("modules.toml entries are not available in ModuleRegistry: {0}")]
    MissingInRegistry(String),
    #[error("modules.toml required flags conflict with ModuleRegistry kinds: {0}")]
    RequiredMismatch(String),
    #[error("modules.toml depends_on conflict with ModuleRegistry dependencies: {0}")]
    DependencyMismatch(String),
}

pub fn validate_module_registry_contract(
    manifest_modules: impl IntoIterator<Item = ManifestModuleContract>,
    registry_modules: impl IntoIterator<Item = RegistryModuleContract>,
) -> Result<(), ModuleRegistryContractError> {
    let manifest_modules = manifest_modules
        .into_iter()
        .map(|module| (module.slug.clone(), module))
        .collect::<BTreeMap<_, _>>();
    let registry_modules = registry_modules
        .into_iter()
        .map(|module| (module.slug.clone(), module))
        .collect::<BTreeMap<_, _>>();

    let missing_in_registry = manifest_modules
        .values()
        .filter(|module| module.has_runtime_entry)
        .filter(|module| !registry_modules.contains_key(&module.slug))
        .map(|module| module.slug.clone())
        .collect::<Vec<_>>();

    if !missing_in_registry.is_empty() {
        return Err(ModuleRegistryContractError::MissingInRegistry(
            missing_in_registry.join(", "),
        ));
    }

    let required_mismatch = registry_modules
        .values()
        .filter_map(|registry_module| {
            manifest_modules
                .get(&registry_module.slug)
                .filter(|manifest_module| manifest_module.required != registry_module.core)
                .map(|manifest_module| {
                    format!(
                        "{} (required={}, core={})",
                        registry_module.slug, manifest_module.required, registry_module.core
                    )
                })
        })
        .collect::<Vec<_>>();

    if !required_mismatch.is_empty() {
        return Err(ModuleRegistryContractError::RequiredMismatch(
            required_mismatch.join(", "),
        ));
    }

    let dependency_mismatch = registry_modules
        .values()
        .filter_map(|registry_module| {
            manifest_modules
                .get(&registry_module.slug)
                .filter(|manifest_module| {
                    manifest_module.dependencies != registry_module.dependencies
                })
                .map(|manifest_module| {
                    format!(
                        "{} (manifest={:?}, registry={:?})",
                        registry_module.slug,
                        manifest_module.dependencies,
                        registry_module.dependencies
                    )
                })
        })
        .collect::<Vec<_>>();

    if !dependency_mismatch.is_empty() {
        return Err(ModuleRegistryContractError::DependencyMismatch(
            dependency_mismatch.join(", "),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_module(
        slug: &str,
        required: bool,
        dependencies: &[&str],
    ) -> ManifestModuleContract {
        ManifestModuleContract {
            slug: slug.to_string(),
            required,
            dependencies: dependencies
                .iter()
                .map(|dependency| (*dependency).to_string())
                .collect(),
            has_runtime_entry: true,
        }
    }

    fn registry_module(slug: &str, core: bool, dependencies: &[&str]) -> RegistryModuleContract {
        RegistryModuleContract {
            slug: slug.to_string(),
            core,
            dependencies: dependencies
                .iter()
                .map(|dependency| (*dependency).to_string())
                .collect(),
        }
    }

    #[test]
    fn accepts_matching_manifest_and_registry_contracts() {
        validate_module_registry_contract(
            [
                manifest_module("content", false, &[]),
                manifest_module("blog", false, &["content"]),
            ],
            [
                registry_module("content", false, &[]),
                registry_module("blog", false, &["content"]),
            ],
        )
        .expect("matching contracts should pass");
    }

    #[test]
    fn rejects_runtime_entry_missing_from_registry() {
        let error = validate_module_registry_contract(
            [manifest_module("blog", false, &[])],
            std::iter::empty(),
        )
        .expect_err("missing runtime entry should fail");

        assert_eq!(
            error,
            ModuleRegistryContractError::MissingInRegistry("blog".to_string())
        );
    }

    #[test]
    fn ignores_manifest_entries_without_runtime_entry() {
        let mut capability = manifest_module("alloy", false, &[]);
        capability.has_runtime_entry = false;

        validate_module_registry_contract([capability], std::iter::empty())
            .expect("capability-only manifest entry should not require a runtime registry entry");
    }

    #[test]
    fn rejects_required_and_core_mismatch() {
        let error = validate_module_registry_contract(
            [manifest_module("tenant", true, &[])],
            [registry_module("tenant", false, &[])],
        )
        .expect_err("required/core mismatch should fail");

        assert!(matches!(
            error,
            ModuleRegistryContractError::RequiredMismatch(_)
        ));
    }

    #[test]
    fn rejects_dependency_mismatch() {
        let error = validate_module_registry_contract(
            [manifest_module("blog", false, &["content"])],
            [registry_module("blog", false, &["comments"])],
        )
        .expect_err("dependency mismatch should fail");

        assert!(matches!(
            error,
            ModuleRegistryContractError::DependencyMismatch(_)
        ));
    }

    #[test]
    fn module_slug_uses_the_shared_short_snake_case_contract() {
        assert!(is_valid_module_slug("sample_module2"));
        assert!(!is_valid_module_slug("SampleModule"));
        assert!(!is_valid_module_slug("sample-module"));
        assert!(!is_valid_module_slug("_sample"));
        assert!(!is_valid_module_slug(&"a".repeat(49)));
    }

    #[test]
    fn static_registry_view_uses_the_transport_field_contract() {
        let view = StaticModuleRegistryView {
            module_slug: "blog".to_string(),
            name: "Blog".to_string(),
            description: "Publishing".to_string(),
            version: "1.2.3".to_string(),
            kind: "optional".to_string(),
            dependencies: vec!["content".to_string()],
            enabled: true,
            lifecycle_revision: 42,
            ownership: "first_party".to_string(),
            trust_level: "verified".to_string(),
            has_admin_ui: true,
            has_storefront_ui: false,
            ui_classification: "admin_only".to_string(),
            recommended_admin_surfaces: vec!["leptos-admin".to_string()],
            showcase_admin_surfaces: vec!["next-admin".to_string()],
        };

        assert_eq!(
            serde_json::to_value(&view).expect("static module registry view serializes"),
            serde_json::json!({
                "moduleSlug": "blog",
                "name": "Blog",
                "description": "Publishing",
                "version": "1.2.3",
                "kind": "optional",
                "dependencies": ["content"],
                "enabled": true,
                "lifecycleRevision": 42,
                "ownership": "first_party",
                "trustLevel": "verified",
                "hasAdminUi": true,
                "hasStorefrontUi": false,
                "uiClassification": "admin_only",
                "recommendedAdminSurfaces": ["leptos-admin"],
                "showcaseAdminSurfaces": ["next-admin"],
            })
        );
        assert!(!view.is_core());
    }
}
