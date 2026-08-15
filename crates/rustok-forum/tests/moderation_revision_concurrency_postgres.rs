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
struct RaceSeed {
    tenant_id: Uuid,
    category_id: Uuid,
    topic_id: Uuid,
    topic_translation_id: Uuid,
    reply_id: Uuid,
    reply_body_id: Uuid,
}

#[tokio::test]
async fn postgres_concurrent_content_edits_fence_topic_lock_and_reply_hide() -> TestResult<()> {
    let Some(database) = PostgresForumTestDb::setup("moderation_revision_concurrency").await?
    else {
        return Ok(());
    };

    let outcome = async {
        topic_translation_edit_fences_permanent_lock(&database).await?;
        reply_body_edit_fences_hide_application(&database).await?;
        Ok(())
    }
    .await;

    database.cleanup().await?;
    outcome
}

async fn topic_translation_edit_fences_permanent_lock(
    database: &PostgresForumTestDb,
) -> TestResult<()> {
    let seed = seed_public_subjects(&database.db).await?;
    let reviewed_revision = topic_revision(&database.db, seed).await?;

    let edit_db = database.peer().await?;
    let edit = edit_db.begin().await?;
    edit.execute(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "UPDATE forum_topic_translations SET title = $1 WHERE tenant_id = $2 AND id = $3",
        vec![
            "Concurrent topic edit".to_string().into(),
            seed.tenant_id.into(),
            seed.topic_translation_id.into(),
        ],
    ))
    .await?;
    assert_eq!(topic_revision_in(&edit, seed).await?, reviewed_revision + 1);

    let adapter = topic_adapter(database.peer().await?)?;
    let command = topic_lock_command(seed, reviewed_revision, Uuid::new_v4())?;
    let decision_id = command.decision_id;
    let mut application = spawn_application(adapter.clone(), seed.tenant_id, command);

    wait_for_processing_receipt(&database.db, seed.tenant_id, decision_id).await?;
    assert_application_waits_while_edit_owns_revision(&mut application, "topic").await?;
    edit.commit().await?;

    let first_error = application.await?.expect_err(
        "overlapping topic edit must prevent lock application against the old revision",
    );
    assert_overlap_error_is_fail_closed(&first_error)?;

    assert_eq!(
        topic_revision(&database.db, seed).await?,
        reviewed_revision + 1
    );
    assert!(!topic_locked(&database.db, seed).await?);
    assert_eq!(
        topic_title(&database.db, seed).await?,
        "Concurrent topic edit"
    );

    // A new delivery against the same stale reviewed revision must deterministically resolve to
    // the semantic revision conflict once the concurrent edit has committed. A fresh decision UUID
    // avoids confusing the subject fence with the first call's owner-receipt state after a possible
    // PostgreSQL serialization retry.
    let stale = topic_lock_command(seed, reviewed_revision, Uuid::new_v4())?;
    let stale_error = adapter
        .apply_moderation_decision(
            application_context(seed.tenant_id, stale.decision_id, "topic-stale-retry"),
            stale,
        )
        .await
        .expect_err("stale topic decision must not lock edited content");
    assert_revision_conflict(&stale_error)?;
    assert!(!topic_locked(&database.db, seed).await?);
    assert_eq!(
        topic_revision(&database.db, seed).await?,
        reviewed_revision + 1
    );
    Ok(())
}

async fn reply_body_edit_fences_hide_application(database: &PostgresForumTestDb) -> TestResult<()> {
    let seed = seed_public_subjects(&database.db).await?;
    let reviewed_revision = reply_revision(&database.db, seed).await?;

    let edit_db = database.peer().await?;
    let edit = edit_db.begin().await?;
    edit.execute(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "UPDATE forum_reply_bodies SET body = $1 WHERE tenant_id = $2 AND id = $3",
        vec![
            "Concurrent reply edit".to_string().into(),
            seed.tenant_id.into(),
            seed.reply_body_id.into(),
        ],
    ))
    .await?;
    assert_eq!(reply_revision_in(&edit, seed).await?, reviewed_revision + 1);

    let adapter = reply_adapter(database.peer().await?)?;
    let command = reply_hide_command(seed, reviewed_revision, Uuid::new_v4())?;
    let decision_id = command.decision_id;
    let mut application = spawn_application(adapter.clone(), seed.tenant_id, command);

    wait_for_processing_receipt(&database.db, seed.tenant_id, decision_id).await?;
    assert_application_waits_while_edit_owns_revision(&mut application, "reply").await?;
    edit.commit().await?;

    let first_error = application.await?.expect_err(
        "overlapping reply edit must prevent hide application against the old revision",
    );
    assert_overlap_error_is_fail_closed(&first_error)?;

    assert_eq!(
        reply_revision(&database.db, seed).await?,
        reviewed_revision + 1
    );
    assert_eq!(reply_status(&database.db, seed).await?, "approved");
    assert_eq!(
        reply_body(&database.db, seed).await?,
        "Concurrent reply edit"
    );
    assert_public_reply_accounting(&database.db, seed).await?;
    assert_eq!(count_reply_status_events(&database.db, seed).await?, 0);

    let stale = reply_hide_command(seed, reviewed_revision, Uuid::new_v4())?;
    let stale_error = adapter
        .apply_moderation_decision(
            application_context(seed.tenant_id, stale.decision_id, "reply-stale-retry"),
            stale,
        )
        .await
        .expect_err("stale reply decision must not hide edited content");
    assert_revision_conflict(&stale_error)?;
    assert_eq!(reply_status(&database.db, seed).await?, "approved");
    assert_eq!(
        reply_revision(&database.db, seed).await?,
        reviewed_revision + 1
    );
    assert_public_reply_accounting(&database.db, seed).await?;
    assert_eq!(count_reply_status_events(&database.db, seed).await?, 0);
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
                application_context(tenant_id, command.decision_id, "overlapping-application"),
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
                "Forum moderation receipt reached unexpected `{status}` state before concurrent edit commit"
            )));
        }
        sleep(Duration::from_millis(10)).await;
    }
    Err(test_error(
        "Forum moderation application did not reach the owner receipt boundary while edit transaction was open",
    ))
}

