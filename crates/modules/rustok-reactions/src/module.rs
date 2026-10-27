/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use async_trait::async_trait;
use rustok_core::{
    HealthStatus, MigrationDependencyDescriptor, MigrationSource, ModuleEventListenerContext,
    ModuleEventListenerRegistry, ModuleKind, ModuleRuntimeExtensions, RusToKModule,
};
use sea_orm_migration::MigrationTrait;
use std::sync::Arc;

use crate::migrations;
use crate::provider::{
    ReactionSubjectRegistry, ensure_reaction_subject_factory_registry,
    ensure_reaction_subject_registry, reaction_subject_registry_from_extensions,
};
use crate::target_lifecycle;

/// Shared bounded reactions with source-owned subject authorization.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReactionsModule;

impl ReactionsModule {
    pub fn new() -> Self {
        Self
    }
}

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

    fn kind(&self) -> ModuleKind {
        ModuleKind::Optional
    }

    fn dependencies(&self) -> &[&'static str] {
        &["outbox"]
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn register_event_listeners(
        &self,
        registry: &mut ModuleEventListenerRegistry,
        ctx: &ModuleEventListenerContext<'_>,
    ) {
        let subject_registry = reaction_subject_registry_from_extensions(ctx.extensions)
            .unwrap_or_else(|| Arc::new(ReactionSubjectRegistry::default()));
        registry.register(target_lifecycle::ReactionTargetDeletionHandler::new(
            ctx.db.clone(),
            subject_registry,
        ));
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

impl MigrationSource for ReactionsModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::reaction_subject_factory_registry_from_extensions;

    #[test]
    fn test_reactions_module_metadata() {
        let module = ReactionsModule;
        assert_eq!(module.slug(), "reactions");
        assert_eq!(module.name(), "Reactions");
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.dependencies(), &["outbox"]);
        assert_eq!(module.migrations().len(), 1);
        assert_eq!(module.migration_dependencies().len(), 1);
    }

    #[tokio::test]
    async fn test_reactions_module_health() {
        let module = ReactionsModule;
        assert_eq!(module.health().await, HealthStatus::Healthy);
    }

    #[test]
    fn module_initializes_registries_and_declares_owner_schema() {
        let module = ReactionsModule;
        let mut extensions = ModuleRuntimeExtensions::default();
        module
            .register_runtime_extensions(&mut extensions)
            .expect("reaction runtime extensions should initialize");

        assert!(reaction_subject_registry_from_extensions(&extensions).is_some());
        assert!(reaction_subject_factory_registry_from_extensions(&extensions).is_some());
    }
}
