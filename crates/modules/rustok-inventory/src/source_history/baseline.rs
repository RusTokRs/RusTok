use chrono::Utc;
use rustok_api::PortContext;
use rustok_commerce_foundation::entities::product_variant;
use rustok_core::resilience::with_timeout;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait, sea_query::Expr,
};
use std::time::{Duration, Instant};
use uuid::Uuid;

use super::{
    InventorySourceError as E, InventorySourceHistory, MAX_SOURCE_LEVELS,
    model::{baseline, change, page, record},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventoryBaselineProgress {
    pub tenant_id: Uuid,
    pub baseline_id: Uuid,
    pub active: bool,
    pub after_variant_id: Option<Uuid>,
    pub record_count: u64,
    pub last_page_id: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventoryBaselinePageRequest {
    pub baseline_id: Uuid,
    pub page_id: Uuid,
    pub after_variant_id: Option<Uuid>,
    pub limit: usize,
}

impl InventorySourceHistory {
    /// Explicit current-state-only declaration. Owner bounds work/acquisition/commit by the
    /// current context budget; commit uncertainty requires status under this original ID.
    pub async fn start_baseline(
        &self,
        context: &PortContext,
        baseline_id: Uuid,
    ) -> Result<InventoryBaselineProgress, E> {
        let (tenant, actor) = super::scope(context, true)?;
        if baseline_id.is_nil() {
            return Err(E::InvalidRequest);
        }
        let (txn, started, budget) = begin(&self.db, context).await?;
        let result = with_timeout(budget.saturating_sub(started.elapsed()), || {
            start_in(&txn, tenant, actor, baseline_id)
        })
        .await
        .unwrap_or(Err(E::DeadlineExceeded));
        finish(txn, result, started, budget).await
    }

    pub async fn advance_baseline(
        &self,
        context: &PortContext,
        request: InventoryBaselinePageRequest,
    ) -> Result<InventoryBaselineProgress, E> {
        let (tenant, actor) = super::scope(context, true)?;
        let publication_actor =
            crate::stock_events::event_actor_id(context).map_err(|_| E::InvalidRequest)?;
        super::reader::page_limit(request.limit)?;
        if request.baseline_id.is_nil()
            || request.page_id.is_nil()
            || request.after_variant_id.is_some_and(|id| id.is_nil())
        {
            return Err(E::InvalidRequest);
        }
        let (txn, started, budget) = begin(&self.db, context).await?;
        let result = with_timeout(budget.saturating_sub(started.elapsed()), || {
            advance_in(&txn, tenant, actor, publication_actor, &request)
        })
        .await
        .unwrap_or(Err(E::DeadlineExceeded));
        finish(txn, result, started, budget).await
    }

    pub async fn baseline_status(
        &self,
        context: &PortContext,
    ) -> Result<Option<InventoryBaselineProgress>, E> {
        let (tenant, _) = super::scope(context, false)?;
        super::read_budget(context, async {
            super::saved_baseline(&self.db, tenant)
                .await?
                .as_ref()
                .map(progress)
                .transpose()
        })
        .await
    }
}

fn progress(saved: &baseline::Model) -> Result<InventoryBaselineProgress, E> {
    Ok(InventoryBaselineProgress {
        tenant_id: saved.tenant_id,
        baseline_id: saved.baseline_id,
        active: saved.active,
        after_variant_id: saved.last_variant_id,
        record_count: u64::try_from(saved.record_count).map_err(|_| E::StateUnverified)?,
        last_page_id: saved.last_page_id,
    })
}

async fn start_in(
    txn: &DatabaseTransaction,
    tenant: Uuid,
    actor: Uuid,
    id: Uuid,
) -> Result<InventoryBaselineProgress, E> {
    super::gate(txn, tenant, true).await?;
    if let Some(saved) = super::saved_baseline(txn, tenant).await? {
        if saved.baseline_id != id || saved.actor_id != actor {
            return Err(E::Conflict);
        }
        return progress(&saved);
    }
    if record::Entity::find()
        .filter(record::Column::TenantId.eq(tenant))
        .one(txn)
        .await?
        .is_some()
        || change::Entity::find()
            .filter(change::Column::TenantId.eq(tenant))
            .one(txn)
            .await?
            .is_some()
        || page::Entity::find()
            .filter(page::Column::TenantId.eq(tenant))
            .one(txn)
            .await?
            .is_some()
    {
        return Err(E::StateUnverified);
    }
    let now = Utc::now();
    let affected = baseline::Entity::insert(baseline::ActiveModel {
        tenant_id: Set(tenant),
        baseline_id: Set(id),
        actor_id: Set(actor),
        active: Set(false),
        last_variant_id: Set(None),
        record_count: Set(0),
        last_page_id: Set(None),
        last_page_after: Set(None),
        last_page_limit: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    })
    .exec_without_returning(txn)
    .await?;
    if affected != 1 {
        return Err(E::StateUnverified);
    }
    let saved = super::saved_baseline(txn, tenant)
        .await?
        .ok_or(E::StateUnverified)?;
    progress(&saved)
}

async fn advance_in(
    txn: &DatabaseTransaction,
    tenant: Uuid,
    actor: Uuid,
    publication_actor: Option<Uuid>,
    request: &InventoryBaselinePageRequest,
) -> Result<InventoryBaselineProgress, E> {
    super::gate(txn, tenant, true).await?;
    let saved = super::saved_baseline(txn, tenant)
        .await?
        .ok_or(E::BaselineUnavailable)?;
    if saved.baseline_id != request.baseline_id || saved.actor_id != actor {
        return Err(E::Conflict);
    }
    if let Some(receipt) = page::Entity::find_by_id((tenant, request.page_id))
        .one(txn)
        .await?
    {
        if receipt.baseline_id != request.baseline_id
            || receipt.after_variant_id != request.after_variant_id
            || receipt.page_limit != request.limit as i32
        {
            return Err(E::Conflict);
        }
        if receipt.record_count < 0 || receipt.next_after.is_some_and(|id| id.is_nil()) {
            return Err(E::StateUnverified);
        }
        // Replay the original durable page result, even after later pages/activation.
        return Ok(InventoryBaselineProgress {
            tenant_id: tenant,
            baseline_id: receipt.baseline_id,
            active: receipt.active,
            after_variant_id: receipt.next_after,
            record_count: receipt.record_count as u64,
            last_page_id: Some(receipt.page_id),
        });
    }
    if saved.active || saved.last_variant_id != request.after_variant_id {
        return Err(E::Conflict);
    }
    // SQLite must acquire writer exclusion before the first snapshot query. A concurrent writer
    // which committed after the baseline read makes this transaction fail instead of copying
    // a stale snapshot; an ignored gate effect cannot activate/copy the page.
    if txn.get_database_backend() == DbBackend::Sqlite {
        let same_count: sea_orm::sea_query::SimpleExpr =
            Expr::col(baseline::Column::RecordCount).into();
        let locked = baseline::Entity::update_many()
            .col_expr(baseline::Column::RecordCount, same_count)
            .filter(baseline::Column::TenantId.eq(tenant))
            .filter(baseline::Column::BaselineId.eq(request.baseline_id))
            .filter(baseline::Column::Active.eq(false))
            .exec(txn)
            .await?;
        if locked.rows_affected != 1 {
            return Err(E::StateUnverified);
        }
    }
    let mut query = product_variant::Entity::find()
        .select_only()
        .column(product_variant::Column::Id)
        .filter(product_variant::Column::TenantId.eq(tenant))
        .order_by_asc(product_variant::Column::Id)
        .limit((request.limit + 1) as u64);
    if let Some(after) = request.after_variant_id {
        query = query.filter(product_variant::Column::Id.gt(after));
    }
    let mut ids = query.into_tuple::<Uuid>().all(txn).await?;
    let more = ids.len() > request.limit;
    ids.truncate(request.limit);
    let mut snapshots = Vec::with_capacity(ids.len());
    let mut level_count = 0_usize;
    for id in &ids {
        let remaining = MAX_SOURCE_LEVELS
            .checked_sub(level_count)
            .ok_or(E::LimitExceeded)?;
        let snapshot = super::snapshot::load_with_budget(txn, tenant, *id, remaining).await?;
        level_count = level_count
            .checked_add(snapshot.level_count)
            .ok_or(E::LimitExceeded)?;
        if level_count > MAX_SOURCE_LEVELS {
            return Err(E::LimitExceeded);
        }
        snapshots.push(snapshot);
    }
    for snapshot in snapshots {
        super::writer::insert(txn, tenant, request.baseline_id, snapshot, "baseline").await?;
    }
    let after = ids.last().copied().or(saved.last_variant_id);
    let count = saved
        .record_count
        .checked_add(ids.len() as i64)
        .ok_or(E::LimitExceeded)?;
    let affected = page::Entity::insert(page::ActiveModel {
        tenant_id: Set(tenant),
        page_id: Set(request.page_id),
        baseline_id: Set(request.baseline_id),
        after_variant_id: Set(request.after_variant_id),
        page_limit: Set(request.limit as i32),
        next_after: Set(after),
        active: Set(!more),
        record_count: Set(count),
        created_at: Set(Utc::now().into()),
    })
    .exec_without_returning(txn)
    .await?;
    if affected != 1 {
        return Err(E::StateUnverified);
    }
    let mut update = baseline::Entity::update_many()
        .col_expr(baseline::Column::Active, Expr::value(!more))
        .col_expr(baseline::Column::LastVariantId, Expr::value(after))
        .col_expr(baseline::Column::RecordCount, Expr::value(count))
        .col_expr(baseline::Column::LastPageId, Expr::value(request.page_id))
        .col_expr(
            baseline::Column::LastPageAfter,
            Expr::value(request.after_variant_id),
        )
        .col_expr(
            baseline::Column::LastPageLimit,
            Expr::value(request.limit as i32),
        )
        .col_expr(baseline::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(baseline::Column::TenantId.eq(tenant))
        .filter(baseline::Column::BaselineId.eq(request.baseline_id))
        .filter(baseline::Column::Active.eq(false))
        .filter(baseline::Column::RecordCount.eq(saved.record_count));
    update = if let Some(after) = saved.last_variant_id {
        update.filter(baseline::Column::LastVariantId.eq(after))
    } else {
        update.filter(baseline::Column::LastVariantId.is_null())
    };
    if update.exec(txn).await?.rows_affected != 1 {
        return Err(E::StateUnverified);
    }
    let saved = super::saved_baseline(txn, tenant)
        .await?
        .ok_or(E::StateUnverified)?;
    if saved.active {
        super::publication::activated(txn, tenant, publication_actor, saved.baseline_id).await?;
    }
    progress(&saved)
}

async fn begin(
    db: &DatabaseConnection,
    context: &PortContext,
) -> Result<(DatabaseTransaction, Instant, Duration), E> {
    let budget = Duration::from_millis(context.deadline_ms.ok_or(E::InvalidRequest)?);
    let started = Instant::now();
    let txn = with_timeout(budget, || db.begin())
        .await
        .map_err(|_| E::DeadlineExceeded)??;
    Ok((txn, started, budget))
}

async fn finish(
    txn: DatabaseTransaction,
    result: Result<InventoryBaselineProgress, E>,
    started: Instant,
    budget: Duration,
) -> Result<InventoryBaselineProgress, E> {
    match result {
        Err(error) => {
            // Cleanup is awaited even if it exceeds the work budget. No known refusal precedes it.
            txn.rollback().await?;
            Err(error)
        }
        Ok(progress) => {
            let remaining = budget.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                txn.rollback().await?;
                return Err(E::DeadlineExceeded);
            }
            // A timeout while committing is an unknown outcome. Preserve the original durable
            // baseline/page identity; status/replay, not key rotation, recovers the result.
            with_timeout(remaining, || txn.commit())
                .await
                .map_err(|_| E::DeadlineExceeded)??;
            Ok(progress)
        }
    }
}
