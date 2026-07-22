mod support;

use std::sync::Arc;

use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{CreateReplyInput, ModerationService, ReplyService};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use uuid::Uuid;

use support::postgres::{PostgresForumTestDb, execute};
use support::{TestResult, test_error};

#[tokio::test]
async fn postgres_enforces_locked_and_moderated_reply_semantics() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("reply_publication").await? else {
        return Ok(());
    };

    let outcome = async {
        let locked = seed_forum(&context, false, true).await?;
        let locked_result = ReplyService::new(context.db.clone(), event_bus(context.db.clone()))
            .create(
                locked.tenant_id,
                customer_security(locked.author_id),
                locked.topic_id,
                reply_input("must be rejected"),
            )
            .await;
        if locked_result.is_ok() {
            return Err(test_error("locked topic accepted an ordinary reply"));
        }

        let moderated = seed_forum(&context, true, false).await?;
        let bus = event_bus(context.db.clone());
        let reply = ReplyService::new(context.db.clone(), bus.clone())
            .create(
                moderated.tenant_id,
                customer_security(moderated.author_id),
                moderated.topic_id,
                reply_input("pending reply"),
            )
            .await?;
        if reply.status != "pending" {
            return Err(test_error(format!(
                "moderated category produced unexpected reply status `{}`",
                reply.status
            )));
        }

        assert_public_state(&context.db, &moderated, 0, 0).await?;

        let moderation = ModerationService::new(context.db.clone(), bus);
        moderation
            .approve_reply(
                moderated.tenant_id,
                reply.id,
                moderated.topic_id,
                admin_security(),
            )
            .await?;
        assert_public_state(&context.db, &moderated, 1, 1).await?;

        moderation
            .hide_reply(
                moderated.tenant_id,
                reply.id,
                moderated.topic_id,
                admin_security(),
            )
            .await?;
        assert_public_state(&context.db, &moderated, 0, 1).await?;

        Ok(())
    }
    .await;

    context.cleanup().await?;
    outcome
}

#[derive(Clone, Copy)]
struct ForumSeed {
    tenant_id: Uuid,
    category_id: Uuid,
    topic_id: Uuid,
    author_id: Uuid,
}

async fn seed_forum(
    context: &PostgresForumTestDb,
    moderated: bool,
    locked: bool,
) -> TestResult<ForumSeed> {
    let seed = ForumSeed {
        tenant_id: Uuid::new_v4(),
        category_id: Uuid::new_v4(),
        topic_id: Uuid::new_v4(),
        author_id: Uuid::new_v4(),
    };
    execute(
        &context.db,
        format!(
            "INSERT INTO forum_categories
                (id, tenant_id, position, moderated, topic_count, reply_count)
             VALUES
                ('{}', '{}', 0, {}, 1, 0);
             INSERT INTO forum_topics
                (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
             VALUES
                ('{}', '{}', '{}', 'open', '{{}}', FALSE, {}, 0);",
            seed.category_id,
            seed.tenant_id,
            if moderated { "TRUE" } else { "FALSE" },
            seed.topic_id,
            seed.tenant_id,
            seed.category_id,
            if locked { "TRUE" } else { "FALSE" },
        ),
    )
    .await?;
    Ok(seed)
}

async fn assert_public_state(
    db: &sea_orm::DatabaseConnection,
    seed: &ForumSeed,
    expected_count: i64,
    expected_replied_events: i64,
) -> TestResult<()> {
    let topic_count = scalar_i64(
        db,
        format!(
            "SELECT reply_count::bigint AS value
             FROM forum_topics
             WHERE tenant_id = '{}' AND id = '{}'",
            seed.tenant_id, seed.topic_id
        ),
    )
    .await?;
    let category_count = scalar_i64(
        db,
        format!(
            "SELECT reply_count::bigint AS value
             FROM forum_categories
             WHERE tenant_id = '{}' AND id = '{}'",
            seed.tenant_id, seed.category_id
        ),
    )
    .await?;
    let user_count = scalar_i64(
        db,
        format!(
            "SELECT reply_count::bigint AS value
             FROM forum_user_stats
             WHERE tenant_id = '{}' AND user_id = '{}'",
            seed.tenant_id, seed.author_id
        ),
    )
    .await?;
    let replied_events = scalar_i64(
        db,
        "SELECT COUNT(*)::bigint AS value
         FROM sys_events
         WHERE event_type = 'forum.topic.replied'",
    )
    .await?;

    if topic_count != expected_count
        || category_count != expected_count
        || user_count != expected_count
        || replied_events != expected_replied_events
    {
        return Err(test_error(format!(
            "unexpected public reply state: topic={topic_count}, category={category_count}, \
             user={user_count}, events={replied_events}; expected count={expected_count}, \
             events={expected_replied_events}"
        )));
    }
    Ok(())
}

fn event_bus(db: sea_orm::DatabaseConnection) -> TransactionalEventBus {
    TransactionalEventBus::new(Arc::new(OutboxTransport::new(db)))
}

fn customer_security(user_id: Uuid) -> SecurityContext {
    SecurityContext::new(UserRole::Customer, Some(user_id))
}

fn admin_security() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

fn reply_input(content: &str) -> CreateReplyInput {
    CreateReplyInput {
        locale: "en".to_string(),
        content: content.to_string(),
        content_format: "markdown".to_string(),
        content_json: None,
        parent_reply_id: None,
    }
}

async fn scalar_i64(db: &sea_orm::DatabaseConnection, sql: impl Into<String>) -> TestResult<i64> {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            sql.into(),
        ))
        .await?
        .ok_or_else(|| test_error("scalar query returned no row"))?;
    Ok(row.try_get("", "value")?)
}
