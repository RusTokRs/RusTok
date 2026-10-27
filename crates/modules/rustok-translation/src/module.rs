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
use rustok_api::{Action, Permission, Resource};
use rustok_core::{
    HealthStatus, MigrationDependencyDescriptor, MigrationSource, ModuleKind,
    ModuleRuntimeExtensions, RusToKModule,
};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TranslationModule;

impl TranslationModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for TranslationModule {
    fn slug(&self) -> &'static str {
        "translation"
    }

    fn name(&self) -> &'static str {
        "Translation"
    }

    fn description(&self) -> &'static str {
        "Owner-safe translation workflow, inventory, memory, glossary, and machine-translation control plane"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Optional
    }

    fn dependencies(&self) -> &[&'static str] {
        &[]
    }

    fn permissions(&self) -> Vec<Permission> {
        let workflow_actions = [
            Action::Create,
            Action::Read,
            Action::Update,
            Action::List,
            Action::Resolve,
            Action::Publish,
            Action::Import,
            Action::Export,
            Action::Run,
            Action::Manage,
        ];
        let library_actions = [
            Action::Create,
            Action::Read,
            Action::Update,
            Action::Delete,
            Action::List,
            Action::Import,
            Action::Export,
            Action::Manage,
        ];
        workflow_actions
            .into_iter()
            .map(|action| Permission::new(Resource::Translations, action))
            .chain(library_actions.into_iter().flat_map(|action| {
                [
                    Permission::new(Resource::TranslationMemory, action),
                    Permission::new(Resource::TranslationGlossaries, action),
                ]
            }))
            .collect()
    }

    fn register_runtime_extensions(
        &self,
        _extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        #[cfg(feature = "runtime")]
        {
            let registrations = _extensions
                .get_or_insert_with::<rustok_runtime::ModuleWorkRegistrations, _>(Default::default);
            registrations.register(std::sync::Arc::new(
                crate::scheduler::TranslationMemoryRetentionWorkRegistration,
            ));
            registrations.register(std::sync::Arc::new(
                crate::scheduler::TranslationInterchangeArtifactExpiryWorkRegistration,
            ));
        }
        Ok(())
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}

impl MigrationSource for TranslationModule {
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

    #[test]
    fn test_translation_module_metadata() {
        let module = TranslationModule::new();
        assert_eq!(module.slug(), "translation");
        assert_eq!(module.name(), "Translation");
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert!(module.dependencies().is_empty());
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
        assert_eq!(module, TranslationModule::default());
        assert_eq!(module, TranslationModule);
    }

    #[tokio::test]
    async fn test_translation_module_health() {
        let module = TranslationModule;
        assert_eq!(module.health().await, HealthStatus::Healthy);
    }

    #[test]
    fn test_translation_module_permissions() {
        let module = TranslationModule;
        let perms = module.permissions();
        assert!(perms.contains(&Permission::new(Resource::Translations, Action::Create)));
        assert!(perms.contains(&Permission::new(Resource::TranslationMemory, Action::Read)));
        assert!(perms.contains(&Permission::new(
            Resource::TranslationGlossaries,
            Action::Manage
        )));
    }
}
