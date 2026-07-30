use std::sync::Arc;

use rustok_core::events::EventTransport;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::DatabaseConnection;

use crate::maintenance::SocialGraphRelationEventMaintenanceService;

impl SocialGraphRelationEventMaintenanceService {
    /// Compose the owner-local replay service with the canonical transactional outbox.
    pub fn with_outbox(db: DatabaseConnection) -> Self {
        let transport = Arc::new(OutboxTransport::new(db.clone())) as Arc<dyn EventTransport>;
        Self::new(db, TransactionalEventBus::new(transport))
    }
}
