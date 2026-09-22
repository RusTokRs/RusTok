mod support;

use support::TestResult;
use support::event_contract::exercise_forum_event_contract;
use support::postgres::PostgresForumTestDb;

#[tokio::test]
async fn postgres_captures_complete_forum_domain_event_contract() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("domain_events").await? else {
        return Ok(());
    };

    let result = exercise_forum_event_contract(&context.db).await;
    context.cleanup().await?;
    result
}
