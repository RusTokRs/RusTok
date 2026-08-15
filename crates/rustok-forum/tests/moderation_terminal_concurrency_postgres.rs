mod support;

use std::{sync::Arc, time::Duration};

use rustok_api::{HostRuntimeContext, PortActor, PortContext, PortError, PortErrorKind};
use rustok_forum::ForumModerationSubjectAdapterFactory;
use rustok_moderation_api::{
    ApplyModerationDecisionCommand, ModerationDecisionEffect, ModerationDecisionEffectAction,
    ModerationDecisionKind, ModerationReasonCode, ModerationSubjectAdapterFactory,
    ModerationSubjectCommandPort, ModerationSubjectKind, ModerationSubjectRef,
    ModerationVisibilityState,
};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction, Statement,
    TransactionTrait,
};
use tokio::time::{sleep, timeout};
use uuid::Uuid;

use support::postgres::{PostgresForumTestDb, execute};
use support::{TestResult, test_error};

const REVISION_CONFLICT: &str = "forum.moderation_subject_revision_conflict";
const DATABASE_UNAVAILABLE: &str = "forum.moderation_database_unavailable";
const MODERATION_ACTOR: &str = "rustok-moderation";
const APPLY_OPERATION: &str = "apply_moderation_decision";

#[derive(Clone, Copy)]
struct ReplyRaceSeed {
    tenant_id: Uuid,
    category_id: Uuid,
    topic_id: Uuid,
    reply_id: Uuid,
    body_id: Uuid,
    author_id: Uuid,
}

#[tokio::test]
async fn postgres_concurrent_reply_edits_fence_reject_and_remove() -> TestResult<()> {
    let Some(database) = PostgresForumTestDb::setup("moderation_terminal_concurrency").await?
    else {
        return Ok(());
    };

    let outcome = async {
        reply_body_edit_fences_reject_publication(&database).await?;
        reply_body_edit_fences_remove_with_accepted_solution(&database).await?;
        Ok(())
    }
    .await;

    database.cleanup().await?;
    outcome
}

async fn reply_body_edit_fences_reject_publication(
    database: &PostgresForumTestDb,
) -> TestResult<()> {
    let seed = seed_approved_reply(&database.db, "reject-race", false).await?;
    exercise_stale_reply_effect_race(
        database,
        seed,
        "Concurrent reject edit",
        reject_command,
        false,
    )
    .await
}

async fn reply_body_edit_fences_remove_with_accepted_solution(
    database: &PostgresForumTestDb,
) -> TestResult<()> {
    let seed = seed_approved_reply(&database.db, "remove-race", true).await?;
    exercise_stale_reply_effect_race(
        database,
        seed,
        "Concurrent remove edit",
        removed_command,
        true,
    )
    .await
}

async fn exercise_stale_reply_effect_race(
    database: &PostgresForumTestDb,
    seed: ReplyRaceSeed,
    edited_body: &str,
    build_command: fn(ReplyRaceSeed, i64, Uuid) -> TestResult<ApplyModerationDecisionCommand>,
    accepted_solution: bool,
) -> TestResult<()> {
    let reviewed_revision = reply_revision(&database.db, seed).await?;

    let edit_db = database.peer().await?;
    let edit = edit_db.begin().await?;
    edit.execute(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "UPDATE forum_reply_bodies SET body = $1 WHERE tenant_id = $2 AND id = $3",
        vec![
            edited_body.to_string().into(),
            seed.tenant_id.into(),
            seed.body_id.into(),
        ],
    ))
    .await?;
    assert_eq!(reply_revision_in(&edit, seed).await?, reviewed_revision + 1);

    let adapter = reply_adapter(database.peer().await?)?;
    let command = build_command(seed, reviewed_revision, Uuid::new_v4())?;
    let decision_id = command.decision_id;
    let mut application = spawn_application(adapter.clone(), seed.tenant_id, command);

    wait_for_processing_receipt(&database.db, seed.tenant_id, decision_id).await?;
    assert_application_waits_while_edit_owns_revision(&mut application).await?;
    edit.commit().await?;

    let first_error = application
        .await?
        .expect_err("overlapping reply edit must prevent stale terminal moderation effect");
    assert_overlap_error_is_fail_closed(&first_error)?;
    assert_unchanged_public_reply(
        &database.db,
        seed,
        edited_body,
        reviewed_revision + 1,
        accepted_solution,
    )
    .await?;

    let stale = build_command(seed, reviewed_revision, Uuid::new_v4())?;
    let stale_error = adapter
        .apply_moderation_decision(
            application_context(seed.tenant_id, stale.decision_id, "terminal-stale-retry"),
            stale,
        )
        .await
        .expect_err("committed body edit must make old reviewed revision stale");
    assert_revision_conflict(&stale_error)?;
    assert_unchanged_public_reply(
        &database.db,
        seed,
        edited_body,
        reviewed_revision + 1,
        accepted_solution,
    )
    .await?;
    Ok(())
}

