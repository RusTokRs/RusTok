pub mod app_lifecycle;
pub mod app_router;
pub mod app_runtime;
pub mod artifact_delivery_tenants;
pub mod artifact_mcp;
pub mod artifact_runtime;
pub mod auth_admin_mutation_provider;
pub mod auth_invite;
pub mod auth_lifecycle;
pub mod auth_lifecycle_provider;
pub mod auth_password_reset;
pub mod build_control;
pub mod build_event_hub;
pub mod build_executor;
pub mod cache_redis_status_monitor;
pub mod cache_runtime;
pub mod channel_cache_invalidation;
#[cfg(test)]
mod channel_cache_invalidation_resolved_value_tests;
#[cfg(test)]
mod channel_cache_invalidation_runtime_tests;
#[cfg(feature = "mod-comments")]
pub mod comments_provider_runtime;
pub mod commerce_provider_runtime;
pub mod dashboard_user_activity;
pub mod effective_module_policy;
pub mod email;
pub mod event_bus;
pub mod event_delivery_control_adapter;
pub mod event_delivery_settings_service;
pub mod event_dlq_duplicate_alert_observability;
pub mod event_dlq_duplicate_alert_observer;
#[cfg(feature = "mod-forum")]
pub mod forum_audience_facts {
    mod membership {
        include!("forum_audience_facts.rs");
    }

    use rustok_forum::{ForumUserTrustAudienceFactsPort, SharedForumAudienceFactsPort};
    use sea_orm::DatabaseConnection;

    /// Stable server composition facade: historical Channel/Groups facts remain
    /// the membership provider and the Forum owner adds authoritative trust.
    pub(crate) struct ServerForumAudienceFactsPort;

    impl ServerForumAudienceFactsPort {
        pub(crate) fn shared(
            db: DatabaseConnection,
            groups: Option<SharedForumAudienceFactsPort>,
        ) -> SharedForumAudienceFactsPort {
            let membership_facts =
                membership::ServerForumAudienceFactsPort::shared(db.clone(), groups);
            ForumUserTrustAudienceFactsPort::shared(db, membership_facts)
        }
    }
}
#[cfg(all(feature = "mod-forum", feature = "mod-groups"))]
pub mod forum_audience_group_facts;
#[cfg(feature = "mod-forum")]
pub mod forum_notification_recipient_context;
#[cfg(feature = "mod-forum")]
pub mod forum_posting_policy_facts;
#[cfg(feature = "mod-forum")]
pub mod forum_search_inbox_worker;
pub mod graphql_schema;
pub mod iggy_connector_control_adapter;
pub mod iggy_connector_settings_service;
pub mod index_replay_runtime_composition;
pub mod marketplace_catalog;
pub mod marketplace_catalog_adapter;
pub mod marketplace_catalog_cache;
#[cfg(feature = "commerce-marketplace-financial")]
pub mod marketplace_financial_worker;
pub mod mcp_management;
pub mod mcp_management_authority;
pub mod mcp_management_guard;
pub mod mcp_management_mutation_provider;
pub mod mcp_runtime;
pub mod mcp_scaffold_workspace;
#[path = "module_event_dispatcher.rs"]
mod module_event_dispatcher_base;
pub mod module_event_dispatcher {
    use std::sync::Arc;

    use rustok_auth::AuthConfig;
    use rustok_core::{ModuleRegistry, ModuleRuntimeExtensions};

    use crate::common::settings::RustokSettings;
    use crate::error::{Error, Result};
    use crate::services::server_runtime_context::ServerRuntimeContext;

    #[cfg(feature = "mod-forum")]
    mod forum_search_category_scope {
        include!("forum_search_category_scope.rs");
    }

    #[cfg(feature = "mod-forum")]
    mod forum_search_owner_revision {
        include!("forum_search_owner_revision.rs");
    }

    #[cfg(feature = "mod-forum")]
    mod forum_search_result_eligibility {
        include!("forum_search_result_eligibility.rs");
    }

    pub use super::module_event_dispatcher_base::{
        build_module_event_dispatcher, build_shared_runtime_extensions,
        spawn_module_event_dispatcher,
    };

