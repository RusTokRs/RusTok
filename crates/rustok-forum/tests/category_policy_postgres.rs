mod support;

use support::category_policy::exercise_category_topic_policy;
use support::postgres::PostgresForumTestDb;
use support::TestResult;

#[tokio::test]
async fn postgres_category_topic_policy_blocks_and_restores_topic_writes() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("category_policy").await? else {
        return Ok(());
    };

    let outcome = exercise_category_topic_policy(&context.db).await;
    context.cleanup().await?;
    outcome
}
