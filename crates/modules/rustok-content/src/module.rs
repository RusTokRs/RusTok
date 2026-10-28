use async_trait::async_trait;
use rustok_api::{Action, Permission, Resource};
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

/// Shared content helpers and cross-domain orchestration module.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContentModule;

impl ContentModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for ContentModule {
    fn slug(&self) -> &'static str {
        "content"
    }

    fn name(&self) -> &'static str {
        "Content"
    }

    fn description(&self) -> &'static str {
        "Shared content helpers and cross-domain orchestration module"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Optional
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::new(Resource::ForumTopics, Action::Create),
            Permission::new(Resource::ForumTopics, Action::Read),
            Permission::new(Resource::ForumTopics, Action::Update),
            Permission::new(Resource::ForumTopics, Action::Delete),
            Permission::new(Resource::ForumTopics, Action::List),
            Permission::new(Resource::ForumTopics, Action::Moderate),
            Permission::new(Resource::BlogPosts, Action::Create),
            Permission::new(Resource::BlogPosts, Action::Read),
            Permission::new(Resource::BlogPosts, Action::Update),
            Permission::new(Resource::BlogPosts, Action::Delete),
            Permission::new(Resource::BlogPosts, Action::List),
            Permission::new(Resource::BlogPosts, Action::Moderate),
        ]
    }
}

impl MigrationSource for ContentModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn content_module_metadata_and_traits() {
        let module = ContentModule::new();
        assert_eq!(module.slug(), "content");
        assert_eq!(module.name(), "Content");
        assert_eq!(
            module.description(),
            "Shared content helpers and cross-domain orchestration module"
        );
        assert!(!module.version().is_empty());
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.health().await, HealthStatus::Healthy);
        assert_eq!(module.permissions().len(), 12);
        assert!(!module.migrations().is_empty());
    }
}
