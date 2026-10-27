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
    HealthStatus, MigrationDependencyDescriptor, MigrationSource, ModuleKind, RusToKModule,
};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProfilesModule;

impl ProfilesModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for ProfilesModule {
    fn slug(&self) -> &'static str {
        "profiles"
    }

    fn name(&self) -> &'static str {
        "Profiles"
    }

    fn description(&self) -> &'static str {
        "Universal public profile domain for platform users"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Optional
    }

    fn dependencies(&self) -> &[&'static str] {
        &["media", "social_graph", "taxonomy"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::PROFILES_CREATE,
            Permission::PROFILES_READ,
            Permission::PROFILES_UPDATE,
            Permission::PROFILES_DELETE,
            Permission::PROFILES_LIST,
            Permission::PROFILES_MANAGE,
        ]
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}

impl MigrationSource for ProfilesModule {
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
    fn test_profiles_module_metadata() {
        let module = ProfilesModule;
        assert_eq!(module.slug(), "profiles");
        assert_eq!(module.name(), "Profiles");
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.dependencies(), &["media", "social_graph", "taxonomy"]);
        assert!(!module.permissions().is_empty());
    }

    #[tokio::test]
    async fn test_profiles_module_health() {
        let module = ProfilesModule;
        assert_eq!(module.health().await, HealthStatus::Healthy);
    }

    #[test]
    fn profiles_permissions_cover_crud_and_list() {
        let permissions = ProfilesModule.permissions();
        for action in [
            Action::Create,
            Action::Read,
            Action::Update,
            Action::Delete,
            Action::List,
            Action::Manage,
        ] {
            assert!(permissions.contains(&Permission::new(Resource::Profiles, action)));
        }
    }
}
