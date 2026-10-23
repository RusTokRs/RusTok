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
use rustok_api::Permission;
use rustok_core::{
    HealthStatus, MigrationDependencyDescriptor, MigrationSource, ModuleKind,
    ModuleRuntimeExtensions, RusToKModule,
};
use sea_orm_migration::MigrationTrait;

use crate::{migrations, seo_targets};

pub struct TaxonomyModule;

#[async_trait]
impl RusToKModule for TaxonomyModule {
    fn slug(&self) -> &'static str {
        "taxonomy"
    }

    fn name(&self) -> &'static str {
        "Taxonomy"
    }

    fn description(&self) -> &'static str {
        "Scope-aware taxonomy dictionary for shared and module-local terms"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Optional
    }

    fn dependencies(&self) -> &[&'static str] {
        &["content", "outbox"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::TAXONOMY_CREATE,
            Permission::TAXONOMY_READ,
            Permission::TAXONOMY_UPDATE,
            Permission::TAXONOMY_DELETE,
            Permission::TAXONOMY_LIST,
            Permission::TAXONOMY_MANAGE,
        ]
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        rustok_seo_targets::register_seo_target_provider(
            extensions,
            seo_targets::TaxonomyCategorySeoTargetProvider,
        )
        .map_err(|error| {
            rustok_core::Error::Validation(format!(
                "taxonomy SEO target registration failed: {error}"
            ))
        })?;
        Ok(())
    }
}

impl MigrationSource for TaxonomyModule {
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
    use rustok_api::{Action, Resource};

    #[test]
    fn test_taxonomy_module_metadata() {
        let module = TaxonomyModule;
        assert_eq!(module.slug(), "taxonomy");
        assert_eq!(module.name(), "Taxonomy");
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.dependencies(), &["content", "outbox"]);
        assert!(!module.permissions().is_empty());
        assert!(!module.migrations().is_empty());
    }

    #[tokio::test]
    async fn test_taxonomy_module_health() {
        let module = TaxonomyModule;
        assert_eq!(module.health().await, HealthStatus::Healthy);
    }

    #[test]
    fn module_permissions_cover_term_crud() {
        let module = TaxonomyModule;
        let permissions = module.permissions();

        assert!(permissions.contains(&Permission::new(Resource::Taxonomy, Action::Create)));
        assert!(permissions.contains(&Permission::new(Resource::Taxonomy, Action::Read)));
        assert!(permissions.contains(&Permission::new(Resource::Taxonomy, Action::Update)));
        assert!(permissions.contains(&Permission::new(Resource::Taxonomy, Action::Delete)));
        assert!(permissions.contains(&Permission::new(Resource::Taxonomy, Action::List)));
        assert!(permissions.contains(&Permission::new(Resource::Taxonomy, Action::Manage)));
    }
}
