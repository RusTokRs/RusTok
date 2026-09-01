use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter};
use uuid::Uuid;

use crate::entities::forum_topic_read_state;

use super::posting_policy::ForumPostingPolicyFactKind;
use super::posting_policy_facts::{
    ForumPostingPolicyOwnerFactPort, ForumPostingPolicyOwnerFactRequest,
    ForumPostingPolicyOwnerFactResponse, ForumPostingPolicyOwnerFactValue,
    SharedForumPostingPolicyOwnerFactPort,
};

const INVALID_REQUEST_CODE: &str = "forum.reading_facts.invalid_request";
const TENANT_MISMATCH_CODE: &str = "forum.reading_facts.tenant_mismatch";
const ACTOR_MISMATCH_CODE: &str = "forum.reading_facts.actor_mismatch";
const STORAGE_UNAVAILABLE_CODE: &str = "forum.reading_facts.storage_unavailable";

/// Forum-owned exact-user adapter over the authoritative topic read-state ledger.
///
/// `TopicsRead` is the number of distinct topic identities for which the exact
/// tenant/user pair retains an explicit read high-water. The read-state primary
/// key guarantees one row per topic. An empty ledger is authoritative zero, not
/// an unavailable capability or a value derived from `forum_user_stats`.
#[derive(Clone)]
pub struct ForumTopicReadPostingFactPort {
    db: DatabaseConnection,
}

impl ForumTopicReadPostingFactPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn shared(db: DatabaseConnection) -> SharedForumPostingPolicyOwnerFactPort {
        Arc::new(Self::new(db))
    }
}

#[async_trait]
impl ForumPostingPolicyOwnerFactPort for ForumTopicReadPostingFactPort {
    fn fact_kind(&self) -> ForumPostingPolicyFactKind {
        ForumPostingPolicyFactKind::TopicsRead
    }

