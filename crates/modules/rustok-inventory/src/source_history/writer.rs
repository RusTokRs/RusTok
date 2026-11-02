use chrono::Utc;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseTransaction, DbErr, EntityTrait, QueryFilter, Set,
    sea_query::Expr,
};
use uuid::Uuid;

use super::{
    InventorySourceError as E,
    model::{change, record},
    snapshot::Snapshot,
};

pub(super) async fn insert(
    conn: &DatabaseTransaction,
    tenant: Uuid,
    baseline: Uuid,
    snapshot: Snapshot,
    kind: &str,
) -> Result<Uuid, E> {
    let change_id = Uuid::new_v4();
    let now = Utc::now();
    let count = record::Entity::insert(record::ActiveModel {
        tenant_id: Set(tenant),
        variant_id: Set(snapshot.variant_id),
        baseline_id: Set(baseline),
        inventory_item_id: Set(snapshot.item_id),
        product_id: Set(snapshot.product_id),
        source_version: Set(1),
        deleted: Set(false),
        stocked: Set(snapshot.stocked),
        reserved: Set(snapshot.reserved),
        incoming: Set(snapshot.incoming),
        binding_digest: Set(snapshot.binding_digest),
        last_change_id: Set(change_id),
        updated_at: Set(now.into()),
    })
    .exec_without_returning(conn)
    .await?;
    if count != 1 {
        return Err(E::StateUnverified);
    }
    append(
        conn,
        tenant,
        baseline,
        snapshot.variant_id,
        1,
        change_id,
        kind,
    )
    .await?;
    Ok(change_id)
}

async fn append<C: ConnectionTrait>(
    conn: &C,
    tenant: Uuid,
    baseline: Uuid,
    variant: Uuid,
    version: i64,
    id: Uuid,
    kind: &str,
) -> Result<(), E> {
    if !matches!(kind, "baseline" | "created" | "changed" | "deleted") {
        return Err(E::StateUnverified);
    }
    let affected = change::Entity::insert(change::ActiveModel {
        change_id: Set(id),
        tenant_id: Set(tenant),
        variant_id: Set(variant),
        baseline_id: Set(baseline),
        source_version: Set(version),
        kind: Set(kind.to_owned()),
        created_at: Set(Utc::now().into()),
    })
    .exec_without_returning(conn)
    .await?;
    if affected != 1 {
        return Err(E::StateUnverified);
    }
    Ok(())
}

pub(crate) async fn record_created(
    conn: &DatabaseTransaction,
    tenant: Uuid,
    variant: Uuid,
    actor: Option<Uuid>,
) -> Result<(), DbErr> {
    async {
        let Some(baseline) = super::tracking_baseline(conn, tenant).await? else {
            return Ok(());
        };
        let snapshot = super::snapshot::load(conn, tenant, variant).await?;
        // Only this fresh Product bootstrap path can establish a new identity after activation.
        // An existing live record or tombstone is not reset/reused, even if its counters are zero.
        let change_id = insert(conn, tenant, baseline, snapshot, "created").await?;
        super::publication::changed(conn, tenant, actor, baseline, variant, 1, change_id).await
    }
    .await
    .map_err(E::into_db_error)
}

pub(crate) async fn record_changed(
    conn: &DatabaseTransaction,
    tenant: Uuid,
    variant: Uuid,
    actor: Option<Uuid>,
) -> Result<(), DbErr> {
    async {
        let Some(baseline) = super::tracking_baseline(conn, tenant).await? else {
            return Ok(());
        };
        let snapshot = super::snapshot::load(conn, tenant, variant).await?;
        let saved = live(conn, tenant, baseline, variant).await?;
        prove_binding(&saved, &snapshot)?;
        // The caller's exact physical/reservation plan proves an actual effect. Even equal global
        // availability is not a no-op when stocked/reserved or original levels moved.
        update(
            conn,
            saved,
            snapshot.stocked,
            snapshot.reserved,
            snapshot.incoming,
            false,
            actor,
        )
        .await
    }
    .await
    .map_err(E::into_db_error)
}

/// Original live source proof must be captured before any level/item is deleted.
/// Private, non-cloneable and consumed by the same concrete caller transaction.
pub(crate) struct PreparedSourceDeletion<'a> {
    conn: &'a DatabaseTransaction,
    saved: Option<record::Model>,
    actor: Option<Uuid>,
}

