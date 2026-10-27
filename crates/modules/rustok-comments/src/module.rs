use async_trait::async_trait;
use rustok_api::{Action, Permission, Resource};
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use rustok_core::{ModuleEventListenerContext, ModuleEventListenerRegistry};
use sea_orm_migration::MigrationTrait;

use crate::{migrations, target_lifecycle};

/// Generic comments domain for blog and other opt-in non-forum discussion surfaces.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CommentsModule;

impl CommentsModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for CommentsModule {
    fn slug(&self) -> &'static str {
        "comments"
    }

    fn name(&self) -> &'static str {
        "Comments"
    }

    fn description(&self) -> &'static str {
        "Generic comments domain for blog and other opt-in non-forum discussion surfaces"
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

    fn register_event_listeners(
        &self,
        registry: &mut ModuleEventListenerRegistry,
        ctx: &ModuleEventListenerContext<'_>,
    ) {
        registry.register(target_lifecycle::CommentTargetDeletionHandler::new(
            ctx.db.clone(),
        ));
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::new(Resource::Comments, Action::Create),
            Permission::new(Resource::Comments, Action::Read),
            Permission::new(Resource::Comments, Action::Update),
            Permission::new(Resource::Comments, Action::Delete),
            Permission::new(Resource::Comments, Action::List),
            Permission::new(Resource::Comments, Action::Moderate),
            Permission::new(Resource::Comments, Action::Manage),
        ]
    }
}

impl MigrationSource for CommentsModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn comments_module_metadata_and_traits() {
        let module = CommentsModule::new();
        assert_eq!(module.slug(), "comments");
        assert_eq!(module.name(), "Comments");
        assert_eq!(
            module.description(),
            "Generic comments domain for blog and other opt-in non-forum discussion surfaces"
        );
        assert!(!module.version().is_empty());
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.health().await, HealthStatus::Healthy);
        assert!(module.dependencies().is_empty());
        assert_eq!(module.permissions().len(), 7);
        assert!(!module.migrations().is_empty());
    }

    #[test]
    fn module_permissions_cover_comment_lifecycle() {
        let module = CommentsModule::new();
        let permissions = module.permissions();

        assert!(permissions.contains(&Permission::new(Resource::Comments, Action::Create)));
        assert!(permissions.contains(&Permission::new(Resource::Comments, Action::Read)));
        assert!(permissions.contains(&Permission::new(Resource::Comments, Action::Update)));
        assert!(permissions.contains(&Permission::new(Resource::Comments, Action::Delete)));
        assert!(permissions.contains(&Permission::new(Resource::Comments, Action::List)));
        assert!(permissions.contains(&Permission::new(Resource::Comments, Action::Moderate)));
        assert!(permissions.contains(&Permission::new(Resource::Comments, Action::Manage)));
    }
}
