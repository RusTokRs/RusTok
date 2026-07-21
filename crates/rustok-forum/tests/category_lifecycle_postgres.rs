mod support;

use support::category_lifecycle::exercise_category_subtree_lifecycle;
use support::postgres::PostgresForumTestDb;
use support::TestResult;

#[tokio::test]
async fn postgres_category_subtree_archive_and_restore_are_atomic() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("category_lifecycle").await? else {
        return Ok(());
    };

    let outcome = exercise_category_subtree_lifecycle(&context.db).await;
    context.cleanup().await?;
    outcome
}
