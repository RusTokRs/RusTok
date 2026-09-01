use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement};
use uuid::Uuid;

use super::posting_policy::{ForumPostingPolicyFactKind, ForumPostingWindowCount};
use super::posting_policy_facts::{
    ForumPostingPolicyOwnerFactPort, ForumPostingPolicyOwnerFactRequest,
    ForumPostingPolicyOwnerFactResponse, ForumPostingPolicyOwnerFactValue,
    SharedForumPostingPolicyOwnerFactPort,
};

const INVALID_REQUEST_CODE: &str = "forum.create_window_facts.invalid_request";
const TENANT_MISMATCH_CODE: &str = "forum.create_window_facts.tenant_mismatch";
const ACTOR_MISMATCH_CODE: &str = "forum.create_window_facts.actor_mismatch";
const STORAGE_UNAVAILABLE_CODE: &str = "forum.create_window_facts.storage_unavailable";
const STORAGE_INVARIANT_CODE: &str = "forum.create_window_facts.storage_invariant";

type CreateWindowClock = Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>;

/// Forum-owned exact-user adapter for persisted topic-create activity.
///
/// Every topic row created inside the exact requested window contributes once,
/// including rows later archived or soft-deleted. Creation limits follow the
/// owner create operation rather than current publication state, so normal
/// deletion cannot reset the observed budget.
#[derive(Clone)]
pub struct ForumTopicCreatesWindowFactPort {
    db: DatabaseConnection,
    now: CreateWindowClock,
}

impl ForumTopicCreatesWindowFactPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            now: Arc::new(Utc::now),
        }
    }

    fn with_clock(db: DatabaseConnection, now: CreateWindowClock) -> Self {
        Self { db, now }
    }

    pub fn shared(db: DatabaseConnection) -> SharedForumPostingPolicyOwnerFactPort {
        Arc::new(Self::new(db))
    }
}

/// Forum-owned exact-user adapter for persisted reply-create activity.
///
/// Every reply row created inside the exact requested window contributes once,
/// regardless of its later moderation or soft-delete state. Failed commands
/// that never created an owner row do not contribute.
#[derive(Clone)]
pub struct ForumReplyCreatesWindowFactPort {
    db: DatabaseConnection,
    now: CreateWindowClock,
}

impl ForumReplyCreatesWindowFactPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            now: Arc::new(Utc::now),
        }
    }

    fn with_clock(db: DatabaseConnection, now: CreateWindowClock) -> Self {
        Self { db, now }
    }

    pub fn shared(db: DatabaseConnection) -> SharedForumPostingPolicyOwnerFactPort {
        Arc::new(Self::new(db))
    }
}

#[async_trait]
impl ForumPostingPolicyOwnerFactPort for ForumTopicCreatesWindowFactPort {
    fn fact_kind(&self) -> ForumPostingPolicyFactKind {
        ForumPostingPolicyFactKind::TopicCreatesWindow
    }

    async fn resolve_forum_posting_policy_fact(
        &self,
        context: PortContext,
        request: ForumPostingPolicyOwnerFactRequest,
    ) -> Result<ForumPostingPolicyOwnerFactResponse, PortError> {
        resolve_create_window_fact(
            &self.db,
            &self.now,
            ForumPostingPolicyFactKind::TopicCreatesWindow,
            context,
            request,
        )
        .await
    }
}

#[async_trait]
impl ForumPostingPolicyOwnerFactPort for ForumReplyCreatesWindowFactPort {
    fn fact_kind(&self) -> ForumPostingPolicyFactKind {
        ForumPostingPolicyFactKind::ReplyCreatesWindow
    }

    async fn resolve_forum_posting_policy_fact(
        &self,
        context: PortContext,
        request: ForumPostingPolicyOwnerFactRequest,
    ) -> Result<ForumPostingPolicyOwnerFactResponse, PortError> {
        resolve_create_window_fact(
            &self.db,
            &self.now,
            ForumPostingPolicyFactKind::ReplyCreatesWindow,
            context,
            request,
        )
        .await
    }
}

