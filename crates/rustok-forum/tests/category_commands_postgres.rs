mod support;

use support::TestResult;
use support::category_commands::exercise_category_commands;
use support::postgres::PostgresForumTestDb;

#[tokio::test]
async fn postgres_category_move_and_reorder_are_atomic_and_tenant_scoped() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("category_commands").await? else {
        return Ok(());
    };

    let outcome = exercise_category_commands(&context.db).await;
    context.cleanup().await?;
    outcome
}