pub(crate) async fn prepare_deletion<'a>(
    conn: &'a DatabaseTransaction,
    tenant: Uuid,
    variant: Uuid,
    item: Uuid,
    actor: Option<Uuid>,
) -> Result<PreparedSourceDeletion<'a>, DbErr> {
    async {
        let Some(baseline) = super::tracking_baseline(conn, tenant).await? else {
            return Ok(PreparedSourceDeletion {
                conn,
                saved: None,
                actor,
            });
        };
        let snapshot = super::snapshot::load(conn, tenant, variant).await?;
        let saved = live(conn, tenant, baseline, variant).await?;
        prove_binding(&saved, &snapshot)?;
        if item.is_nil()
            || saved.inventory_item_id != item
            || saved.stocked != 0
            || saved.reserved != 0
            || saved.incoming != 0
            || snapshot.stocked != 0
            || snapshot.reserved != 0
            || snapshot.incoming != 0
        {
            return Err(E::StateUnverified);
        }
        Ok(PreparedSourceDeletion {
            conn,
            saved: Some(saved),
            actor,
        })
    }
    .await
    .map_err(E::into_db_error)
}

impl PreparedSourceDeletion<'_> {
    pub(crate) async fn record_deleted(self) -> Result<(), DbErr> {
        if let Some(saved) = self.saved {
            update(self.conn, saved, 0, 0, 0, true, self.actor)
                .await
                .map_err(E::into_db_error)?;
        }
        Ok(())
    }
}

fn prove_binding(saved: &record::Model, snapshot: &Snapshot) -> Result<(), E> {
    if saved.inventory_item_id != snapshot.item_id
        || saved.product_id != snapshot.product_id
        || saved.binding_digest != snapshot.binding_digest
    {
        return Err(E::StateUnverified);
    }
    Ok(())
}

async fn live<C: ConnectionTrait>(
    conn: &C,
    tenant: Uuid,
    baseline: Uuid,
    variant: Uuid,
) -> Result<record::Model, E> {
    let saved = record::Entity::find_by_id((tenant, variant))
        .one(conn)
        .await?
        .ok_or(E::StateUnverified)?;
    super::reader::validate_record(&saved, tenant, baseline)?;
    if saved.deleted {
        return Err(E::StateUnverified);
    }
    let head = change::Entity::find_by_id(saved.last_change_id)
        .one(conn)
        .await?
        .ok_or(E::StateUnverified)?;
    super::reader::validate_change(&head, tenant, baseline)?;
    if head.variant_id != variant
        || head.source_version != saved.source_version
        || head.kind == "deleted"
    {
        return Err(E::StateUnverified);
    }
    Ok(saved)
}

pub(super) fn next_version(version: i64) -> Result<i64, E> {
    if version <= 0 {
        return Err(E::StateUnverified);
    }
    version.checked_add(1).ok_or(E::StateUnverified)
}

async fn update(
    conn: &DatabaseTransaction,
    saved: record::Model,
    stocked: i64,
    reserved: i64,
    incoming: i64,
    deleted: bool,
    actor: Option<Uuid>,
) -> Result<(), E> {
    let version = next_version(saved.source_version)?;
    let id = Uuid::new_v4();
    let result = record::Entity::update_many()
        .col_expr(record::Column::SourceVersion, Expr::value(version))
        .col_expr(record::Column::Stocked, Expr::value(stocked))
        .col_expr(record::Column::Reserved, Expr::value(reserved))
        .col_expr(record::Column::Incoming, Expr::value(incoming))
        .col_expr(record::Column::Deleted, Expr::value(deleted))
        .col_expr(record::Column::LastChangeId, Expr::value(id))
        .col_expr(record::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(record::Column::TenantId.eq(saved.tenant_id))
        .filter(record::Column::VariantId.eq(saved.variant_id))
        .filter(record::Column::BaselineId.eq(saved.baseline_id))
        .filter(record::Column::InventoryItemId.eq(saved.inventory_item_id))
        .filter(record::Column::ProductId.eq(saved.product_id))
        .filter(record::Column::SourceVersion.eq(saved.source_version))
        .filter(record::Column::LastChangeId.eq(saved.last_change_id))
        .filter(record::Column::Deleted.eq(false))
        .exec(conn)
        .await?;
    if result.rows_affected != 1 {
        return Err(E::StateUnverified);
    }
    append(
        conn,
        saved.tenant_id,
        saved.baseline_id,
        saved.variant_id,
        version,
        id,
        if deleted { "deleted" } else { "changed" },
    )
    .await?;
    super::publication::changed(
        conn,
        saved.tenant_id,
        actor,
        saved.baseline_id,
        saved.variant_id,
        version,
        id,
    )
    .await
}
