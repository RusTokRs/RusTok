use sea_orm_migration::prelude::*;

/// Adds the index the outbox relay claims its next batch through.
///
/// The relay claim reads `status = 'pending'` rows in created-at order, so a
/// schema without this index sorts the whole pending set on every relay
/// iteration; a schema with it reads one ordered range of claimable
/// generations. The table and the index are owned by `rustok-outbox`; this
/// migration only carries the outbox-owned helper into the platform migrator
/// (the same split as `m20260803_000001_create_owner_operation_receipts`).
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        rustok_outbox::migration::create_sys_events_claim_index(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        rustok_outbox::migration::drop_sys_events_claim_index(manager).await
    }
}
