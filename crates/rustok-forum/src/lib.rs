use async_trait::async_trait;
use rustok_core::permissions::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod constants;
pub mod controllers;
pub mod dto;
pub mod entities;
pub mod error;
pub mod graphql;
pub mod locale;
pub mod migrations;
pub mod services;
pub mod state_machine;

pub use constants::*;
pub use dto::*;
pub use entities::*;
pub use error::{ForumError, ForumResult};
pub use graphql::{ForumMutation, ForumQuery};
pub use services::{
    CategoryService, ModerationService, ReplyService, SubscriptionService, TopicService,
    UserStatsService, VoteService,
};
pub use state_machine::{ReplyStatus, TopicStatus};

pub struct ForumModule;

#[async_trait]
impl RusToKModule for ForumModule {
    fn slug(&self) -> &'static str {
        "forum"
    }

    fn name(&self) -> &'static str {
        "Forum"
    }

    fn description(&self) -> &'static str {
        "Forum categories, topics, replies, and moderation workflows"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["content", "taxonomy"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::FORUM_CATEGORIES_CREATE,
            Permission::FORUM_CATEGORIES_READ,
            Permission::FORUM_CATEGORIES_UPDATE,
            Permission::FORUM_CATEGORIES_DELETE,
            Permission::FORUM_CATEGORIES_LIST,
            Permission::FORUM_CATEGORIES_MANAGE,
            Permission::FORUM_TOPICS_CREATE,
            Permission::FORUM_TOPICS_READ,
            Permission::FORUM_TOPICS_UPDATE,
            Permission::FORUM_TOPICS_DELETE,
            Permission::FORUM_TOPICS_LIST,
            Permission::FORUM_TOPICS_MODERATE,
            Permission::FORUM_TOPICS_MANAGE,
            Permission::FORUM_REPLIES_CREATE,
            Permission::FORUM_REPLIES_READ,
            Permission::FORUM_REPLIES_UPDATE,
            Permission::FORUM_REPLIES_DELETE,
            Permission::FORUM_REPLIES_LIST,
            Permission::FORUM_REPLIES_MODERATE,
            Permission::FORUM_REPLIES_MANAGE,
        ]
    }
}

impl MigrationSource for ForumModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[cfg(test)]
mod contract_tests;
