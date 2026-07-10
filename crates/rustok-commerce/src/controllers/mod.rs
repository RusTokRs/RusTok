pub mod admin;
mod common;
pub mod products;
pub mod store;

use rustok_api::HostRuntimeContext;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;

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

impl CommerceHttpRuntime {
    fn from_host(runtime: &HostRuntimeContext) -> anyhow::Result<Self> {
        let event_bus = runtime
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Commerce HTTP routes require TransactionalEventBus in HostRuntimeContext"
                )
            })?;
        Ok(Self {
            db: runtime.db_clone(),
            event_bus,
        })
    }
}

pub fn axum_router(runtime: &HostRuntimeContext) -> anyhow::Result<axum::Router> {
    let state = CommerceHttpRuntime::from_host(runtime)?;
    Ok(axum::Router::new()
        .nest("/store", store::axum_router())
        .nest("/admin", admin::axum_router())
        .with_state(state))
}
