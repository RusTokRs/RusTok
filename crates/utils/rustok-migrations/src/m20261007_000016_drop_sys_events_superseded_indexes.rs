use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Drops the two `sys_events` indexes the relay claim no longer reads.
///
/// `idx_sys_events_pending_next_attempt (status, next_attempt_at)` and
/// `idx_sys_events_claimed_at (claimed_at)` predate the claim index
/// (`m20261007_000014_add_sys_events_claim_index`). The claim reads
/// `status = 'pending'` in `ORDER BY created_at LIMIT batch_size`, with
/// `claimed_at` / `next_attempt_at` as residual predicates of that ordered
/// range, and no query in the platform filters on either column on its own, so
/// the pair costs every write and serves nothing. Append-only, like the index
/// it supersedes: a deployed schema still carries the pair, a fresh schema never
/// creates it, and both drops are `if_exists`.
///
/// `down` restores the pair so a rollback returns the schema to its previous
/// shape rather than a third variant.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        rustok_outbox::migration::drop_sys_events_superseded_indexes(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        rustok_outbox::migration::create_sys_events_superseded_indexes(manager).await
    }
}