async fn resolve_create_window_fact(
    db: &DatabaseConnection,
    now: &CreateWindowClock,
    expected_fact: ForumPostingPolicyFactKind,
    context: PortContext,
    request: ForumPostingPolicyOwnerFactRequest,
) -> Result<ForumPostingPolicyOwnerFactResponse, PortError> {
    let request = request.normalize().map_err(|_| {
        PortError::validation(
            INVALID_REQUEST_CODE,
            "Forum create-window fact request is invalid",
        )
    })?;
    validate_context(&context, request.tenant_id, request.user_id)?;
    if request.fact != expected_fact {
        return Err(PortError::validation(
            INVALID_REQUEST_CODE,
            "Forum create-window adapter received a different fact kind",
        ));
    }
    let window_seconds = request.window_seconds.ok_or_else(|| {
        PortError::invariant_violation(
            STORAGE_INVARIANT_CODE,
            "Forum create-window request omitted its normalized observation window",
        )
    })?;
    let observed_at = now();
    let cutoff = observed_at
        .checked_sub_signed(Duration::seconds(i64::from(window_seconds)))
        .ok_or_else(|| {
            PortError::invariant_violation(
                STORAGE_INVARIANT_CODE,
                "Forum create-window observation boundary is outside the supported range",
            )
        })?;

    let statement = create_window_statement(
        db.get_database_backend(),
        expected_fact,
        request.tenant_id,
        request.user_id,
        cutoff,
        observed_at,
    )?;
    let row = db
        .query_one_raw(statement)
        .await
        .map_err(|_| {
            PortError::unavailable(
                STORAGE_UNAVAILABLE_CODE,
                "Forum create-window owner storage is unavailable",
            )
        })?
        .ok_or_else(|| {
            PortError::invariant_violation(
                STORAGE_INVARIANT_CODE,
                "Forum create-window owner returned no aggregate row",
            )
        })?;
    let count = read_count(&row)?;
    let window = ForumPostingWindowCount {
        count,
        window_seconds,
    };
    let value = match expected_fact {
        ForumPostingPolicyFactKind::TopicCreatesWindow => {
            ForumPostingPolicyOwnerFactValue::TopicCreatesWindow(window)
        }
        ForumPostingPolicyFactKind::ReplyCreatesWindow => {
            ForumPostingPolicyOwnerFactValue::ReplyCreatesWindow(window)
        }
        _ => {
            return Err(PortError::invariant_violation(
                STORAGE_INVARIANT_CODE,
                "Forum create-window adapter was configured with an unsupported fact kind",
            ));
        }
    };

    Ok(ForumPostingPolicyOwnerFactResponse {
        tenant_id: request.tenant_id,
        user_id: request.user_id,
        action: request.action,
        fact: request.fact,
        value,
    })
}

fn create_window_statement(
    backend: DbBackend,
    fact: ForumPostingPolicyFactKind,
    tenant_id: Uuid,
    user_id: Uuid,
    cutoff: DateTime<Utc>,
    observed_at: DateTime<Utc>,
) -> Result<Statement, PortError> {
    let table = match fact {
        ForumPostingPolicyFactKind::TopicCreatesWindow => "forum_topics",
        ForumPostingPolicyFactKind::ReplyCreatesWindow => "forum_replies",
        _ => {
            return Err(PortError::invariant_violation(
                STORAGE_INVARIANT_CODE,
                "Forum create-window statement requires a create-window fact kind",
            ));
        }
    };
    let values = vec![
        tenant_id.into(),
        user_id.into(),
        cutoff.into(),
        observed_at.into(),
    ];
    match backend {
        DbBackend::Postgres => Ok(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "SELECT COUNT(*)::bigint AS create_count \
                 FROM {table} \
                 WHERE tenant_id = $1 \
                   AND author_id = $2 \
                   AND created_at >= $3 \
                   AND created_at <= $4"
            ),
            values,
        )),
        DbBackend::Sqlite => Ok(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            format!(
                "SELECT COUNT(*) AS create_count \
                 FROM {table} \
                 WHERE tenant_id = ?1 \
                   AND author_id = ?2 \
                   AND created_at >= ?3 \
                   AND created_at <= ?4"
            ),
            values,
        )),
        _ => Err(PortError::invariant_violation(
            STORAGE_INVARIANT_CODE,
            "Forum create-window owner requires PostgreSQL or SQLite",
        )),
    }
}

