use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, ModuleRuntimeExtensions, RusToKModule};
use rustok_notifications_api::register_notification_source_provider_factory;
use rustok_seo_targets::register_seo_target_provider;
use sea_orm_migration::MigrationTrait;

pub mod category_presentation;
pub mod constants;
pub mod controllers;
pub mod dto;
pub mod entities;
#[allow(clippy::explicit_auto_deref)]
pub mod error;
pub mod graphql;
pub mod locale;
#[allow(clippy::collapsible_if, clippy::redundant_closure)]
pub mod mentions;
pub mod migrations;
mod notification_source;
pub mod openapi;
mod seo_targets;
pub mod services;
pub mod state_machine;
pub mod subscription;

pub use constants::*;
pub use dto::*;
pub use entities::*;
pub use error::{ForumError, ForumResult};
pub use graphql::{ForumMutation, ForumQuery};
pub use mentions::*;
pub use services::{
    CategoryService, ForumEventService, ForumQuoteCommandService, ForumReadModelService,
    ForumRelationReadService, ForumTopicReadState, ForumTopicReadStateService,
    ForumWidgetContractService, MarkForumTopicReadInput, MarkForumTopicsReadBatchInput,
    MarkForumTopicsReadBatchResult, ModerationService, ReplyService, RevisionService,
    SubscriptionService, TopicService, UserStatsService, VoteService,
};
pub use state_machine::{ReplyStatus, TopicStatus};
pub use subscription::{ForumDigestMode, ForumSubscriptionLevel, ForumSubscriptionPreferences};

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

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        register_seo_target_provider(extensions, seo_targets::ForumCategorySeoTargetProvider)
            .map_err(|error| {
                rustok_core::Error::Validation(format!(
                    "forum category SEO target registration failed: {error}"
                ))
            })?;
        register_seo_target_provider(extensions, seo_targets::ForumTopicSeoTargetProvider)
            .map_err(|error| {
                rustok_core::Error::Validation(format!(
                    "forum topic SEO target registration failed: {error}"
                ))
            })?;
        register_notification_source_provider_factory(
            extensions,
            notification_source::ForumNotificationSourceProviderFactory,
        )
        .map_err(|error| {
            rustok_core::Error::Validation(format!(
                "forum notification source factory registration failed: {error}"
            ))
        })?;
        Ok(())
    }
}

impl MigrationSource for ForumModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}

#[cfg(test)]
mod contract_tests;
