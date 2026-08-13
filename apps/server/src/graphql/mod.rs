#[cfg(feature = "mod-blog")]
pub mod blog_rate_limit;
pub mod dashboard_security;
#[cfg(all(feature = "mod-forum", feature = "mod-notifications"))]
pub mod forum_notification_reconciliation;
pub mod forum_principal_security;
pub mod index_drift_diagnosis;
pub mod index_drift_source_page_diagnosis;
pub mod index_replay;
#[cfg(test)]
mod index_replay_locale_tests;
#[cfg(test)]
mod index_replay_shutdown_tests;
pub mod legacy_disable_user;
pub mod loaders;
#[cfg(feature = "mod-moderation")]
pub mod moderation_recovery;
pub mod module_security;
pub mod module_settings_cas;
pub mod mutations;
pub mod observability;
pub mod persisted;
pub mod principal_tenant_security;
#[cfg(feature = "mod-profiles")]
pub mod profile_summary_policy;
pub mod queries;
pub mod rbac_runtime;
pub mod schema;
pub mod search_rate_limit;
pub mod security;
pub mod settings;
pub mod storefront_principal_security;
pub mod subscriptions;
pub mod system;
pub mod tenant_security;
pub mod types;

pub use schema::{AppSchema, GraphqlSchemaDependencies, SharedGraphqlSchema, build_schema};
