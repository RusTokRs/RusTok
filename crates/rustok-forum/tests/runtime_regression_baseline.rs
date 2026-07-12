mod support;

use std::collections::BTreeSet;

use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

use support::postgres::PostgresForumTestDb;
use support::{test_error, TestResult};

const REQUIRED_TENANT_CONSTRAINTS: &[&str] = &[
    "fk_forum_categories_parent_tenant",
    "fk_forum_category_translations_category_tenant",
    "fk_forum_topics_category_tenant",
    "fk_forum_replies_topic_tenant",
    "fk_forum_replies_parent_reply_tenant",
    "fk_forum_topic_translations_topic_tenant",
    "fk_forum_reply_bodies_reply_tenant",
    "fk_forum_topic_channel_access_topic_tenant",
    "fk_forum_topic_votes_topic_tenant",
    "fk_forum_reply_votes_reply_tenant",
    "fk_forum_category_subscriptions_category_tenant",
    "fk_forum_topic_subscriptions_topic_tenant",
    "fk_forum_solutions_topic_tenant",
    "fk_forum_solutions_reply_topic_tenant",
    "fk_forum_topic_tags_topic_tenant",
    "fk_forum_topic_tags_term_tenant",
];

const REQUIRED_LIFECYCLE_CONSTRAINTS: &[&str] = &[
    "chk_forum_topics_status",
    "chk_forum_replies_status",
];

const REQUIRED_TENANT_INDEXES: &[&str] = &[
    "uq_forum_category_translations_tenant_category_locale",
    "uq_forum_topic_translations_tenant_topic_locale",
    "uq_forum_reply_bodies_tenant_reply_locale",
    "idx_forum_topic_channel_access_tenant_channel",
    "uq_forum_solutions_tenant_reply",
    "uq_forum_topic_tags_tenant_topic_term",
];

#[tokio::test]
async fn postgres_forum_tenant_schema_baseline_is_green() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("runtime_baseline").await? else {
        return Ok(());
    };

    let outcome = verify_schema(&context).await;
    context.cleanup().await?;
    outcome
}

async fn verify_schema(context: &PostgresForumTestDb) -> TestResult<()> {
    let constraint_rows = context
        .db
        .query_all(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT conname
             FROM pg_constraint
             WHERE connamespace = (
                 SELECT oid FROM pg_namespace WHERE nspname = current_schema()
             )"
            .to_string(),
        ))
        .await?;
    let constraints = constraint_rows
        .into_iter()
        .map(|row| row.try_get("", "conname"))
        .collect::<Result<BTreeSet<String>, _>>()?;

    for (label, required) in [
        ("tenant", REQUIRED_TENANT_CONSTRAINTS),
        ("lifecycle", REQUIRED_LIFECYCLE_CONSTRAINTS),
    ] {
        let missing_constraints = required
            .iter()
            .filter(|name| !constraints.contains(*name))
            .copied()
            .collect::<Vec<_>>();
        if !missing_constraints.is_empty() {
            return Err(test_error(format!(
                "forum {label} baseline is missing constraints: {}",
                missing_constraints.join(", ")
            )));
        }
    }

    let index_rows = context
        .db
        .query_all(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT indexname
             FROM pg_indexes
             WHERE schemaname = current_schema()"
                .to_string(),
        ))
        .await?;
    let indexes = index_rows
        .into_iter()
        .map(|row| row.try_get("", "indexname"))
        .collect::<Result<BTreeSet<String>, _>>()?;

    let missing_indexes = REQUIRED_TENANT_INDEXES
        .iter()
        .filter(|name| !indexes.contains(*name))
        .copied()
        .collect::<Vec<_>>();
    if !missing_indexes.is_empty() {
        return Err(test_error(format!(
            "forum tenant baseline is missing indexes: {}",
            missing_indexes.join(", ")
        )));
    }

    Ok(())
}
