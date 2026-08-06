use std::sync::Arc;

use async_trait::async_trait;
use rustok_core::{ModuleRuntimeExtensions, RusToKModule};
use rustok_reactions_api::{
    ReactionSubjectRegistry, ReactionSubjectRegistryEntry,
    ensure_reaction_subject_factory_registry, ensure_reaction_subject_registry,
    reaction_subject_registry_from_extensions,
};

pub use rustok_reactions_api as api;

#[derive(Clone)]
pub struct ReactionsService {
    subjects: Arc<ReactionSubjectRegistry>,
}

impl ReactionsService {
    pub fn from_runtime_extensions(extensions: &ModuleRuntimeExtensions) -> Self {
        let subjects = reaction_subject_registry_from_extensions(extensions)
            .unwrap_or_else(|| Arc::new(ReactionSubjectRegistry::default()));
        Self { subjects }
    }

    pub fn subject_sources(&self) -> Vec<ReactionSubjectRegistryEntry> {
        self.subjects.entries()
    }

    pub fn subject_source_count(&self) -> usize {
        self.subjects.len()
    }

    pub fn has_subject_sources(&self) -> bool {
        !self.subjects.is_empty()
    }
}

pub struct ReactionsModule;

#[async_trait]
impl RusToKModule for ReactionsModule {
    fn slug(&self) -> &'static str {
        "reactions"
    }

    fn name(&self) -> &'static str {
        "Reactions"
    }

    fn description(&self) -> &'static str {
        "Shared bounded reactions with source-owned subject authorization"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["outbox"]
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        let _ = ensure_reaction_subject_registry(extensions);
        let _ = ensure_reaction_subject_factory_registry(extensions);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rustok_core::{ModuleRuntimeExtensions, RusToKModule};
    use rustok_reactions_api::{
        reaction_subject_factory_registry_from_extensions,
        reaction_subject_registry_from_extensions,
    };

    use super::{ReactionsModule, ReactionsService};

    #[test]
    fn module_initializes_empty_subject_registries() {
        let module = ReactionsModule;
        assert_eq!(module.slug(), "reactions");
        assert_eq!(module.dependencies(), &["outbox"]);

        let mut extensions = ModuleRuntimeExtensions::default();
        module
            .register_runtime_extensions(&mut extensions)
            .expect("reaction runtime extensions should initialize");

        assert!(reaction_subject_registry_from_extensions(&extensions).is_some());
        assert!(reaction_subject_factory_registry_from_extensions(&extensions).is_some());

        let service = ReactionsService::from_runtime_extensions(&extensions);
        assert_eq!(service.subject_source_count(), 0);
        assert!(!service.has_subject_sources());
    }
}
