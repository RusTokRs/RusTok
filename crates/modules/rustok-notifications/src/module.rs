use std::sync::Arc;

use async_trait::async_trait;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use rustok_core::ModuleRuntimeExtensions;
use rustok_notifications_api::ensure_notification_source_registry;
use sea_orm_migration::MigrationTrait;

use crate::inbox_reconcile::NotificationInboxReconciliationInspectPortFactoryImpl;
use crate::migrations;

/// Notification inbox, preferences, bounded fan-out, grouping, digests, and delivery orchestration.
pub struct NotificationsModule;

#[async_trait]
impl RusToKModule for NotificationsModule {
    fn slug(&self) -> &'static str {
        "notifications"
    }

    fn name(&self) -> &'static str {
        "Notifications"
    }

    fn description(&self) -> &'static str {
        "Notification inbox, preferences, bounded fan-out, grouping, digests, and delivery orchestration"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }

    fn dependencies(&self) -> &[&'static str] {
        &["outbox"]
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        let _ = ensure_notification_source_registry(extensions);
        if extensions.contains::<
            Arc<dyn rustok_notifications_api::NotificationInboxReconciliationInspectPortFactory>,
        >() {
            return Err(rustok_core::Error::Validation(
                "notifications reconciliation inspection port factory is already registered"
                    .to_string(),
            ));
        }
        let factory: Arc<
            dyn rustok_notifications_api::NotificationInboxReconciliationInspectPortFactory,
        > = Arc::new(NotificationInboxReconciliationInspectPortFactoryImpl);
        extensions.insert(factory);
        Ok(())
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}

impl MigrationSource for NotificationsModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
    use rustok_core::ModuleRuntimeExtensions;
    use rustok_notifications_api::{
        NotificationInboxReconciliationInspectPortFactory,
        notification_source_registry_from_extensions,
    };

    use super::NotificationsModule;
    use crate::service::NotificationsService;

    #[tokio::test]
    async fn module_initializes_source_registry_and_persistence_migrations() {
        let module = NotificationsModule;
        assert_eq!(module.slug(), "notifications");
        assert_eq!(module.name(), "Notifications");
        assert_eq!(module.kind(), ModuleKind::Core);
        assert_eq!(module.dependencies(), &["outbox"]);
        assert_eq!(module.health().await, HealthStatus::Healthy);
        assert_eq!(module.migrations().len(), 7);
        assert_eq!(module.migration_dependencies().len(), 7);

        let mut extensions = ModuleRuntimeExtensions::default();
        module
            .register_runtime_extensions(&mut extensions)
            .expect("notification runtime extensions should initialize");
        assert!(notification_source_registry_from_extensions(&extensions).is_some());
        assert!(
            extensions
                .get::<Arc<dyn NotificationInboxReconciliationInspectPortFactory>>()
                .is_some()
        );
        assert!(module.register_runtime_extensions(&mut extensions).is_err());

        let service = NotificationsService::from_runtime_extensions(&extensions);
        assert_eq!(service.source_count(), 0);
        assert!(!service.has_sources());
    }
}
