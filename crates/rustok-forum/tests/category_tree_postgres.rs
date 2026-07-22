mod support;

use support::TestResult;
use support::category_tree::exercise_category_tree_read_model;
use support::postgres::PostgresForumTestDb;

#[tokio::test]
async fn postgres_forum_category_tree_is_nested_bounded_and_tenant_scoped() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("category_tree").await? else {
        return Ok(());
    };

    let outcome = exercise_category_tree_read_model(&context.db).await;
    context.cleanup().await?;
    outcome
}