fn spawn_application(
    adapter: Arc<dyn ModerationSubjectCommandPort>,
    tenant_id: Uuid,
    command: ApplyModerationDecisionCommand,
) -> tokio::task::JoinHandle<Result<rustok_moderation_api::ModerationDecisionApplication, PortError>>
{
    tokio::spawn(async move {
        adapter
            .apply_moderation_decision(
                application_context(tenant_id, command.decision_id, "terminal-overlap"),
                command,
            )
            .await
    })
}

async fn wait_for_processing_receipt(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    decision_id: Uuid,
) -> TestResult<()> {
    for _ in 0..100 {
        let row = db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT status FROM owner_operation_receipts WHERE tenant_id = $1 AND owner_slug = 'forum' AND idempotency_key = $2 AND operation = $3",
                vec![
                    tenant_id.into(),
                    decision_id.to_string().into(),
                    APPLY_OPERATION.to_string().into(),
                ],
            ))
            .await?;
        if let Some(row) = row {
            let status: String = row.try_get("", "status")?;
            if status == "processing" {
                return Ok(());
            }
            return Err(test_error(format!(
                "Forum terminal moderation receipt reached unexpected `{status}` state before edit commit"
            )));
        }
        sleep(Duration::from_millis(10)).await;
    }
    Err(test_error(
        "Forum terminal moderation call did not cross owner receipt admission while edit stayed open",
    ))
}

async fn assert_application_waits_while_edit_owns_revision(
    application: &mut tokio::task::JoinHandle<
        Result<rustok_moderation_api::ModerationDecisionApplication, PortError>,
    >,
) -> TestResult<()> {
    if let Ok(result) = timeout(Duration::from_millis(50), application).await {
        let completed = result?;
        return Err(test_error(format!(
            "terminal moderation application completed while concurrent edit still held revision lock: {completed:?}"
        )));
    }
    Ok(())
}

fn assert_overlap_error_is_fail_closed(error: &PortError) -> TestResult<()> {
    match error.code.as_str() {
        REVISION_CONFLICT => assert_revision_conflict(error),
        DATABASE_UNAVAILABLE => {
            if error.kind != PortErrorKind::Unavailable || !error.retryable {
                return Err(test_error(format!(
                    "serialization/storage overlap must remain retryable unavailable, got {error}"
                )));
            }
            Ok(())
        }
        other => Err(test_error(format!(
            "terminal moderation overlap returned unexpected error code `{other}`: {error}"
        ))),
    }
}

fn assert_revision_conflict(error: &PortError) -> TestResult<()> {
    if error.kind != PortErrorKind::Conflict || error.retryable || error.code != REVISION_CONFLICT {
        return Err(test_error(format!(
            "expected non-retryable Forum moderation revision conflict, got {error}"
        )));
    }
    Ok(())
}

fn reply_adapter(db: DatabaseConnection) -> TestResult<Arc<dyn ModerationSubjectCommandPort>> {
    Ok(ForumModerationSubjectAdapterFactory::reply().build(&HostRuntimeContext::new(db))?)
}