    async fn resolve_forum_posting_policy_fact(
        &self,
        context: PortContext,
        request: ForumPostingPolicyOwnerFactRequest,
    ) -> Result<ForumPostingPolicyOwnerFactResponse, PortError> {
        let request = request.normalize().map_err(|_| {
            PortError::validation(
                INVALID_REQUEST_CODE,
                "Forum reading fact request is invalid",
            )
        })?;
        validate_context(&context, request.tenant_id, request.user_id)?;
        if request.fact != ForumPostingPolicyFactKind::TopicsRead {
            return Err(PortError::validation(
                INVALID_REQUEST_CODE,
                "Forum reading adapter accepts only topics-read facts",
            ));
        }

        let topics_read = forum_topic_read_state::Entity::find()
            .filter(forum_topic_read_state::Column::TenantId.eq(request.tenant_id))
            .filter(forum_topic_read_state::Column::UserId.eq(request.user_id))
            .count(&self.db)
            .await
            .map_err(|_| {
                PortError::unavailable(
                    STORAGE_UNAVAILABLE_CODE,
                    "Forum topic read-state storage is unavailable",
                )
            })?;

        Ok(ForumPostingPolicyOwnerFactResponse {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            action: request.action,
            fact: request.fact,
            value: ForumPostingPolicyOwnerFactValue::TopicsRead(topics_read),
        })
    }
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
            "Forum reading fact tenant does not match the caller context",
        ));
    }
    if context.actor.kind != PortActorKind::User
        || Uuid::parse_str(&context.actor.id).ok() != Some(user_id)
    {
        return Err(PortError::forbidden(
            ACTOR_MISMATCH_CODE,
            "Forum reading facts require the exact requested user actor",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rustok_api::{PortActor, PortErrorKind};
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

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
            "forum-reading-facts-test",
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
            fact: ForumPostingPolicyFactKind::TopicsRead,
            window_seconds: None,
        }
    }

    async fn test_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite should connect");
        db.execute_raw(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TABLE forum_topic_read_states (\
                tenant_id TEXT NOT NULL, \
                topic_id TEXT NOT NULL, \
                user_id TEXT NOT NULL, \
                last_read_position INTEGER NOT NULL, \
                last_read_revision INTEGER NOT NULL, \
                created_at TEXT NOT NULL, \
                updated_at TEXT NOT NULL, \
                PRIMARY KEY (tenant_id, topic_id, user_id)\
            )"
            .to_string(),
        ))
        .await
        .expect("topic read-state fixture table should initialize");
        db
    }

    async fn insert_read_state(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        topic_id: Uuid,
        user_id: Uuid,
    ) {
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO forum_topic_read_states (\
                tenant_id, topic_id, user_id, last_read_position, \
                last_read_revision, created_at, updated_at\
            ) VALUES (?1, ?2, ?3, 0, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![tenant_id.into(), topic_id.into(), user_id.into()],
        ))
        .await
        .expect("topic read-state fixture should insert");
    }

    #[tokio::test]
    async fn exact_user_topic_read_rows_are_counted_once_each() {
        let db = test_db().await;
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        insert_read_state(&db, tenant_id, Uuid::new_v4(), user_id).await;
        insert_read_state(&db, tenant_id, Uuid::new_v4(), user_id).await;
        insert_read_state(&db, tenant_id, Uuid::new_v4(), Uuid::new_v4()).await;
        insert_read_state(&db, Uuid::new_v4(), Uuid::new_v4(), user_id).await;
        let provider = ForumTopicReadPostingFactPort::new(db);

        let response = provider
            .resolve_forum_posting_policy_fact(
                context(tenant_id, user_id),
                request(tenant_id, user_id, ForumPostingAction::CreateReply),
            )
            .await
            .expect("authoritative topic reading count should resolve");

        assert_eq!(
            response.value,
            ForumPostingPolicyOwnerFactValue::TopicsRead(2)
        );
    }

    #[tokio::test]
    async fn empty_exact_user_read_ledger_is_authoritative_zero() {
        let db = test_db().await;
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let provider = ForumTopicReadPostingFactPort::new(db);

        let response = provider
            .resolve_forum_posting_policy_fact(
                context(tenant_id, user_id),
                request(tenant_id, user_id, ForumPostingAction::CreateTopic),
            )
            .await
            .expect("empty authoritative ledger should resolve");

        assert_eq!(
            response.value,
            ForumPostingPolicyOwnerFactValue::TopicsRead(0)
        );
    }

    #[tokio::test]
    async fn reading_provider_composes_exact_required_fact() {
        let db = test_db().await;
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        insert_read_state(&db, tenant_id, Uuid::new_v4(), user_id).await;
        insert_read_state(&db, tenant_id, Uuid::new_v4(), user_id).await;
        let composer =
            ForumPostingPolicyFactsComposer::new(vec![ForumTopicReadPostingFactPort::shared(db)])
                .expect("unique reading provider should compose");
        let rules = ForumPostingPolicyRules {
            minimum_topics_read: Some(2),
            ..ForumPostingPolicyRules::default()
        };

        let input = composer
            .compose(
                context(tenant_id, user_id),
                &rules,
                ForumPostingPolicyCompositionRequest {
                    tenant_id,
                    user_id,
                    action: ForumPostingAction::CreateReply,
                    candidate: ForumPostingCandidateMetrics {
                        body_bytes: 64,
                        link_count: 0,
                        mention_count: 0,
                        attachment_count: 0,
                    },
                },
            )
            .await
            .expect("reading fact should compose");

        assert_eq!(input.facts.topics_read, Some(2));
        assert!(input.facts.unavailable_facts.is_empty());
    }

    #[tokio::test]
    async fn storage_failure_is_retryable_unavailable() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite should connect");
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let provider = ForumTopicReadPostingFactPort::new(db);

        let error = provider
            .resolve_forum_posting_policy_fact(
                context(tenant_id, user_id),
                request(tenant_id, user_id, ForumPostingAction::CreateReply),
            )
            .await
            .expect_err("missing owner table must remain a capability failure");

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
        let provider = ForumTopicReadPostingFactPort::new(db);

        let error = provider
            .resolve_forum_posting_policy_fact(
                context(tenant_id, Uuid::new_v4()),
                request(tenant_id, user_id, ForumPostingAction::CreateReply),
            )
            .await
            .expect_err("foreign actor must fail before the missing table is read");

        assert_eq!(error.kind, PortErrorKind::Forbidden);
        assert_eq!(error.code, ACTOR_MISMATCH_CODE);
    }
}
