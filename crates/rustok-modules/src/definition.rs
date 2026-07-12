//! Artifact-aware module identity, separate from compiled implementation handles.

use std::collections::BTreeMap;

use rustok_core::{ModuleKind, ModuleRegistry};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ArtifactModuleKind, ArtifactPermissionDescriptor, ArtifactReleaseRef, ModuleArtifactDescriptor,
    ModuleDependencyConstraint, ModuleRuntimeBinding,
};

/// Whether a definition is permanently active platform infrastructure or can
/// be enabled for an installation scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleDefinitionKind {
    Core,
    Optional,
}

impl From<ModuleKind> for ModuleDefinitionKind {
    fn from(value: ModuleKind) -> Self {
        match value {
            ModuleKind::Core => Self::Core,
            ModuleKind::Optional => Self::Optional,
        }
    }
}

/// The executable implementation class selected for a module definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ModuleDefinitionSource {
    Static { distribution_version: String },
    Artifact { release: ArtifactReleaseRef },
}

/// All metadata that policy and dispatch will resolve without inspecting a
/// `RusToKModule` trait object. Fields not supported by static modules are
/// intentionally explicit empty values rather than inferred at runtime.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleDefinition {
    pub slug: String,
    pub version: String,
    pub kind: ModuleDefinitionKind,
    pub source: ModuleDefinitionSource,
    #[serde(default)]
    pub dependencies: Vec<ModuleDependencyConstraint>,
    #[serde(default)]
    pub permissions: Vec<ArtifactPermissionDescriptor>,
    #[serde(default)]
    pub settings_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub bindings: Vec<ModuleRuntimeBinding>,
    #[serde(default)]
    pub ui: Option<serde_json::Value>,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

impl ModuleDefinition {
    /// Adapts only stable identity/topology metadata from compiled modules.
    /// The registry remains responsible for their in-process runtime handles.
    pub fn from_static_registry_module(module: &dyn rustok_core::RusToKModule) -> Self {
        Self {
            slug: module.slug().to_string(),
            version: module.version().to_string(),
            kind: module.kind().into(),
            source: ModuleDefinitionSource::Static {
                distribution_version: module.version().to_string(),
            },
            dependencies: module
                .dependencies()
                .iter()
                .map(|dependency| ModuleDependencyConstraint {
                    slug: (*dependency).to_string(),
                    version_requirement: "*".to_string(),
                })
                .collect(),
            permissions: module
                .permissions()
                .into_iter()
                .map(|permission| {
                    let key = permission.to_string();
                    ArtifactPermissionDescriptor {
                        label: key.clone(),
                        key,
                    }
                })
                .collect(),
            settings_schema: None,
            bindings: Vec::new(),
            ui: None,
            capabilities: Vec::new(),
        }
    }

    /// Adapts immutable artifact identity while preserving descriptor metadata.
    pub fn from_artifact_descriptor(descriptor: &ModuleArtifactDescriptor) -> Self {
        Self {
            slug: descriptor.slug.clone(),
            version: descriptor.version.clone(),
            kind: match descriptor.module_kind {
                ArtifactModuleKind::Core => ModuleDefinitionKind::Core,
                ArtifactModuleKind::Optional => ModuleDefinitionKind::Optional,
            },
            source: ModuleDefinitionSource::Artifact {
                release: descriptor.release_ref(),
            },
            dependencies: descriptor.dependencies.clone(),
            permissions: descriptor.permissions.clone(),
            settings_schema: None,
            bindings: descriptor.bindings.clone(),
            ui: None,
            capabilities: descriptor
                .capabilities
                .iter()
                .map(|capability| capability.as_str().to_string())
                .collect(),
        }
    }

    pub fn binding(&self, id: &str) -> Option<&ModuleRuntimeBinding> {
        self.bindings.iter().find(|binding| binding.id == id)
    }
}

/// Resolved definition set for one platform composition revision.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ModuleDefinitionCatalog {
    definitions: BTreeMap<String, ModuleDefinition>,
}

impl ModuleDefinitionCatalog {
    pub fn from_static_registry(registry: &ModuleRegistry) -> Result<Self, ModuleDefinitionError> {
        let mut catalog = Self::default();
        for module in registry.list() {
            catalog.insert(ModuleDefinition::from_static_registry_module(module))?;
        }
        Ok(catalog)
    }

    pub fn insert(&mut self, definition: ModuleDefinition) -> Result<(), ModuleDefinitionError> {
        if definition.slug.trim().is_empty() {
            return Err(ModuleDefinitionError::EmptySlug);
        }
        if let Some(existing) = self.definitions.get(&definition.slug) {
            return Err(ModuleDefinitionError::AmbiguousActiveDefinition {
                slug: definition.slug,
                existing: existing.source.clone(),
                incoming: definition.source,
            });
        }
        self.definitions.insert(definition.slug.clone(), definition);
        Ok(())
    }

    pub fn get(&self, slug: &str) -> Option<&ModuleDefinition> {
        self.definitions.get(slug)
    }

    pub fn definitions(&self) -> impl Iterator<Item = &ModuleDefinition> {
        self.definitions.values()
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ModuleDefinitionError {
    #[error("module definition slug must not be empty")]
    EmptySlug,
    #[error("module `{slug}` has ambiguous active implementations")]
    AmbiguousActiveDefinition {
        slug: String,
        existing: ModuleDefinitionSource,
        incoming: ModuleDefinitionSource,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModulesModule;

    #[test]
    fn static_adapter_preserves_compiled_module_topology() {
        let catalog = ModuleDefinitionCatalog::from_static_registry(
            &ModuleRegistry::new().register(ModulesModule),
        )
        .expect("static catalog");
        let definition = catalog.get("modules").expect("modules definition");
        assert_eq!(definition.kind, ModuleDefinitionKind::Core);
        assert!(matches!(
            definition.source,
            ModuleDefinitionSource::Static { .. }
        ));
    }

    #[test]
    fn catalog_rejects_ambiguous_active_implementations() {
        let mut catalog = ModuleDefinitionCatalog::default();
        let definition = ModuleDefinition::from_static_registry_module(&ModulesModule);
        catalog
            .insert(definition.clone())
            .expect("first definition");
        assert!(matches!(
            catalog.insert(definition),
            Err(ModuleDefinitionError::AmbiguousActiveDefinition { .. })
        ));
    }
}
