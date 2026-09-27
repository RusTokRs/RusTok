//! Runtime service handle and constructor for content orchestration.

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
mod enabled {
    use std::sync::Arc;
    use sea_orm::DatabaseConnection;
    use rustok_content::ContentOrchestrationService;
    use rustok_outbox::TransactionalEventBus;

    use crate::bridge::ServerContentOrchestrationBridge;

    #[derive(Clone)]
    pub struct SharedContentOrchestrationService(pub Arc<ContentOrchestrationService>);

    pub fn build_content_orchestration_service(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> SharedContentOrchestrationService {
        let bridge = Arc::new(ServerContentOrchestrationBridge::new(db.clone()));
        let service = Arc::new(ContentOrchestrationService::new(db, event_bus, bridge));
        SharedContentOrchestrationService(service)
    }

    pub fn content_orchestration_from_shared(
        shared: &SharedContentOrchestrationService,
    ) -> Arc<ContentOrchestrationService> {
        shared.0.clone()
    }
}

#[cfg(not(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
)))]
mod disabled {
    #[derive(Clone)]
    pub struct SharedContentOrchestrationService;

    pub fn build_content_orchestration_service(
        _db: sea_orm::DatabaseConnection,
        _event_bus: rustok_outbox::TransactionalEventBus,
    ) -> SharedContentOrchestrationService {
        SharedContentOrchestrationService
    }
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
pub use enabled::*;

#[cfg(not(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
)))]
pub use disabled::*;
