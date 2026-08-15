mod support;

use std::{sync::Arc, time::Duration};

use rustok_api::{HostRuntimeContext, PortActor, PortContext, PortErrorKind};
use rustok_forum::ForumModerationSubjectAdapterFactory;
use rustok_moderation_api::{
    ApplyModerationDecisionCommand, ModerationDecisionEffect, ModerationDecisionEffectAction,
    ModerationDecisionKind, ModerationReasonCode, ModerationSubjectAdapterFactory,
    ModerationSubjectCommandPort, ModerationSubjectKind, ModerationSubjectRef,
    ModerationVisibilityState,
};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};
use uuid::Uuid;

use support::postgres::{PostgresForumTestDb, execute};
use support::{TestResult, test_error};

const MODERATION_ACTOR: &str = "rustok-moderation";
const APPLY_OPERATION: &str = "apply_moderation_decision";
const UNSUPPORTED_EFFECT: &str = "forum.moderation_effect_unsupported";

#[derive(Clone, Copy)]
struct ReplySeed {
    tenant_id: Uuid,
    category_id: Uuid,
    topic_id: Uuid,
    reply_id: Uuid,
    author_id: Uuid,
}

#[tokio::test]
async fn postgres_moderation_effects_preserve_accounting_tombstones_and_unpublished_boundary()
-> TestResult<()> {
    let Some(database) = PostgresForumTestDb::setup("moderation_effect_contract").await? else {
        return Ok(());
    };

    let outcome = async {
        reject_publication_accounts_once_and_replays(&database.db).await?;
        removed_solution_reply_tombstones_once_and_replays(&database.db).await?;
        unpublished_visibility_fails_closed_without_forum_mutation(&database.db).await?;
        Ok(())
    }
    .await;

    database.cleanup().await?;
    outcome
}

async fn reject_publication_accounts_once_and_replays(db: &DatabaseConnection) -> TestResult<()> {
    let seed = seed_approved_reply(db, "reject-publication", false).await?;
    let reviewed_revision = reply_revision(db, seed).await?;
    let adapter = reply_adapter(db.clone())?;
    let decision_id = Uuid::new_v4();
    let command = reject_command(seed, reviewed_revision, decision_id)?;

    let first = adapter
        .apply_moderation_decision(
            application_context(seed.tenant_id, decision_id, "reject-first"),
            command.clone(),
        )
        .await?;
    assert_eq!(first.decision_id, decision_id);
    assert!(first.applied_revision > reviewed_revision);
    assert_rejected_state(db, seed).await?;
    assert_eq!(status_event_count(db, seed.tenant_id).await?, 1);
    assert_receipt(db, seed.tenant_id, decision_id, "completed").await?;

    let revision_after_first = reply_revision(db, seed).await?;
    let replay = adapter
        .apply_moderation_decision(
            application_context(seed.tenant_id, decision_id, "reject-replay"),
            command,
        )
        .await?;
    assert_eq!(replay, first);
    assert_eq!(reply_revision(db, seed).await?, revision_after_first);
    assert_rejected_state(db, seed).await?;
    assert_eq!(status_event_count(db, seed.tenant_id).await?, 1);
    assert_receipt(db, seed.tenant_id, decision_id, "completed").await?;

    let no_op_id = Uuid::new_v4();
    let no_op = reject_command(seed, revision_after_first, no_op_id)?;
    let no_op_application = adapter
        .apply_moderation_decision(
            application_context(seed.tenant_id, no_op_id, "reject-no-op"),
            no_op,
        )
        .await?;
    assert_eq!(no_op_application.applied_revision, revision_after_first);
    assert_eq!(reply_revision(db, seed).await?, revision_after_first);
    assert_rejected_state(db, seed).await?;
    assert_eq!(status_event_count(db, seed.tenant_id).await?, 1);
    assert_receipt(db, seed.tenant_id, no_op_id, "completed").await?;
    Ok(())
}

