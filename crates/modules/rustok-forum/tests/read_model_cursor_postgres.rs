mod support;

use support::TestResult;
use support::postgres::PostgresForumTestDb;
use support::read_model::exercise_bounded_cursor_read_models;

#[tokio::test]
async fn postgres_forum_read_models_are_bounded_and_cursor_stable() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("cursor_read_model").await? else {
        return Ok(());
    };

    let outcome = exercise_bounded_cursor_read_models(&context.db).await;
    context.cleanup().await?;
    outcome
}