fn reject_command(
    seed: ReplyRaceSeed,
    revision: i64,
    decision_id: Uuid,
) -> TestResult<ApplyModerationDecisionCommand> {
    command(
        seed,
        revision,
        decision_id,
        ModerationDecisionKind::RejectPublication,
        ModerationDecisionEffectAction::RejectPublication,
        'd',
    )
}

fn removed_command(
    seed: ReplyRaceSeed,
    revision: i64,
    decision_id: Uuid,
) -> TestResult<ApplyModerationDecisionCommand> {
    command(
        seed,
        revision,
        decision_id,
        ModerationDecisionKind::Remove,
        ModerationDecisionEffectAction::SetVisibility {
            state: ModerationVisibilityState::Removed,
        },
        'e',
    )
}

fn command(
    seed: ReplyRaceSeed,
    revision: i64,
    decision_id: Uuid,
    decision_kind: ModerationDecisionKind,
    action: ModerationDecisionEffectAction,
    hash_char: char,
) -> TestResult<ApplyModerationDecisionCommand> {
    Ok(ApplyModerationDecisionCommand {
        decision_id,
        subject: ModerationSubjectRef {
            module: "forum".to_string(),
            kind: ModerationSubjectKind::ForumPost,
            id: seed.reply_id,
            revision,
        },
        decision_kind,
        reason_code: ModerationReasonCode::Other,
        effect: ModerationDecisionEffect::v1(action)?,
        decision_hash: std::iter::repeat_n(hash_char, 64).collect(),
    })
}

fn application_context(tenant_id: Uuid, decision_id: Uuid, correlation: &str) -> PortContext {
    let decision = decision_id.to_string();
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service(MODERATION_ACTOR),
        "und",
        correlation,
    )
    .with_causation_id(decision.clone())
    .with_idempotency_key(decision)
    .with_deadline(Duration::from_secs(5))
}