async fn removed_solution_reply_tombstones_once_and_replays(
    db: &DatabaseConnection,
) -> TestResult<()> {
    let seed = seed_approved_reply(db, "removed-solution", true).await?;
    let reviewed_revision = reply_revision(db, seed).await?;
    let adapter = reply_adapter(db.clone())?;
    let decision_id = Uuid::new_v4();
    let command = removed_command(seed, reviewed_revision, decision_id)?;

    let first = adapter
        .apply_moderation_decision(
            application_context(seed.tenant_id, decision_id, "remove-first"),
            command.clone(),
        )
        .await?;
    assert_eq!(first.decision_id, decision_id);
    assert!(first.applied_revision > reviewed_revision);
    assert_removed_solution_tombstone(db, seed).await?;
    assert_eq!(status_event_count(db, seed.tenant_id).await?, 1);
    assert_receipt(db, seed.tenant_id, decision_id, "completed").await?;

    let revision_after_first = reply_revision(db, seed).await?;
    let replay = adapter
        .apply_moderation_decision(
            application_context(seed.tenant_id, decision_id, "remove-replay"),
            command,
        )
        .await?;
    assert_eq!(replay, first);
    assert_eq!(reply_revision(db, seed).await?, revision_after_first);
    assert_removed_solution_tombstone(db, seed).await?;
    assert_eq!(status_event_count(db, seed.tenant_id).await?, 1);
    assert_receipt(db, seed.tenant_id, decision_id, "completed").await?;
    Ok(())
}

async fn unpublished_visibility_fails_closed_without_forum_mutation(
    db: &DatabaseConnection,
) -> TestResult<()> {
    let seed = seed_approved_reply(db, "unpublished-fail-closed", false).await?;
    let reviewed_revision = reply_revision(db, seed).await?;
    let adapter = reply_adapter(db.clone())?;
    let decision_id = Uuid::new_v4();
    let command = unpublished_command(seed, reviewed_revision, decision_id)?;

    let error = adapter
        .apply_moderation_decision(
            application_context(seed.tenant_id, decision_id, "unpublished"),
            command,
        )
        .await
        .expect_err("Forum must not approximate Unpublished as hidden or rejected");
    if error.kind != PortErrorKind::Validation
        || error.retryable
        || error.code != UNSUPPORTED_EFFECT
    {
        return Err(test_error(format!(
            "expected fail-closed unsupported Forum visibility effect, got {error}"
        )));
    }

    assert_eq!(reply_revision(db, seed).await?, reviewed_revision);
    assert_approved_state(db, seed, false).await?;
    assert_eq!(status_event_count(db, seed.tenant_id).await?, 0);
    assert_receipt(db, seed.tenant_id, decision_id, "failed").await?;
    Ok(())
}

fn reply_adapter(db: DatabaseConnection) -> TestResult<Arc<dyn ModerationSubjectCommandPort>> {
    Ok(ForumModerationSubjectAdapterFactory::reply().build(&HostRuntimeContext::new(db))?)
}

fn reject_command(
    seed: ReplySeed,
    revision: i64,
    decision_id: Uuid,
) -> TestResult<ApplyModerationDecisionCommand> {
    command(
        seed,
        revision,
        decision_id,
        ModerationDecisionKind::RejectPublication,
        ModerationDecisionEffectAction::RejectPublication,
        'a',
    )
}

fn removed_command(
    seed: ReplySeed,
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
        'b',
    )
}

fn unpublished_command(
    seed: ReplySeed,
    revision: i64,
    decision_id: Uuid,
) -> TestResult<ApplyModerationDecisionCommand> {
    command(
        seed,
        revision,
        decision_id,
        ModerationDecisionKind::Unpublish,
        ModerationDecisionEffectAction::SetVisibility {
            state: ModerationVisibilityState::Unpublished,
        },
        'c',
    )
}

