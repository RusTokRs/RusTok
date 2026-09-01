mod support;

use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde_json::Value;
use uuid::Uuid;

use support::postgres::{PostgresForumTestDb, execute};
use support::{TestResult, test_error};

const TOPIC_INDEX: &str = "idx_forum_topics_tenant_author_retained";
const REPLY_INDEX: &str = "idx_forum_replies_tenant_author_approved_retained";

#[tokio::test]
async fn approved_posts_aggregate_uses_partial_author_indexes_on_postgres() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("approved_posts_index").await? else {
        return Ok(());
    };

    let outcome = async {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let sql = approved_posts_proof_sql(tenant_id, user_id);

        execute(&context.db, "SET enable_seqscan = off").await?;
        let plan_result = explain_json(&context.db, &sql).await;
        let reset_result = execute(&context.db, "RESET enable_seqscan").await;
        let plan = plan_result?;
        reset_result?;

        let root = plan
            .as_array()
            .and_then(|items| items.first())
            .and_then(|item| item.get("Plan"))
            .ok_or_else(|| test_error("approved-post EXPLAIN JSON is missing the root Plan"))?;
        let mut nodes = Vec::new();
        collect_plan_nodes(root, &mut nodes);
        let index_names = nodes
            .iter()
            .filter_map(|node| node.get("Index Name").and_then(Value::as_str))
            .collect::<Vec<_>>();

        for index_name in [TOPIC_INDEX, REPLY_INDEX] {
            if !index_names.iter().any(|observed| observed == &index_name) {
                return Err(test_error(format!(
                    "approved-post plan must use {index_name}; observed indexes: {index_names:?}"
                )));
            }
        }

        assert_index_definition(
            &context.db,
            TOPIC_INDEX,
            &[
                "tenant_id",
                "author_id",
                "author_id IS NOT NULL",
                "deleted_at IS NULL",
            ],
        )
        .await?;
        assert_index_definition(
            &context.db,
            REPLY_INDEX,
            &[
                "tenant_id",
                "author_id",
                "topic_id",
                "author_id IS NOT NULL",
                "approved",
                "deleted_at IS NULL",
            ],
        )
        .await?;

        Ok(())
    }
    .await;

    context.cleanup().await?;
    outcome
}

/// Proof-only mirror of the owner aggregate in
/// `services/posting_policy_approved_facts.rs`.
fn approved_posts_proof_sql(tenant_id: Uuid, user_id: Uuid) -> String {
    format!(
        r#"
SELECT
    (
        SELECT COUNT(*)::bigint
        FROM forum_topics topic
        WHERE topic.tenant_id = '{tenant_id}'
          AND topic.author_id = '{user_id}'
          AND topic.deleted_at IS NULL
    ) AS approved_topics,
    (
        SELECT COUNT(*)::bigint
        FROM forum_replies reply
        JOIN forum_topics topic
          ON topic.tenant_id = reply.tenant_id
         AND topic.id = reply.topic_id
        WHERE reply.tenant_id = '{tenant_id}'
          AND reply.author_id = '{user_id}'
          AND reply.status = 'approved'
          AND reply.deleted_at IS NULL
          AND topic.deleted_at IS NULL
    ) AS approved_replies
"#
    )
}

async fn explain_json(db: &sea_orm::DatabaseConnection, sql: &str) -> TestResult<Value> {
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!("EXPLAIN (COSTS OFF, FORMAT JSON) {sql}"),
        ))
        .await?
        .ok_or_else(|| test_error("approved-post EXPLAIN JSON returned no row"))?;
    Ok(row.try_get("", "QUERY PLAN")?)
}

async fn assert_index_definition(
    db: &sea_orm::DatabaseConnection,
    index_name: &str,
    required_fragments: &[&str],
) -> TestResult<()> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT indexdef FROM pg_indexes WHERE schemaname = current_schema() AND indexname = $1",
            vec![index_name.into()],
        ))
        .await?
        .ok_or_else(|| test_error(format!("PostgreSQL migration should create {index_name}")))?;
    let definition = row.try_get::<String>("", "indexdef")?;
    for fragment in required_fragments {
        if !definition.contains(fragment) {
            return Err(test_error(format!(
                "{index_name} is missing `{fragment}`: {definition}"
            )));
        }
    }
    Ok(())
}

fn collect_plan_nodes<'a>(node: &'a Value, nodes: &mut Vec<&'a Value>) {
    nodes.push(node);
    if let Some(children) = node.get("Plans").and_then(Value::as_array) {
        for child in children {
            collect_plan_nodes(child, nodes);
        }
    }
}
