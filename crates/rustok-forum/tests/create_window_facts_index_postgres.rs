mod support;

use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde_json::Value;
use uuid::Uuid;

use support::postgres::{PostgresForumTestDb, execute};
use support::{TestResult, test_error};

const TOPIC_INDEX: &str = "idx_forum_topics_tenant_author_created_at";
const REPLY_INDEX: &str = "idx_forum_replies_tenant_author_created_at";

#[tokio::test]
async fn create_window_queries_use_author_time_indexes_on_postgres() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("create_window_index").await? else {
        return Ok(());
    };

    let outcome = async {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        execute(&context.db, "SET enable_seqscan = off").await?;
        let proof_result = async {
            for (table, index_name) in [
                ("forum_topics", TOPIC_INDEX),
                ("forum_replies", REPLY_INDEX),
            ] {
                let sql = create_window_proof_sql(table, tenant_id, user_id);
                let plan = explain_json(&context.db, &sql).await?;
                let root = plan
                    .as_array()
                    .and_then(|items| items.first())
                    .and_then(|item| item.get("Plan"))
                    .ok_or_else(|| {
                        test_error(format!(
                            "{table} create-window EXPLAIN JSON is missing the root Plan"
                        ))
                    })?;
                let mut nodes = Vec::new();
                collect_plan_nodes(root, &mut nodes);
                let index_names = nodes
                    .iter()
                    .filter_map(|node| node.get("Index Name").and_then(Value::as_str))
                    .collect::<Vec<_>>();
                if !index_names.iter().any(|observed| observed == &index_name) {
                    return Err(test_error(format!(
                        "{table} create-window plan must use {index_name}; observed indexes: {index_names:?}"
                    )));
                }

                assert_index_definition(
                    &context.db,
                    index_name,
                    &["tenant_id", "author_id", "created_at DESC", "author_id IS NOT NULL"],
                )
                .await?;
            }
            Ok(())
        }
        .await;
        let reset_result = execute(&context.db, "RESET enable_seqscan").await;
        proof_result?;
        reset_result?;
        Ok(())
    }
    .await;

    context.cleanup().await?;
    outcome
}

/// Proof-only mirror of the owner query in
/// `services/posting_policy_create_window_facts.rs`.
fn create_window_proof_sql(table: &str, tenant_id: Uuid, user_id: Uuid) -> String {
    format!(
        r#"
SELECT COUNT(*)::bigint AS create_count
FROM {table}
WHERE tenant_id = '{tenant_id}'
  AND author_id = '{user_id}'
  AND created_at >= TIMESTAMPTZ '2026-07-28 11:00:00+00'
  AND created_at <= TIMESTAMPTZ '2026-07-28 12:00:00+00'
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
        .ok_or_else(|| test_error("create-window EXPLAIN JSON returned no row"))?;
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
