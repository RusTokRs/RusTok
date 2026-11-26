use std::sync::Arc;

use rustok_cli_core::{CliCoreError, CliCoreResult};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_runtime::{RuntimeComposition, db_clone};
use sea_orm::DatabaseConnection;

pub(crate) fn owner_resources(
    runtime: &RuntimeComposition,
) -> CliCoreResult<(DatabaseConnection, TransactionalEventBus)> {
    let host = runtime
        .require_host()
        .map_err(|error| CliCoreError::CommandFailed {
            message: format!("blog CLI requires a database runtime: {error}"),
        })?;
    let db = db_clone(host);
    let events = host
        .shared_get::<TransactionalEventBus>()
        .unwrap_or_else(|| TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone()))));
    Ok((db, events))
}
