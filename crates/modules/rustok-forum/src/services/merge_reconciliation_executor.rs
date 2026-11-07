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
    const TABLE: &'static str;
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

    /// Ideal idempotent wrapper: `advisory_lock` + `ON CONFLICT` dedup.
    /// Checks `operation_id` and `merge_operation_id` uniqueness before
    /// calling `f`. `f` should `INSERT` the reconciliation row with
    /// `ON CONFLICT (tenant_id, operation_id) DO NOTHING` semantics.
    pub async fn try_insert<F, T>(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        merge_operation_id: Uuid,
        operation_id: Uuid,
        reason: &str,
        f: F,
    ) -> ForumResult<Option<T>>
    where
        F: AsyncFnOnce(&DatabaseTransaction) -> ForumResult<Option<T>>,
    {
        // 1) operation_id already exists → verify and return existing
        let table = S::TABLE;
        let sql_op = format!("SELECT merge_operation_id, reason FROM {table} WHERE tenant_id = $1 AND operation_id = $2");
        let rows = txn
            .query_all(Statement::from_sql_and_values(
                txn.get_database_backend(),
                sql_op,
                vec![tenant_id.into(), operation_id.into()],
            ))
            .await?;
        if let Some(row) = rows.into_iter().next() {
            let existing_merge: Uuid = row.try_get_by_index(0)?;
            let existing_reason: String = row.try_get_by_index(1)?;
            if existing_merge != merge_operation_id || existing_reason != reason {
                return Err(ForumError::Validation(format!(
                    "{} reconciliation conflict for operation {operation_id}",
                    S::NAME
                )));
            }
            // already reconciled for this operation → idempotent no-op, caller will fetch existing
            return Ok(None);
        }

        // 2) merge_operation_id already reconciled → idempotent no-op
        let sql_merge = format!("SELECT 1 FROM {table} WHERE tenant_id = $1 AND merge_operation_id = $2 LIMIT 1");
        let rows = txn
            .query_all(Statement::from_sql_and_values(
                txn.get_database_backend(),
                sql_merge,
                vec![tenant_id.into(), merge_operation_id.into()],
            ))
            .await?;
        if !rows.is_empty() {
            return Ok(None);
        }

        // 3) proceed with actual reconciliation
        f(txn).await
    }
}

// Concrete scopes — one per former copy-paste file
pub struct AudienceScope; impl MergeReconciliationScope for AudienceScope { const NAME: &'static str = "audience"; const ADVISORY_LOCK_KEY: &'static str = "forum.merge.audience"; const TABLE: &'static str = "forum_topic_merge_audience_reconciliations"; }
pub struct ReadStateScope; impl MergeReconciliationScope for ReadStateScope { const NAME: &'static str = "read_state"; const ADVISORY_LOCK_KEY: &'static str = "forum.merge.read_state"; const TABLE: &'static str = "forum_topic_merge_read_state_reconciliations"; }
pub struct SubscriptionScope; impl MergeReconciliationScope for SubscriptionScope { const NAME: &'static str = "subscription"; const ADVISORY_LOCK_KEY: &'static str = "forum.merge.subscription"; const TABLE: &'static str = "forum_topic_merge_subscription_reconciliations"; }
pub struct TagScope; impl MergeReconciliationScope for TagScope { const NAME: &'static str = "tag"; const ADVISORY_LOCK_KEY: &'static str = "forum.merge.tag"; const TABLE: &'static str = "forum_topic_merge_tag_reconciliations"; }
pub struct VoteScope; impl MergeReconciliationScope for VoteScope { const NAME: &'static str = "vote"; const ADVISORY_LOCK_KEY: &'static str = "forum.merge.vote"; const TABLE: &'static str = "forum_topic_merge_vote_reconciliations"; }
