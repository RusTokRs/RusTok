use async_graphql::{
    dataloader::DataLoader, extensions::Analyzer, MergedObject, MergedSubscription, Schema,
};
use rustok_core::ModuleRuntimeExtensions;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use rustok_core::EventBus;
use rustok_outbox::TransactionalEventBus;
#[cfg(feature = "mod-profiles")]
use rustok_profiles::ProfileSummaryLoader;

#[cfg(feature = "mod-media")]
use rustok_storage::StorageService;

mod schema_codegen {
    include!(concat!(env!("OUT_DIR"), "/graphql_schema_codegen.rs"));
}

use super::ai::{AiMutation, AiQuery, AiSubscription};
use super::auth::{AuthMutation, AuthQuery};
use super::flex::{FlexMutation, FlexQuery};
use super::loaders::TenantNameLoader;
#[cfg(feature = "mod-content")]
use super::loaders::{NodeBodyLoader, NodeLoader, NodeTranslationLoader};
use super::mcp::{McpMutation, McpQuery};
use super::mutations::RootMutation;
use super::oauth::{OAuthMutation, OAuthQuery};
use super::observability::GraphqlObservability;
use super::queries::RootQuery;
use super::rbac::{RbacMutation, RbacQuery};
use super::search::{SearchMutationRoot, SearchQueryRoot};
use super::security::GraphqlSecurityPolicy;
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
    pub const MEDIA: &str = "media";
    pub const WORKFLOW: &str = "workflow";
}

#[derive(MergedObject, Default)]
pub struct Query(
    RootQuery,
    AiQuery,
    SearchQueryRoot,
    AuthQuery,
    OAuthQuery,
    McpQuery,
    RbacQuery,
    SettingsQuery,
    SystemQuery,
    FlexQuery,
    schema_codegen::OptionalModuleQuery,
);

#[derive(MergedObject, Default)]
pub struct Mutation(
    RootMutation,
    AiMutation,
    SearchMutationRoot,
    AuthMutation,
    OAuthMutation,
    McpMutation,
    RbacMutation,
    SettingsMutation,
    FlexMutation,
    schema_codegen::OptionalModuleMutation,
);

#[derive(MergedSubscription, Default)]
pub struct Subscription(BuildSubscription, AiSubscription);

pub type AppSchema = Schema<Query, Mutation, Subscription>;

#[derive(Clone)]
pub struct SharedGraphqlSchema(pub Arc<AppSchema>);

pub fn build_schema(
    db: DatabaseConnection,
    event_bus: EventBus,
    transactional_event_bus: TransactionalEventBus,
    build_event_hub: Arc<BuildEventHub>,
    field_definition_cache: FieldDefinitionCache,
    runtime_extensions: Arc<ModuleRuntimeExtensions>,
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
    .extension(GraphqlSecurityPolicy)
    .extension(GraphqlObservability)
    // DataLoaders for efficient batched queries
    .data(DataLoader::new(
        TenantNameLoader::new(db.clone()),
        tokio::spawn,
    ));

    #[cfg(feature = "mod-content")]
    let builder = builder
        .data(DataLoader::new(NodeLoader::new(db.clone()), tokio::spawn))
        .data(DataLoader::new(
            NodeTranslationLoader::new(db.clone()),
            tokio::spawn,
        ))
        .data(DataLoader::new(
            NodeBodyLoader::new(db.clone()),
            tokio::spawn,
        ));

    #[cfg(feature = "mod-profiles")]
    let builder = builder.data(DataLoader::new(
        ProfileSummaryLoader::new(db.clone()),
        tokio::spawn,
    ));

    let builder = builder
        .data(db)
        .data(event_bus)
        .data(transactional_event_bus)
        .data(build_event_hub)
        .data(build_field_def_registry())
        .data(field_definition_cache)
        .data(runtime_extensions);

    #[cfg(feature = "mod-media")]
    let builder = builder.data(storage);

    builder.finish()
}
