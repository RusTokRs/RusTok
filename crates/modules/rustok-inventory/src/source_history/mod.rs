//! Inventory-owned current-state baseline, serialized revisions and retained deletes.
//! This journal is not broker delivery, Index storage, a global watermark or authorization.

mod baseline;
mod model;
mod publication;
mod reader;
mod snapshot;
mod writer;

use rustok_api::{Permission, PortCallPolicy, PortContext, PortError};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, DbErr, EntityTrait, QueryFilter,
    Statement,
};
use uuid::Uuid;

pub use rustok_events::{INVENTORY_SOURCE_CHANGED_EVENT_TYPE, InventorySourceEvent};

pub use baseline::{InventoryBaselinePageRequest, InventoryBaselineProgress};
pub use reader::{InventorySourceChange, InventorySourcePage, InventorySourceRecord};
pub(crate) use writer::{prepare_deletion, record_changed, record_created};

pub const MAX_SOURCE_PAGE: usize = 128;
pub const MAX_SOURCE_LEVELS: usize = 4096;

#[derive(Debug, thiserror::Error)]
pub enum InventorySourceError {
    #[error("Inventory source request is invalid")]
    InvalidRequest,
    #[error("Inventory source operation is not authorized")]
    Forbidden,
    #[error("Inventory source work exceeds its complete page bound")]
    LimitExceeded,
    #[error("Inventory source baseline is uninitialized or still building")]
    BaselineUnavailable,
    #[error("Inventory source identity, counters or persisted revision are unverified")]
    StateUnverified,
    #[error("Inventory source baseline/page identity conflicts; recover the original operation")]
    Conflict,
    #[error("Inventory source operation requires PostgreSQL READ COMMITTED or SQLite")]
    Unsupported,
    #[error("Inventory source deadline expired; recover the original baseline and page outcome")]
    DeadlineExceeded,
    #[error("Inventory source canonical publication is unavailable")]
    PublicationUnavailable,
    #[error("Inventory source storage is unavailable")]
    Database(#[from] DbErr),
}

impl InventorySourceError {
    pub fn port_error(&self) -> PortError {
        match self {
            Self::PublicationUnavailable => PortError::unavailable(
                "inventory.source_publication_unavailable",
                "Recover the original Inventory operation; canonical source publication is unavailable",
            ),
            Self::DeadlineExceeded => PortError::timeout(
                "inventory.source_timeout",
                "Recover the original Inventory baseline and page outcome; timeout is not proof of no effect",
            ),
            Self::InvalidRequest => PortError::validation(
                "inventory.source_invalid",
                "Inventory source request is invalid",
            ),
            Self::Forbidden => PortError::forbidden(
                "inventory.source_forbidden",
                "Inventory source permission is required",
            ),
            Self::LimitExceeded => PortError::validation(
                "inventory.source_limit",
                "Use a smaller complete Inventory source page without truncating state",
            ),
            Self::Conflict => PortError::conflict(
                "inventory.source_conflict",
                "Recover the original baseline and page identity; do not reset history",
            ),
            Self::Database(_) => PortError::unavailable(
                "inventory.source_storage_unavailable",
                "Inventory source storage is unavailable; recover the original operation",
            ),
            Self::BaselineUnavailable => PortError::invariant_violation(
                "inventory.source_baseline_unavailable",
                "Complete the explicit Inventory current-state baseline; history is not inferred",
            ),
            Self::StateUnverified => PortError::invariant_violation(
                "inventory.source_state_unverified",
                "Reconcile Inventory source identity and counters without recreating missing history",
            ),
            Self::Unsupported => PortError::invariant_violation(
                "inventory.source_backend_unsupported",
                "Inventory source baseline requires PostgreSQL READ COMMITTED or SQLite",
            ),
        }
    }

    /// Existing domain error envelopes cannot encode this new owner family yet. Keep all source
    /// provenance failures nonterminal storage/infrastructure failures, never client validation.
    pub(crate) fn into_db_error(self) -> DbErr {
        match self {
            Self::Database(error) => error,
            _ => DbErr::Custom(
                "Inventory source provenance is unverified; recover the original owner operation"
                    .into(),
            ),
        }
    }
}

#[derive(Clone)]
pub struct InventorySourceHistory {
    pub(super) db: DatabaseConnection,
}

impl InventorySourceHistory {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

async fn read_budget<T>(
    context: &PortContext,
    future: impl std::future::Future<Output = Result<T, InventorySourceError>>,
) -> Result<T, InventorySourceError> {
    let budget = std::time::Duration::from_millis(
        context
            .deadline_ms
            .ok_or(InventorySourceError::InvalidRequest)?,
    );
    rustok_core::resilience::with_timeout(budget, || future)
        .await
        .map_err(|_| InventorySourceError::DeadlineExceeded)?
}

fn scope(context: &PortContext, write: bool) -> Result<(Uuid, Uuid), InventorySourceError> {
    context
        .require_policy(if write {
            PortCallPolicy::write()
        } else {
            PortCallPolicy::read()
        })
        .map_err(|_| InventorySourceError::InvalidRequest)?;
    let tenant =
        Uuid::parse_str(&context.tenant_id).map_err(|_| InventorySourceError::InvalidRequest)?;
    let actor =
        Uuid::parse_str(&context.actor.id).map_err(|_| InventorySourceError::InvalidRequest)?;
    if tenant.is_nil()
        || actor.is_nil()
        || !matches!(
            context.actor.kind,
            rustok_api::PortActorKind::User | rustok_api::PortActorKind::Service
        )
        || context.deadline_ms.is_none_or(|ms| ms == 0 || ms > 30_000)
    {
        return Err(InventorySourceError::InvalidRequest);
    }
    let manage = Permission::INVENTORY_MANAGE.to_string();
    let read = Permission::INVENTORY_READ.to_string();
    if !context
        .claims
        .iter()
        .any(|claim| claim == &manage || (!write && claim == &read))
    {
        return Err(InventorySourceError::Forbidden);
    }
    Ok((tenant, actor))
}

async fn current_snapshot<C: ConnectionTrait>(conn: &C) -> Result<(), InventorySourceError> {
    if conn.get_database_backend() == DbBackend::Postgres {
        let row = conn
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SHOW transaction_isolation",
            ))
            .await?
            .ok_or(InventorySourceError::Unsupported)?;
        let isolation: String = row.try_get("", "transaction_isolation")?;
        if !isolation.eq_ignore_ascii_case("read committed") {
            return Err(InventorySourceError::Unsupported);
        }
    }
    Ok(())
}

