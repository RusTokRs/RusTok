mod support;

use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{
    ForumReadModelService, ForumTopicReadStateService, MarkForumTopicReadInput,
};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde_json::Value;
use uuid::Uuid;

use support::postgres::{PostgresForumTestDb, execute};
use support::{TestResult, test_error};

const TOPIC_COUNT: usize = 128;
const REPLIES_PER_TOPIC: i64 = 64;
const LAST_READ_REPLY_POSITION: i64 = REPLIES_PER_TOPIC - 1;
const REVISIONS_PER_TOPIC: i64 = 4;
const PROJECTION_TOPIC_COUNT: usize = 100;

#[tokio::test]
async fn concurrent_devices_converge_to_component_wise_maximum_on_postgres() -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("topic_read_state_concurrency").await? else {
        return Ok(());
    };

    let outcome = async {
        let tenant_id = Uuid::new_v4();
        let concurrent_user_id = Uuid::new_v4();
        let topic_id = seed_concurrency_topic(&context.db, tenant_id).await?;
        let latest_revision = latest_topic_revision(&context.db, tenant_id, topic_id).await?;
        let peer = context.peer().await?;

        let first_security = SecurityContext::new(UserRole::Customer, Some(concurrent_user_id));
        let second_security = first_security.clone();
        let first = ForumTopicReadStateService::new(context.db.clone());
        let second = ForumTopicReadStateService::new(peer.clone());

        let (position_result, revision_result) = tokio::join!(
            first.mark_topic_read(
                tenant_id,
                topic_id,
                first_security,
                MarkForumTopicReadInput {
                    last_read_position: REPLIES_PER_TOPIC,
                    last_read_revision: 0,
                },
            ),
            second.mark_topic_read(
                tenant_id,
                topic_id,
                second_security,
                MarkForumTopicReadInput {
                    last_read_position: REPLIES_PER_TOPIC / 2,
                    last_read_revision: latest_revision,
                },
            )
        );
        position_result?;
        revision_result?;

        let final_state = ForumTopicReadStateService::new(context.db.clone())
            .get_topic_read_state(
                tenant_id,
                topic_id,
                SecurityContext::new(UserRole::Customer, Some(concurrent_user_id)),
            )
            .await?;
        if final_state.last_read_position != REPLIES_PER_TOPIC
            || final_state.last_read_revision != latest_revision
        {
            return Err(test_error(format!(
                "concurrent read state did not converge: position={}, revision={}, expected_position={REPLIES_PER_TOPIC}, expected_revision={latest_revision}",
                final_state.last_read_position, final_state.last_read_revision
            )));
        }

        let direct_regression = peer
            .execute_unprepared(&format!(
                "UPDATE forum_topic_read_states
                    SET last_read_position = 0,
                        last_read_revision = 0
                  WHERE tenant_id = '{tenant_id}'
                    AND topic_id = '{topic_id}'
                    AND user_id = '{concurrent_user_id}'"
            ))
            .await;
        if direct_regression.is_ok() {
            return Err(test_error(
                "PostgreSQL read-state trigger accepted a direct regression",
            ));
        }

        Ok(())
    }
    .await;

    context.cleanup().await?;
    outcome
}