async fn seed_approved_reply(
    db: &DatabaseConnection,
    label: &str,
    accepted_solution: bool,
) -> TestResult<ReplyRaceSeed> {
    let seed = ReplyRaceSeed {
        tenant_id: Uuid::new_v4(),
        category_id: Uuid::new_v4(),
        topic_id: Uuid::new_v4(),
        reply_id: Uuid::new_v4(),
        body_id: Uuid::new_v4(),
        author_id: Uuid::new_v4(),
    };
    let solution_count = if accepted_solution { 1 } else { 0 };
    execute(
        db,
        format!(
            r#"
INSERT INTO forum_categories
    (id, tenant_id, position, moderated, topic_count, reply_count)
VALUES
    ('{}', '{}', 0, FALSE, 1, 1);

INSERT INTO forum_topics
    (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
VALUES
    ('{}', '{}', '{}', 'open', '{{"fixture":"{}"}}', FALSE, FALSE, 1);

INSERT INTO forum_replies
    (id, tenant_id, topic_id, author_id, status, position)
VALUES
    ('{}', '{}', '{}', '{}', 'approved', 1);

INSERT INTO forum_reply_bodies
    (id, tenant_id, reply_id, locale, body)
VALUES
    ('{}', '{}', '{}', 'en', 'Reviewed terminal moderation body');

INSERT INTO forum_user_stats
    (tenant_id, user_id, topic_count, reply_count, solution_count)
VALUES
    ('{}', '{}', 0, 1, {});
"#,
            seed.category_id,
            seed.tenant_id,
            seed.topic_id,
            seed.tenant_id,
            seed.category_id,
            label,
            seed.reply_id,
            seed.tenant_id,
            seed.topic_id,
            seed.author_id,
            seed.body_id,
            seed.tenant_id,
            seed.reply_id,
            seed.tenant_id,
            seed.author_id,
            solution_count,
        ),
    )
    .await?;

    if accepted_solution {
        execute(
            db,
            format!(
                r#"
INSERT INTO forum_solutions
    (tenant_id, topic_id, reply_id, marked_by_user_id)
VALUES
    ('{}', '{}', '{}', '{}')
"#,
                seed.tenant_id,
                seed.topic_id,
                seed.reply_id,
                Uuid::new_v4(),
            ),
        )
        .await?;
    }
    Ok(seed)
}

async fn assert_unchanged_public_reply(
    db: &DatabaseConnection,
    seed: ReplyRaceSeed,
    edited_body: &str,
    expected_revision: i64,
    accepted_solution: bool,
) -> TestResult<()> {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                r#"
SELECT
    reply.status::text AS status,
    reply.deleted_at IS NOT NULL AS is_deleted,
    body.body,
    topic.reply_count::bigint AS topic_reply_count,
    category.reply_count::bigint AS category_reply_count,
    stats.reply_count::bigint AS user_reply_count,
    stats.solution_count::bigint AS user_solution_count,
    (SELECT COUNT(*)::bigint FROM forum_solutions solution
      WHERE solution.tenant_id = reply.tenant_id
        AND solution.reply_id = reply.id) AS solution_rows,
    (SELECT revision::bigint FROM forum_reply_moderation_subject_revisions revision
      WHERE revision.tenant_id = reply.tenant_id
        AND revision.reply_id = reply.id) AS moderation_revision,
    (SELECT COUNT(*)::bigint FROM sys_events event
      WHERE event.event_type = 'forum.reply.status_changed'
        AND event.payload->>'tenant_id' = '{}') AS status_events
FROM forum_replies reply
JOIN forum_reply_bodies body
  ON body.tenant_id = reply.tenant_id
 AND body.reply_id = reply.id
JOIN forum_topics topic
  ON topic.tenant_id = reply.tenant_id
 AND topic.id = reply.topic_id
JOIN forum_categories category
  ON category.tenant_id = topic.tenant_id
 AND category.id = topic.category_id
JOIN forum_user_stats stats
  ON stats.tenant_id = reply.tenant_id
 AND stats.user_id = reply.author_id
WHERE reply.tenant_id = '{}' AND reply.id = '{}'
"#,
                seed.tenant_id, seed.tenant_id, seed.reply_id
            ),
        ))
        .await?
        .ok_or_else(|| test_error("Forum terminal concurrency fixture disappeared"))?;

    let expected_solution_count: i64 = if accepted_solution { 1 } else { 0 };
    assert_eq!(row.try_get::<String>("", "status")?, "approved");
    assert!(!row.try_get::<bool>("", "is_deleted")?);
    assert_eq!(row.try_get::<String>("", "body")?, edited_body);
    assert_eq!(row.try_get::<i64>("", "topic_reply_count")?, 1);
    assert_eq!(row.try_get::<i64>("", "category_reply_count")?, 1);
    assert_eq!(row.try_get::<i64>("", "user_reply_count")?, 1);
    assert_eq!(
        row.try_get::<i64>("", "user_solution_count")?,
        expected_solution_count
    );
    assert_eq!(
        row.try_get::<i64>("", "solution_rows")?,
        expected_solution_count
    );
    assert_eq!(
        row.try_get::<i64>("", "moderation_revision")?,
        expected_revision
    );
    assert_eq!(row.try_get::<i64>("", "status_events")?, 0);
    Ok(())
}

async fn reply_revision(db: &DatabaseConnection, seed: ReplyRaceSeed) -> TestResult<i64> {
    scalar_i64_on(
        db,
        format!(
            "SELECT revision::bigint AS value FROM forum_reply_moderation_subject_revisions WHERE tenant_id = '{}' AND reply_id = '{}'",
            seed.tenant_id, seed.reply_id
        ),
    )
    .await
}

async fn reply_revision_in(db: &DatabaseTransaction, seed: ReplyRaceSeed) -> TestResult<i64> {
    scalar_i64_on(
        db,
        format!(
            "SELECT revision::bigint AS value FROM forum_reply_moderation_subject_revisions WHERE tenant_id = '{}' AND reply_id = '{}'",
            seed.tenant_id, seed.reply_id
        ),
    )
    .await
}

async fn scalar_i64_on<C>(db: &C, sql: String) -> TestResult<i64>
where
    C: ConnectionTrait,
{
    let row = db
        .query_one(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await?
        .ok_or_else(|| test_error("scalar PostgreSQL query returned no row"))?;
    Ok(row.try_get("", "value")?)
}
