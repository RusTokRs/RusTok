mod support;

use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use uuid::Uuid;

use support::postgres::{PostgresForumTestDb, execute, expect_rejected};
use support::{TestResult, test_error};

#[tokio::test]
async fn reply_positions_use_a_monotonic_topic_allocator() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("wave_reply_allocator").await? else {
        return Ok(());
    };

    let outcome = async {
        let tenant_id = Uuid::new_v4();
        let category_id = Uuid::new_v4();
        let topic_id = Uuid::new_v4();
        let first_reply_id = Uuid::new_v4();
        let second_reply_id = Uuid::new_v4();

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
                    ('{topic_id}', '{tenant_id}', '{category_id}', 'open', '{{}}', FALSE, FALSE, 0);
                 INSERT INTO forum_replies
                    (id, tenant_id, topic_id, status, position)
                 VALUES
                    ('{first_reply_id}', '{tenant_id}', '{topic_id}', 'approved', 999);
                 INSERT INTO forum_replies
                    (id, tenant_id, topic_id, status, position)
                 VALUES
                    ('{second_reply_id}', '{tenant_id}', '{topic_id}', 'approved', 999);"
            ),
        )
        .await?;

        let row = context
            .db
            .query_one(Statement::from_string(
                DatabaseBackend::Postgres,
                format!(
                    "SELECT
                        COUNT(*)::bigint AS reply_count,
                        COUNT(DISTINCT position)::bigint AS distinct_count,
                        MIN(position)::bigint AS min_position,
                        MAX(position)::bigint AS max_position,
                        (SELECT next_reply_position
                           FROM forum_topics
                          WHERE tenant_id = '{tenant_id}' AND id = '{topic_id}')::bigint
                            AS next_position
                     FROM forum_replies
                     WHERE tenant_id = '{tenant_id}' AND topic_id = '{topic_id}'"
                ),
            ))
            .await?
            .ok_or_else(|| test_error("reply allocator query returned no row"))?;

        let reply_count: i64 = row.try_get("", "reply_count")?;
        let distinct_count: i64 = row.try_get("", "distinct_count")?;
        let min_position: i64 = row.try_get("", "min_position")?;
        let max_position: i64 = row.try_get("", "max_position")?;
        let next_position: i64 = row.try_get("", "next_position")?;

        if (reply_count, distinct_count, min_position, max_position, next_position)
            != (2, 2, 1, 2, 3)
        {
            return Err(test_error(format!(
                "unexpected reply allocator state: count={reply_count}, distinct={distinct_count}, min={min_position}, max={max_position}, next={next_position}"
            )));
        }

        Ok(())
    }
    .await;

    context.cleanup().await?;
    outcome
}

#[tokio::test]
async fn nonempty_category_physical_delete_is_rejected() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("wave_category_delete_guard").await? else {
        return Ok(());
    };

    let outcome = async {
        let tenant_id = Uuid::new_v4();
        let category_id = Uuid::new_v4();
        let topic_id = Uuid::new_v4();

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

        expect_rejected(
            &context.db,
            format!(
                "DELETE FROM forum_categories WHERE tenant_id = '{tenant_id}' AND id = '{category_id}'"
            ),
            "non-empty category physical delete",
        )
        .await
    }
    .await;

    context.cleanup().await?;
    outcome
}
