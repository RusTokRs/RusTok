//! Artifact-aware module identity, separate from compiled implementation handles.

use std::collections::BTreeMap;

use rustok_api::ArtifactPermissionLocalization;
use rustok_core::{ModuleKind, ModuleRegistry};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::artifact::canonical_schema_digest;
use crate::{
    ArtifactModuleKind, ArtifactPermissionDescriptor, ArtifactReleaseRef, ArtifactSchemaDocument,
    ArtifactUiContribution, ModuleArtifactDescriptor, ModuleDependencyConstraint,
    ModuleRuntimeBinding, ModuleStaticDistributionExecutorMode, ModuleStaticDistributionRelease,
    ModuleStaticDistributionReleaseStatus,
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
    PlatformNative {
        distribution_version: String,
    },
    PromotedNative {
        distribution_version: String,
        distribution_release_id: uuid::Uuid,
        distribution_release_revision: u64,
        registry_release_id: String,
        promotion_id: uuid::Uuid,
        promotion_revision: u64,
        distribution_artifact_digest: String,
        executor_mode: ModuleStaticDistributionExecutorMode,
    },
    Artifact {
        release: ArtifactReleaseRef,
    },
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
    pub settings_schema_digest: Option<String>,
    #[serde(default)]
    pub schema_documents: Vec<ArtifactSchemaDocument>,
    #[serde(default)]
    pub bindings: Vec<ModuleRuntimeBinding>,
    #[serde(default)]
    pub ui: Vec<ArtifactUiContribution>,
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
            source: ModuleDefinitionSource::PlatformNative {
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
                        localizations: vec![ArtifactPermissionLocalization {
                            locale: "en".to_string(),
                            label: key.clone(),
                            description: key.clone(),
                        }],
                        key,
                    }
                })
                .collect(),
            settings_schema_digest: None,
            schema_documents: Vec::new(),
            bindings: Vec::new(),
            ui: Vec::new(),
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
            settings_schema_digest: descriptor.settings_schema_digest.clone(),
            schema_documents: descriptor.schema_documents.clone(),
            bindings: descriptor.bindings.clone(),
            ui: descriptor.ui_contributions.clone(),
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

    /// Resolves only the settings schema named by the immutable artifact
    /// selector. Static definitions intentionally have no artifact schema.
    pub fn settings_schema(&self) -> Option<&serde_json::Value> {
        self.settings_schema_digest.as_ref().and_then(|digest| {
            self.schema_documents
                .iter()
                .find(|schema| schema.digest == *digest)
                .map(|schema| &schema.document)
        })
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
        match (
            &definition.source,
            definition.settings_schema_digest.as_ref(),
        ) {
            (
                ModuleDefinitionSource::PlatformNative { .. }
                | ModuleDefinitionSource::PromotedNative { .. },
                Some(_),
            ) => {
                return Err(ModuleDefinitionError::StaticArtifactSettingsSchema {
                    slug: definition.slug,
                });
            }
            (ModuleDefinitionSource::Artifact { .. }, Some(digest)) => {
                let schema_is_admitted = definition
                    .settings_schema()
                    .is_some_and(|schema| canonical_schema_digest(schema) == *digest);
                if !schema_is_admitted {
                    return Err(ModuleDefinitionError::ArtifactSettingsSchemaNotAdmitted {
                        slug: definition.slug,
                    });
                }
            }
            (_, None) => {}
        }
        if let Some(existing) = self.definitions.get(&definition.slug) {
            return Err(ModuleDefinitionError::AmbiguousActiveDefinition {
                slug: definition.slug,
                existing: Box::new(existing.source.clone()),
                incoming: Box::new(definition.source),
            });
        }
        self.definitions.insert(definition.slug.clone(), definition);
        Ok(())
    }

    pub fn get(&self, slug: &str) -> Option<&ModuleDefinition> {
        self.definitions.get(slug)
    }

    /// Binds compiled promoted implementations to the exact verified
    /// distribution and registry release that produced the running binary.
    /// Platform-native definitions absent from the promotion selection retain
    /// their platform identity.
    pub fn from_static_distribution(
        registry: &ModuleRegistry,
        release: &ModuleStaticDistributionRelease,
    ) -> Result<Self, ModuleDefinitionError> {
        if release.status == ModuleStaticDistributionReleaseStatus::Revoked {
            return Err(ModuleDefinitionError::RevokedStaticDistributionRelease {
                distribution_release_id: release.distribution_release_id,
            });
        }
        let mut catalog = Self::from_static_registry(registry)?;
        for item in &release.items {
            if item.executor_mode != ModuleStaticDistributionExecutorMode::StaticNative {
                return Err(
                    ModuleDefinitionError::InvalidStaticDistributionExecutorMode {
                        slug: item.module_slug.clone(),
                    },
                );
            }
            let definition = catalog
                .definitions
                .get_mut(&item.module_slug)
                .ok_or_else(
                    || ModuleDefinitionError::MissingPromotedNativeImplementation {
                        slug: item.module_slug.clone(),
                        registry_release_id: item.release_id.clone(),
                    },
                )?;
            if definition.version != item.module_version {
                return Err(ModuleDefinitionError::PromotedNativeVersionMismatch {
                    slug: item.module_slug.clone(),
                    expected: item.module_version.clone(),
                    actual: definition.version.clone(),
                });
            }
            if !matches!(
                &definition.source,
                ModuleDefinitionSource::PlatformNative { .. }
            ) {
                return Err(ModuleDefinitionError::PromotedNativeSourceConflict {
                    slug: item.module_slug.clone(),
                });
            }
            definition.source = ModuleDefinitionSource::PromotedNative {
                distribution_version: definition.version.clone(),
                distribution_release_id: release.distribution_release_id,
                distribution_release_revision: release.release_revision,
                registry_release_id: item.release_id.clone(),
                promotion_id: item.promotion_id,
                promotion_revision: item.promotion_revision,
                distribution_artifact_digest: release.evidence.artifact_digest.clone(),
                executor_mode: item.executor_mode,
            };
        }
        Ok(catalog)
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
        existing: Box<ModuleDefinitionSource>,
        incoming: Box<ModuleDefinitionSource>,
    },
    #[error("artifact module `{slug}` selects a settings schema absent from its admitted bundle")]
    ArtifactSettingsSchemaNotAdmitted { slug: String },
    #[error("static module `{slug}` cannot select an artifact settings schema")]
    StaticArtifactSettingsSchema { slug: String },
    #[error("static distribution release `{distribution_release_id}` is revoked")]
    RevokedStaticDistributionRelease { distribution_release_id: uuid::Uuid },
    #[error(
        "promoted native module `{slug}` from registry release `{registry_release_id}` is absent from the compiled registry"
    )]
    MissingPromotedNativeImplementation {
        slug: String,
        registry_release_id: String,
    },
    #[error(
        "promoted native module `{slug}` version mismatch: expected `{expected}`, compiled `{actual}`"
    )]
    PromotedNativeVersionMismatch {
        slug: String,
        expected: String,
        actual: String,
    },
    #[error("promoted native module `{slug}` does not use the static/native executor")]
    InvalidStaticDistributionExecutorMode { slug: String },
    #[error("promoted native module `{slug}` conflicts with a non-platform native definition")]
    PromotedNativeSourceConflict { slug: String },
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{ArtifactReleaseRef, ModulesModule};

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
            ModuleDefinitionSource::PlatformNative { .. }
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

    #[test]
    fn artifact_definition_requires_its_selected_settings_schema() {
        let schema = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object"
        });
        let digest = canonical_schema_digest(&schema);
        let mut definition = ModuleDefinition::from_static_registry_module(&ModulesModule);
        definition.source = ModuleDefinitionSource::Artifact {
            release: ArtifactReleaseRef {
                slug: definition.slug.clone(),
                version: definition.version.clone(),
                digest: format!("sha256:{}", "a".repeat(64)),
            },
        };
        definition.settings_schema_digest = Some(digest.clone());

        assert!(matches!(
            ModuleDefinitionCatalog::default().insert(definition.clone()),
            Err(ModuleDefinitionError::ArtifactSettingsSchemaNotAdmitted { .. })
        ));

        definition.schema_documents.push(ArtifactSchemaDocument {
            digest,
            document: schema,
        });
        ModuleDefinitionCatalog::default()
            .insert(definition)
            .expect("admitted settings schema");
    }
}
