//! Artifact-aware module identity, separate from compiled implementation handles.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use rustok_api::{
    ArtifactPermissionLocalization, is_valid_locale_tag, is_valid_module_slug,
    manifest_hash::canonical_json_bytes,
};
use rustok_core::{ModuleKind, ModuleRegistry};
use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::artifact::{canonical_schema_digest, parse_module_version_requirement};
use crate::trust::is_sha256_digest;
use crate::{
    ArtifactPermissionDescriptor, ArtifactReleaseRef, ArtifactSchemaDocument,
    ArtifactUiContribution, ModuleArtifactDescriptor, ModuleDependencyConstraint,
    ModuleRuntimeBinding, ModuleRuntimeBindingKind, ModuleStaticDistributionExecutorMode,
    ModuleStaticDistributionRelease, ModuleStaticDistributionReleaseStatus,
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
        distribution_bundle_root_digest: String,
        distribution_role_set_digest: String,
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
    ///
    /// Static topology carries no versioned dependency constraints
    /// (`RusToKModule::dependencies` yields slugs only): the requirement is
    /// the explicit unconstrained `"*"`, which is accurate for modules
    /// compiled into one binary — their versions move together and cannot
    /// be pinned independently. Static permission localizations likewise
    /// repeat the permission key as label/description because compiled
    /// modules declare keys only; human-readable text for static modules is
    /// owned by the host/RBAC catalog projections, not this descriptor
    /// shape. `distribution_version` repeats the module version: a
    /// platform-native module has no distribution distinct from the
    /// platform build that compiled it. Promoted definitions are rebound
    /// to real distribution facts by `from_static_distribution`.
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
    ///
    /// Artifact-sourced definitions are always `Optional`: `Core` status is
    /// granted only by platform composition, never by an artifact payload.
    /// Descriptor validation rejects `ArtifactModuleKind::Core` outright;
    /// this adapter additionally refuses to ever emit a Core definition so
    /// that a descriptor which bypassed validation still cannot escalate.
    pub fn from_artifact_descriptor(descriptor: &ModuleArtifactDescriptor) -> Self {
        Self {
            slug: descriptor.slug.clone(),
            version: descriptor.version.clone(),
            kind: ModuleDefinitionKind::Optional,
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
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ModuleDefinitionCatalog {
    definitions: BTreeMap<String, ModuleDefinition>,
    /// Cached canonical JSON bytes of the serialized catalog (the
    /// `{"definitions":{…}}` envelope). The catalog is shared behind an
    /// `Arc` by the lifecycle owner and treated as immutable once built;
    /// effective-policy revision hashing splices these bytes instead of
    /// re-serializing and re-canonicalizing every schema document on each
    /// resolve. The cache is a pure function of `definitions`, is never
    /// serialized, and is invalidated by every mutation path.
    #[serde(skip)]
    canonical_json_cache: Mutex<Option<Arc<Vec<u8>>>>,
}

impl Clone for ModuleDefinitionCatalog {
    fn clone(&self) -> Self {
        Self {
            definitions: self.definitions.clone(),
            canonical_json_cache: Mutex::new(
                self.canonical_json_cache
                    .lock()
                    .ok()
                    .and_then(|cache| (*cache).clone()),
            ),
        }
    }
}

impl PartialEq for ModuleDefinitionCatalog {
    fn eq(&self, other: &Self) -> bool {
        self.definitions == other.definitions
    }
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
        validate_definition(&definition)?;
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
        self.ensure_dependency_graph_acyclic(&definition)?;
        self.definitions.insert(definition.slug.clone(), definition);
        self.invalidate_canonical_json_cache();
        Ok(())
    }

    pub fn get(&self, slug: &str) -> Option<&ModuleDefinition> {
        self.definitions.get(slug)
    }

    /// Canonical compact-JSON bytes of this catalog exactly as
    /// [`canonical_json_bytes`] produces them, computed once and then
    /// served from the interior cache. A poisoned lock degrades to
    /// recomputation; the cached bytes are a pure function of the
    /// definitions, so a lost cache entry can never change results.
    pub(crate) fn canonical_json_bytes_cached(&self) -> Result<Arc<Vec<u8>>, serde_json::Error> {
        if let Ok(cache) = self.canonical_json_cache.lock()
            && let Some(bytes) = cache.as_ref()
        {
            return Ok(Arc::clone(bytes));
        }
        let bytes = Arc::new(canonical_json_bytes(self)?);
        if let Ok(mut cache) = self.canonical_json_cache.lock() {
            *cache = Some(Arc::clone(&bytes));
        }
        Ok(bytes)
    }

    fn invalidate_canonical_json_cache(&mut self) {
        if let Ok(mut cache) = self.canonical_json_cache.lock() {
            *cache = None;
        }
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
                distribution_bundle_root_digest: release.evidence.bundle_root_digest.clone(),
                distribution_role_set_digest: release.evidence.role_set_digest.clone(),
                executor_mode: item.executor_mode,
            };
        }
        for definition in catalog.definitions.values() {
            validate_definition(definition)?;
        }
        // Sources were rebound in place above; the canonical-bytes cache
        // (still empty on this construction path) must not survive a
        // mutation that bypassed `insert`.
        catalog.invalidate_canonical_json_cache();
        Ok(catalog)
    }

    pub fn definitions(&self) -> impl Iterator<Item = &ModuleDefinition> {
        self.definitions.values()
    }

    fn ensure_dependency_graph_acyclic(
        &self,
        candidate: &ModuleDefinition,
    ) -> Result<(), ModuleDefinitionError> {
        let mut graph = self
            .definitions
            .values()
            .map(|definition| {
                (
                    definition.slug.clone(),
                    definition
                        .dependencies
                        .iter()
                        .map(|dependency| dependency.slug.clone())
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        graph.insert(
            candidate.slug.clone(),
            candidate
                .dependencies
                .iter()
                .map(|dependency| dependency.slug.clone())
                .collect(),
        );

        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Mark {
            Visiting,
            Done,
        }

        let mut marks = BTreeMap::<&str, Mark>::new();
        for root in graph.keys() {
            if marks.get(root.as_str()) == Some(&Mark::Done) {
                continue;
            }
            let mut stack = vec![(root.as_str(), 0_usize)];
            marks.insert(root.as_str(), Mark::Visiting);
            while let Some(frame) = stack.last_mut() {
                let slug = frame.0;
                let dependencies = &graph[slug];
                if frame.1 < dependencies.len() {
                    let dependency = dependencies[frame.1].as_str();
                    frame.1 += 1;
                    if !graph.contains_key(dependency) {
                        continue;
                    }
                    match marks.get(dependency) {
                        Some(Mark::Visiting) => {
                            return Err(ModuleDefinitionError::DependencyCycle {
                                slug: dependency.to_string(),
                            });
                        }
                        Some(Mark::Done) => {}
                        None => {
                            marks.insert(dependency, Mark::Visiting);
                            stack.push((dependency, 0));
                        }
                    }
                } else {
                    marks.insert(slug, Mark::Done);
                    stack.pop();
                }
            }
        }
        Ok(())
    }
}

fn validate_definition(definition: &ModuleDefinition) -> Result<(), ModuleDefinitionError> {
    if definition.slug.trim().is_empty() {
        return Err(ModuleDefinitionError::EmptySlug);
    }
    if !is_valid_module_slug(&definition.slug) {
        return Err(ModuleDefinitionError::InvalidSlug {
            slug: definition.slug.clone(),
        });
    }
    Version::parse(&definition.version).map_err(|_| ModuleDefinitionError::InvalidVersion {
        slug: definition.slug.clone(),
        version: definition.version.clone(),
    })?;

    match &definition.source {
        ModuleDefinitionSource::PlatformNative {
            distribution_version,
        } => {
            Version::parse(distribution_version).map_err(|_| {
                ModuleDefinitionError::InvalidVersion {
                    slug: definition.slug.clone(),
                    version: distribution_version.clone(),
                }
            })?;
        }
        ModuleDefinitionSource::PromotedNative {
            distribution_version,
            distribution_release_id,
            distribution_release_revision,
            registry_release_id,
            promotion_id,
            promotion_revision,
            distribution_bundle_root_digest,
            distribution_role_set_digest,
            executor_mode,
        } => {
            Version::parse(distribution_version).map_err(|_| {
                ModuleDefinitionError::InvalidVersion {
                    slug: definition.slug.clone(),
                    version: distribution_version.clone(),
                }
            })?;
            if distribution_release_id.is_nil()
                || *distribution_release_revision == 0
                || registry_release_id.trim().is_empty()
                || registry_release_id.trim() != registry_release_id
                || promotion_id.is_nil()
                || *promotion_revision == 0
                || !is_sha256_digest(distribution_bundle_root_digest)
                || !is_sha256_digest(distribution_role_set_digest)
                || *executor_mode != ModuleStaticDistributionExecutorMode::StaticNative
            {
                return Err(ModuleDefinitionError::InvalidSourceIdentity {
                    slug: definition.slug.clone(),
                });
            }
        }
        ModuleDefinitionSource::Artifact { release } => {
            if definition.kind != ModuleDefinitionKind::Optional
                || release.validate().is_err()
                || release.slug != definition.slug
                || release.version != definition.version
            {
                return Err(ModuleDefinitionError::InvalidSourceIdentity {
                    slug: definition.slug.clone(),
                });
            }
        }
    }

    let mut dependency_slugs = BTreeSet::new();
    for dependency in &definition.dependencies {
        if !is_valid_module_slug(&dependency.slug) {
            return Err(ModuleDefinitionError::InvalidDependency {
                slug: definition.slug.clone(),
                dependency: dependency.slug.clone(),
            });
        }
        if dependency.slug == definition.slug {
            return Err(ModuleDefinitionError::SelfDependency {
                slug: definition.slug.clone(),
            });
        }
        if !dependency_slugs.insert(dependency.slug.as_str()) {
            return Err(ModuleDefinitionError::DuplicateDependency {
                slug: definition.slug.clone(),
                dependency: dependency.slug.clone(),
            });
        }
        parse_module_version_requirement(&dependency.version_requirement).map_err(|_| {
            ModuleDefinitionError::InvalidDependencyVersionRequirement {
                slug: definition.slug.clone(),
                dependency: dependency.slug.clone(),
                requirement: dependency.version_requirement.clone(),
            }
        })?;
    }

    let mut permission_keys = BTreeSet::new();
    for permission in &definition.permissions {
        if permission.key.trim().is_empty()
            || permission.key.trim() != permission.key
            || permission.localizations.is_empty()
            || permission.localizations.iter().any(|localization| {
                !is_valid_locale_tag(&localization.locale)
                    || localization.label.trim().is_empty()
                    || localization.description.trim().is_empty()
            })
            || permission
                .localizations
                .iter()
                .enumerate()
                .any(|(index, localization)| {
                    permission.localizations[..index]
                        .iter()
                        .any(|previous| previous.locale == localization.locale)
                })
        {
            return Err(ModuleDefinitionError::InvalidPermission {
                slug: definition.slug.clone(),
                permission: permission.key.clone(),
            });
        }
        if !permission_keys.insert(permission.key.as_str()) {
            return Err(ModuleDefinitionError::DuplicatePermission {
                slug: definition.slug.clone(),
                permission: permission.key.clone(),
            });
        }
    }

    if let Some(digest) = &definition.settings_schema_digest
        && !is_sha256_digest(digest)
    {
        return Err(ModuleDefinitionError::InvalidSchemaDocument {
            slug: definition.slug.clone(),
            digest: digest.clone(),
        });
    }
    let mut schema_digests = BTreeSet::new();
    for schema in &definition.schema_documents {
        if !is_sha256_digest(&schema.digest)
            || canonical_schema_digest(&schema.document) != schema.digest
        {
            return Err(ModuleDefinitionError::InvalidSchemaDocument {
                slug: definition.slug.clone(),
                digest: schema.digest.clone(),
            });
        }
        if !schema_digests.insert(schema.digest.as_str()) {
            return Err(ModuleDefinitionError::DuplicateSchemaDocument {
                slug: definition.slug.clone(),
                digest: schema.digest.clone(),
            });
        }
    }

    let mut binding_ids = BTreeSet::new();
    for (index, binding) in definition.bindings.iter().enumerate() {
        let invalid_binding = || ModuleDefinitionError::InvalidBinding {
            slug: definition.slug.clone(),
            binding: binding.id.clone(),
        };
        if binding.id.trim().is_empty()
            || binding.id.trim() != binding.id
            || binding.entrypoint.trim().is_empty()
            || binding.permission.trim().is_empty()
            || binding.limit_profile.trim().is_empty()
            || !is_sha256_digest(&binding.input_schema_digest)
            || !is_sha256_digest(&binding.output_schema_digest)
            || (binding.kind == ModuleRuntimeBindingKind::Http) != binding.http.is_some()
            || (binding.kind == ModuleRuntimeBindingKind::Schedule) != binding.schedule.is_some()
        {
            return Err(invalid_binding());
        }
        if !binding_ids.insert(binding.id.as_str()) {
            return Err(ModuleDefinitionError::DuplicateBinding {
                slug: definition.slug.clone(),
                binding: binding.id.clone(),
            });
        }
        if matches!(
            binding.kind,
            ModuleRuntimeBindingKind::PreEnable
                | ModuleRuntimeBindingKind::PostEnable
                | ModuleRuntimeBindingKind::PreDisable
                | ModuleRuntimeBindingKind::PostDisable
        ) && definition.bindings[..index]
            .iter()
            .any(|previous| previous.kind == binding.kind)
        {
            return Err(invalid_binding());
        }
        if let Some(http) = &binding.http
            && definition.bindings[..index].iter().any(|previous| {
                previous.http.as_ref().is_some_and(|previous_http| {
                    previous_http.method == http.method && previous_http.path == http.path
                })
            })
        {
            return Err(invalid_binding());
        }
    }

    let mut capabilities = BTreeSet::new();
    for capability in &definition.capabilities {
        if capability.trim().is_empty() || capability.trim() != capability {
            return Err(ModuleDefinitionError::InvalidCapability {
                slug: definition.slug.clone(),
                capability: capability.clone(),
            });
        }
        if !capabilities.insert(capability.as_str()) {
            return Err(ModuleDefinitionError::DuplicateCapability {
                slug: definition.slug.clone(),
                capability: capability.clone(),
            });
        }
    }

    Ok(())
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
    #[error(
        "module `{slug}` dependency `{dependency}` has invalid version requirement `{requirement}`"
    )]
    InvalidDependencyVersionRequirement {
        slug: String,
        dependency: String,
        requirement: String,
    },
    #[error("module definition slug `{slug}` is invalid")]
    InvalidSlug { slug: String },
    #[error("module `{slug}` has invalid version `{version}`")]
    InvalidVersion { slug: String, version: String },
    #[error("module `{slug}` source identity is inconsistent with its definition")]
    InvalidSourceIdentity { slug: String },
    #[error("module `{slug}` declares invalid dependency `{dependency}`")]
    InvalidDependency { slug: String, dependency: String },
    #[error("module `{slug}` depends on itself")]
    SelfDependency { slug: String },
    #[error("module `{slug}` declares dependency `{dependency}` more than once")]
    DuplicateDependency { slug: String, dependency: String },
    #[error("module dependency graph contains a cycle at `{slug}`")]
    DependencyCycle { slug: String },
    #[error("module `{slug}` declares invalid permission `{permission}`")]
    InvalidPermission { slug: String, permission: String },
    #[error("module `{slug}` declares permission `{permission}` more than once")]
    DuplicatePermission { slug: String, permission: String },
    #[error("module `{slug}` declares invalid schema document `{digest}`")]
    InvalidSchemaDocument { slug: String, digest: String },
    #[error("module `{slug}` declares schema document `{digest}` more than once")]
    DuplicateSchemaDocument { slug: String, digest: String },
    #[error("module `{slug}` declares invalid binding `{binding}`")]
    InvalidBinding { slug: String, binding: String },
    #[error("module `{slug}` declares binding `{binding}` more than once")]
    DuplicateBinding { slug: String, binding: String },
    #[error("module `{slug}` declares invalid capability `{capability}`")]
    InvalidCapability { slug: String, capability: String },
    #[error("module `{slug}` declares capability `{capability}` more than once")]
    DuplicateCapability { slug: String, capability: String },
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
    fn catalog_rejects_empty_dependency_version_requirement() {
        let mut definition = ModuleDefinition::from_static_registry_module(&ModulesModule);
        definition.dependencies.push(ModuleDependencyConstraint {
            slug: "commerce".to_string(),
            version_requirement: String::new(),
        });
        assert!(matches!(
            ModuleDefinitionCatalog::default().insert(definition),
            Err(ModuleDefinitionError::InvalidDependencyVersionRequirement { .. })
        ));
    }

    #[test]
    fn catalog_rejects_invalid_versions_self_dependencies_and_cycles() {
        let base = ModuleDefinition::from_static_registry_module(&ModulesModule);

        let mut invalid_version = base.clone();
        invalid_version.version = "1.0".to_string();
        assert!(matches!(
            ModuleDefinitionCatalog::default().insert(invalid_version),
            Err(ModuleDefinitionError::InvalidVersion { .. })
        ));

        let mut self_dependency = base.clone();
        self_dependency.dependencies = vec![ModuleDependencyConstraint {
            slug: self_dependency.slug.clone(),
            version_requirement: "*".to_string(),
        }];
        assert!(matches!(
            ModuleDefinitionCatalog::default().insert(self_dependency),
            Err(ModuleDefinitionError::SelfDependency { .. })
        ));

        let mut first = base.clone();
        first.slug = "cycle_a".to_string();
        first.dependencies = vec![ModuleDependencyConstraint {
            slug: "cycle_b".to_string(),
            version_requirement: "*".to_string(),
        }];
        let mut catalog = ModuleDefinitionCatalog::default();
        catalog.insert(first).expect("first cycle definition");

        let mut second = base;
        second.slug = "cycle_b".to_string();
        second.dependencies = vec![ModuleDependencyConstraint {
            slug: "cycle_a".to_string(),
            version_requirement: "*".to_string(),
        }];
        assert!(matches!(
            catalog.insert(second),
            Err(ModuleDefinitionError::DependencyCycle { .. })
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
        definition.kind = ModuleDefinitionKind::Optional;
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
