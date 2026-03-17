use async_graphql::{
    MergedObject, MergedSubscription, Schema, dataloader::DataLoader, extensions::Analyzer,
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use rustok_core::EventBus;
use rustok_outbox::TransactionalEventBus;

#[cfg(feature = "mod-media")]
use rustok_storage::StorageService;

#[cfg(feature = "mod-alloy")]
use super::alloy::{AlloyMutation, AlloyQuery, AlloyState};
#[cfg(not(feature = "mod-alloy"))]
#[derive(Clone, Default)]
pub struct AlloyState;
#[cfg(feature = "mod-workflow")]
use super::workflow::{WorkflowMutation, WorkflowQuery};

use super::auth::{AuthMutation, AuthQuery};
#[cfg(feature = "mod-blog")]
use super::blog::{BlogMutation, BlogQuery};
#[cfg(feature = "mod-commerce")]
use super::commerce::{CommerceMutation, CommerceQuery};
#[cfg(feature = "mod-content")]
use super::content::{ContentMutation, ContentQuery};
use super::flex::{FlexMutation, FlexQuery};
#[cfg(feature = "mod-forum")]
use super::forum::{ForumMutation, ForumQuery};
use super::loaders::{NodeBodyLoader, NodeLoader, NodeTranslationLoader, TenantNameLoader};
#[cfg(feature = "mod-media")]
use super::media::{MediaMutation, MediaQuery};
use super::mutations::RootMutation;
use super::oauth::{OAuthMutation, OAuthQuery};
use super::observability::GraphqlObservability;
#[cfg(feature = "mod-pages")]
use super::pages::{PagesMutation, PagesQuery};
use super::queries::RootQuery;
use super::settings::{SettingsMutation, SettingsQuery};
use super::subscriptions::BuildSubscription;
use super::system::SystemQuery;
use crate::services::build_event_hub::BuildEventHub;
use crate::services::field_definition_cache::FieldDefinitionCache;
use crate::services::field_definition_registry_bootstrap::build_field_def_registry;

/// Slugs used for runtime `tenant_modules.is_enabled()` guards.
pub mod module_slug {
    pub const COMMERCE: &str = "commerce";
    pub const CONTENT: &str = "content";
    pub const BLOG: &str = "blog";
    pub const FORUM: &str = "forum";
    pub const PAGES: &str = "pages";
    pub const ALLOY: &str = "alloy";
    pub const MEDIA: &str = "media";
    pub const WORKFLOW: &str = "workflow";
}

#[derive(MergedObject, Default)]
pub struct Query(
    RootQuery,
    AuthQuery,
    OAuthQuery,
    SettingsQuery,
    SystemQuery,
    FlexQuery,
    #[cfg(feature = "mod-commerce")] CommerceQuery,
    #[cfg(feature = "mod-content")] ContentQuery,
    #[cfg(feature = "mod-blog")] BlogQuery,
    #[cfg(feature = "mod-forum")] ForumQuery,
    #[cfg(feature = "mod-pages")] PagesQuery,
    #[cfg(feature = "mod-alloy")] AlloyQuery,
    #[cfg(feature = "mod-media")] MediaQuery,
    #[cfg(feature = "mod-workflow")] WorkflowQuery,
);

#[derive(MergedObject, Default)]
pub struct Mutation(
    RootMutation,
    AuthMutation,
    OAuthMutation,
    SettingsMutation,
    FlexMutation,
    #[cfg(feature = "mod-commerce")] CommerceMutation,
    #[cfg(feature = "mod-content")] ContentMutation,
    #[cfg(feature = "mod-blog")] BlogMutation,
    #[cfg(feature = "mod-forum")] ForumMutation,
    #[cfg(feature = "mod-pages")] PagesMutation,
    #[cfg(feature = "mod-alloy")] AlloyMutation,
    #[cfg(feature = "mod-media")] MediaMutation,
    #[cfg(feature = "mod-workflow")] WorkflowMutation,
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
    field_definition_cache: FieldDefinitionCache,
    alloy_state: AlloyState,
    #[cfg(feature = "mod-media")] storage: StorageService,
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
    .data(build_event_hub)
    .data(build_field_def_registry())
    .data(field_definition_cache);

    #[cfg(feature = "mod-alloy")]
    let builder = builder.data(alloy_state);
    #[cfg(not(feature = "mod-alloy"))]
    let _ = alloy_state;

    #[cfg(feature = "mod-media")]
    let builder = builder.data(storage);

    builder.finish()
}
