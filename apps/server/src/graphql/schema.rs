use async_graphql::{
    dataloader::DataLoader, extensions::Analyzer, MergedObject, MergedSubscription, Schema,
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use rustok_core::EventBus;
use rustok_outbox::TransactionalEventBus;

#[cfg(feature = "mod-alloy")]
use super::alloy::{AlloyMutation, AlloyQuery, AlloyState};
#[cfg(not(feature = "mod-alloy"))]
#[derive(Clone, Default)]
pub struct AlloyState;

use super::auth::{AuthMutation, AuthQuery};
#[cfg(feature = "mod-blog")]
use super::blog::{BlogMutation, BlogQuery};
#[cfg(feature = "mod-commerce")]
use super::commerce::{CommerceMutation, CommerceQuery};
#[cfg(feature = "mod-content")]
use super::content::{ContentMutation, ContentQuery};
#[cfg(feature = "mod-forum")]
use super::forum::{ForumMutation, ForumQuery};
use super::loaders::{NodeBodyLoader, NodeLoader, NodeTranslationLoader, TenantNameLoader};
use super::mutations::RootMutation;
use super::oauth::{OAuthMutation, OAuthQuery};
use super::observability::GraphqlObservability;
#[cfg(feature = "mod-pages")]
use super::pages::{PagesMutation, PagesQuery};
use super::queries::RootQuery;
use super::settings::{SettingsMutation, SettingsQuery};
use super::subscriptions::BuildSubscription;
use crate::services::build_event_hub::BuildEventHub;

/// Slugs used for runtime `tenant_modules.is_enabled()` guards.
pub mod module_slug {
    pub const COMMERCE: &str = "commerce";
    pub const CONTENT: &str = "content";
    pub const BLOG: &str = "blog";
    pub const FORUM: &str = "forum";
    pub const PAGES: &str = "pages";
    pub const ALLOY: &str = "alloy";
}

#[derive(MergedObject, Default)]
pub struct Query(
    RootQuery,
    AuthQuery,
    OAuthQuery,
    SettingsQuery,
    #[cfg(feature = "mod-commerce")] CommerceQuery,
    #[cfg(feature = "mod-content")]  ContentQuery,
    #[cfg(feature = "mod-blog")]     BlogQuery,
    #[cfg(feature = "mod-forum")]    ForumQuery,
    #[cfg(feature = "mod-pages")]    PagesQuery,
    #[cfg(feature = "mod-alloy")]    AlloyQuery,
);

#[derive(MergedObject, Default)]
pub struct Mutation(
    RootMutation,
    AuthMutation,
    OAuthMutation,
    SettingsMutation,
    #[cfg(feature = "mod-commerce")] CommerceMutation,
    #[cfg(feature = "mod-content")]  ContentMutation,
    #[cfg(feature = "mod-blog")]     BlogMutation,
    #[cfg(feature = "mod-forum")]    ForumMutation,
    #[cfg(feature = "mod-pages")]    PagesMutation,
    #[cfg(feature = "mod-alloy")]    AlloyMutation,
);

#[derive(MergedSubscription, Default)]
pub struct Subscription(BuildSubscription);

pub type AppSchema = Schema<Query, Mutation, Subscription>;

#[derive(Clone)]
pub struct SharedGraphqlSchema(pub Arc<AppSchema>);

pub fn build_schema(
    db: DatabaseConnection,
    event_bus: EventBus,
    transactional_event_bus: TransactionalEventBus,
    build_event_hub: Arc<BuildEventHub>,
    alloy_state: AlloyState,
) -> AppSchema {
    let builder = Schema::build(
        Query::default(),
        Mutation::default(),
        Subscription::default(),
    )
    .limit_depth(12)
    .limit_complexity(600)
    .extension(Analyzer)
    .extension(GraphqlObservability)
    // DataLoaders for efficient batched queries
    .data(DataLoader::new(
        TenantNameLoader::new(db.clone()),
        tokio::spawn,
    ))
    .data(DataLoader::new(NodeLoader::new(db.clone()), tokio::spawn))
    .data(DataLoader::new(
        NodeTranslationLoader::new(db.clone()),
        tokio::spawn,
    ))
    .data(DataLoader::new(
        NodeBodyLoader::new(db.clone()),
        tokio::spawn,
    ))
    .data(db)
    .data(event_bus)
    .data(transactional_event_bus)
    .data(build_event_hub);

    #[cfg(feature = "mod-alloy")]
    let builder = builder.data(alloy_state);
    #[cfg(not(feature = "mod-alloy"))]
    let _ = alloy_state;

    builder.finish()
}