async fn gate<C: ConnectionTrait>(
    conn: &C,
    tenant: Uuid,
    exclusive: bool,
) -> Result<(), InventorySourceError> {
    if tenant.is_nil() {
        return Err(InventorySourceError::StateUnverified);
    }
    current_snapshot(conn).await?;
    match conn.get_database_backend() {
        DbBackend::Postgres => {
            // Both variants are nonblocking. A baseline must not wait on a writer whose existing
            // Product/Variant/item locks it needs, and a late writer must not wait on baseline.
            let sql = if exclusive {
                "SELECT pg_try_advisory_xact_lock(hashtextextended($1, 0)) AS admitted"
            } else {
                "SELECT pg_try_advisory_xact_lock_shared(hashtextextended($1, 0)) AS admitted"
            };
            let row = conn
                .query_one_raw(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    sql,
                    [format!("inventory:source-baseline:{tenant}").into()],
                ))
                .await?
                .ok_or(InventorySourceError::StateUnverified)?;
            if !row.try_get::<bool>("", "admitted")? {
                return Err(InventorySourceError::Conflict);
            }
        }
        DbBackend::Sqlite => {}
        DbBackend::MySql if exclusive => return Err(InventorySourceError::Unsupported),
        DbBackend::MySql => {}
        _ => return Err(InventorySourceError::Unsupported),
    }
    Ok(())
}

async fn saved_baseline<C: ConnectionTrait>(
    conn: &C,
    tenant: Uuid,
) -> Result<Option<model::baseline::Model>, InventorySourceError> {
    let saved = model::baseline::Entity::find_by_id(tenant)
        .one(conn)
        .await?;
    if let Some(saved) = &saved {
        if saved.tenant_id != tenant
            || saved.baseline_id.is_nil()
            || saved.actor_id.is_nil()
            || saved.record_count < 0
            || saved.last_variant_id.is_some_and(|id| id.is_nil())
            || saved.last_page_id.is_some_and(|id| id.is_nil())
            || saved.last_page_after.is_some_and(|id| id.is_nil())
            || saved.last_page_id.is_some() != saved.last_page_limit.is_some()
            || saved
                .last_page_limit
                .is_some_and(|n| !(1..=MAX_SOURCE_PAGE as i32).contains(&n))
            || (saved.active && saved.last_page_id.is_none())
        {
            return Err(InventorySourceError::StateUnverified);
        }
        if saved.last_page_id.is_none()
            && (saved.record_count != 0
                || saved.last_variant_id.is_some()
                || saved.last_page_after.is_some())
        {
            return Err(InventorySourceError::StateUnverified);
        }
        if let Some(page_id) = saved.last_page_id {
            let page = model::page::Entity::find_by_id((tenant, page_id))
                .one(conn)
                .await?
                .ok_or(InventorySourceError::StateUnverified)?;
            if page.baseline_id != saved.baseline_id
                || page.after_variant_id != saved.last_page_after
                || Some(page.page_limit) != saved.last_page_limit
                || page.next_after != saved.last_variant_id
                || page.record_count != saved.record_count
                || page.active != saved.active
            {
                return Err(InventorySourceError::StateUnverified);
            }
        }
    }
    Ok(saved)
}

async fn tracking_baseline<C: ConnectionTrait>(
    conn: &C,
    tenant: Uuid,
) -> Result<Option<Uuid>, InventorySourceError> {
    gate(conn, tenant, false).await?;
    let Some(saved) = saved_baseline(conn, tenant).await? else {
        // Uninitialized is a deliberate pre-cutover mode, not a zero stock/source assertion.
        let stranded = model::record::Entity::find()
            .filter(model::record::Column::TenantId.eq(tenant))
            .one(conn)
            .await?
            .is_some()
            || model::change::Entity::find()
                .filter(model::change::Column::TenantId.eq(tenant))
                .one(conn)
                .await?
                .is_some()
            || model::page::Entity::find()
                .filter(model::page::Column::TenantId.eq(tenant))
                .one(conn)
                .await?
                .is_some();
        if stranded {
            return Err(InventorySourceError::StateUnverified);
        }
        return Ok(None);
    };
    if conn.get_database_backend() == DbBackend::MySql {
        return Err(InventorySourceError::Unsupported);
    }
    if !saved.active {
        return Err(InventorySourceError::BaselineUnavailable);
    }
    Ok(Some(saved.baseline_id))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod physical_tests;
