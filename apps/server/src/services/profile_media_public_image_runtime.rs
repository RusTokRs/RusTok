use std::sync::Arc;

use rustok_core::ModuleRuntimeExtensions;

use crate::services::server_runtime_context::ServerRuntimeContext;

/// Selects the Profiles public-image provider once and publishes the same typed wrapper to every
/// host surface through `ModuleRuntimeExtensions`.
///
/// Selection order is deployment-owned:
/// 1. a provider pre-seeded in `ServerRuntimeContext` (for example an extracted Media adapter);
/// 2. an existing module runtime extension provider;
/// 3. the embedded Media owner service when local storage is available.
///
/// Profiles consumers remain transport-neutral. They receive only
/// `ProfileMediaPublicImageProvider` and never inspect gRPC endpoints, storage credentials, or
/// Media public route construction.
pub fn attach_profile_media_public_image_provider(
    ctx: &ServerRuntimeContext,
    extensions: Arc<ModuleRuntimeExtensions>,
) -> Arc<ModuleRuntimeExtensions> {
    #[cfg(all(feature = "mod-profiles", feature = "mod-media"))]
    {
        use rustok_media::{MediaPublicImageReadPort, MediaPublicImageService};
        use rustok_profiles::ProfileMediaPublicImageProvider;

        let selected = ctx
            .shared_get::<ProfileMediaPublicImageProvider>()
            .or_else(|| extensions.get::<ProfileMediaPublicImageProvider>().cloned())
            .or_else(|| {
                ctx.shared_get::<rustok_storage::StorageRuntime>()
                    .map(|storage| {
                        let provider: Arc<dyn MediaPublicImageReadPort> =
                            Arc::new(MediaPublicImageService::new(ctx.db_clone(), storage));
                        ProfileMediaPublicImageProvider::new(provider)
                    })
            });

        let Some(selected) = selected else {
            return extensions;
        };

        let mut enriched = extensions.as_ref().clone();
        enriched.insert(selected.clone());
        let enriched = Arc::new(enriched);

        // Persist both values so later GraphQL and server-function composition observes the same
        // provider even when it builds a fresh HostRuntimeContext.
        ctx.shared_insert(selected);
        ctx.shared_insert(enriched.clone());
        enriched
    }

    #[cfg(not(all(feature = "mod-profiles", feature = "mod-media")))]
    {
        let _ = ctx;
        extensions
    }
}

#[cfg(all(test, feature = "mod-profiles", feature = "mod-media"))]
mod tests {
    use super::attach_profile_media_public_image_provider;
    use crate::common::settings::RustokSettings;
    use crate::services::server_runtime_context::ServerRuntimeContext;
    use rustok_core::ModuleRuntimeExtensions;
    use rustok_media::{MediaPublicImageReadPort, MediaPublicImageService};
    use rustok_profiles::ProfileMediaPublicImageProvider;
    use rustok_storage::{LocalStorageConfig, StorageRuntime};
    use sea_orm::Database;
    use std::sync::Arc;

    fn local_storage(directory: &tempfile::TempDir) -> StorageRuntime {
        StorageRuntime::local(&LocalStorageConfig {
            base_dir: directory.path().display().to_string(),
            base_url: String::new(),
            fsync: false,
        })
        .expect("local media storage should initialize")
    }

    #[tokio::test]
    async fn deployment_seeded_provider_wins_and_reaches_host_runtime() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        let runtime = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        let directory = tempfile::tempdir().expect("temporary media directory should exist");
        let storage = local_storage(&directory);
        runtime.shared_insert(storage.clone());

        let extension_port: Arc<dyn MediaPublicImageReadPort> =
            Arc::new(MediaPublicImageService::new(db.clone(), storage.clone()));
        let deployment_port: Arc<dyn MediaPublicImageReadPort> =
            Arc::new(MediaPublicImageService::new(db.clone(), storage));

        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(ProfileMediaPublicImageProvider::new(extension_port));
        runtime.shared_insert(ProfileMediaPublicImageProvider::new(
            deployment_port.clone(),
        ));

        let resolved = attach_profile_media_public_image_provider(&runtime, Arc::new(extensions));
        let resolved_provider = resolved
            .get::<ProfileMediaPublicImageProvider>()
            .expect("selected provider should be published");
        assert!(Arc::ptr_eq(&deployment_port, &resolved_provider.port()));

        let host = resolved.apply_to_host_runtime(rustok_api::HostRuntimeContext::new(db));
        let host_provider = host
            .shared_get::<ProfileMediaPublicImageProvider>()
            .expect("selected provider should reach server functions");
        assert!(Arc::ptr_eq(&deployment_port, &host_provider.port()));
    }

    #[tokio::test]
    async fn embedded_provider_is_registered_when_no_override_exists() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        let runtime = ServerRuntimeContext::new(db, RustokSettings::default());
        let directory = tempfile::tempdir().expect("temporary media directory should exist");
        runtime.shared_insert(local_storage(&directory));

        let resolved = attach_profile_media_public_image_provider(
            &runtime,
            Arc::new(ModuleRuntimeExtensions::default()),
        );

        assert!(resolved.contains::<ProfileMediaPublicImageProvider>());
        assert!(
            runtime
                .shared_get::<ProfileMediaPublicImageProvider>()
                .is_some()
        );
    }
}
