use std::sync::Arc;

use rustok_core::ModuleRuntimeExtensions;
use rustok_notifications_api::{
    notification_source_registry_from_extensions, NotificationSourceProvider,
    NotificationSourceRegistry, NotificationSourceRegistryEntry, NotificationSourceSlug,
};

/// Owner-facing access to the registered semantic notification sources.
///
/// Persistence, fan-out, inbox, preferences and delivery orchestration are added
/// by later notification tasks. This service intentionally exposes no producer
/// database or outbox transport internals.
#[derive(Clone, Default)]
pub struct NotificationsService {
    registry: Arc<NotificationSourceRegistry>,
}

impl NotificationsService {
    pub fn new(registry: Arc<NotificationSourceRegistry>) -> Self {
        Self { registry }
    }

    pub fn from_runtime_extensions(extensions: &ModuleRuntimeExtensions) -> Self {
        let registry = notification_source_registry_from_extensions(extensions)
            .unwrap_or_else(|| Arc::new(NotificationSourceRegistry::default()));
        Self::new(registry)
    }

    pub fn source_entries(&self) -> Vec<NotificationSourceRegistryEntry> {
        self.registry.entries()
    }

    pub fn source(
        &self,
        slug: &NotificationSourceSlug,
    ) -> Option<Arc<dyn NotificationSourceProvider>> {
        self.registry.get(slug)
    }

    pub fn source_count(&self) -> usize {
        self.registry.len()
    }

    pub fn has_sources(&self) -> bool {
        !self.registry.is_empty()
    }
}
