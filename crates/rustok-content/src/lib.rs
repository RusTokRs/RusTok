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
use rustok_api::{Action, Permission, Resource};
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod analytics;
pub mod dto;
pub mod entities;
pub mod error;
#[cfg(feature = "graphql")]
pub mod graphql;
pub mod locale;
pub mod migrations;
pub mod services;
pub mod state_machine;

#[cfg(test)]
mod state_machine_proptest;

pub use analytics::{ContentCountSnapshot, load_post_stats_snapshot};
pub use dto::*;
pub use entities::{
    Body, CanonicalUrl, Category, CategoryTranslation, Node, NodeTranslation, UrlAlias,
};
pub use error::{ContentError, ContentResult};
pub use locale::{
    ResolvedLocale, available_locales_from, normalize_locale_code, resolve_by_locale,
    resolve_by_locale_with_fallback,
};
pub use services::{
    CanonicalUrlMutation, CanonicalUrlService, CategoryService, ContentOrchestrationBridge,
    ContentOrchestrationService, DemotePostToTopicInput, DemotePostToTopicOutput, MergeTopicsInput,
    MergeTopicsOutput, OrchestrationResult, PromoteTopicToPostInput, PromoteTopicToPostOutput,
    ResolvedContentRoute, RetiredCanonicalTarget, SplitTopicInput, SplitTopicOutput,
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
        "Shared content helpers and cross-domain orchestration module"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
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
mod contract_tests;
