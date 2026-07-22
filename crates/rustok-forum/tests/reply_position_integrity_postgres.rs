mod support;

use std::sync::Arc;

use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{CreateReplyInput, ReplyService};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use tokio::sync::Barrier;
use uuid::Uuid;

use support::postgres::{PostgresForumTestDb, execute, expect_rejected};
use support::{TestResult, test_error};

#[tokio::test]
async fn postgres_allocates_unique_contiguous_reply_positions() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("reply_positions").await? else {
        return Ok(());
    };

    let outcome = async {
        let seed = seed_forum(&context.db).await?;
        install_reply_insert_delay(&context.db).await?;
        create_concurrent_replies(&context, seed, 8).await?;

        let positions = query_positions(&context.db, seed.tenant_id, seed.topic_id).await?;
        let expected = (1_i64..=8).collect::<Vec<_>>();
        if positions != expected {
            return Err(test_error(format!(
                "reply positions are not unique and contiguous: {positions:?}"
            )));
        }

        let direct = seed_forum(&context.db).await?;
        let first_reply_id = Uuid::new_v4();
        let second_reply_id = Uuid::new_v4();
        execute(
            &context.db,
            format!(
                "INSERT INTO forum_replies
                    (id, tenant_id, topic_id, status, position)
                 VALUES
                    ('{first_reply_id}', '{}', '{}', 'approved', 777),
                    ('{second_reply_id}', '{}', '{}', 'approved', 777)",
                direct.tenant_id, direct.topic_id, direct.tenant_id, direct.topic_id,
            ),
        )
        .await?;

        let direct_positions =
            query_positions(&context.db, direct.tenant_id, direct.topic_id).await?;
        if direct_positions != vec![1, 2] {
            return Err(test_error(format!(
                "database allocator did not normalize direct inserts: {direct_positions:?}"
            )));
        }

        expect_rejected(
            &context.db,
            format!(
                "UPDATE forum_replies
                 SET position = 1
                 WHERE tenant_id = '{}' AND id = '{second_reply_id}'",
                direct.tenant_id
            ),
            "duplicate reply position update",
        )
        .await?;

        expect_rejected(
            &context.db,
            format!(
                "UPDATE forum_replies
                 SET position = 0
                 WHERE tenant_id = '{}' AND id = '{first_reply_id}'",
                direct.tenant_id
            ),
            "non-positive reply position update",
        )
        .await?;

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
}

async fn seed_forum(db: &sea_orm::DatabaseConnection) -> TestResult<ForumSeed> {
    let seed = ForumSeed {
        tenant_id: Uuid::new_v4(),
        category_id: Uuid::new_v4(),
        topic_id: Uuid::new_v4(),
    };
    execute(
        db,
        format!(
            "INSERT INTO forum_categories
                (id, tenant_id, position, moderated, topic_count, reply_count)
             VALUES
                ('{}', '{}', 0, FALSE, 1, 0);
             INSERT INTO forum_topics
                (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
             VALUES
                ('{}', '{}', '{}', 'open', '{{}}', FALSE, FALSE, 0);",
            seed.category_id, seed.tenant_id, seed.topic_id, seed.tenant_id, seed.category_id,
        ),
    )
    .await?;
    Ok(seed)
}

async fn install_reply_insert_delay(db: &sea_orm::DatabaseConnection) -> TestResult<()> {
    execute(
        db,
        r#"
CREATE FUNCTION forum_test_delay_reply_position_insert()
RETURNS trigger AS $$
BEGIN
    PERFORM pg_sleep(0.15);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER forum_99_test_delay_reply_position_insert
BEFORE INSERT ON forum_replies
FOR EACH ROW EXECUTE FUNCTION forum_test_delay_reply_position_insert();
"#,
    )
    .await
}

async fn create_concurrent_replies(
    context: &PostgresForumTestDb,
    seed: ForumSeed,
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
                    seed.tenant_id,
                    customer_security(),
                    seed.topic_id,
                    reply_input(&format!("concurrent reply {index}")),
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

async fn query_positions(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> TestResult<Vec<i64>> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT position::bigint AS value
                 FROM forum_replies
                 WHERE tenant_id = '{tenant_id}' AND topic_id = '{topic_id}'
                 ORDER BY position"
            ),
        ))
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| row.try_get("", "value"))
        .collect::<Result<Vec<i64>, _>>()?)
}

fn event_bus(db: sea_orm::DatabaseConnection) -> TransactionalEventBus {
    TransactionalEventBus::new(Arc::new(OutboxTransport::new(db)))
}

fn customer_security() -> SecurityContext {
    SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()))
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