#[tokio::test]
async fn bounded_unread_aggregate_matches_large_fixture_and_plan_contract_on_postgres(
) -> TestResult<()> {
    let Some(context) = PostgresForumTestDb::setup("topic_unread_aggregate_plan").await? else {
        return Ok(());
    };

    let outcome = async {
        let tenant_id = Uuid::new_v4();
        let reader_id = Uuid::new_v4();
        let topic_ids = seed_production_sized_read_fixture(&context.db, tenant_id, reader_id).await?;
        let projection_ids = topic_ids
            .into_iter()
            .take(PROJECTION_TOPIC_COUNT)
            .collect::<Vec<_>>();
        let security = SecurityContext::new(UserRole::Customer, Some(reader_id));

        let summaries = ForumReadModelService::new(context.db.clone())
            .summarize_topic_ids(tenant_id, security, projection_ids.clone())
            .await?;
        if summaries.len() != PROJECTION_TOPIC_COUNT {
            return Err(test_error(format!(
                "bounded unread projection returned {} rows instead of {PROJECTION_TOPIC_COUNT}",
                summaries.len()
            )));
        }

        for (index, summary) in summaries.iter().enumerate() {
            match index {
                0..=31 => {
                    if !summary.read_state_explicit
                        || summary.unread_count != 1
                        || summary.has_unread_topic_revision
                        || !summary.is_unread
                    {
                        return Err(test_error(format!(
                            "reply-unread fixture mismatch at index {index}: {summary:?}"
                        )));
                    }
                }
                32..=63 => {
                    if !summary.read_state_explicit
                        || summary.unread_count != 0
                        || !summary.has_unread_topic_revision
                        || !summary.is_unread
                    {
                        return Err(test_error(format!(
                            "revision-unread fixture mismatch at index {index}: {summary:?}"
                        )));
                    }
                }
                64..=95 => {
                    if !summary.read_state_explicit
                        || summary.unread_count != 0
                        || summary.has_unread_topic_revision
                        || summary.is_unread
                    {
                        return Err(test_error(format!(
                            "read fixture mismatch at index {index}: {summary:?}"
                        )));
                    }
                }
                _ => {
                    if summary.read_state_explicit
                        || summary.unread_count != REPLIES_PER_TOPIC
                        || !summary.has_unread_topic_revision
                        || !summary.is_unread
                    {
                        return Err(test_error(format!(
                            "unseen fixture mismatch at index {index}: {summary:?}"
                        )));
                    }
                }
            }
        }

        let sql = unread_summary_proof_sql(tenant_id, reader_id, &projection_ids);
        let natural_plan = explain_json(&context.db, &sql, true).await?;
        assert_plan_is_bounded(&natural_plan, PROJECTION_TOPIC_COUNT as i64)?;

        execute(&context.db, "SET enable_seqscan = off").await?;
        let index_plan_result = explain_json(&context.db, &sql, false).await;
        let reset_result = execute(&context.db, "RESET enable_seqscan").await;
        let index_plan = index_plan_result?;
        reset_result?;
        assert_index_capability(&index_plan)?;

        Ok(())
    }
    .await;

    context.cleanup().await?;
    outcome
}

