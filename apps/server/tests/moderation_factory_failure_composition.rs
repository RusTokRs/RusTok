#![cfg(feature = "mod-moderation")]

use std::sync::Arc;

use rustok_auth::AuthConfig;
use rustok_core::{MigrationSource, ModuleRegistry, ModuleRuntimeExtensions, RusToKModule};
use rustok_index::IndexModule;
use rustok_moderation::{
    ModerationModule, ModerationSubjectAdapterBuildError, ModerationSubjectAdapterFactory,
    ModerationSubjectAdapterKey, ModerationSubjectCommandPort, ModerationSubjectKind,
    register_moderation_subject_adapter_factory,
};
use rustok_server::common::settings::RustokSettings;
use rustok_server::error::{Error, Result};
use rustok_server::services::module_event_dispatcher::build_shared_runtime_extensions_with_host_providers;
use rustok_server::services::server_runtime_context::ServerRuntimeContext;
use sea_orm::Database;
use sea_orm_migration::MigrationTrait;

const TEST_AUTH_SECRET: &str = "test-secret-key-for-unit-tests-only-32bytes!";
const FAILING_MODULE: &str = "broken_moderation_producer";

struct FailingModerationFactory;

impl ModerationSubjectAdapterFactory for FailingModerationFactory {
    fn key(&self) -> ModerationSubjectAdapterKey {
        ModerationSubjectAdapterKey::new(FAILING_MODULE, ModerationSubjectKind::ForumPost)
            .expect("test factory key must be valid")
    }

    fn build(
        &self,
        _host: &rustok_api::HostRuntimeContext,
    ) -> std::result::Result<
        Arc<dyn ModerationSubjectCommandPort>,
        ModerationSubjectAdapterBuildError,
    > {
        Err(ModerationSubjectAdapterBuildError::InvalidConfiguration)
    }
}

struct FailingModerationProducerModule;

impl MigrationSource for FailingModerationProducerModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}

impl RusToKModule for FailingModerationProducerModule {
    fn slug(&self) -> &'static str {
        FAILING_MODULE
    }

    fn name(&self) -> &'static str {
        "Broken Moderation Producer Test Module"
    }

    fn description(&self) -> &'static str {
        "Test-only module that publishes a moderation adapter factory which cannot initialize"
    }

    fn version(&self) -> &'static str {
        "0.0.0-test"
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        register_moderation_subject_adapter_factory(extensions, FailingModerationFactory)
            .map_err(|error| rustok_core::Error::Validation(error.to_string()))
    }
}

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

#[tokio::test]
async fn selected_moderation_host_fails_closed_when_subject_factory_build_fails() {
    let registry = ModuleRegistry::new()
        .register(IndexModule)
        .register(ModerationModule)
        .register(FailingModerationProducerModule);

    let error = match compose(&registry).await {
        Ok(_) => panic!("host composition must reject a producer factory that cannot initialize"),
        Err(error) => error,
    };

    assert!(matches!(&error, Error::Message(_)));
    let message = error.to_string();
    assert!(message.contains("moderation subject adapter materialization failed"));
    assert!(message.contains("broken_moderation_producer/forum_post"));
    assert!(message.contains("moderation subject adapter configuration is invalid"));
}
