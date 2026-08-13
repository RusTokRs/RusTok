//! Blog module for RusToK platform
//!
//! This module provides blog functionality built on top of blog-owned storage and `rustok-comments`.
//! It implements posts, comments, categories, and tags with proper state management.
//!
//! # Architecture
//!
//! The blog module is currently a bounded-context module that:
//! - Uses module-owned tables for posts, categories, and post-tag relations
//! - Uses `rustok-comments` for comment storage and lifecycle
//! - Uses `rustok-taxonomy` as the shared vocabulary dictionary behind blog tags
//! - Adds blog-specific business logic and validation
//! - Provides a type-safe state machine for post lifecycle
//! - Publishes blog-specific domain events
//! - Full i18n support with locale fallback chain: requested → en → first available
//!
//! # Example
//!
//! ```rust,ignore
//! use rustok_blog::{PostService, CreatePostInput};
//!
//! let service = PostService::new(db, event_bus);
//!
//! let input = CreatePostInput {
//!     locale: "ru".to_string(),
//!     title: "Мой первый пост".to_string(),
//!     content: rustok_api::RichTextDocument::single_paragraph("Привет, мир!"),
//!     excerpt: Some("Введение".to_string()),
//!     slug: Some("my-first-post".to_string()),
//!     publish: false,
//!     tags: vec!["rust".to_string()],
//!     category_id: None,
//!     featured_image_url: None,
//!     seo_title: None,
//!     seo_description: None,
//!     metadata: None,
//! };
//!
//! let post_id = service.create_post(tenant_id, security, input).await?;
//! ```

use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{
    MigrationSource, ModuleEventListenerContext, ModuleEventListenerRegistry,
    ModuleRuntimeExtensions, RusToKModule,
};
use rustok_reactions_api::register_reaction_subject_provider_factory;
use rustok_seo_targets::register_seo_target_provider;
use sea_orm_migration::MigrationTrait;

pub mod controllers;
pub mod dto;
pub mod entities;
pub mod error;
pub mod graphql;
pub mod migrations;
pub mod openapi;
pub mod public_comments_snapshot;
mod reaction_subject;
pub mod richtext;
mod seo_targets;
pub mod services;
pub mod state_machine;
mod translation_evidence;
pub mod translation_target;

#[cfg(test)]
mod state_machine_proptest;

#[cfg(test)]
mod tag_tenant_integrity_tests;

#[cfg(test)]
mod translation_target_tests;

pub use dto::{
    CategoryListItem, CategoryListResponse, CategoryResponse, CommentListItem, CommentResponse,
    CreateCategoryInput, CreateCommentInput, CreatePostInput, CreateTagInput, ListCategoriesFilter,
    ListCommentsFilter, ListTagsFilter, ModerateCommentInput, ModerateCommentStatus, PostListQuery,
    PostListResponse, PostResponse, PostSummary, TagListItem, TagResponse, UpdateCategoryInput,
    UpdateCommentInput, UpdatePostInput, UpdateTagInput,
};
pub use entities::*;
pub use error::{BlogError, BlogResult};
pub use graphql::{BlogMutation, BlogQuery};
pub use public_comments_snapshot::{
    PublicCommentsAvailability, PublicCommentsRead, PublicCommentsSnapshotStore,
    list_public_comments_with_snapshot,
};
pub use reaction_subject::{
    BLOG_POST_REACTION_KIND, BLOG_REACTION_SOURCE, BLOG_REACTION_V1_KEY,
    BlogReactionSubjectProviderFactory,
};
pub use rustok_comments::CommentsThreadPort;
pub use services::{CategoryService, CommentService, PostService, TagService};
pub use state_machine::{
    Archived, BlogPost, BlogPostStatus, CommentStatus, Draft, Published, ToBlogPostStatus,
};
pub use translation_target::BlogCategoryTranslationTargetProvider;

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

#[cfg(test)]
mod contract_tests;