async fn seed_concurrency_topic(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<Uuid> {
    let category_id = Uuid::new_v4();
    let topic_id = Uuid::new_v4();
    execute(
        db,
        format!(
            r#"
INSERT INTO forum_categories
    (id, tenant_id, position, moderated, topic_count, reply_count)
VALUES
    ('{category_id}', '{tenant_id}', 0, FALSE, 0, 0);

INSERT INTO forum_topics
    (id, tenant_id, category_id, status, metadata, is_pinned, is_locked,
     reply_count, created_at, updated_at)
VALUES
    ('{topic_id}', '{tenant_id}', '{category_id}', 'open', '{{}}'::jsonb,
     FALSE, FALSE, 0, CURRENT_TIMESTAMP - INTERVAL '2 days',
     CURRENT_TIMESTAMP - INTERVAL '2 days');

INSERT INTO forum_replies
    (id, tenant_id, topic_id, status, position, created_at, updated_at)
SELECT
    md5('{topic_id}:reply:' || reply_no::text)::uuid,
    '{tenant_id}',
    '{topic_id}',
    'approved',
    reply_no,
    CURRENT_TIMESTAMP - INTERVAL '1 day' + reply_no * INTERVAL '1 second',
    CURRENT_TIMESTAMP - INTERVAL '1 day' + reply_no * INTERVAL '1 second'
FROM generate_series(1, {REPLIES_PER_TOPIC}) AS reply_no;

INSERT INTO forum_topic_revisions
    (tenant_id, topic_id, locale, title, slug, body, body_format, metadata,
     revision_reason, created_at)
SELECT
    '{tenant_id}',
    '{topic_id}',
    'en',
    'Concurrent read-state proof topic',
    NULL,
    'Revision ' || revision_no::text,
    'markdown',
    '{{}}'::jsonb,
    'edit',
    CURRENT_TIMESTAMP - INTERVAL '12 hours' + revision_no * INTERVAL '1 second'
FROM generate_series(1, {REVISIONS_PER_TOPIC}) AS revision_no;
"#
        ),
    )
    .await?;
    Ok(topic_id)
}

async fn seed_production_sized_read_fixture(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    reader_id: Uuid,
) -> TestResult<Vec<Uuid>> {
    let category_id = Uuid::new_v4();
    execute(
        db,
        format!(
            r#"
INSERT INTO forum_categories
    (id, tenant_id, position, moderated, topic_count, reply_count)
VALUES
    ('{category_id}', '{tenant_id}', 0, FALSE, 0, 0);

INSERT INTO forum_topics
    (id, tenant_id, category_id, status, metadata, is_pinned, is_locked,
     reply_count, created_at, updated_at)
SELECT
    md5('{tenant_id}:topic:' || topic_no::text)::uuid,
    '{tenant_id}',
    '{category_id}',
    'open',
    '{{}}'::jsonb,
    FALSE,
    FALSE,
    0,
    CURRENT_TIMESTAMP - INTERVAL '2 days' + topic_no * INTERVAL '1 second',
    CURRENT_TIMESTAMP - INTERVAL '2 days' + topic_no * INTERVAL '1 second'
FROM generate_series(1, {TOPIC_COUNT}) AS topic_no;

INSERT INTO forum_replies
    (id, tenant_id, topic_id, status, position, created_at, updated_at)
SELECT
    md5(topic.id::text || ':reply:' || reply_no::text)::uuid,
    '{tenant_id}',
    topic.id,
    'approved',
    reply_no,
    CURRENT_TIMESTAMP - INTERVAL '1 day' + reply_no * INTERVAL '1 second',
    CURRENT_TIMESTAMP - INTERVAL '1 day' + reply_no * INTERVAL '1 second'
FROM forum_topics AS topic
CROSS JOIN generate_series(1, {REPLIES_PER_TOPIC}) AS reply_no
WHERE topic.tenant_id = '{tenant_id}';

INSERT INTO forum_topic_revisions
    (tenant_id, topic_id, locale, title, slug, body, body_format, metadata,
     revision_reason, created_at)
SELECT
    '{tenant_id}',
    topic.id,
    'en',
    'Read-state proof topic',
    NULL,
    'Revision ' || revision_no::text,
    'markdown',
    '{{}}'::jsonb,
    'edit',
    CURRENT_TIMESTAMP - INTERVAL '12 hours' + revision_no * INTERVAL '1 second'
FROM forum_topics AS topic
CROSS JOIN generate_series(1, {REVISIONS_PER_TOPIC}) AS revision_no
WHERE topic.tenant_id = '{tenant_id}';

WITH ranked_topics AS (
    SELECT
        topic.id,
        row_number() OVER (ORDER BY topic.id) AS ordinal
    FROM forum_topics AS topic
    WHERE topic.tenant_id = '{tenant_id}'
), revision_marks AS (
    SELECT
        ranked.id,
        ranked.ordinal,
        latest.id AS latest_revision,
        previous.id AS previous_revision
    FROM ranked_topics AS ranked
    CROSS JOIN LATERAL (
        SELECT revision.id
        FROM forum_topic_revisions AS revision
        WHERE revision.tenant_id = '{tenant_id}'
          AND revision.topic_id = ranked.id
        ORDER BY revision.id DESC
        LIMIT 1
    ) AS latest
    CROSS JOIN LATERAL (
        SELECT revision.id
        FROM forum_topic_revisions AS revision
        WHERE revision.tenant_id = '{tenant_id}'
          AND revision.topic_id = ranked.id
        ORDER BY revision.id DESC
        OFFSET 1 LIMIT 1
    ) AS previous
)
INSERT INTO forum_topic_read_states
    (tenant_id, topic_id, user_id, last_read_position, last_read_revision,
     created_at, updated_at)
SELECT
    '{tenant_id}',
    mark.id,
    '{reader_id}',
    CASE WHEN mark.ordinal <= 32 THEN {LAST_READ_REPLY_POSITION} ELSE {REPLIES_PER_TOPIC} END,
    CASE
        WHEN mark.ordinal BETWEEN 33 AND 64 THEN mark.previous_revision
        ELSE mark.latest_revision
    END,
    CURRENT_TIMESTAMP,
    CURRENT_TIMESTAMP
FROM revision_marks AS mark
WHERE mark.ordinal <= 96;

ANALYZE forum_topics;
ANALYZE forum_replies;
ANALYZE forum_topic_revisions;
ANALYZE forum_topic_read_states;
"#
        ),
    )
    .await?;

    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT id
                   FROM forum_topics
                  WHERE tenant_id = '{tenant_id}'
                  ORDER BY id"
            ),
        ))
        .await?;
    let topic_ids = rows
        .into_iter()
        .map(|row| row.try_get::<Uuid>("", "id"))
        .collect::<Result<Vec<_>, _>>()?;
    if topic_ids.len() != TOPIC_COUNT {
        return Err(test_error(format!(
            "fixture created {} topics instead of {TOPIC_COUNT}",
            topic_ids.len()
        )));
    }
    Ok(topic_ids)
}

async fn latest_topic_revision(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> TestResult<i64> {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "SELECT MAX(id)::bigint AS revision_id
                   FROM forum_topic_revisions
                  WHERE tenant_id = '{tenant_id}'
                    AND topic_id = '{topic_id}'"
            ),
        ))
        .await?
        .ok_or_else(|| test_error("latest topic revision query returned no row"))?;
    Ok(row.try_get("", "revision_id")?)
}

