use async_graphql::{
    dataloader::DataLoader, extensions::Analyzer, MergedObject, MergedSubscription, Schema,
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use rustok_core::EventBus;
use rustok_outbox::TransactionalEventBus;

use super::alloy::{AlloyMutation, AlloyQuery, AlloyState};
use super::auth::{AuthMutation, AuthQuery};
use super::blog::{BlogMutation, BlogQuery};
use super::commerce::{CommerceMutation, CommerceQuery};
use super::content::{ContentMutation, ContentQuery};
use super::forum::{ForumMutation, ForumQuery};
use super::loaders::{NodeBodyLoader, NodeLoader, NodeTranslationLoader, TenantNameLoader};
use super::mutations::RootMutation;
use super::oauth::{OAuthMutation, OAuthQuery};
use super::observability::GraphqlObservability;
use super::pages::{PagesMutation, PagesQuery};
use super::queries::RootQuery;
use super::subscriptions::BuildSubscription;
use crate::services::build_event_hub::BuildEventHub;

#[derive(MergedObject, Default)]
pub struct Query(
    RootQuery,
    AuthQuery,
    CommerceQuery,
    ContentQuery,
    BlogQuery,
    ForumQuery,
    AlloyQuery,
    PagesQuery,
    OAuthQuery,
);

#[derive(MergedObject, Default)]
pub struct Mutation(
    RootMutation,
    AuthMutation,
    CommerceMutation,
    ContentMutation,
    BlogMutation,
    ForumMutation,
    AlloyMutation,
    PagesMutation,
    OAuthMutation,
);

#[derive(MergedSubscription, Default)]
pub struct Subscription(BuildSubscription);

pub type AppSchema = Schema<Query, Mutation, Subscription>;

pub fn build_schema(
    db: DatabaseConnection,
    event_bus: EventBus,
    transactional_event_bus: TransactionalEventBus,
    build_event_hub: Arc<BuildEventHub>,
    alloy_state: AlloyState,
) -> AppSchema {
    Schema::build(
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
    .data(alloy_state)
    .finish()
}
