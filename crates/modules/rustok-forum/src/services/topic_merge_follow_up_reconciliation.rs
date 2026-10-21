//! Composes the Forum topic merge follow-up reconciliation owners inside the
//! merge transaction itself.
//!
//! A merge archives the source topic and republishes replies into the
//! retained target, but votes, subscriptions, tags, read states and audience
//! memberships live on dedicated topic-scoped relations. Running their idempotent
//! reconciliation owners immediately after the merge receipt — inside the same
//! transaction — guarantees those relations follow the replies atomically
//! instead of depending on a later manual recovery write.

use sea_orm::{DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

use crate::error::ForumResult;
use crate::services::{
    ForumTopicMergeAudienceReconciliationService, ForumTopicMergeReadStateReconciliationService,
    ForumTopicMergeSubscriptionReconciliationService, ForumTopicMergeTagReconciliationService,
    ForumTopicMergeVoteReconciliationService,
};

/// Actor-facing audit reason for merge-driven subscription reconciliation.
const FOLLOW_UP_REASON_SUBSCRIPTIONS: &str = "Merged: subscription coverage follows the retained target topic";
/// Actor-facing audit reason for merge-driven read state reconciliation.
const FOLLOW_UP_REASON_READ_STATES: &str = "Merged: read progress follows the retained target topic";
/// Actor-facing audit reason for merge-driven vote reconciliation.
const FOLLOW_UP_REASON_VOTES: &str = "Merged: vote totals follow the retained target topic";
/// Actor-facing audit reason for merge-driven tag reconciliation.
const FOLLOW_UP_REASON_TAGS: &str = "Merged: tag coverage follows the retained target topic";
/// Actor-facing audit reason for merge-driven audience reconciliation.
const FOLLOW_UP_REASON_AUDIENCE: &str = "Merged: audience membership follows the retained target topic";

/// Deterministic key domain tag for derived subscription reconciliation operations.
const FOLLOW_UP_DOMAIN_SUBSCRIPTIONS: &str = "subscriptions";
/// Deterministic key domain tag for derived read state reconciliation operations.
const FOLLOW_UP_DOMAIN_READ_STATES: &str = "read-states";
/// Deterministic key domain tag for derived vote reconciliation operations.
const FOLLOW_UP_DOMAIN_VOTES: &str = "votes";
/// Deterministic key domain tag for derived tag reconciliation operations.
const FOLLOW_UP_DOMAIN_TAGS: &str = "tags";
/// Deterministic key domain tag for derived audience reconciliation operations.
const FOLLOW_UP_DOMAIN_AUDIENCE: &str = "audience";

/// Derives the deterministic operation id for one domain's merge-driven
/// reconciliation audit row. The derivation depends only on the merge
/// operation id and the domain tag, so an exact retry of the merge replays
/// against the same audit identity and stays idempotent.
fn derive_follow_up_operation_id(merge_operation_id: Uuid, domain_tag: &str) -> Uuid {
    let mut name = b"forum-topic-merge-follow-up:".to_vec();
    name.extend_from_slice(domain_tag.as_bytes());
    name.push(b':');
    name.extend_from_slice(merge_operation_id.as_bytes());
    Uuid::new_v5(&Uuid::NAMESPACE_URL, &name)
}

/// Runs every merge follow-up reconciliation owner inside `txn`.
///
/// Same-category merges reconcile all five topic-scoped relations; the source
/// row is the archived locked tombstone and the retained target accepts the
/// relations under each owner's conflict policy (idempotent replay, exact-hash
/// equality, source-emptiness verification). Cross-category merges keep the
/// relations stranded on the tombstone on purpose: audience/subscription
/// semantics are category-local and the per-domain owners reject category
/// mismatches — moving them there would silently change membership meaning.
/// The merge receipt (`forum.topic.merged` event plus the merge operation row)
/// keeps the operator-facing record, so the reconciliation remains replayable
/// through the public per-domain entry points whenever a later process moves
/// the topics back into one category (the durable merge receipt is the
/// recovery hook for that repair).
pub(crate) async fn reconcile_topic_merge_follow_up_relations_in_tx(
    db: &DatabaseConnection,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    merge_operation_id: Uuid,
    actor_id: Uuid,
    is_cross_category: bool,
) -> ForumResult<()> {
    if is_cross_category {
        return Ok(());
    }

    ForumTopicMergeSubscriptionReconciliationService::new(db.clone())
        .reconcile_in_tx(
            txn,
            tenant_id,
            merge_operation_id,
            actor_id,
            derive_follow_up_operation_id(merge_operation_id, FOLLOW_UP_DOMAIN_SUBSCRIPTIONS),
            FOLLOW_UP_REASON_SUBSCRIPTIONS,
        )
        .await?;
    ForumTopicMergeReadStateReconciliationService::new(db.clone())
        .reconcile_in_tx(
            txn,
            tenant_id,
            merge_operation_id,
            actor_id,
            derive_follow_up_operation_id(merge_operation_id, FOLLOW_UP_DOMAIN_READ_STATES),
            FOLLOW_UP_REASON_READ_STATES,
        )
        .await?;
    ForumTopicMergeVoteReconciliationService::new(db.clone())
        .reconcile_in_tx(
            txn,
            tenant_id,
            merge_operation_id,
            actor_id,
            derive_follow_up_operation_id(merge_operation_id, FOLLOW_UP_DOMAIN_VOTES),
            FOLLOW_UP_REASON_VOTES,
        )
        .await?;
    ForumTopicMergeTagReconciliationService::new(db.clone())
        .reconcile_in_tx(
            txn,
            tenant_id,
            merge_operation_id,
            actor_id,
            derive_follow_up_operation_id(merge_operation_id, FOLLOW_UP_DOMAIN_TAGS),
            FOLLOW_UP_REASON_TAGS,
        )
        .await?;
    ForumTopicMergeAudienceReconciliationService::new(db.clone())
        .reconcile_in_tx(
            txn,
            tenant_id,
            merge_operation_id,
            actor_id,
            derive_follow_up_operation_id(merge_operation_id, FOLLOW_UP_DOMAIN_AUDIENCE),
            FOLLOW_UP_REASON_AUDIENCE,
        )
        .await?;

    Ok(())
}