    /// Adds host-owned adapters after module/distribution registration, materializes the
    /// canonical Index query and replay runtimes, and then activates selected
    /// non-authoritative shadows.
    pub fn build_shared_runtime_extensions_with_host_providers(
        registry: &ModuleRegistry,
        settings: &RustokSettings,
        runtime_ctx: ServerRuntimeContext,
        auth_config: AuthConfig,
    ) -> Result<Arc<ModuleRuntimeExtensions>> {
        let db = runtime_ctx.db_clone();
        let base = super::module_event_dispatcher_base::build_shared_runtime_extensions_with_host_providers(
            registry,
            settings,
            runtime_ctx,
            auth_config,
        )?;
        let mut extensions = Arc::try_unwrap(base).map_err(|_| {
            Error::Message(
                "module runtime extensions must remain uniquely owned during final host composition"
                    .to_string(),
            )
        })?;

        #[cfg(feature = "mod-comments")]
        super::comments_provider_runtime::register_comments_provider_runtime(&mut extensions)
            .map_err(Error::BadRequest)?;

        #[cfg(feature = "mod-forum")]
        {
            let audience_facts = extensions
                .get::<rustok_forum::SharedForumAudienceFactsPort>()
                .cloned();
            let category_scope =
                forum_search_category_scope::ServerForumSearchCategoryScopePort::shared(
                    db.clone(),
                    audience_facts.clone(),
                );
            let owner_revision =
                forum_search_owner_revision::ServerForumProjectionOwnerRevisionSourcePort::shared(
                    db.clone(),
                );
            let result_eligibility =
                forum_search_result_eligibility::ServerForumSearchResultEligibilityPort::shared(
                    db.clone(),
                    audience_facts,
                );
            extensions.insert(category_scope);
            extensions.insert(owner_revision);
            extensions.insert(result_eligibility);
        }

        rustok_index::materialize_postgres_index_query_runtime(&mut extensions, db.clone())
            .map_err(|error| {
                Error::Message(format!("Index query runtime composition failed: {error}"))
            })?;
        super::index_replay_runtime_composition::materialize_index_replay_runtime(
            &mut extensions,
            db.clone(),
        )?;

        #[cfg(all(
            feature = "mod-notifications",
            feature = "mod-profiles",
            feature = "mod-social_graph"
        ))]
        if crate::services::notification_recipient_policy::social_graph_index_privacy_shadow_enabled()
            .map_err(Error::Message)?
        {
            rustok_telemetry::social_graph_index_privacy_shadow_metrics::ensure_registered()
                .map_err(|error| {
                    Error::Message(format!(
                        "Social Graph Index privacy shadow metrics registration failed: {error}"
                    ))
                })?;
            let index_runtime = extensions
                .get::<rustok_index::SharedIndexQueryRuntime>()
                .cloned()
                .ok_or_else(|| {
                    Error::Message(
                        "Index query runtime is required when Social Graph Index privacy shadow is enabled"
                            .to_string(),
                    )
                })?;
            let policy = crate::services::notification_recipient_policy::ServerNotificationRecipientPolicy::compose_with_index_shadow_runtime(
                db,
                &extensions,
                index_runtime,
            );
            extensions.insert(policy);
        }

        Ok(Arc::new(extensions))
    }

    #[cfg(all(test, feature = "mod-social_graph"))]
    mod tests {
        use rustok_core::{ModuleRegistry, ModuleRuntimeExtensions};
        use rustok_index::{
            IndexModule, SharedIndexMutationEventRegistry, SharedIndexQueryRuntime,
            SharedIndexReplayRuntime, SharedIndexSchemaRegistry,
        };
        use sea_orm::Database;

        use super::build_shared_runtime_extensions_with_host_providers;
        use crate::auth::AuthConfig;
        use crate::common::settings::RustokSettings;
        use crate::services::server_runtime_context::ServerRuntimeContext;

        #[tokio::test]
        async fn host_materializes_social_graph_index_query_replay_and_event_runtimes() {
            let registry = ModuleRegistry::new()
                .register(IndexModule)
                .register(rustok_social_graph::SocialGraphModule);
            let settings = RustokSettings::default();
            let db = Database::connect("sqlite::memory:")
                .await
                .expect("in-memory sqlite should connect");
            let runtime_ctx = ServerRuntimeContext::new(db.clone(), settings.clone());

            let extensions = build_shared_runtime_extensions_with_host_providers(
                &registry,
                &settings,
                runtime_ctx,
                AuthConfig::new("test-secret-key-for-unit-tests-only-32bytes!".to_string()),
            )
            .expect("host Index runtime should compose");

            assert!(extensions.contains::<SharedIndexSchemaRegistry>());
            assert!(extensions.contains::<SharedIndexQueryRuntime>());
            assert!(extensions.contains::<SharedIndexReplayRuntime>());
            let event_registry = extensions
                .get::<SharedIndexMutationEventRegistry>()
                .expect("Social Graph event registry should be materialized");
            assert!(
                event_registry
                    .get(
                        rustok_social_graph::index_source::SOCIAL_GRAPH_RELATION_INDEX_EVENT_DOMAIN
                    )
                    .is_some()
            );
            #[cfg(feature = "mod-forum")]
            assert!(
                extensions.contains::<rustok_search::SharedStorefrontSearchCategoryScopePort>()
            );
            #[cfg(feature = "mod-forum")]
            assert!(
                extensions
                    .contains::<rustok_search::SharedForumProjectionOwnerRevisionSourcePort>()
            );
            #[cfg(feature = "mod-forum")]
            assert!(
                extensions.contains::<rustok_search::SharedStorefrontSearchResultEligibilityPort>()
            );
            #[cfg(all(feature = "mod-notifications", feature = "mod-profiles"))]
            assert!(
                extensions.contains::<rustok_notifications::NotificationRecipientPolicyRuntime>()
            );
            let host = extensions.apply_to_host_runtime(rustok_api::HostRuntimeContext::new(db));
            assert!(host.shared_get::<SharedIndexQueryRuntime>().is_some());
            assert!(host.shared_get::<SharedIndexReplayRuntime>().is_some());
            assert!(
                host.shared_get::<SharedIndexMutationEventRegistry>()
                    .is_some()
            );
            #[cfg(feature = "mod-forum")]
            assert!(
                host.shared_get::<rustok_search::SharedStorefrontSearchCategoryScopePort>()
                    .is_some()
            );
            #[cfg(feature = "mod-forum")]
            assert!(
                host.shared_get::<rustok_search::SharedForumProjectionOwnerRevisionSourcePort>()
                    .is_some()
            );
            #[cfg(feature = "mod-forum")]
            assert!(
                host.shared_get::<rustok_search::SharedStorefrontSearchResultEligibilityPort>()
                    .is_some()
            );
        }

        #[test]
        fn facade_keeps_module_extensions_type_visible() {
            fn accepts(_: &ModuleRuntimeExtensions) {}
            let extensions = ModuleRuntimeExtensions::default();
            accepts(&extensions);
        }
    }
}
pub mod module_lifecycle;
pub mod module_rollout_promotion_settings;
#[cfg(feature = "mod-notifications")]
pub mod notification_candidate_worker;
#[cfg(feature = "mod-notifications")]
pub mod notification_fanout_worker;
#[cfg(feature = "mod-notifications")]
pub mod notification_outbox_intake_worker;
#[cfg(all(feature = "mod-notifications", feature = "mod-profiles"))]
pub mod notification_recipient_policy;
pub mod oauth_admin_guard;
pub mod oauth_app;
pub mod oauth_consent_service;
pub mod oauth_token_service;
#[cfg(feature = "mod-pages")]
pub mod pages_cache_invalidation;
#[cfg(feature = "mod-commerce")]
pub mod paid_order_label_worker;
#[cfg(feature = "mod-payment")]
pub mod payment_provider_event_worker;
#[cfg(feature = "mod-payment")]
pub mod payment_provider_runtime;
pub mod platform_composition;
pub mod product_catalog_deployment;
#[cfg(feature = "mod-product")]
pub mod product_index_refresh_worker;
pub mod profile_media_public_image_deployment;
pub mod profile_media_public_image_runtime;

