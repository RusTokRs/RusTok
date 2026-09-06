use std::sync::Arc;

use rustok_core::MigrationSource;
use rustok_core::events::EventTransport;
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_social_graph::SocialGraphService;
use sea_orm::DatabaseConnection;
use sea_orm_migration::SchemaManager;

pub fn write_service(db: DatabaseConnection) -> SocialGraphService {
    let transport: Arc<dyn EventTransport> = Arc::new(OutboxTransport::new(db.clone()));
    SocialGraphService::with_event_bus(db, TransactionalEventBus::new(transport))
}

pub async fn migrate_outbox(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in OutboxModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("outbox migration should apply");
    }
}
