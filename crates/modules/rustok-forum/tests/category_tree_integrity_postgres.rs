mod support;

use uuid::Uuid;

use support::TestResult;
use support::postgres::{PostgresForumTestDb, execute, expect_rejected};

#[tokio::test]
async fn postgres_rejects_self_parent_and_category_cycles() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("category_tree").await? else {
        return Ok(());
    };

    let outcome = async {
        let tenant_id = Uuid::new_v4();
        let root_id = Uuid::new_v4();
        let child_id = Uuid::new_v4();
        let grandchild_id = Uuid::new_v4();

        execute(
            &context.db,
            format!(
                r#"
INSERT INTO forum_categories
    (id, tenant_id, position, moderated, topic_count, reply_count)
VALUES
    ('{root_id}', '{tenant_id}', 0, FALSE, 0, 0),
    ('{child_id}', '{tenant_id}', 1, FALSE, 0, 0),
    ('{grandchild_id}', '{tenant_id}', 2, FALSE, 0, 0);

UPDATE forum_categories SET parent_id = '{root_id}' WHERE id = '{child_id}';
UPDATE forum_categories SET parent_id = '{child_id}' WHERE id = '{grandchild_id}';
"#,
            ),
        )
        .await?;

        expect_rejected(
            &context.db,
            format!("UPDATE forum_categories SET parent_id = '{root_id}' WHERE id = '{root_id}'"),
            "self-parent category",
        )
        .await?;

        expect_rejected(
            &context.db,
            format!(
                "UPDATE forum_categories SET parent_id = '{grandchild_id}' WHERE id = '{root_id}'"
            ),
            "three-level category cycle",
        )
        .await?;

        execute(
            &context.db,
            format!(
                "UPDATE forum_categories SET parent_id = '{root_id}' WHERE id = '{grandchild_id}'"
            ),
        )
        .await?;
        execute(
            &context.db,
            format!("UPDATE forum_categories SET parent_id = NULL WHERE id = '{grandchild_id}'"),
        )
        .await?;

        Ok(())
    }
    .await;

    context.cleanup().await?;
    outcome
}
