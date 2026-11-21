use sea_orm_migration::prelude::*;

/// Adds the `sys_events.tenant_id` projection column, its operator-read
/// index, and the table integrity constraints.
///
/// Tenant-scoped operator reads (server DLQ API, module admin counters)
/// previously extracted the tenant from the JSON payload on every query —
/// a full scan per counter and a second, fragile source of tenant truth.
/// The column is owned by `rustok-outbox` like the rest of the `sys_events`
/// shape; this migration only orders the outbox helpers on the platform
/// timeline. The backfill copies the canonical payload tenant into the
/// column for pre-existing rows; new writers populate it at insert time.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        rustok_outbox::migration::add_sys_events_tenant_column(manager).await?;
        rustok_outbox::migration::create_sys_events_tenant_index(manager).await?;
        rustok_outbox::migration::add_sys_events_integrity_checks(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        rustok_outbox::migration::drop_sys_events_integrity_checks(manager).await?;
        rustok_outbox::migration::drop_sys_events_tenant_column(manager).await
    }
}
