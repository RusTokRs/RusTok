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
//!     body: "Привет, мир!".to_string(),
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
use rustok_core::permissions::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod controllers;
pub mod dto;
pub mod entities;
pub mod error;
pub mod graphql;
pub mod locale;
pub mod migrations;
pub mod services;
pub mod state_machine;

#[cfg(test)]
mod state_machine_proptest;

pub use dto::{
    CategoryListItem, CategoryResponse, CommentListItem, CommentResponse, CreateCategoryInput,
    CreateCommentInput, CreatePostInput, CreateTagInput, ListCategoriesFilter, ListCommentsFilter,
    ListTagsFilter, PostListQuery, PostListResponse, PostResponse, PostSummary, TagListItem,
    TagResponse, UpdateCategoryInput, UpdateCommentInput, UpdatePostInput, UpdateTagInput,
};
pub use entities::*;
pub use error::{BlogError, BlogResult};
pub use graphql::{BlogMutation, BlogQuery};
pub use services::{CategoryService, CommentService, PostService, TagService};
pub use state_machine::{
    Archived, BlogPost, BlogPostStatus, CommentStatus, Draft, Published, ToBlogPostStatus,
};

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
        &["content", "comments", "taxonomy"]
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
        ]
    }
}

impl MigrationSource for BlogModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_core::permissions::{Action, Resource};

    #[test]
    fn module_metadata() {
        let module = BlogModule;
        assert_eq!(module.slug(), "blog");
        assert_eq!(module.name(), "Blog");
        assert_eq!(module.description(), "Posts, Comments, Categories, Tags");
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
        assert_eq!(module.dependencies(), &["content", "comments", "taxonomy"]);
    }

    #[test]
    fn module_permissions() {
        let module = BlogModule;
        let permissions = module.permissions();

        assert!(permissions
            .iter()
            .any(|p| { p.resource == Resource::BlogPosts && p.action == Action::Create }));
        assert!(permissions
            .iter()
            .any(|p| { p.resource == Resource::BlogPosts && p.action == Action::Publish }));
        assert!(permissions
            .iter()
            .any(|p| { p.resource == Resource::BlogPosts && p.action == Action::Manage }));
    }

    #[test]
    fn module_has_owned_migrations() {
        let module = BlogModule;
        assert!(!module.migrations().is_empty());
    }
}

#[cfg(test)]
mod contract_tests;
