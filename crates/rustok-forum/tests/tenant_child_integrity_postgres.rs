mod support;

use sea_orm::DatabaseConnection;
use uuid::Uuid;

use support::TestResult;
use support::postgres::{PostgresForumTestDb, execute, expect_rejected as assert_rejected};

#[tokio::test]
async fn postgres_rejects_cross_tenant_forum_child_rows() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("tenant_children").await? else {
        return Ok(());
    };

    let result = exercise_constraints(&context.db).await;
    context.cleanup().await?;
    result
}

async fn exercise_constraints(db: &DatabaseConnection) -> TestResult<()> {
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let category_a = Uuid::new_v4();
    let category_b = Uuid::new_v4();
    let topic_a = Uuid::new_v4();
    let topic_b = Uuid::new_v4();
    let reply_a = Uuid::new_v4();

    execute(
        db,
        format!(
            r#"
INSERT INTO forum_categories
    (id, tenant_id, position, moderated, topic_count, reply_count)
VALUES
    ('{category_a}', '{tenant_a}', 0, FALSE, 0, 0),
    ('{category_b}', '{tenant_b}', 0, FALSE, 0, 0)
"#
        ),
    )
    .await?;

    execute(
        db,
        format!(
            r#"
INSERT INTO forum_topics
    (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
VALUES
    ('{topic_a}', '{tenant_a}', '{category_a}', 'open', '{{}}', FALSE, FALSE, 0),
    ('{topic_b}', '{tenant_b}', '{category_b}', 'open', '{{}}', FALSE, FALSE, 0)
"#
        ),
    )
    .await?;

    execute(
        db,
        format!(
            r#"
INSERT INTO forum_replies
    (id, tenant_id, topic_id, status, position)
VALUES
    ('{reply_a}', '{tenant_a}', '{topic_a}', 'approved', 1)
"#
        ),
    )
    .await?;

    assert_rejected(
        db,
        format!(
            r#"
INSERT INTO forum_topic_translations
    (id, tenant_id, topic_id, locale, title, body, body_format)
VALUES
    ('{}', '{tenant_b}', '{topic_a}', 'en-US', 'Wrong tenant', 'Body', 'markdown')
"#,
            Uuid::new_v4()
        ),
        "cross-tenant topic translation",
    )
    .await?;

    assert_rejected(
        db,
        format!(
            r#"
INSERT INTO forum_reply_bodies
    (id, tenant_id, reply_id, locale, body, body_format)
VALUES
    ('{}', '{tenant_b}', '{reply_a}', 'en-US', 'Wrong tenant', 'markdown')
"#,
            Uuid::new_v4()
        ),
        "cross-tenant reply body",
    )
    .await?;

    assert_rejected(
        db,
        format!(
            r#"
INSERT INTO forum_topic_channel_access
    (tenant_id, topic_id, channel_slug)
VALUES
    ('{tenant_b}', '{topic_a}', 'private')
"#
        ),
        "cross-tenant topic channel access",
    )
    .await?;

    execute(
        db,
        format!(
            r#"
INSERT INTO forum_topic_translations
    (id, tenant_id, topic_id, locale, title, body, body_format)
VALUES
    ('{}', '{tenant_a}', '{topic_a}', 'zh-Hant-HK', 'Valid', 'Body', 'markdown')
"#,
            Uuid::new_v4()
        ),
    )
    .await?;

    execute(
        db,
        format!(
            r#"
INSERT INTO forum_reply_bodies
    (id, tenant_id, reply_id, locale, body, body_format)
VALUES
    ('{}', '{tenant_a}', '{reply_a}', 'zh-Hant-HK', 'Valid', 'markdown')
"#,
            Uuid::new_v4()
        ),
    )
    .await?;

    execute(
        db,
        format!(
            r#"
INSERT INTO forum_topic_channel_access
    (tenant_id, topic_id, channel_slug)
VALUES
    ('{tenant_a}', '{topic_a}', 'public')
"#
        ),
    )
    .await?;

    Ok(())
}
