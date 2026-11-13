use async_trait::async_trait;
use rustok_api::{Action, Permission, Resource};
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use rustok_core::{
    MigrationDependencyDescriptor, ModuleEventListenerContext, ModuleEventListenerRegistry,
    ModuleRuntimeExtensions,
};
use rustok_seo_targets::register_seo_target_provider;
use sea_orm_migration::MigrationTrait;

use crate::cache_invalidation::{PageCacheInvalidationEventHandler, PagesCacheInvalidationRuntime};
use crate::provider_health_binding::{
    PAGES_PROVIDER_HEALTH_ACCEPTANCE_PATH_ENV,
    page_builder_provider_health_authority_from_environment,
};
use crate::services::{PAGES_INLINE_EDIT_HMAC_KEY_ENV, page_inline_edit_keyring_from_environment};
use crate::{migrations, search_projection, seo_targets};

/// Pages module instance.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PagesModule;

impl PagesModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for PagesModule {
    fn slug(&self) -> &'static str {
        "pages"
    }

    fn name(&self) -> &'static str {
        "Pages"
    }

    fn description(&self) -> &'static str {
        "Pages, visual documents and published artifacts"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Optional
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn dependencies(&self) -> &[&'static str] {
        &["content", "page_builder", "outbox"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::new(Resource::Pages, Action::Create),
            Permission::new(Resource::Pages, Action::Read),
            Permission::new(Resource::Pages, Action::Update),
            Permission::new(Resource::Pages, Action::Delete),
            Permission::new(Resource::Pages, Action::List),
            Permission::new(Resource::Pages, Action::Publish),
            Permission::new(Resource::Pages, Action::Manage),
        ]
    }

    fn register_event_listeners(
        &self,
        registry: &mut ModuleEventListenerRegistry,
        ctx: &ModuleEventListenerContext<'_>,
    ) {
        let Some(runtime) = ctx
            .extensions
            .get::<PagesCacheInvalidationRuntime>()
            .cloned()
        else {
            tracing::warn!(
                "Pages cache invalidation runtime is not configured; no Pages cache listener registered"
            );
            return;
        };
        registry.register(PageCacheInvalidationEventHandler::new(runtime));
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        rustok_core::search_projection::register_search_projection_source(
            extensions,
            search_projection::PageSearchProjectionSourceFactory,
        )
        .map_err(|error| {
            rustok_core::Error::Validation(format!(
                "pages Search projection source registration failed: {error}"
            ))
        })?;
        register_seo_target_provider(extensions, seo_targets::PagesSeoTargetProvider).map_err(
            |error| {
                rustok_core::Error::Validation(format!(
                    "pages SEO target registration failed: {error}"
                ))
            },
        )?;
        match page_inline_edit_keyring_from_environment().map_err(|error| {
            rustok_core::Error::Validation(format!(
                "pages inline edit signing runtime configuration failed: {error}"
            ))
        })? {
            Some(keyring) => {
                extensions.insert(keyring);
                tracing::debug!("Pages inline edit signing runtime registered");
            }
            None => {
                tracing::debug!(
                    env = PAGES_INLINE_EDIT_HMAC_KEY_ENV,
                    "Pages inline edit signing runtime is not configured"
                );
            }
        }

        match page_builder_provider_health_authority_from_environment() {
            Ok(Some(authority)) => {
                tracing::info!(
                    source_commit = %authority.source_commit(),
                    deployment_id = %authority.deployment_id(),
                    provider_health_state = authority
                        .current_snapshot()
                        .map(|snapshot| snapshot.state.as_str())
                        .unwrap_or("unobserved"),
                    "Pages Page Builder provider-health authority registered"
                );
                extensions.insert(authority);
            }
            Ok(None) => {
                tracing::debug!(
                    env = PAGES_PROVIDER_HEALTH_ACCEPTANCE_PATH_ENV,
                    "Pages Page Builder provider-health authority is not configured"
                );
            }
            Err(error) => {
                tracing::warn!(
                    error_code = error.code(),
                    "Pages Page Builder provider-health authority rejected; provider health remains unobserved"
                );
            }
        }

        #[cfg(feature = "runtime")]
        {
            let registrations = extensions
                .get_or_insert_with::<rustok_runtime::ModuleWorkRegistrations, _>(Default::default);
            registrations.register(std::sync::Arc::new(
                crate::services::PageScheduledPublishWorkRegistration,
            ));
        }
        Ok(())
    }
}

impl MigrationSource for PagesModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pages_module_metadata_and_traits() {
        let module = PagesModule::new();
        assert_eq!(module.slug(), "pages");
        assert_eq!(module.name(), "Pages");
        assert_eq!(
            module.description(),
            "Pages, visual documents and published artifacts"
        );
        assert!(!module.version().is_empty());
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.health().await, HealthStatus::Healthy);
        assert_eq!(
            module.dependencies(),
            &["content", "page_builder", "outbox"]
        );
        assert_eq!(module.permissions().len(), 7);
        assert!(!module.migrations().is_empty());
        assert!(!module.migration_dependencies().is_empty());
    }

    /// Scheduled publishing only fires if the deployed host learns about the queue. This pins the
    /// registration to `register_runtime_extensions`, which is the single place the host reads
    /// module work from: dropping it would leave schedules inert while every other Pages surface
    /// kept working.
    #[cfg(feature = "runtime")]
    #[test]
    fn pages_registers_scheduled_publishing_module_work() {
        let module = PagesModule::new();
        let mut extensions = ModuleRuntimeExtensions::default();
        module
            .register_runtime_extensions(&mut extensions)
            .expect("pages runtime extension registration");
        let registrations = extensions
            .get::<rustok_runtime::ModuleWorkRegistrations>()
            .expect("pages must publish module work registrations");
        assert!(!registrations.is_empty());
    }

    /// The registered worker slug is a contract, not an internal detail: it names the durable
    /// queue in scheduler logs and metrics, so a rename has to be a deliberate migration.
    #[cfg(feature = "runtime")]
    #[test]
    fn scheduled_publishing_worker_slug_is_stable() {
        assert_eq!(
            crate::services::PAGE_SCHEDULED_PUBLISH_WORKER,
            "page_scheduled_publish"
        );
    }
}
