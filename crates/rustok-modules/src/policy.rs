use std::collections::HashSet;

use rustok_core::ModuleRegistry;
use thiserror::Error;

/// A persisted tenant-level module enablement override.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TenantModuleOverride {
    pub module_slug: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ModuleToggleValidationError {
    #[error("unknown module")]
    UnknownModule,
    #[error("module `{0}` is a core platform module and cannot be disabled")]
    CoreModuleCannotBeDisabled(String),
    #[error("missing module dependencies: {0:?}")]
    MissingDependencies(Vec<String>),
    #[error("module has enabled dependents: {0:?}")]
    HasDependents(Vec<String>),
}

/// Validates a requested module enablement change against the effective module
/// set and registry topology. Persistence, operation journaling and lifecycle
/// hooks are intentionally outside this owner policy function.
pub fn validate_module_toggle(
    registry: &ModuleRegistry,
    enabled_modules: &HashSet<String>,
    module_slug: &str,
    enabled: bool,
) -> Result<(), ModuleToggleValidationError> {
    let Some(module) = registry.get(module_slug) else {
        return Err(ModuleToggleValidationError::UnknownModule);
    };

    if !enabled && registry.is_core(module.slug()) {
        return Err(ModuleToggleValidationError::CoreModuleCannotBeDisabled(
            module_slug.to_string(),
        ));
    }

    if enabled {
        let missing = module
            .dependencies()
            .iter()
            .filter(|dependency| !enabled_modules.contains(**dependency))
            .map(|dependency| (*dependency).to_string())
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(ModuleToggleValidationError::MissingDependencies(missing));
        }
    } else {
        let dependents = registry
            .list()
            .into_iter()
            .filter(|candidate| enabled_modules.contains(candidate.slug()))
            .filter(|candidate| candidate.dependencies().contains(&module_slug))
            .map(|candidate| candidate.slug().to_string())
            .collect::<Vec<_>>();
        if !dependents.is_empty() {
            return Err(ModuleToggleValidationError::HasDependents(dependents));
        }
    }

    Ok(())
}

/// Resolves the effective module set from the platform defaults and tenant
/// overrides.
///
/// Core modules are always present and neither defaults nor tenant overrides
/// can disable them. Defaults and overrides are accepted only for registered
/// Optional modules. Database and manifest loading deliberately remain outside
/// this pure owner policy function.
pub fn resolve_effective_modules(
    registry: &ModuleRegistry,
    default_enabled: impl IntoIterator<Item = String>,
    tenant_overrides: impl IntoIterator<Item = TenantModuleOverride>,
) -> HashSet<String> {
    let mut enabled = registry
        .list()
        .into_iter()
        .filter(|module| registry.is_core(module.slug()))
        .map(|module| module.slug().to_string())
        .collect::<HashSet<_>>();

    for slug in default_enabled {
        if registry
            .get(&slug)
            .is_some_and(|module| !registry.is_core(module.slug()))
        {
            enabled.insert(slug);
        }
    }

    for module in tenant_overrides {
        let Some(registered) = registry.get(&module.module_slug) else {
            continue;
        };
        if registry.is_core(registered.slug()) {
            continue;
        }
        if module.enabled {
            enabled.insert(module.module_slug);
        } else {
            enabled.remove(&module.module_slug);
        }
    }

    enabled
}

#[cfg(test)]
mod tests {
    use super::{resolve_effective_modules, TenantModuleOverride};
    use crate::ModulesModule;
    use rustok_core::ModuleRegistry;

    #[test]
    fn core_is_immutable_and_overrides_require_registered_optional_modules() {
        let registry = ModuleRegistry::new().register(ModulesModule);
        let enabled = resolve_effective_modules(
            &registry,
            ["modules".to_string(), "missing".to_string()],
            [
                TenantModuleOverride {
                    module_slug: "modules".to_string(),
                    enabled: false,
                },
                TenantModuleOverride {
                    module_slug: "persisted-legacy-override".to_string(),
                    enabled: true,
                },
            ],
        );

        assert!(enabled.contains("modules"));
        assert!(!enabled.contains("missing"));
        assert!(!enabled.contains("persisted-legacy-override"));
    }
}