pub mod event_transport_factory;
pub mod order_field_service;
pub mod product_field_service;
pub mod rbac_authoritative;
pub mod rbac_cache_invalidation;
pub mod rbac_committed_mutations;
pub mod rbac_consistency;
pub mod rbac_invalidation_generation;
pub mod rbac_persistence;
pub mod rbac_repair;
pub mod rbac_request_scope;
pub mod rbac_runtime;
pub mod rbac_service;
pub mod redis_runtime;
pub mod registry_governance;
pub mod registry_principal;
pub mod registry_remote_runner;
pub mod registry_remote_transitions;
pub mod runtime_guardrails;
pub mod search_product_channel_reconciliation;
#[cfg(feature = "mod-seo")]
pub mod seo_redirect_cache_reconciliation;
pub mod server_bootstrap;
pub mod server_runtime_context;
pub mod settings_service;
#[cfg(feature = "mod-social_graph")]
pub mod social_graph_index_poison_observer;
#[cfg(feature = "mod-social_graph")]
pub mod social_graph_index_position_observer;
#[cfg(feature = "mod-social_graph")]
pub mod social_graph_index_worker;
pub mod tenant_cache_generation;
pub mod tenant_cache_generation_status;
pub mod tenant_generation_delivery_gate;
pub mod tenant_locale_generation;
pub mod topic_field_service;
pub mod user_admin_guard;
pub mod user_field_service;

pub mod field_definition_cache;
pub mod field_definition_registry_bootstrap;
pub mod flex_attached_values;
pub mod flex_standalone_service;
