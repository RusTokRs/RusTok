use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Adds the index the delivered-event retention prune reads through.
///
/// The prune selects `status = 'dispatched'` rows older than the configured
/// window in `ORDER BY dispatched_at LIMIT batch_size`
/// (`rustok_outbox::OutboxRetention::prune_once`), so the index leads with the
/// status and carries the ordered column. Append-only: a deployment that is
/// already running receives the index here, while the owner helper creates it
/// for module/test schemas through `SysEventsMigration`.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        rustok_outbox::migration::create_sys_events_retention_index(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        rustok_outbox::migration::drop_sys_events_retention_index(manager).await
    }
}