fn read_count(row: &QueryResult) -> Result<u32, PortError> {
    let value = row.try_get::<i64>("", "create_count").map_err(|_| {
        PortError::invariant_violation(
            STORAGE_INVARIANT_CODE,
            "Forum create-window owner returned an invalid aggregate value",
        )
    })?;
    u32::try_from(value).map_err(|_| {
        PortError::invariant_violation(
            STORAGE_INVARIANT_CODE,
            "Forum create-window count is negative or exceeds the supported range",
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
            "Forum create-window fact tenant does not match the caller context",
        ));
    }
    if context.actor.kind != PortActorKind::User
        || Uuid::parse_str(&context.actor.id).ok() != Some(user_id)
    {
        return Err(PortError::forbidden(
            ACTOR_MISMATCH_CODE,
            "Forum create-window facts require the exact requested user actor",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration as StdDuration;

    use chrono::{TimeZone, Timelike};
    use rustok_api::{PortActor, PortErrorKind};
    use sea_orm::{Database, DbBackend, Statement};

    use super::*;
    use crate::services::{
        ForumPostingAction, ForumPostingCandidateMetrics, ForumPostingPolicyCompositionRequest,
        ForumPostingPolicyFactsComposer, ForumPostingPolicyRules, ForumPostingWindowLimit,
    };

    fn context(tenant_id: Uuid, user_id: Uuid) -> PortContext {
        PortContext::new(
            tenant_id.to_string(),
            PortActor::user(user_id.to_string()),
            "en",
            "forum-create-window-facts-test",
        )
        .with_deadline(StdDuration::from_secs(5))
    }

    fn request(
        tenant_id: Uuid,
        user_id: Uuid,
        action: ForumPostingAction,
        fact: ForumPostingPolicyFactKind,
        window_seconds: u32,
    ) -> ForumPostingPolicyOwnerFactRequest {
        ForumPostingPolicyOwnerFactRequest {
            tenant_id,
            user_id,
            action,
            fact,
            window_seconds: Some(window_seconds),
        }
    }

    fn fixed_clock(now: DateTime<Utc>) -> CreateWindowClock {
        Arc::new(move || now)
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
                created_at TEXT NOT NULL, \
                deleted_at TEXT\
            )",
            "CREATE TABLE forum_replies (\
                id TEXT PRIMARY KEY, \
                tenant_id TEXT NOT NULL, \
                topic_id TEXT NOT NULL, \
                author_id TEXT, \
                status TEXT NOT NULL, \
                created_at TEXT NOT NULL, \
                deleted_at TEXT\
            )",
        ] {
            db.execute_raw(Statement::from_string(
                DbBackend::Sqlite,
                statement.to_string(),
            ))
            .await
            .expect("create-window fixture table should initialize");
        }
        db
    }

    async fn insert_topic(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        author_id: Uuid,
        created_at: DateTime<Utc>,
        status: &str,
        deleted: bool,
    ) {
        let deleted_at = if deleted { "?6" } else { "NULL" };
        let mut values = vec![
            Uuid::new_v4().into(),
            tenant_id.into(),
            author_id.into(),
            status.to_string().into(),
            created_at.into(),
        ];
        if deleted {
            values.push(created_at.into());
        }
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            format!(
                "INSERT INTO forum_topics \
                 (id, tenant_id, author_id, status, created_at, deleted_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, {deleted_at})"
            ),
            values,
        ))
        .await
        .expect("create-window topic fixture should insert");
    }

    async fn insert_reply(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        author_id: Uuid,
        created_at: DateTime<Utc>,
        status: &str,
        deleted: bool,
    ) {
        let deleted_at = if deleted { "?7" } else { "NULL" };
        let mut values = vec![
            Uuid::new_v4().into(),
            tenant_id.into(),
            Uuid::new_v4().into(),
            author_id.into(),
            status.to_string().into(),
            created_at.into(),
        ];
        if deleted {
            values.push(created_at.into());
        }
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            format!(
                "INSERT INTO forum_replies \
                 (id, tenant_id, topic_id, author_id, status, created_at, deleted_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, {deleted_at})"
            ),
            values,
        ))
        .await
        .expect("create-window reply fixture should insert");
    }

    #[tokio::test]
    async fn create_windows_count_all_persisted_owner_rows_in_exact_scope() {
        let db = test_db().await;
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let now = Utc
            .with_ymd_and_hms(2026, 7, 28, 12, 0, 0)
            .single()
            .expect("fixed observation time")
            .with_nanosecond(0)
            .expect("whole-second observation time");

        insert_topic(
            &db,
            tenant_id,
            user_id,
            now - Duration::minutes(30),
            "open",
            false,
        )
        .await;
        insert_topic(
            &db,
            tenant_id,
            user_id,
            now - Duration::minutes(10),
            "archived",
            true,
        )
        .await;
        insert_topic(
            &db,
            tenant_id,
            user_id,
            now - Duration::hours(2),
            "open",
            false,
        )
        .await;
        insert_topic(
            &db,
            tenant_id,
            Uuid::new_v4(),
            now - Duration::minutes(5),
            "open",
            false,
        )
        .await;
        insert_topic(
            &db,
            Uuid::new_v4(),
            user_id,
            now - Duration::minutes(5),
            "open",
            false,
        )
        .await;

        for (minutes, status, deleted) in [
            (50, "approved", false),
            (20, "pending", false),
            (5, "rejected", true),
        ] {
            insert_reply(
                &db,
                tenant_id,
                user_id,
                now - Duration::minutes(minutes),
                status,
                deleted,
            )
            .await;
        }
        insert_reply(
            &db,
            tenant_id,
            user_id,
            now - Duration::hours(3),
            "approved",
            false,
        )
        .await;
        insert_reply(
            &db,
            tenant_id,
            Uuid::new_v4(),
            now - Duration::minutes(1),
            "approved",
            false,
        )
        .await;

        let topic = ForumTopicCreatesWindowFactPort::with_clock(db.clone(), fixed_clock(now))
            .resolve_forum_posting_policy_fact(
                context(tenant_id, user_id),
                request(
                    tenant_id,
                    user_id,
                    ForumPostingAction::CreateTopic,
                    ForumPostingPolicyFactKind::TopicCreatesWindow,
                    3_600,
                ),
            )
            .await
            .expect("topic-create window should resolve");
        assert_eq!(
            topic.value,
            ForumPostingPolicyOwnerFactValue::TopicCreatesWindow(ForumPostingWindowCount {
                count: 2,
                window_seconds: 3_600,
            })
        );

        let reply = ForumReplyCreatesWindowFactPort::with_clock(db, fixed_clock(now))
            .resolve_forum_posting_policy_fact(
                context(tenant_id, user_id),
                request(
                    tenant_id,
                    user_id,
                    ForumPostingAction::CreateReply,
                    ForumPostingPolicyFactKind::ReplyCreatesWindow,
                    3_600,
                ),
            )
            .await
            .expect("reply-create window should resolve");
        assert_eq!(
            reply.value,
            ForumPostingPolicyOwnerFactValue::ReplyCreatesWindow(ForumPostingWindowCount {
                count: 3,
                window_seconds: 3_600,
            })
        );
    }

    #[tokio::test]
    async fn composer_publishes_both_exact_create_windows() {
        let db = test_db().await;
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let now = Utc
            .with_ymd_and_hms(2026, 7, 28, 12, 0, 0)
            .single()
            .expect("fixed observation time");
        insert_topic(
            &db,
            tenant_id,
            user_id,
            now - Duration::minutes(10),
            "open",
            false,
        )
        .await;
        insert_reply(
            &db,
            tenant_id,
            user_id,
            now - Duration::seconds(30),
            "pending",
            false,
        )
        .await;

        let composer = ForumPostingPolicyFactsComposer::new(vec![
            Arc::new(ForumTopicCreatesWindowFactPort::with_clock(
                db.clone(),
                fixed_clock(now),
            )),
            Arc::new(ForumReplyCreatesWindowFactPort::with_clock(
                db,
                fixed_clock(now),
            )),
        ])
        .expect("create-window providers should be unique");

        let topic_input = composer
            .compose(
                context(tenant_id, user_id),
                &ForumPostingPolicyRules {
                    topic_create_limit: Some(ForumPostingWindowLimit {
                        maximum_count: 10,
                        window_seconds: 3_600,
                    }),
                    ..ForumPostingPolicyRules::default()
                },
                ForumPostingPolicyCompositionRequest {
                    tenant_id,
                    user_id,
                    action: ForumPostingAction::CreateTopic,
                    candidate: ForumPostingCandidateMetrics::default(),
                },
            )
            .await
            .expect("topic-create window should compose");
        assert_eq!(topic_input.facts.topic_creates_window.unwrap().count, 1);

        let reply_input = composer
            .compose(
                context(tenant_id, user_id),
                &ForumPostingPolicyRules {
                    reply_create_limit: Some(ForumPostingWindowLimit {
                        maximum_count: 10,
                        window_seconds: 60,
                    }),
                    ..ForumPostingPolicyRules::default()
                },
                ForumPostingPolicyCompositionRequest {
                    tenant_id,
                    user_id,
                    action: ForumPostingAction::CreateReply,
                    candidate: ForumPostingCandidateMetrics::default(),
                },
            )
            .await
            .expect("reply-create window should compose");
        assert_eq!(reply_input.facts.reply_creates_window.unwrap().count, 1);
    }

    #[tokio::test]
    async fn foreign_actor_is_rejected_before_storage_access() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite should connect");
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let error = ForumTopicCreatesWindowFactPort::new(db)
            .resolve_forum_posting_policy_fact(
                context(tenant_id, Uuid::new_v4()),
                request(
                    tenant_id,
                    user_id,
                    ForumPostingAction::CreateTopic,
                    ForumPostingPolicyFactKind::TopicCreatesWindow,
                    60,
                ),
            )
            .await
            .expect_err("foreign actor must fail before the missing table is read");
        assert_eq!(error.kind, PortErrorKind::Forbidden);
        assert_eq!(error.code, ACTOR_MISMATCH_CODE);
    }
}
