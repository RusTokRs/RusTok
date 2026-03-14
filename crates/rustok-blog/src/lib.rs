//! Blog module for RusToK platform
//!
//! This module provides blog functionality built on top of the content module.
//! It implements posts, comments, categories, and tags with proper state management.
//!
//! # Architecture
//!
//! The blog module is a "wrapper" module that:
//! - Uses `rustok-content` tables for storage (no own database schema)
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
use rustok_core::permissions::{Action, Permission, Resource};
use rustok_core::RusToKModule;

pub mod dto;
pub mod entities;
pub mod error;
pub mod locale;
pub mod services;
pub mod state_machine;

#[cfg(test)]
mod state_machine_proptest;

pub use dto::{
    CreatePostInput, PostListQuery, PostListResponse, PostResponse, PostSummary, UpdatePostInput,
};
pub use error::{BlogError, BlogResult};
pub use services::{CommentService, PostService};
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
        &["content"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::new(Resource::Posts, Action::Create),
            Permission::new(Resource::Posts, Action::Read),
            Permission::new(Resource::Posts, Action::Update),
            Permission::new(Resource::Posts, Action::Delete),
            Permission::new(Resource::Posts, Action::List),
            Permission::new(Resource::Posts, Action::Publish),
            Permission::new(Resource::Comments, Action::Create),
            Permission::new(Resource::Comments, Action::Read),
            Permission::new(Resource::Comments, Action::Update),
            Permission::new(Resource::Comments, Action::Delete),
            Permission::new(Resource::Comments, Action::List),
            Permission::new(Resource::Comments, Action::Moderate),
            Permission::new(Resource::Categories, Action::Create),
            Permission::new(Resource::Categories, Action::Read),
            Permission::new(Resource::Categories, Action::Update),
            Permission::new(Resource::Categories, Action::Delete),
            Permission::new(Resource::Categories, Action::List),
            Permission::new(Resource::Tags, Action::Create),
            Permission::new(Resource::Tags, Action::Read),
            Permission::new(Resource::Tags, Action::Update),
            Permission::new(Resource::Tags, Action::Delete),
            Permission::new(Resource::Tags, Action::List),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_metadata() {
        let module = BlogModule;
        assert_eq!(module.slug(), "blog");
        assert_eq!(module.name(), "Blog");
        assert_eq!(module.description(), "Posts, Comments, Categories, Tags");
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn module_permissions() {
        let module = BlogModule;
        let permissions = module.permissions();

        assert!(permissions
            .iter()
            .any(|p| { p.resource == Resource::Posts && p.action == Action::Create }));
        assert!(permissions
            .iter()
            .any(|p| { p.resource == Resource::Posts && p.action == Action::Publish }));
        assert!(permissions
            .iter()
            .any(|p| { p.resource == Resource::Comments && p.action == Action::Moderate }));
        assert!(permissions
            .iter()
            .any(|p| p.resource == Resource::Categories));
        assert!(permissions.iter().any(|p| p.resource == Resource::Tags));
    }

    #[test]
    fn module_migrations_empty() {
        let module = BlogModule;
        assert!(module.migrations().is_empty());
    }
}

#[cfg(test)]
mod contract_tests;
