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

use super::loaders::TenantNameLoader;
use super::mutations::RootMutation;
use super::observability::GraphqlObservability;
use super::queries::RootQuery;
use super::security::GraphqlSecurityPolicy;
use super::settings::{SettingsMutation, SettingsQuery};
use super::subscriptions::BuildSubscription;
use super::system::SystemQuery;
use crate::services::build_event_hub::BuildEventHub;
use crate::services::field_definition_cache::FieldDefinitionCache;
use crate::services::field_definition_registry_bootstrap::build_field_def_registry;
use crate::services::flex_standalone_service::FlexStandaloneSeaOrmService;
use flex::graphql::FlexGraphqlRuntime;
use rustok_ai::graphql::{AiMutation, AiQuery, AiSubscription};
use rustok_auth::graphql::{AuthMutation, AuthQuery, OAuthMutation, OAuthQuery};
#[cfg(feature = "mod-content")]
use rustok_content::graphql::{NodeBodyLoader, NodeLoader, NodeTranslationLoader};
use rustok_mcp::graphql::{McpMutation, McpQuery};
use rustok_rbac::graphql::{RbacGraphqlRoleWriterHandle, RbacMutation, RbacQuery};
use rustok_search::graphql::{SearchGraphqlRateLimiterHandle, SearchMutationRoot, SearchQueryRoot};

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
    schema_codegen::OptionalModuleQuery,
);

#[derive(MergedObject, Default)]
pub struct Mutation(
    RootMutation,
    #[cfg(all(
        feature = "mod-content",
        feature = "mod-blog",
        feature = "mod-forum",
        feature = "mod-comments"
    ))]
    rustok_content_orchestration::graphql::ContentOrchestrationMutation,
    AiMutation,
    SearchMutationRoot,
    AuthMutation,
    OAuthMutation,
    McpMutation,
    RbacMutation,
    SettingsMutation,
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
    ai_runtime: rustok_ai::AiHostRuntime,
    build_event_hub: Arc<BuildEventHub>,
    field_definition_cache: FieldDefinitionCache,
    runtime_extensions: Arc<ModuleRuntimeExtensions>,
    rbac_role_writer: RbacGraphqlRoleWriterHandle,
    search_rate_limiter: Option<SearchGraphqlRateLimiterHandle>,
    #[cfg(feature = "mod-alloy")] alloy_runtime: alloy::SharedAlloyRuntime,
    #[cfg(all(
        feature = "mod-content",
        feature = "mod-blog",
        feature = "mod-forum",
        feature = "mod-comments"
    ))]
    content_orchestration: rustok_content_orchestration::SharedContentOrchestrationService,
    #[cfg(feature = "mod-media")] storage: StorageService,
) -> AppSchema {
    let ai_role_slug_provider = rustok_ai::AiGraphqlRoleSlugProviderHandle::new(Arc::new(
        rustok_ai::SeaOrmAiGraphqlRoleSlugProvider::new(db.clone()),
    ));
    let flex_runtime = FlexGraphqlRuntime::new(
        Arc::new(FlexStandaloneSeaOrmService::new(db.clone())),
        db.clone(),
        build_field_def_registry(),
        Arc::new(field_definition_cache),
    );
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
        .data(ai_role_slug_provider)
        .data(ai_runtime)
        .data(db)
        .data(event_bus)
        .data(transactional_event_bus)
        .data(build_event_hub)
        .data(flex_runtime)
        .data(runtime_extensions)
        .data(rbac_role_writer);

    let builder = if let Some(search_rate_limiter) = search_rate_limiter {
        builder.data(search_rate_limiter)
    } else {
        builder
    };

    #[cfg(feature = "mod-alloy")]
    let builder = builder.data(alloy_runtime);

    #[cfg(all(
        feature = "mod-content",
        feature = "mod-blog",
        feature = "mod-forum",
        feature = "mod-comments"
    ))]
    let builder = builder.data(content_orchestration);

    #[cfg(feature = "mod-media")]
    let builder = builder.data(storage);

    builder.finish()
}
