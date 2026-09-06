use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement};
use uuid::Uuid;

use super::posting_policy::ForumPostingPolicyFactKind;
use super::posting_policy_facts::{
    ForumPostingPolicyOwnerFactPort, ForumPostingPolicyOwnerFactRequest,
    ForumPostingPolicyOwnerFactResponse, ForumPostingPolicyOwnerFactValue,
    SharedForumPostingPolicyOwnerFactPort,
};

const INVALID_REQUEST_CODE: &str = "forum.approved_posts_facts.invalid_request";
const TENANT_MISMATCH_CODE: &str = "forum.approved_posts_facts.tenant_mismatch";
const ACTOR_MISMATCH_CODE: &str = "forum.approved_posts_facts.actor_mismatch";
const STORAGE_UNAVAILABLE_CODE: &str = "forum.approved_posts_facts.storage_unavailable";
const STORAGE_INVARIANT_CODE: &str = "forum.approved_posts_facts.storage_invariant";

/// Forum-owned exact-user adapter over retained topic and reply persistence.
///
/// `ApprovedPosts` is the current retained contribution count for the exact
/// tenant/user pair. Every non-deleted topic counts because topic creation is
/// immediately public and topic lifecycle status expresses open/closed/archive,
/// not moderation approval. Replies count only while they remain `approved`,
/// non-deleted and attached to a non-deleted parent topic. The adapter never
/// reads `forum_user_stats` or reconstructs historical approval transitions.
#[derive(Clone)]
pub struct ForumApprovedPostsFactPort {
    db: DatabaseConnection,
}

impl ForumApprovedPostsFactPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn shared(db: DatabaseConnection) -> SharedForumPostingPolicyOwnerFactPort {
        Arc::new(Self::new(db))
    }
}

#[async_trait]
impl ForumPostingPolicyOwnerFactPort for ForumApprovedPostsFactPort {
    fn fact_kind(&self) -> ForumPostingPolicyFactKind {
        ForumPostingPolicyFactKind::ApprovedPosts
    }

    async fn resolve_forum_posting_policy_fact(
        &self,
        context: PortContext,
        request: ForumPostingPolicyOwnerFactRequest,
    ) -> Result<ForumPostingPolicyOwnerFactResponse, PortError> {
        let request = request.normalize().map_err(|_| {
            PortError::validation(
                INVALID_REQUEST_CODE,
                "Forum approved-posts fact request is invalid",
            )
        })?;
        validate_context(&context, request.tenant_id, request.user_id)?;
        if request.fact != ForumPostingPolicyFactKind::ApprovedPosts {
            return Err(PortError::validation(
                INVALID_REQUEST_CODE,
                "Forum approved-posts adapter accepts only approved-posts facts",
            ));
        }

        let statement = approved_posts_statement(
            self.db.get_database_backend(),
            request.tenant_id,
            request.user_id,
        )?;
        let row = self
            .db
            .query_one_raw(statement)
            .await
            .map_err(|_| {
                PortError::unavailable(
                    STORAGE_UNAVAILABLE_CODE,
                    "Forum approved-post owner storage is unavailable",
                )
            })?
            .ok_or_else(|| {
                PortError::invariant_violation(
                    STORAGE_INVARIANT_CODE,
                    "Forum approved-post owner returned no aggregate row",
                )
            })?;

        let approved_topics = read_count(&row, "approved_topics")?;
        let approved_replies = read_count(&row, "approved_replies")?;
        let approved_posts = approved_topics
            .checked_add(approved_replies)
            .ok_or_else(|| {
                PortError::invariant_violation(
                    STORAGE_INVARIANT_CODE,
                    "Forum approved-post count exceeds the supported range",
                )
            })?;

        Ok(ForumPostingPolicyOwnerFactResponse {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            action: request.action,
            fact: request.fact,
            value: ForumPostingPolicyOwnerFactValue::ApprovedPosts(approved_posts),
        })
    }
}

fn approved_posts_statement(
    backend: DbBackend,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<Statement, PortError> {
    let values = vec![tenant_id.into(), user_id.into()];
    match backend {
        DbBackend::Postgres => Ok(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT
                (
                    SELECT COUNT(*)::bigint
                    FROM forum_topics topic
                    WHERE topic.tenant_id = $1
                      AND topic.author_id = $2
                      AND topic.deleted_at IS NULL
                ) AS approved_topics,
                (
                    SELECT COUNT(*)::bigint
                    FROM forum_replies reply
                    JOIN forum_topics topic
                      ON topic.tenant_id = reply.tenant_id
                     AND topic.id = reply.topic_id
                    WHERE reply.tenant_id = $1
                      AND reply.author_id = $2
                      AND reply.status = 'approved'
                      AND reply.deleted_at IS NULL
                      AND topic.deleted_at IS NULL
                ) AS approved_replies
            "#,
            values,
        )),
        DbBackend::Sqlite => Ok(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            r#"
            SELECT
                (
                    SELECT COUNT(*)
                    FROM forum_topics topic
                    WHERE topic.tenant_id = ?1
                      AND topic.author_id = ?2
                      AND topic.deleted_at IS NULL
                ) AS approved_topics,
                (
                    SELECT COUNT(*)
                    FROM forum_replies reply
                    JOIN forum_topics topic
                      ON topic.tenant_id = reply.tenant_id
                     AND topic.id = reply.topic_id
                    WHERE reply.tenant_id = ?1
                      AND reply.author_id = ?2
                      AND reply.status = 'approved'
                      AND reply.deleted_at IS NULL
                      AND topic.deleted_at IS NULL
                ) AS approved_replies
            "#,
            values,
        )),
        _ => Err(PortError::invariant_violation(
            STORAGE_INVARIANT_CODE,
            "Forum approved-post owner requires PostgreSQL or SQLite",
        )),
    }
}

