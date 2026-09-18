use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{
    MigrationSource, ModuleEventListenerContext, ModuleEventListenerRegistry,
    ModuleRuntimeExtensions, RusToKModule,
};
use rustok_reactions_api::register_reaction_subject_provider_factory;
use rustok_seo_targets::register_seo_target_provider;
use sea_orm_migration::MigrationTrait;

use crate::integrations::{reaction_subject, seo_targets};
use crate::{migrations, services};

pub struct BlogModule;

#[async_trait]
impl RusToKModule for BlogModule {
    fn slug(&self) -> &'static str {
        "blog"
    }

    fn name(&self) -> &'static str {
        "Blog"
    }

    fn description(&self) -> &'static str {
        "Posts, Comments, Categories, Tags"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["content", "comments", "taxonomy", "outbox"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::BLOG_POSTS_CREATE,
            Permission::BLOG_POSTS_READ,
            Permission::BLOG_POSTS_UPDATE,
            Permission::BLOG_POSTS_DELETE,
            Permission::BLOG_POSTS_LIST,
            Permission::BLOG_POSTS_PUBLISH,
            Permission::BLOG_POSTS_MANAGE,
            Permission::BLOG_CATEGORIES_CREATE,
            Permission::BLOG_CATEGORIES_READ,
            Permission::BLOG_CATEGORIES_UPDATE,
            Permission::BLOG_CATEGORIES_DELETE,
            Permission::BLOG_CATEGORIES_LIST,
            Permission::BLOG_CATEGORIES_MANAGE,
        ]
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        register_seo_target_provider(extensions, seo_targets::BlogSeoTargetProvider).map_err(
            |error| {
                rustok_core::Error::Validation(format!(
                    "blog SEO target registration failed: {error}"
                ))
            },
        )?;
        register_reaction_subject_provider_factory(
            extensions,
            reaction_subject::BlogReactionSubjectProviderFactory,
        )
        .map_err(|error| {
            rustok_core::Error::Validation(format!(
                "blog reaction subject factory registration failed: {error}"
            ))
        })?;
        Ok(())
    }

    fn register_event_listeners(
        &self,
        registry: &mut ModuleEventListenerRegistry,
        ctx: &ModuleEventListenerContext<'_>,
    ) {
        registry.register(services::BlogCommentProjectionHandler::new(ctx.db.clone()));
    }
}

impl MigrationSource for BlogModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::{Action, Resource};
    use rustok_events::DomainEvent;
    use rustok_test_utils::setup_test_db;
    use uuid::Uuid;

    #[test]
    fn module_metadata() {
        let module = BlogModule;
        assert_eq!(module.slug(), "blog");
        assert_eq!(module.name(), "Blog");
        assert_eq!(module.description(), "Posts, Comments, Categories, Tags");
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
        assert_eq!(
            module.dependencies(),
            &["content", "comments", "taxonomy", "outbox"]
        );
    }

    #[test]
    fn module_permissions() {
        let module = BlogModule;
        let permissions = module.permissions();

        assert!(
            permissions
                .iter()
                .any(|p| { p.resource == Resource::BlogPosts && p.action == Action::Create })
        );
        assert!(
            permissions
                .iter()
                .any(|p| { p.resource == Resource::BlogPosts && p.action == Action::Publish })
        );
        assert!(
            permissions
                .iter()
                .any(|p| { p.resource == Resource::BlogPosts && p.action == Action::Manage })
        );
        assert!(
            permissions
                .iter()
                .any(|p| { p.resource == Resource::BlogCategories && p.action == Action::Create })
        );
        assert!(
            permissions
                .iter()
                .any(|p| { p.resource == Resource::BlogCategories && p.action == Action::Manage })
        );
        assert!(
            !permissions
                .iter()
                .any(|p| p.resource == Resource::Categories)
        );
    }

    #[test]
    fn module_has_owned_migrations() {
        let module = BlogModule;
        assert!(!module.migrations().is_empty());
    }

    #[tokio::test]
    async fn module_registers_comment_projection_handler_with_host_routing() {
        let db = setup_test_db().await;
        let extensions = ModuleRuntimeExtensions::default();
        let context = ModuleEventListenerContext {
            db,
            extensions: &extensions,
        };
        let mut registry = ModuleEventListenerRegistry::new();

        BlogModule.register_event_listeners(&mut registry, &context);

        let handlers = registry.into_handlers();
        assert_eq!(handlers.len(), 1);
        let handler = handlers
            .first()
            .expect("Blog must register its Comments projection handler");
        assert_eq!(handler.name(), "blog_comment_projection");

        let blog_created = DomainEvent::CommentCreated {
            comment_id: Uuid::from_u128(1),
            target_type: "blog_post".to_string(),
            target_id: Uuid::from_u128(2),
            author_id: Uuid::from_u128(3),
        };
        let blog_deleted = DomainEvent::CommentDeleted {
            comment_id: Uuid::from_u128(4),
            target_type: "blog_post".to_string(),
            target_id: Uuid::from_u128(5),
            author_id: Uuid::from_u128(6),
        };
        let forum_created = DomainEvent::CommentCreated {
            comment_id: Uuid::from_u128(7),
            target_type: "forum_topic".to_string(),
            target_id: Uuid::from_u128(8),
            author_id: Uuid::from_u128(9),
        };

        assert!(handler.handles(&blog_created));
        assert!(handler.handles(&blog_deleted));
        assert!(!handler.handles(&forum_created));
    }
}
