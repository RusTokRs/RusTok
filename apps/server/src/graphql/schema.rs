use async_graphql::{
    MergedObject, MergedSubscription, Schema, dataloader::DataLoader, extensions::Analyzer,
};
use rustok_core::ModuleRuntimeExtensions;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use rustok_core::EventBus;
use rustok_outbox::TransactionalEventBus;
#[cfg(feature = "mod-profiles")]
use rustok_profiles::ProfileSummaryLoader;

#[cfg(feature = "mod-media")]
use rustok_storage::StorageRuntime;

mod schema_codegen {
    include!(concat!(env!("OUT_DIR"), "/graphql_schema_codegen.rs"));
}

use super::dashboard_security::GraphqlDashboardSecurityPolicy;
use super::forum_principal_security::ForumPrincipalPolicy;
use super::legacy_disable_user::LegacyDisableUserPolicy;
use super::loaders::TenantNameLoader;
use super::module_security::GraphqlModuleSecurityPolicy;
use super::mutations::RootMutation;
use super::observability::GraphqlObservability;
use super::principal_tenant_security::GraphqlPrincipalTenantPolicy;
use super::queries::RootQuery;
use super::security::GraphqlSecurityPolicy;
use super::settings::{SettingsMutation, SettingsQuery};
use super::storefront_principal_security::StorefrontPrincipalPolicy;
use super::subscriptions::BuildSubscription;
use super::system::SystemQuery;
use super::tenant_security::GraphqlTenantPolicy;
use crate::services::build_event_hub::BuildEventHub;
use crate::services::field_definition_cache::FieldDefinitionCache;
use crate::services::field_definition_registry_bootstrap::build_field_def_registry;
use crate::services::flex_standalone_service::FlexStandaloneSeaOrmService;
use flex::graphql::FlexGraphqlRuntime;
use rustok_auth::graphql::{AuthMutation, AuthQuery, OAuthMutation, OAuthQuery};
#[cfg(feature = "mod-blog")]
use rustok_blog::graphql::{BlogGraphqlRateLimitPolicy, BlogGraphqlRateLimiterHandle};
#[cfg(feature = "mod-content")]
use rustok_content::graphql::{NodeBodyLoader, NodeLoader, NodeTranslationLoader};
#[cfg(feature = "mod-forum")]
use rustok_forum::graphql::ForumGraphqlErrorExtension;
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
    SearchMutationRoot,
    AuthMutation,
    OAuthMutation,
    McpMutation,
    RbacMutation,
    SettingsMutation,
    schema_codegen::OptionalModuleMutation,
);

#[derive(MergedSubscription, Default)]
pub struct Subscription(
    BuildSubscription,
    schema_codegen::OptionalModuleSubscription,
);

pub type AppSchema = Schema<Query, Mutation, Subscription>;

#[derive(Clone)]
pub struct SharedGraphqlSchema(pub Arc<AppSchema>);

pub struct GraphqlSchemaDependencies {
    pub db: DatabaseConnection,
    pub event_bus: EventBus,
    pub transactional_event_bus: TransactionalEventBus,
    pub graphql_runtime_inputs: rustok_api::graphql::GraphqlRuntimeInputs,
    pub build_event_hub: Arc<BuildEventHub>,
    pub field_definition_cache: FieldDefinitionCache,
    pub runtime_extensions: Arc<ModuleRuntimeExtensions>,
    pub rbac_role_writer: RbacGraphqlRoleWriterHandle,
    pub search_rate_limiter: Option<SearchGraphqlRateLimiterHandle>,
    #[cfg(feature = "mod-blog")]
    pub blog_rate_limiter: Option<BlogGraphqlRateLimiterHandle>,
    #[cfg(feature = "mod-alloy")]
    pub alloy_runtime: alloy::SharedAlloyRuntime,
    #[cfg(feature = "mod-alloy")]
    pub alloy_release_governance: alloy::AlloyReleaseGovernanceHandle,
    #[cfg(all(
        feature = "mod-content",
        feature = "mod-blog",
        feature = "mod-forum",
        feature = "mod-comments"
    ))]
    pub content_orchestration: rustok_content_orchestration::SharedContentOrchestrationService,
    #[cfg(feature = "mod-media")]
    pub storage: StorageRuntime,
}

pub fn build_schema(dependencies: GraphqlSchemaDependencies) -> AppSchema {
    let GraphqlSchemaDependencies {
        db,
        event_bus,
        transactional_event_bus,
        graphql_runtime_inputs,
        build_event_hub,
        field_definition_cache,
        runtime_extensions,
        rbac_role_writer,
        search_rate_limiter,
        #[cfg(feature = "mod-blog")]
        blog_rate_limiter,
        #[cfg(feature = "mod-alloy")]
        alloy_runtime,
        #[cfg(feature = "mod-alloy")]
        alloy_release_governance,
        #[cfg(all(
            feature = "mod-content",
            feature = "mod-blog",
            feature = "mod-forum",
            feature = "mod-comments"
        ))]
        content_orchestration,
        #[cfg(feature = "mod-media")]
        storage,
    } = dependencies;
    let marketplace_catalog = graphql_runtime_inputs
        .shared_get::<rustok_modules::SharedModuleMarketplaceCatalog>()
        .expect("module marketplace catalog must be composed before GraphQL schema construction");
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
    .extension(GraphqlPrincipalTenantPolicy)
    .extension(GraphqlTenantPolicy)
    .extension(GraphqlSecurityPolicy)
    .extension(GraphqlModuleSecurityPolicy)
    .extension(GraphqlDashboardSecurityPolicy)
    .extension(LegacyDisableUserPolicy)
    .extension(StorefrontPrincipalPolicy)
    .extension(ForumPrincipalPolicy)
    .extension(GraphqlObservability)
    // DataLoaders for efficient batched queries
    .data(DataLoader::new(
        TenantNameLoader::new(db.clone()),
        tokio::spawn,
    ));

    #[cfg(feature = "mod-forum")]
    let builder = builder.extension(ForumGraphqlErrorExtension);

    #[cfg(feature = "mod-blog")]
    let builder = builder.extension(BlogGraphqlRateLimitPolicy::new(blog_rate_limiter));

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

    tracing::debug!(
        factories = ?schema_codegen::MODULE_GRAPHQL_RUNTIME_DATA_FACTORIES,
        "Attaching manifest-declared GraphQL runtime data"
    );
    let builder = schema_codegen::attach_module_graphql_data(builder, &graphql_runtime_inputs)
        .expect("manifest GraphQL runtime-data factory must materialize");
    let builder = builder
        .data(db)
        .data(event_bus)
        .data(transactional_event_bus)
        .data(build_event_hub)
        .data(flex_runtime)
        .data(marketplace_catalog)
        .data(runtime_extensions)
        .data(rbac_role_writer);

    let builder = if let Some(search_rate_limiter) = search_rate_limiter {
        builder.data(search_rate_limiter)
    } else {
        builder
    };

    #[cfg(feature = "mod-alloy")]
    let builder = builder.data(alloy_runtime).data(alloy_release_governance);

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
