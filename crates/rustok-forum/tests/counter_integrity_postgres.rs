mod support;

use std::sync::Arc;

use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{CreateReplyInput, CreateTopicInput, ReplyService, TopicService};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use tokio::sync::Barrier;
use uuid::Uuid;

use support::postgres::{PostgresForumTestDb, execute};
use support::{TestResult, test_error};

#[tokio::test]
async fn concurrent_replies_preserve_atomic_counters() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("atomic_counters").await? else {
        return Ok(());
    };

    let outcome = async {
        let tenant_id = Uuid::new_v4();
        let category_id = Uuid::new_v4();
        let topic_id = Uuid::new_v4();
        let author_id = Uuid::new_v4();

        execute(
            &context.db,
            format!(
                "INSERT INTO forum_categories
                    (id, tenant_id, position, moderated, topic_count, reply_count)
                 VALUES
                    ('{category_id}', '{tenant_id}', 0, FALSE, 1, 0);
                 INSERT INTO forum_topics
                    (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
                 VALUES
                    ('{topic_id}', '{tenant_id}', '{category_id}', 'open', '{{}}', FALSE, FALSE, 0);"
            ),
        )
        .await?;
        install_reply_insert_delay(&context).await?;
        create_concurrent_replies(&context, tenant_id, topic_id, author_id, 8).await?;

        let topic_count = scalar_i64(
            &context.db,
            format!(
                "SELECT reply_count::bigint AS value FROM forum_topics WHERE id = '{topic_id}'"
            ),
        )
        .await?;
        let category_count = scalar_i64(
            &context.db,
            format!(
                "SELECT reply_count::bigint AS value FROM forum_categories WHERE id = '{category_id}'"
            ),
        )
        .await?;
        let user_count = scalar_i64(
            &context.db,
            format!(
                "SELECT reply_count::bigint AS value
                   FROM forum_user_stats
                  WHERE tenant_id = '{tenant_id}' AND user_id = '{author_id}'"
            ),
        )
        .await?;

        if topic_count != 8 || category_count != 8 || user_count != 8 {
            return Err(test_error(format!(
                "lost counter update: topic={topic_count}, category={category_count}, user={user_count}, expected=8"
            )));
        }
        Ok(())
    }
    .await;

    context.cleanup().await?;
    outcome
}

#[tokio::test]
async fn concurrent_topics_preserve_atomic_counters() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("atomic_topic_counters").await? else {
        return Ok(());
    };

    let outcome = async {
        let tenant_id = Uuid::new_v4();
        let category_id = Uuid::new_v4();
        let author_id = Uuid::new_v4();

        execute(
            &context.db,
            format!(
                "INSERT INTO forum_categories
                    (id, tenant_id, position, moderated, topic_count, reply_count)
                 VALUES
                    ('{category_id}', '{tenant_id}', 0, FALSE, 0, 0);"
            ),
        )
        .await?;
        install_topic_insert_delay(&context).await?;
        create_concurrent_topics(&context, tenant_id, category_id, author_id, 8).await?;

        let category_count = scalar_i64(
            &context.db,
            format!(
                "SELECT topic_count::bigint AS value FROM forum_categories WHERE id = '{category_id}'"
            ),
        )
        .await?;
        let user_count = scalar_i64(
            &context.db,
            format!(
                "SELECT topic_count::bigint AS value
                   FROM forum_user_stats
                  WHERE tenant_id = '{tenant_id}' AND user_id = '{author_id}'"
            ),
        )
        .await?;

        if category_count != 8 || user_count != 8 {
            return Err(test_error(format!(
                "lost topic counter update: category={category_count}, user={user_count}, expected=8"
            )));
        }
        Ok(())
    }
    .await;

    context.cleanup().await?;
    outcome
}

async fn install_reply_insert_delay(context: &PostgresForumTestDb) -> TestResult<()> {
    execute(
        &context.db,
        r#"
CREATE FUNCTION forum_test_delay_reply_insert()
RETURNS trigger AS $$
BEGIN
    PERFORM pg_sleep(0.25);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER forum_test_delay_reply_insert
BEFORE INSERT ON forum_replies
FOR EACH ROW EXECUTE FUNCTION forum_test_delay_reply_insert();
"#,
    )
    .await
}

async fn create_concurrent_replies(
    context: &PostgresForumTestDb,
    tenant_id: Uuid,
    topic_id: Uuid,
    author_id: Uuid,
    count: usize,
) -> TestResult<()> {
    let barrier = Arc::new(Barrier::new(count));
    let mut handles = Vec::with_capacity(count);

    for index in 0..count {
        let db = context.peer().await?;
        let barrier = barrier.clone();
        handles.push(tokio::spawn(async move {
            let service = ReplyService::new(db.clone(), event_bus(db));
            barrier.wait().await;
            service
                .create(
                    tenant_id,
                    SecurityContext::new(UserRole::Customer, Some(author_id)),
                    topic_id,
                    CreateReplyInput {
                        locale: "en".to_string(),
                        content: format!("concurrent reply {index}"),
                        content_format: "markdown".to_string(),
                        content_json: None,
                        parent_reply_id: None,
                    },
                )
                .await
                .map(|_| ())
                .map_err(|error| error.to_string())
        }));
    }

    for handle in handles {
        let result = handle
            .await
            .map_err(|error| test_error(format!("reply task failed to join: {error}")))?;
        result.map_err(test_error)?;
    }
    Ok(())
}

async fn install_topic_insert_delay(context: &PostgresForumTestDb) -> TestResult<()> {
    execute(
        &context.db,
        r#"
CREATE FUNCTION forum_test_delay_topic_insert()
RETURNS trigger AS $$
BEGIN
    PERFORM pg_sleep(0.25);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER forum_test_delay_topic_insert
BEFORE INSERT ON forum_topics
FOR EACH ROW EXECUTE FUNCTION forum_test_delay_topic_insert();
"#,
    )
    .await
}

async fn create_concurrent_topics(
    context: &PostgresForumTestDb,
    tenant_id: Uuid,
    category_id: Uuid,
    author_id: Uuid,
    count: usize,
) -> TestResult<()> {
    let barrier = Arc::new(Barrier::new(count));
    let mut handles = Vec::with_capacity(count);

    for index in 0..count {
        let db = context.peer().await?;
        let barrier = barrier.clone();
        handles.push(tokio::spawn(async move {
            let service = TopicService::new(db.clone(), event_bus(db));
            barrier.wait().await;
            service
                .create(
                    tenant_id,
                    SecurityContext::new(UserRole::Customer, Some(author_id)),
                    CreateTopicInput {
                        locale: "en".to_string(),
                        category_id,
                        title: format!("Concurrent topic {index}"),
                        slug: Some(format!("concurrent-topic-{index}")),
                        body: "Body".to_string(),
                        body_format: "markdown".to_string(),
                        content_json: None,
                        metadata: serde_json::json!({}),
                        tags: vec![],
                        channel_slugs: None,
                    },
                )
                .await
                .map(|_| ())
                .map_err(|error| error.to_string())
        }));
    }

    for handle in handles {
        let result = handle
            .await
            .map_err(|error| test_error(format!("topic task failed to join: {error}")))?;
        result.map_err(test_error)?;
    }
    Ok(())
}

fn event_bus(db: sea_orm::DatabaseConnection) -> TransactionalEventBus {
    TransactionalEventBus::new(Arc::new(OutboxTransport::new(db)))
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
