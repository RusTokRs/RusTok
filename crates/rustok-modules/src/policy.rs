use std::collections::HashSet;

use thiserror::Error;

use crate::{ModuleDefinitionCatalog, ModuleDefinitionKind};

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
/// set and definition topology. Persistence, operation journaling and lifecycle
/// hooks are intentionally outside this owner policy function.
pub fn validate_module_toggle(
    catalog: &ModuleDefinitionCatalog,
    enabled_modules: &HashSet<String>,
    module_slug: &str,
    enabled: bool,
) -> Result<(), ModuleToggleValidationError> {
    let Some(module) = catalog.get(module_slug) else {
        return Err(ModuleToggleValidationError::UnknownModule);
    };

    if !enabled && module.kind == ModuleDefinitionKind::Core {
        return Err(ModuleToggleValidationError::CoreModuleCannotBeDisabled(
            module_slug.to_string(),
        ));
    }

    if enabled {
        let missing = module
            .dependencies
            .iter()
            .filter(|dependency| !enabled_modules.contains(&dependency.slug))
            .map(|dependency| dependency.slug.clone())
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(ModuleToggleValidationError::MissingDependencies(missing));
        }
    } else {
        let dependents = catalog
            .definitions()
            .filter(|candidate| enabled_modules.contains(&candidate.slug))
            .filter(|candidate| {
                candidate
                    .dependencies
                    .iter()
                    .any(|dependency| dependency.slug == module_slug)
            })
            .map(|candidate| candidate.slug.clone())
            .collect::<Vec<_>>();
        if !dependents.is_empty() {
            return Err(ModuleToggleValidationError::HasDependents(dependents));
        }
    }

    Ok(())
}

/// Owner-owned effective-availability query. Host adapters supply their
/// distribution defaults and persisted tenant overrides; this query applies the
/// canonical catalog semantics equally to static and artifact definitions.
pub struct ModuleEffectivePolicyQuery<'a> {
    catalog: &'a ModuleDefinitionCatalog,
    default_enabled: Vec<String>,
    tenant_overrides: Vec<TenantModuleOverride>,
}

/// The resolved module set used by lifecycle, routing, and installer adapters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleEffectivePolicy {
    enabled_modules: HashSet<String>,
}

impl<'a> ModuleEffectivePolicyQuery<'a> {
    pub fn new(
        catalog: &'a ModuleDefinitionCatalog,
        default_enabled: impl IntoIterator<Item = String>,
        tenant_overrides: impl IntoIterator<Item = TenantModuleOverride>,
    ) -> Self {
        Self {
            catalog,
            default_enabled: default_enabled.into_iter().collect(),
            tenant_overrides: tenant_overrides.into_iter().collect(),
        }
    }

    /// Resolves the immutable core set, selected optional defaults, and tenant
    /// intent. Unknown and legacy overrides are ignored rather than becoming
    /// active definitions.
    pub fn execute(self) -> ModuleEffectivePolicy {
        let mut enabled = self
            .catalog
            .definitions()
            .filter(|definition| definition.kind == ModuleDefinitionKind::Core)
            .map(|definition| definition.slug.clone())
            .collect::<HashSet<_>>();

        for slug in self.default_enabled {
            if self
                .catalog
                .get(&slug)
                .is_some_and(|definition| definition.kind == ModuleDefinitionKind::Optional)
            {
                enabled.insert(slug);
            }
        }

        for module in self.tenant_overrides {
            let Some(definition) = self.catalog.get(&module.module_slug) else {
                continue;
            };
            if definition.kind == ModuleDefinitionKind::Core {
                continue;
            }
            if module.enabled {
                enabled.insert(module.module_slug);
            } else {
                enabled.remove(&module.module_slug);
            }
        }

        ModuleEffectivePolicy {
            enabled_modules: enabled,
        }
    }
}

impl ModuleEffectivePolicy {
    pub fn contains(&self, module_slug: &str) -> bool {
        self.enabled_modules.contains(module_slug)
    }

    pub fn into_enabled_modules(self) -> HashSet<String> {
        self.enabled_modules
    }
}

#[cfg(test)]
mod tests {
    use super::{ModuleEffectivePolicyQuery, TenantModuleOverride};
    use crate::{ModuleDefinitionCatalog, ModulesModule};
    use rustok_core::ModuleRegistry;

    #[test]
    fn core_is_immutable_and_overrides_require_registered_optional_modules() {
        let catalog = ModuleDefinitionCatalog::from_static_registry(
            &ModuleRegistry::new().register(ModulesModule),
        )
        .expect("catalog");
        let policy = ModuleEffectivePolicyQuery::new(
            &catalog,
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
        )
        .execute();

        assert!(policy.contains("modules"));
        assert!(!policy.contains("missing"));
        assert!(!policy.contains("persisted-legacy-override"));
    }
}