/// Proof-only mirror of the owner aggregate in `services/read_model.rs`.
/// The static verifier binds the material query clauses to the owner source so
/// this EXPLAIN evidence cannot silently drift into a different policy query.
fn unread_summary_proof_sql(tenant_id: Uuid, user_id: Uuid, topic_ids: &[Uuid]) -> String {
    let topic_ids = topic_ids
        .iter()
        .map(|topic_id| format!("'{topic_id}'"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"
SELECT
    topic.id AS topic_id,
    state.user_id AS state_user_id,
    COALESCE(state.last_read_position, 0) AS last_read_position,
    COALESCE(state.last_read_revision, 0) AS last_read_revision,
    COUNT(DISTINCT unread_reply.id) AS unread_count,
    COUNT(DISTINCT unread_revision.id) AS unread_revision_count
FROM forum_topics topic
LEFT JOIN forum_topic_read_states state
  ON state.tenant_id = topic.tenant_id
 AND state.topic_id = topic.id
 AND state.user_id = '{user_id}'
LEFT JOIN forum_replies unread_reply
  ON unread_reply.tenant_id = topic.tenant_id
 AND unread_reply.topic_id = topic.id
 AND unread_reply.status = 'approved'
 AND (
      unread_reply.position > COALESCE(state.last_read_position, 0)
      OR unread_reply.updated_at > state.updated_at
 )
LEFT JOIN forum_topic_revisions unread_revision
  ON unread_revision.tenant_id = topic.tenant_id
 AND unread_revision.topic_id = topic.id
 AND unread_revision.id > COALESCE(state.last_read_revision, 0)
WHERE topic.tenant_id = '{tenant_id}'
  AND topic.id IN ({topic_ids})
GROUP BY
    topic.id,
    state.user_id,
    state.last_read_position,
    state.last_read_revision
"#
    )
}

async fn explain_json(
    db: &sea_orm::DatabaseConnection,
    sql: &str,
    analyze: bool,
) -> TestResult<Value> {
    let options = if analyze {
        "ANALYZE, BUFFERS, COSTS OFF, FORMAT JSON"
    } else {
        "COSTS OFF, FORMAT JSON"
    };
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            format!("EXPLAIN ({options}) {sql}"),
        ))
        .await?
        .ok_or_else(|| test_error("EXPLAIN JSON returned no row"))?;
    Ok(row.try_get("", "QUERY PLAN")?)
}

fn assert_plan_is_bounded(plan: &Value, expected_rows: i64) -> TestResult<()> {
    let root = plan
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item.get("Plan"))
        .ok_or_else(|| test_error("EXPLAIN JSON is missing the root Plan"))?;
    let actual_rows = root
        .get("Actual Rows")
        .and_then(Value::as_i64)
        .ok_or_else(|| test_error("natural EXPLAIN plan is missing Actual Rows"))?;
    if actual_rows != expected_rows {
        return Err(test_error(format!(
            "natural unread aggregate plan returned {actual_rows} rows instead of {expected_rows}"
        )));
    }

    let mut nodes = Vec::new();
    collect_plan_nodes(root, &mut nodes);
    if nodes.iter().any(|node| {
        node.get("Parent Relationship")
            .and_then(Value::as_str)
            .is_some_and(|relationship| relationship == "SubPlan")
    }) {
        return Err(test_error(
            "unread aggregate plan contains a per-row SubPlan",
        ));
    }

    let relation_names = nodes
        .iter()
        .filter_map(|node| node.get("Relation Name").and_then(Value::as_str))
        .collect::<Vec<_>>();
    for relation in [
        "forum_topics",
        "forum_topic_read_states",
        "forum_replies",
        "forum_topic_revisions",
    ] {
        if !relation_names.iter().any(|name| name == &relation) {
            return Err(test_error(format!(
                "natural unread aggregate plan is missing relation {relation}: {relation_names:?}"
            )));
        }
    }
    Ok(())
}

fn assert_index_capability(plan: &Value) -> TestResult<()> {
    let root = plan
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item.get("Plan"))
        .ok_or_else(|| test_error("index-capability EXPLAIN JSON is missing the root Plan"))?;
    let mut nodes = Vec::new();
    collect_plan_nodes(root, &mut nodes);
    let index_names = nodes
        .iter()
        .filter_map(|node| node.get("Index Name").and_then(Value::as_str))
        .collect::<Vec<_>>();

    for (label, candidates) in [
        (
            "topic read-state lookup",
            &["forum_topic_read_states_pkey"][..],
        ),
        (
            "approved reply lookup",
            &[
                "uq_forum_replies_tenant_topic_position",
                "idx_forum_replies_cursor",
                "idx_forum_replies_tenant_topic_deleted",
                "idx_forum_replies_topic_position",
            ][..],
        ),
        (
            "topic revision lookup",
            &["idx_forum_topic_revisions_tenant_topic_created"][..],
        ),
    ] {
        if !candidates
            .iter()
            .any(|candidate| index_names.iter().any(|name| name == candidate))
        {
            return Err(test_error(format!(
                "index-capability plan is missing {label}; observed indexes: {index_names:?}"
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
