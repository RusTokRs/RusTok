mod support;

use sea_orm::DatabaseConnection;
use uuid::Uuid;

use support::TestResult;
use support::postgres::{PostgresForumTestDb, execute, expect_rejected as assert_rejected};

#[tokio::test]
async fn postgres_rejects_cross_tenant_forum_core_relations() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("tenant_core").await? else {
        return Ok(());
    };

    let result = exercise_tenant_constraints(&context.db).await;
    context.cleanup().await?;
    result
}

async fn exercise_tenant_constraints(db: &DatabaseConnection) -> TestResult<()> {
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

    assert_rejected(
        db,
        format!(
            r#"
INSERT INTO forum_categories
    (id, tenant_id, parent_id, position, moderated, topic_count, reply_count)
VALUES
    ('{}', '{tenant_b}', '{category_a}', 0, FALSE, 0, 0)
"#,
            Uuid::new_v4()
        ),
        "cross-tenant category parent",
    )
    .await?;

    assert_rejected(
        db,
        format!(
            r#"
INSERT INTO forum_category_translations
    (id, category_id, tenant_id, locale, name, slug)
VALUES
    ('{}', '{category_a}', '{tenant_b}', 'en-US', 'Wrong tenant', 'wrong-tenant')
"#,
            Uuid::new_v4()
        ),
        "cross-tenant category translation",
    )
    .await?;

    assert_rejected(
        db,
        format!(
            r#"
INSERT INTO forum_topics
    (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
VALUES
    ('{topic_b}', '{tenant_b}', '{category_a}', 'open', '{{}}', FALSE, FALSE, 0)
"#
        ),
        "cross-tenant topic category",
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

    assert_rejected(
        db,
        format!(
            r#"
INSERT INTO forum_replies
    (id, tenant_id, topic_id, status, position)
VALUES
    ('{}', '{tenant_b}', '{topic_a}', 'approved', 1)
"#,
            Uuid::new_v4()
        ),
        "cross-tenant reply topic",
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
INSERT INTO forum_replies
    (id, tenant_id, topic_id, parent_reply_id, status, position)
VALUES
    ('{}', '{tenant_b}', '{topic_b}', '{reply_a}', 'approved', 1)
"#,
            Uuid::new_v4()
        ),
        "cross-tenant parent reply",
    )
    .await?;

    execute(
        db,
        format!(
            r#"
INSERT INTO forum_category_translations
    (id, category_id, tenant_id, locale, name, slug)
VALUES
    ('{}', '{category_a}', '{tenant_a}',
     'zh-Hant-HK', 'Long locale', 'long-locale')
"#,
            Uuid::new_v4()
        ),
    )
    .await?;

    Ok(())
}