fn command(
    seed: ReplySeed,
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
) -> TestResult<ReplySeed> {
    let seed = ReplySeed {
        tenant_id: Uuid::new_v4(),
        category_id: Uuid::new_v4(),
        topic_id: Uuid::new_v4(),
        reply_id: Uuid::new_v4(),
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
    ('{}', '{}', '{}', 'en', 'Moderation effect fixture');

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
            Uuid::new_v4(),
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

    assert_approved_state(db, seed, accepted_solution).await?;
    Ok(seed)
}

async fn assert_approved_state(
    db: &DatabaseConnection,
    seed: ReplySeed,
    accepted_solution: bool,
) -> TestResult<()> {
    let row = reply_state_row(db, seed).await?;
    let expected_solution_count = if accepted_solution { 1 } else { 0 };
    assert_eq!(row.try_get::<String>("", "status")?, "approved");
    assert!(!row.try_get::<bool>("", "is_deleted")?);
    assert_eq!(row.try_get::<i64>("", "topic_reply_count")?, 1);
    assert_eq!(row.try_get::<i64>("", "category_reply_count")?, 1);
    assert_eq!(row.try_get::<i64>("", "user_reply_count")?, 1);
    assert_eq!(
        row.try_get::<i64>("", "solution_rows")?,
        expected_solution_count
    );
    assert_eq!(
        row.try_get::<i64>("", "user_solution_count")?,
        expected_solution_count
    );
    Ok(())
}

async fn assert_rejected_state(db: &DatabaseConnection, seed: ReplySeed) -> TestResult<()> {
    let row = reply_state_row(db, seed).await?;
    assert_eq!(row.try_get::<String>("", "status")?, "rejected");
    assert!(!row.try_get::<bool>("", "is_deleted")?);
    assert_eq!(row.try_get::<i64>("", "topic_reply_count")?, 0);
    assert_eq!(row.try_get::<i64>("", "category_reply_count")?, 0);
    assert_eq!(row.try_get::<i64>("", "user_reply_count")?, 0);
    assert_eq!(row.try_get::<i64>("", "solution_rows")?, 0);
    assert_eq!(row.try_get::<i64>("", "user_solution_count")?, 0);
    Ok(())
}

async fn assert_removed_solution_tombstone(
    db: &DatabaseConnection,
    seed: ReplySeed,
) -> TestResult<()> {
    let row = reply_state_row(db, seed).await?;
    assert_eq!(row.try_get::<String>("", "status")?, "deleted");
    assert!(row.try_get::<bool>("", "is_deleted")?);
    assert_eq!(
        row.try_get::<String>("", "body")?,
        "Moderation effect fixture"
    );
    for field in [
        "topic_reply_count",
        "category_reply_count",
        "user_reply_count",
        "solution_rows",
        "user_solution_count",
    ] {
        assert_eq!(row.try_get::<i64>("", field)?, 0, "field `{field}`");
    }
    Ok(())
}

async fn reply_state_row(
    db: &DatabaseConnection,
    seed: ReplySeed,
) -> TestResult<sea_orm::QueryResult> {
    db.query_one(Statement::from_string(
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
        AND solution.reply_id = reply.id) AS solution_rows
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
            seed.tenant_id, seed.reply_id
        ),
    ))
    .await?
    .ok_or_else(|| test_error("Forum moderation reply fixture disappeared"))
}

async fn reply_revision(db: &DatabaseConnection, seed: ReplySeed) -> TestResult<i64> {
    scalar_i64(
        db,
        format!(
            "SELECT revision::bigint AS value FROM forum_reply_moderation_subject_revisions WHERE tenant_id = '{}' AND reply_id = '{}'",
            seed.tenant_id, seed.reply_id
        ),
    )
    .await
}

async fn status_event_count(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<i64> {
    scalar_i64(
        db,
        format!(
            "SELECT COUNT(*)::bigint AS value FROM sys_events WHERE event_type = 'forum.reply.status_changed' AND payload->>'tenant_id' = '{}'",
            tenant_id
        ),
    )
    .await
}

async fn assert_receipt(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    decision_id: Uuid,
    expected_status: &str,
) -> TestResult<()> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT status, COUNT(*) OVER ()::bigint AS receipt_count FROM owner_operation_receipts WHERE tenant_id = $1 AND owner_slug = 'forum' AND idempotency_key = $2 AND operation = $3",
            vec![
                tenant_id.into(),
                decision_id.to_string().into(),
                APPLY_OPERATION.to_string().into(),
            ],
        ))
        .await?
        .ok_or_else(|| test_error("Forum moderation owner receipt is missing"))?;
    assert_eq!(row.try_get::<String>("", "status")?, expected_status);
    assert_eq!(row.try_get::<i64>("", "receipt_count")?, 1);
    Ok(())
}

async fn scalar_i64(db: &DatabaseConnection, sql: String) -> TestResult<i64> {
    let row = db
        .query_one(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await?
        .ok_or_else(|| test_error("scalar PostgreSQL query returned no row"))?;
    Ok(row.try_get("", "value")?)
}
