use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

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
}