fn read_count(row: &QueryResult, column: &str) -> Result<u64, PortError> {
    let value = row.try_get::<i64>("", column).map_err(|_| {
        PortError::invariant_violation(
            STORAGE_INVARIANT_CODE,
            "Forum approved-post owner returned an invalid aggregate value",
        )
    })?;
    u64::try_from(value).map_err(|_| {
        PortError::invariant_violation(
            STORAGE_INVARIANT_CODE,
            "Forum approved-post owner returned a negative aggregate value",
        )
    })
}

fn validate_context(
    context: &PortContext,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::read())?;
    if context.tenant_id != tenant_id.to_string() {
        return Err(PortError::validation(
            TENANT_MISMATCH_CODE,
            "Forum approved-posts fact tenant does not match the caller context",
        ));
    }
    if context.actor.kind != PortActorKind::User
        || Uuid::parse_str(&context.actor.id).ok() != Some(user_id)
    {
        return Err(PortError::forbidden(
            ACTOR_MISMATCH_CODE,
            "Forum approved-posts facts require the exact requested user actor",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rustok_api::{PortActor, PortErrorKind};
    use sea_orm::{Database, DbBackend, Statement};

    use super::*;
    use crate::services::{
        ForumPostingAction, ForumPostingCandidateMetrics, ForumPostingPolicyCompositionRequest,
        ForumPostingPolicyFactsComposer, ForumPostingPolicyRules,
    };

    fn context(tenant_id: Uuid, user_id: Uuid) -> PortContext {
        PortContext::new(
            tenant_id.to_string(),
            PortActor::user(user_id.to_string()),
            "en",
            "forum-approved-posts-facts-test",
        )
        .with_deadline(Duration::from_secs(5))
    }

    fn request(
        tenant_id: Uuid,
        user_id: Uuid,
        action: ForumPostingAction,
    ) -> ForumPostingPolicyOwnerFactRequest {
        ForumPostingPolicyOwnerFactRequest {
            tenant_id,
            user_id,
            action,
            fact: ForumPostingPolicyFactKind::ApprovedPosts,
            window_seconds: None,
        }
    }

    async fn test_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite should connect");
        for statement in [
            "CREATE TABLE forum_topics (\
                id TEXT PRIMARY KEY, \
                tenant_id TEXT NOT NULL, \
                author_id TEXT, \
                status TEXT NOT NULL, \
                deleted_at TEXT\
            )",
            "CREATE TABLE forum_replies (\
                id TEXT PRIMARY KEY, \
                tenant_id TEXT NOT NULL, \
                topic_id TEXT NOT NULL, \
                author_id TEXT, \
                status TEXT NOT NULL, \
                deleted_at TEXT\
            )",
        ] {
            db.execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                statement.to_string(),
            ))
            .await
            .expect("approved-post fixture table should initialize");
        }
        db
    }

    async fn insert_topic(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        topic_id: Uuid,
        author_id: Uuid,
        status: &str,
        deleted: bool,
    ) {
        let deleted_at = if deleted { "CURRENT_TIMESTAMP" } else { "NULL" };
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            format!(
                "INSERT INTO forum_topics (id, tenant_id, author_id, status, deleted_at) \
                 VALUES (?1, ?2, ?3, ?4, {deleted_at})"
            ),
            vec![
                topic_id.into(),
                tenant_id.into(),
                author_id.into(),
                status.to_string().into(),
            ],
        ))
        .await
        .expect("approved-post topic fixture should insert");
    }

    async fn insert_reply(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        topic_id: Uuid,
        author_id: Uuid,
        status: &str,
        deleted: bool,
    ) {
        let deleted_at = if deleted { "CURRENT_TIMESTAMP" } else { "NULL" };
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            format!(
                "INSERT INTO forum_replies \
                 (id, tenant_id, topic_id, author_id, status, deleted_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, {deleted_at})"
            ),
            vec![
                Uuid::new_v4().into(),
                tenant_id.into(),
                topic_id.into(),
                author_id.into(),
                status.to_string().into(),
            ],
        ))
        .await
        .expect("approved-post reply fixture should insert");
    }

    #[tokio::test]
    async fn retained_topics_and_current_approved_replies_are_counted() {
        let db = test_db().await;
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let retained_topic = Uuid::new_v4();
        let closed_topic = Uuid::new_v4();
        let archived_topic = Uuid::new_v4();
        let deleted_topic = Uuid::new_v4();

        insert_topic(&db, tenant_id, retained_topic, user_id, "open", false).await;
        insert_topic(&db, tenant_id, closed_topic, user_id, "closed", false).await;
        insert_topic(&db, tenant_id, archived_topic, user_id, "archived", false).await;
        insert_topic(&db, tenant_id, deleted_topic, user_id, "archived", true).await;
        insert_topic(
            &db,
            tenant_id,
            Uuid::new_v4(),
            Uuid::new_v4(),
            "open",
            false,
        )
        .await;
        insert_topic(&db, Uuid::new_v4(), Uuid::new_v4(), user_id, "open", false).await;

        insert_reply(&db, tenant_id, retained_topic, user_id, "approved", false).await;
        insert_reply(&db, tenant_id, closed_topic, user_id, "approved", false).await;
        insert_reply(&db, tenant_id, retained_topic, user_id, "approved", true).await;
        insert_reply(&db, tenant_id, deleted_topic, user_id, "approved", false).await;
        for status in ["pending", "rejected", "hidden", "flagged", "deleted"] {
            insert_reply(&db, tenant_id, retained_topic, user_id, status, false).await;
        }
        insert_reply(
            &db,
            tenant_id,
            retained_topic,
            Uuid::new_v4(),
            "approved",
            false,
        )
        .await;
        insert_reply(
            &db,
            Uuid::new_v4(),
            retained_topic,
            user_id,
            "approved",
            false,
        )
        .await;

        let response = ForumApprovedPostsFactPort::new(db)
            .resolve_forum_posting_policy_fact(
                context(tenant_id, user_id),
                request(tenant_id, user_id, ForumPostingAction::CreateReply),
            )
            .await
            .expect("authoritative approved-post count should resolve");

        assert_eq!(
            response.value,
            ForumPostingPolicyOwnerFactValue::ApprovedPosts(5)
        );
    }

    #[tokio::test]
    async fn empty_exact_user_contribution_set_is_authoritative_zero() {
        let db = test_db().await;
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let response = ForumApprovedPostsFactPort::new(db)
            .resolve_forum_posting_policy_fact(
                context(tenant_id, user_id),
                request(tenant_id, user_id, ForumPostingAction::CreateTopic),
            )
            .await
            .expect("empty authoritative contribution set should resolve");

        assert_eq!(
            response.value,
            ForumPostingPolicyOwnerFactValue::ApprovedPosts(0)
        );
    }

    #[tokio::test]
    async fn approved_posts_provider_composes_exact_required_fact() {
        let db = test_db().await;
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let topic_id = Uuid::new_v4();
        insert_topic(&db, tenant_id, topic_id, user_id, "open", false).await;
        insert_reply(&db, tenant_id, topic_id, user_id, "approved", false).await;
        let composer =
            ForumPostingPolicyFactsComposer::new(vec![ForumApprovedPostsFactPort::shared(db)])
                .expect("unique approved-post provider should compose");
        let rules = ForumPostingPolicyRules {
            minimum_approved_posts: Some(2),
            ..ForumPostingPolicyRules::default()
        };

        let input = composer
            .compose(
                context(tenant_id, user_id),
                &rules,
                ForumPostingPolicyCompositionRequest {
                    tenant_id,
                    user_id,
                    action: ForumPostingAction::CreateTopic,
                    candidate: ForumPostingCandidateMetrics {
                        body_bytes: 64,
                        link_count: 0,
                        mention_count: 0,
                        attachment_count: 0,
                    },
                },
            )
            .await
            .expect("approved-post fact should compose");

        assert_eq!(input.facts.approved_posts, Some(2));
        assert!(input.facts.unavailable_facts.is_empty());
    }

    #[tokio::test]
    async fn storage_failure_is_retryable_unavailable() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite should connect");
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let error = ForumApprovedPostsFactPort::new(db)
            .resolve_forum_posting_policy_fact(
                context(tenant_id, user_id),
                request(tenant_id, user_id, ForumPostingAction::CreateReply),
            )
            .await
            .expect_err("missing owner tables must remain a capability failure");

        assert_eq!(error.kind, PortErrorKind::Unavailable);
        assert!(error.retryable);
        assert_eq!(error.code, STORAGE_UNAVAILABLE_CODE);
    }

    #[tokio::test]
    async fn foreign_actor_is_rejected_before_storage_access() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite should connect");
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let error = ForumApprovedPostsFactPort::new(db)
            .resolve_forum_posting_policy_fact(
                context(tenant_id, Uuid::new_v4()),
                request(tenant_id, user_id, ForumPostingAction::CreateReply),
            )
            .await
            .expect_err("foreign actor must fail before the missing tables are read");

        assert_eq!(error.kind, PortErrorKind::Forbidden);
        assert_eq!(error.code, ACTOR_MISMATCH_CODE);
    }
}
