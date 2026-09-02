use rustok_server::{
    auth::AuthConfig,
    common::settings::RustokSettings,
    modules,
    services::{
        module_event_dispatcher::build_shared_runtime_extensions_with_host_providers,
        server_runtime_context::ServerRuntimeContext,
    },
};
use rustok_storage::{LocalStorageConfig, StorageRuntime};
use rustok_translation_targets::{OwnerSlug, ResourceKind, translation_target_registry};
use sea_orm::Database;

#[tokio::test]
async fn isolated_media_host_composes_translation_target_provider() {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("isolated Media composition database should connect");
    let settings = RustokSettings::default();
    let runtime_ctx = ServerRuntimeContext::new(database.clone(), settings.clone());
    let storage_dir = tempfile::tempdir().expect("isolated Media storage directory should exist");
    let storage = StorageRuntime::local(&LocalStorageConfig {
        base_dir: storage_dir.path().display().to_string(),
        base_url: "/media".to_string(),
        fsync: false,
    })
    .expect("isolated Media storage runtime should initialize");
    runtime_ctx.shared_insert(storage);

    let registry = modules::build_registry();
    assert!(registry.contains("media"));

    let extensions = build_shared_runtime_extensions_with_host_providers(
        &registry,
        &settings,
        runtime_ctx,
        AuthConfig::new("media-isolated-composition-secret-32bytes!".to_string()),
    )
    .expect("isolated Media host composition should succeed");
    let targets = translation_target_registry(extensions.as_ref())
        .expect("Media host composition must publish the Translation target registry");
    let owner_slug = OwnerSlug::new("media").expect("Media owner slug should be valid");
    let resource_kind = ResourceKind::new("asset").expect("Media resource kind should be valid");
    let provider = targets
        .get(&owner_slug, &resource_kind)
        .expect("isolated Media host must register media/asset");
    let descriptor = provider.descriptor();

    assert_eq!(descriptor.owner_slug, owner_slug);
    assert_eq!(descriptor.resource_kind, resource_kind);
    assert!(descriptor.read_permission_floor.contains("media:read"));
    assert!(descriptor.apply_permission_floor.contains("media:update"));

    database
        .close()
        .await
        .expect("isolated Media composition database should close");
}
