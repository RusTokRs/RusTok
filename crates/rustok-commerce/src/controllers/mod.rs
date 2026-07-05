pub mod admin;
mod common;
pub mod products;
pub mod store;

use loco_rs::{app::AppContext, controller::Routes};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

#[derive(Clone)]
pub struct CommerceHttpRuntime {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl CommerceHttpRuntime {
    fn db_clone(&self) -> DatabaseConnection {
        self.db.clone()
    }

    fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    fn event_bus(&self) -> TransactionalEventBus {
        self.event_bus.clone()
    }
}

impl axum::extract::FromRef<AppContext> for CommerceHttpRuntime {
    fn from_ref(input: &AppContext) -> Self {
        let transport = Arc::new(OutboxTransport::new(input.db.clone()));
        Self {
            db: input.db.clone(),
            event_bus: TransactionalEventBus::new(transport),
        }
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .nest("/store", store::routes())
        .nest("/admin", admin::routes())
}
