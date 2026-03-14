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

pub use dto::*;
pub use entities::{Body, Node, NodeTranslation};
pub use error::{ContentError, ContentResult};
pub use locale::{
    available_locales_from, resolve_by_locale, resolve_by_locale_with_fallback, ResolvedLocale,
    PLATFORM_FALLBACK_LOCALE,
};
pub use services::{
    ContentOrchestrationService, DemotePostToTopicInput, MergeTopicsInput, NodeService,
    OrchestrationResult, PromoteTopicToPostInput, SplitTopicInput,
};
pub use state_machine::{Archived, ContentNode, Draft, Published, ToContentStatus};

pub struct ContentModule;

#[async_trait]
impl RusToKModule for ContentModule {
    fn slug(&self) -> &'static str {
        "content"
    }

    fn name(&self) -> &'static str {
        "Content"
    }

    fn description(&self) -> &'static str {
        "Core CMS Module (Nodes, Bodies, Categories)"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            // Pages
            Permission::new(Resource::Pages, Action::Create),
            Permission::new(Resource::Pages, Action::Read),
            Permission::new(Resource::Pages, Action::Update),
            Permission::new(Resource::Pages, Action::Delete),
            Permission::new(Resource::Pages, Action::List),
            // Posts
            Permission::new(Resource::Posts, Action::Create),
            Permission::new(Resource::Posts, Action::Read),
            Permission::new(Resource::Posts, Action::Update),
            Permission::new(Resource::Posts, Action::Delete),
            Permission::new(Resource::Posts, Action::List),
            // Media
            Permission::new(Resource::Media, Action::Create),
            Permission::new(Resource::Media, Action::Read),
            Permission::new(Resource::Media, Action::Update),
            Permission::new(Resource::Media, Action::Delete),
            Permission::new(Resource::Media, Action::List),
            // Comments
            Permission::new(Resource::Comments, Action::Create),
            Permission::new(Resource::Comments, Action::Read),
            Permission::new(Resource::Comments, Action::Update),
            Permission::new(Resource::Comments, Action::Delete),
            Permission::new(Resource::Comments, Action::List),
            // Forum topics (used by orchestration runtime checks)
            Permission::new(Resource::ForumTopics, Action::Create),
            Permission::new(Resource::ForumTopics, Action::Read),
            Permission::new(Resource::ForumTopics, Action::Update),
            Permission::new(Resource::ForumTopics, Action::Delete),
            Permission::new(Resource::ForumTopics, Action::List),
            Permission::new(Resource::ForumTopics, Action::Moderate),
            // Forum categories
            Permission::new(Resource::ForumCategories, Action::Create),
            Permission::new(Resource::ForumCategories, Action::Read),
            Permission::new(Resource::ForumCategories, Action::Update),
            Permission::new(Resource::ForumCategories, Action::Delete),
            Permission::new(Resource::ForumCategories, Action::List),
            // Forum replies
            Permission::new(Resource::ForumReplies, Action::Create),
            Permission::new(Resource::ForumReplies, Action::Read),
            Permission::new(Resource::ForumReplies, Action::Update),
            Permission::new(Resource::ForumReplies, Action::Delete),
            Permission::new(Resource::ForumReplies, Action::List),
            Permission::new(Resource::ForumReplies, Action::Moderate),
            // Blog posts (used by orchestration runtime checks)
            Permission::new(Resource::BlogPosts, Action::Create),
            Permission::new(Resource::BlogPosts, Action::Read),
            Permission::new(Resource::BlogPosts, Action::Update),
            Permission::new(Resource::BlogPosts, Action::Delete),
            Permission::new(Resource::BlogPosts, Action::List),
            Permission::new(Resource::BlogPosts, Action::Moderate),
        ]
    }
}

#[cfg(test)]
mod contract_tests;
