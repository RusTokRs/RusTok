use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Orders the outbox-owned `sys_events` schema on the platform timeline.
///
/// `sys_events` is owned by `rustok-outbox`: the table shape lives once in
/// [`rustok_outbox::migration::create_sys_events_schema`], module and test
/// schemas reach it through `SysEventsMigration`, and this append-only wrapper
/// only decides when a platform deployment receives it. The alternative — a
/// second copy of the DDL here — is what the receipts wrapper
/// (`m20260803_000001_create_owner_operation_receipts`) already avoids, and it
/// would have made every outbox column change a two-file edit with silent drift
/// when one of the two was missed.
///
/// The indexes are deliberately not part of the schema helper: each access path
/// carries its own append-only migration
/// (`m20261007_000014_add_sys_events_claim_index`,
/// `m20261007_000015_add_sys_events_retention_index`), so a delivered-event
/// index can be added to a deployment that is already running.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        rustok_outbox::migration::create_sys_events_schema(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        rustok_outbox::migration::drop_sys_events_schema(manager).await
    }
}
