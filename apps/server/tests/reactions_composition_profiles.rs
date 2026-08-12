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

#[cfg(all(feature = "mod-forum", not(feature = "mod-reactions")))]
#[tokio::test]
async fn forum_without_reactions_keeps_forum_host_composition_available() {
    let registry = ModuleRegistry::new()
        .register(IndexModule)
        .register(rustok_forum::ForumModule);

    let extensions = compose(&registry)
        .await
        .expect("Forum-only host composition must remain available");

    assert!(extensions.contains::<rustok_forum::SharedForumAudienceFactsPort>());
    assert!(extensions.contains::<rustok_forum::SharedForumNotificationRecipientContextPort>());
}

#[cfg(all(
    feature = "mod-reactions",
    not(feature = "mod-forum"),
    not(feature = "mod-blog")
))]
#[tokio::test]
async fn reactions_without_forum_materializes_an_empty_subject_registry() {
    let registry = ModuleRegistry::new()
        .register(IndexModule)
        .register(rustok_reactions::ReactionsModule);

    let extensions = compose(&registry)
        .await
        .expect("Reactions-only host composition must initialize");
    let subjects =
        rustok_reactions::api::reaction_subject_registry_from_extensions(extensions.as_ref())
            .expect("selected Reactions owner must publish a materialized subject registry");

    assert!(subjects.is_empty());
}

#[cfg(all(feature = "mod-forum", feature = "mod-reactions"))]
#[tokio::test]
async fn forum_with_reactions_materializes_topic_and_reply_provider() {
    let registry = ModuleRegistry::new()
        .register(IndexModule)
        .register(rustok_reactions::ReactionsModule)
        .register(rustok_forum::ForumModule);

    let extensions = compose(&registry)
        .await
        .expect("Forum plus Reactions host composition must initialize");
    let subjects =
        rustok_reactions::api::reaction_subject_registry_from_extensions(extensions.as_ref())
            .expect("selected Reactions owner must publish a materialized subject registry");
    let forum = subjects
        .get_by_str("forum")
        .expect("Forum producer must materialize when both modules are selected");
    let mut kinds = forum
        .supported_kinds()
        .into_iter()
        .map(|kind| kind.as_str().to_string())
        .collect::<Vec<_>>();
    kinds.sort();

    assert_eq!(kinds, vec!["reply".to_string(), "topic".to_string()]);
}

#[cfg(all(feature = "mod-blog", feature = "mod-reactions"))]
#[tokio::test]
async fn blog_with_reactions_materializes_post_provider() {
    let registry = ModuleRegistry::new()
        .register(IndexModule)
        .register(rustok_reactions::ReactionsModule)
        .register(rustok_blog::BlogModule);

    let extensions = compose(&registry)
        .await
        .expect("Blog plus Reactions host composition must initialize");
    let subjects =
        rustok_reactions::api::reaction_subject_registry_from_extensions(extensions.as_ref())
            .expect("selected Reactions owner must publish a materialized subject registry");
    let blog = subjects
        .get_by_str("blog")
        .expect("Blog producer must materialize when both modules are selected");
    let kinds = blog
        .supported_kinds()
        .into_iter()
        .map(|kind| kind.as_str().to_string())
        .collect::<Vec<_>>();

    assert_eq!(kinds, vec!["post".to_string()]);
}

#[cfg(feature = "mod-reactions")]
#[tokio::test]
async fn selected_reactions_feature_fails_when_owner_module_is_missing() {
    let registry = ModuleRegistry::new().register(IndexModule);

    let error = match compose(&registry).await {
        Ok(_) => panic!("selected Reactions feature must reject a registry without its owner"),
        Err(error) => error,
    };

    assert!(matches!(&error, Error::Message(_)));
    assert_eq!(
        error.to_string(),
        "Reactions feature is selected but ReactionsModule is missing from ModuleRegistry"
    );
}
