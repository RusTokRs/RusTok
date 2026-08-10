use std::sync::Arc;

use rustok_auth::AuthConfig;
use rustok_core::{ModuleRegistry, ModuleRuntimeExtensions};
use rustok_index::IndexModule;
use rustok_server::common::settings::RustokSettings;
use rustok_server::error::Result;
use rustok_server::services::module_event_dispatcher::build_shared_runtime_extensions_with_host_providers;
use rustok_server::services::server_runtime_context::ServerRuntimeContext;
use sea_orm::Database;

const TEST_AUTH_SECRET: &str = "test-secret-key-for-unit-tests-only-32bytes!";

async fn compose(registry: &ModuleRegistry) -> Result<Arc<ModuleRuntimeExtensions>> {
    let settings = RustokSettings::default();
    let database = Database::connect("sqlite::memory:").await?;
    let runtime = ServerRuntimeContext::new(database, settings.clone());

    build_shared_runtime_extensions_with_host_providers(
        registry,
        &settings,
        runtime,
        AuthConfig::new(TEST_AUTH_SECRET.to_string()),
    )
}

#[cfg(all(feature = "mod-forum", not(feature = "mod-moderation")))]
#[tokio::test]
async fn forum_without_moderation_keeps_forum_host_composition_available() {
    let registry = ModuleRegistry::new()
        .register(IndexModule)
        .register(rustok_forum::ForumModule);

    let extensions = compose(&registry)
        .await
        .expect("Forum-only host composition must remain available");

    assert!(extensions.contains::<rustok_forum::SharedForumAudienceFactsPort>());
    assert!(
        extensions.contains::<rustok_forum::SharedForumNotificationRecipientContextPort>()
    );
}

#[cfg(all(feature = "mod-moderation", not(feature = "mod-forum")))]
#[tokio::test]
async fn moderation_without_forum_materializes_an_empty_subject_registry() {
    let registry = ModuleRegistry::new()
        .register(IndexModule)
        .register(rustok_moderation::ModerationModule);

    let extensions = compose(&registry)
        .await
        .expect("Moderation-only host composition must initialize");
    let subjects = rustok_moderation::moderation_subject_adapter_registry_from_extensions(
        extensions.as_ref(),
    )
    .expect("selected Moderation owner must publish a materialized subject registry");

    assert!(subjects.is_empty());
}

#[cfg(all(feature = "mod-forum", feature = "mod-moderation"))]
#[tokio::test]
async fn forum_with_moderation_materializes_topic_and_reply_adapters() {
    let registry = ModuleRegistry::new()
        .register(IndexModule)
        .register(rustok_moderation::ModerationModule)
        .register(rustok_forum::ForumModule);

    let extensions = compose(&registry)
        .await
        .expect("Forum plus Moderation host composition must initialize");
    let subjects = rustok_moderation::moderation_subject_adapter_registry_from_extensions(
        extensions.as_ref(),
    )
    .expect("selected Moderation owner must publish a materialized subject registry");

    assert_eq!(subjects.len(), 2);
    assert!(subjects.contains("forum", rustok_moderation::ModerationSubjectKind::ForumTopic));
    assert!(subjects.contains("forum", rustok_moderation::ModerationSubjectKind::ForumPost));
}

#[cfg(feature = "mod-moderation")]
#[tokio::test]
async fn selected_moderation_feature_fails_when_owner_module_is_missing() {
    let registry = ModuleRegistry::new().register(IndexModule);

    let error = match compose(&registry).await {
        Ok(_) => panic!("selected Moderation feature must reject a registry without its owner"),
        Err(error) => error,
    };

    assert!(matches!(&error, Error::Message(_)));
    assert_eq!(
        error.to_string(),
        "Moderation feature is selected but ModerationModule is missing from ModuleRegistry"
    );
}
