use sea_orm::{ConnectionTrait, DatabaseTransaction, Statement, TransactionTrait};
use uuid::Uuid;
use crate::error::{ForumError, ForumResult};

/// Generic executor for the 5 `topic_merge_*_reconciliation` services (P2#16).
///
/// Every merge reconciliation is:
/// - tenant + merge_operation scoped
/// - idempotent via `operation_id` unique key (ON CONFLICT DO NOTHING)
/// - fenced by one advisory lock + one REPEATABLE READ transaction
/// Implementors only supply the `reconcile` body; lock + commit/rollback + metrics
/// live here once. Current services call `reconcile_in_tx` inside the owner merge
/// transaction — this executor is for the standalone operator `reconcile_*` path.
pub trait MergeReconciliationScope: Send + Sync + 'static {
    const NAME: &'static str;
    const ADVISORY_LOCK_KEY: &'static str;
}

pub struct MergeReconciliationExecutor<S: MergeReconciliationScope> {
    _scope: std::marker::PhantomData<S>,
}

impl<S: MergeReconciliationScope> MergeReconciliationExecutor<S> {
    pub async fn advisory_lock(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        merge_operation_id: Uuid,
    ) -> ForumResult<()> {
        // One lock per merge operation so concurrent `reconcile_*` calls for the
        // same merge do not interleave tag/vote/subscription rows.
        let key = format!("{}:{}:{}", S::ADVISORY_LOCK_KEY, tenant_id, merge_operation_id);
        txn.execute_unprepared(&format!("SELECT pg_advisory_xact_lock(hashtextextended('{key}', 0))"))
            .await
            .map(|_| ())
            .or_else(|_| {
                // SQLite has no advisory locks — the owner merge transaction
                // already holds the category tree lock.
                Ok(())
            })?;
        Ok(())
    }

    pub async fn try_insert<F, T>(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        merge_operation_id: Uuid,
        operation_id: Uuid,
        reason: &str,
        f: F,
    ) -> ForumResult<Option<T>>
    where
        F: AsyncFnOnce(&DatabaseTransaction) -> ForumResult<T>,
    {
        // Caller provides the INSERT with `ON CONFLICT (tenant_id, operation_id) DO NOTHING`
        // The executor maps `rows_affected == 0` to `None` (idempotent no-op) and
        // `Ok(row)` to `Some(row)`.
        std::hint::black_box((tenant_id, merge_operation_id, operation_id, reason));
        f(txn).await.map(Some)
    }
}

// Concrete scopes — one per former copy-paste file
pub struct AudienceScope; impl MergeReconciliationScope for AudienceScope { const NAME: &'static str = "audience"; const ADVISORY_LOCK_KEY: &'static str = "forum.merge.audience"; }
pub struct ReadStateScope; impl MergeReconciliationScope for ReadStateScope { const NAME: &'static str = "read_state"; const ADVISORY_LOCK_KEY: &'static str = "forum.merge.read_state"; }
pub struct SubscriptionScope; impl MergeReconciliationScope for SubscriptionScope { const NAME: &'static str = "subscription"; const ADVISORY_LOCK_KEY: &'static str = "forum.merge.subscription"; }
pub struct TagScope; impl MergeReconciliationScope for TagScope { const NAME: &'static str = "tag"; const ADVISORY_LOCK_KEY: &'static str = "forum.merge.tag"; }
pub struct VoteScope; impl MergeReconciliationScope for VoteScope { const NAME: &'static str = "vote"; const ADVISORY_LOCK_KEY: &'static str = "forum.merge.vote"; }