async fn assert_application_waits_while_edit_owns_revision(
    application: &mut tokio::task::JoinHandle<
        Result<rustok_moderation_api::ModerationDecisionApplication, PortError>,
    >,
    label: &str,
) -> TestResult<()> {
    if let Ok(result) = timeout(Duration::from_millis(50), application).await {
        let completed = result?;
        return Err(test_error(format!(
            "{label} moderation application completed while the concurrent edit still held the subject revision lock: {completed:?}"
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
            "concurrent Forum moderation application returned unexpected error code `{other}`: {error}"
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

fn topic_adapter(db: DatabaseConnection) -> TestResult<Arc<dyn ModerationSubjectCommandPort>> {
    Ok(ForumModerationSubjectAdapterFactory::topic().build(&HostRuntimeContext::new(db))?)
}

fn reply_adapter(db: DatabaseConnection) -> TestResult<Arc<dyn ModerationSubjectCommandPort>> {
    Ok(ForumModerationSubjectAdapterFactory::reply().build(&HostRuntimeContext::new(db))?)
}

fn topic_lock_command(
    seed: RaceSeed,
    reviewed_revision: i64,
    decision_id: Uuid,
) -> TestResult<ApplyModerationDecisionCommand> {
    Ok(ApplyModerationDecisionCommand {
        decision_id,
        subject: ModerationSubjectRef {
            module: "forum".to_string(),
            kind: ModerationSubjectKind::ForumTopic,
            id: seed.topic_id,
            revision: reviewed_revision,
        },
        decision_kind: ModerationDecisionKind::Lock,
        reason_code: ModerationReasonCode::Other,
        effect: ModerationDecisionEffect::v1(ModerationDecisionEffectAction::Lock {
            effective_until: None,
        })?,
        decision_hash: "a".repeat(64),
    })
}

fn reply_hide_command(
    seed: RaceSeed,
    reviewed_revision: i64,
    decision_id: Uuid,
) -> TestResult<ApplyModerationDecisionCommand> {
    Ok(ApplyModerationDecisionCommand {
        decision_id,
        subject: ModerationSubjectRef {
            module: "forum".to_string(),
            kind: ModerationSubjectKind::ForumPost,
            id: seed.reply_id,
            revision: reviewed_revision,
        },
        decision_kind: ModerationDecisionKind::Hide,
        reason_code: ModerationReasonCode::Other,
        effect: ModerationDecisionEffect::v1(ModerationDecisionEffectAction::SetVisibility {
            state: ModerationVisibilityState::Hidden,
        })?,
        decision_hash: "b".repeat(64),
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

async fn seed_public_subjects(db: &DatabaseConnection) -> TestResult<RaceSeed> {
    let seed = RaceSeed {
        tenant_id: Uuid::new_v4(),
        category_id: Uuid::new_v4(),
        topic_id: Uuid::new_v4(),
        topic_translation_id: Uuid::new_v4(),
        reply_id: Uuid::new_v4(),
        reply_body_id: Uuid::new_v4(),
    };
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
                ('{}', '{}', '{}', 'open', '{{}}', FALSE, FALSE, 1);
            INSERT INTO forum_topic_translations
                (id, tenant_id, topic_id, locale, title, slug, body)
            VALUES
                ('{}', '{}', '{}', 'en', 'Reviewed topic', 'reviewed-topic', 'Reviewed topic body');
            INSERT INTO forum_replies
                (id, tenant_id, topic_id, status, position)
            VALUES
                ('{}', '{}', '{}', 'approved', 1);
            INSERT INTO forum_reply_bodies
                (id, tenant_id, reply_id, locale, body)
            VALUES
                ('{}', '{}', '{}', 'en', 'Reviewed reply body');
            "#,
            seed.category_id,
            seed.tenant_id,
            seed.topic_id,
            seed.tenant_id,
            seed.category_id,
            seed.topic_translation_id,
            seed.tenant_id,
            seed.topic_id,
            seed.reply_id,
            seed.tenant_id,
            seed.topic_id,
            seed.reply_body_id,
            seed.tenant_id,
            seed.reply_id,
        ),
    )
    .await?;
    Ok(seed)
}

async fn topic_revision(db: &DatabaseConnection, seed: RaceSeed) -> TestResult<i64> {
    scalar_i64(
        db,
        format!(
            "SELECT revision::bigint AS value FROM forum_topic_moderation_subject_revisions WHERE tenant_id = '{}' AND topic_id = '{}'",
            seed.tenant_id, seed.topic_id
        ),
    )
    .await
}

async fn topic_revision_in(db: &DatabaseTransaction, seed: RaceSeed) -> TestResult<i64> {
    scalar_i64_on(
        db,
        format!(
            "SELECT revision::bigint AS value FROM forum_topic_moderation_subject_revisions WHERE tenant_id = '{}' AND topic_id = '{}'",
            seed.tenant_id, seed.topic_id
        ),
    )
    .await
}

async fn reply_revision(db: &DatabaseConnection, seed: RaceSeed) -> TestResult<i64> {
    scalar_i64(
        db,
        format!(
            "SELECT revision::bigint AS value FROM forum_reply_moderation_subject_revisions WHERE tenant_id = '{}' AND reply_id = '{}'",
            seed.tenant_id, seed.reply_id
        ),
    )
    .await
}

async fn reply_revision_in(db: &DatabaseTransaction, seed: RaceSeed) -> TestResult<i64> {
    scalar_i64_on(
        db,
        format!(
            "SELECT revision::bigint AS value FROM forum_reply_moderation_subject_revisions WHERE tenant_id = '{}' AND reply_id = '{}'",
            seed.tenant_id, seed.reply_id
        ),
    )
    .await
}

async fn topic_locked(db: &DatabaseConnection, seed: RaceSeed) -> TestResult<bool> {
    scalar_bool(
        db,
        format!(
            "SELECT is_locked AS value FROM forum_topics WHERE tenant_id = '{}' AND id = '{}'",
            seed.tenant_id, seed.topic_id
        ),
    )
    .await
}

async fn topic_title(db: &DatabaseConnection, seed: RaceSeed) -> TestResult<String> {
    scalar_string(
        db,
        format!(
            "SELECT title AS value FROM forum_topic_translations WHERE tenant_id = '{}' AND id = '{}'",
            seed.tenant_id, seed.topic_translation_id
        ),
    )
    .await
}

async fn reply_status(db: &DatabaseConnection, seed: RaceSeed) -> TestResult<String> {
    scalar_string(
        db,
        format!(
            "SELECT status AS value FROM forum_replies WHERE tenant_id = '{}' AND id = '{}'",
            seed.tenant_id, seed.reply_id
        ),
    )
    .await
}

async fn reply_body(db: &DatabaseConnection, seed: RaceSeed) -> TestResult<String> {
    scalar_string(
        db,
        format!(
            "SELECT body AS value FROM forum_reply_bodies WHERE tenant_id = '{}' AND id = '{}'",
            seed.tenant_id, seed.reply_body_id
        ),
    )
    .await
}

async fn assert_public_reply_accounting(db: &DatabaseConnection, seed: RaceSeed) -> TestResult<()> {
    assert_eq!(
        scalar_i64(
            db,
            format!(
                "SELECT reply_count::bigint AS value FROM forum_topics WHERE tenant_id = '{}' AND id = '{}'",
                seed.tenant_id, seed.topic_id
            ),
        )
        .await?,
        1
    );
    assert_eq!(
        scalar_i64(
            db,
            format!(
                "SELECT reply_count::bigint AS value FROM forum_categories WHERE tenant_id = '{}' AND id = '{}'",
                seed.tenant_id, seed.category_id
            ),
        )
        .await?,
        1
    );
    Ok(())
}

async fn count_reply_status_events(db: &DatabaseConnection, seed: RaceSeed) -> TestResult<i64> {
    scalar_i64(
        db,
        format!(
            "SELECT COUNT(*)::bigint AS value FROM sys_events WHERE event_type = 'forum.reply.status_changed' AND payload->>'tenant_id' = '{}'",
            seed.tenant_id
        ),
    )
    .await
}

async fn scalar_i64(db: &DatabaseConnection, sql: String) -> TestResult<i64> {
    scalar_i64_on(db, sql).await
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

async fn scalar_bool(db: &DatabaseConnection, sql: String) -> TestResult<bool> {
    let row = db
        .query_one(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await?
        .ok_or_else(|| test_error("boolean PostgreSQL query returned no row"))?;
    Ok(row.try_get("", "value")?)
}

async fn scalar_string(db: &DatabaseConnection, sql: String) -> TestResult<String> {
    let row = db
        .query_one(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await?
        .ok_or_else(|| test_error("string PostgreSQL query returned no row"))?;
    Ok(row.try_get("", "value")?)
}
