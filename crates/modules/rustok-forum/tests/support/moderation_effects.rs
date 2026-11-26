//! Trusted adapter behavior matrix shared by SQLite and PostgreSQL owner tests.
use std::{sync::Arc, time::Duration};

use rustok_api::{HostRuntimeContext, PortActor, PortContext};
use rustok_core::{SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, ForumModerationSubjectAdapterFactory, ModerationService, ReplyService,
    TopicService,
    entities::{forum_category, forum_reply, forum_solution, forum_topic, forum_user_stat},
};
use rustok_moderation::{
    ApplyModerationDecisionCommand, ModerationDecisionEffect, ModerationDecisionEffectAction,
    ModerationDecisionKind, ModerationReasonCode, ModerationSubjectAdapterFactory,
    ModerationSubjectKind, ModerationSubjectRef, ModerationVisibilityState,
};
use rustok_outbox::{OutboxTransport, TransactionalEventBus, entity as outbox};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, Statement,
};
use uuid::Uuid;

use super::{
    make_category, make_topic, posting_cooldown, reply_input, reply_restore_checkpoint, setup,
    support,
};

#[tokio::test]
async fn trusted_moderation_effects_are_atomic_on_sqlite() {
    let (db, events, _) = setup().await;
    exercise(&db, events).await;
}

#[tokio::test]
async fn trusted_moderation_effects_are_atomic_on_postgres() -> support::TestResult<()> {
    let Some(context) = support::postgres::PostgresForumTestDb::setup("trusted_effects").await?
    else {
        return Ok(());
    };
    let events = TransactionalEventBus::new(Arc::new(OutboxTransport::new(context.db.clone())));
    exercise(&context.db, events).await;
    context.cleanup().await?;
    Ok(())
}

fn context(tenant: Uuid, decision: Uuid) -> PortContext {
    PortContext::new(
        tenant.to_string(),
        PortActor::service("rustok-moderation"),
        "en",
        "trusted-effects-test",
    )
    .with_idempotency_key(decision.to_string())
    .with_causation_id(decision.to_string())
    .with_deadline(Duration::from_secs(10))
}

async fn revision(db: &DatabaseConnection, tenant: Uuid, reply: Uuid) -> i64 {
    db.query_one_raw(Statement::from_sql_and_values(db.get_database_backend(),
        "SELECT revision FROM forum_reply_moderation_subject_revisions WHERE tenant_id = $1 AND reply_id = $2",
        vec![tenant.into(), reply.into()],
    )).await.unwrap().unwrap().try_get("", "revision").unwrap()
}

async fn event_count(db: &DatabaseConnection, tenant: Uuid, kind: &str) -> u64 {
    outbox::Entity::find()
        .filter(outbox::Column::TenantId.eq(tenant))
        .filter(outbox::Column::EventType.eq(kind))
        .count(db)
        .await
        .unwrap()
}

async fn projection_count(db: &DatabaseConnection, tenant: Uuid, target: Uuid) -> i64 {
    if db.get_database_backend() != DatabaseBackend::Postgres {
        return 0;
    }
    db.query_one_raw(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "SELECT COUNT(*) AS count FROM forum_projection_revision_ledger WHERE tenant_id = $1 AND target_id = $2",
        vec![tenant.into(), target.into()],
    )).await.unwrap().unwrap().try_get("", "count").unwrap()
}

async fn receipt_status(db: &DatabaseConnection, tenant: Uuid, decision: Uuid) -> String {
    db.query_one_raw(Statement::from_sql_and_values(db.get_database_backend(),
        "SELECT status FROM owner_operation_receipts WHERE tenant_id = $1 AND owner_slug = 'forum' AND idempotency_key = $2 AND operation = 'apply_moderation_decision'",
        vec![tenant.into(), decision.to_string().into()],
    )).await.unwrap().unwrap().try_get("", "status").unwrap()
}

async fn fail_status_publication(db: &DatabaseConnection, install: bool) {
    let sql = match (db.get_database_backend(), install) {
        (DatabaseBackend::Postgres, true) => {
            r#"
            CREATE FUNCTION forum_test_fail_status() RETURNS trigger AS $$
            BEGIN
                IF NEW.event_type = 'forum.reply.status_changed' THEN
                    RAISE EXCEPTION 'injected forum status publication failure';
                END IF;
                RETURN NEW;
            END; $$ LANGUAGE plpgsql;
            CREATE TRIGGER forum_test_fail_status BEFORE INSERT ON sys_events
            FOR EACH ROW EXECUTE FUNCTION forum_test_fail_status();
        "#
        }
        (DatabaseBackend::Postgres, false) => {
            "DROP TRIGGER forum_test_fail_status ON sys_events; DROP FUNCTION forum_test_fail_status();"
        }
        (DatabaseBackend::Sqlite, true) => {
            r#"
            CREATE TRIGGER forum_test_fail_status BEFORE INSERT ON sys_events
            WHEN NEW.event_type = 'forum.reply.status_changed'
            BEGIN SELECT RAISE(ABORT, 'injected forum status publication failure'); END;
        "#
        }
        (DatabaseBackend::Sqlite, false) => "DROP TRIGGER forum_test_fail_status;",
        _ => panic!("unsupported test backend"),
    };
    db.execute_unprepared(sql).await.unwrap();
}

async fn exercise(db: &DatabaseConnection, events: TransactionalEventBus) {
    for archived in [false, true] {
        for solution in [false, true] {
            for (decision_kind, action, expected) in [
                (
                    ModerationDecisionKind::Hide,
                    ModerationDecisionEffectAction::SetVisibility {
                        state: ModerationVisibilityState::Hidden,
                    },
                    "hidden",
                ),
                (
                    ModerationDecisionKind::RejectPublication,
                    ModerationDecisionEffectAction::RejectPublication,
                    "rejected",
                ),
                (
                    ModerationDecisionKind::Remove,
                    ModerationDecisionEffectAction::SetVisibility {
                        state: ModerationVisibilityState::Removed,
                    },
                    "deleted",
                ),
            ] {
                let tenant = Uuid::new_v4();
                let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
                let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
                let providers = posting_cooldown::zero_cooldown_providers(); // Author deletion remains false.
                let topics = TopicService::new(db.clone(), events.clone())
                    .with_settings_providers(providers.clone());
                let replies = ReplyService::new(db.clone(), events.clone())
                    .with_settings_providers(providers);
                let moderation = ModerationService::new(db.clone(), events.clone());
                let category = make_category(
                    &CategoryService::new(db.clone()),
                    tenant,
                    &admin,
                    "trusted-effect",
                    false,
                )
                .await;
                let topic = make_topic(&topics, tenant, &admin, category, "trusted-effect").await;
                let reply = replies
                    .create(
                        tenant,
                        author.clone(),
                        topic,
                        reply_input("trusted original body", None),
                    )
                    .await
                    .unwrap();
                if solution {
                    moderation
                        .mark_solution(tenant, topic, reply.id, admin.clone())
                        .await
                        .unwrap();
                }
                if archived {
                    moderation
                        .archive_topic(tenant, topic, admin)
                        .await
                        .unwrap();
                }
                let reviewed = revision(db, tenant, reply.id).await;
                let adapter = ForumModerationSubjectAdapterFactory::reply()
                    .build(&HostRuntimeContext::new(db.clone()))
                    .unwrap();
                let mut command = ApplyModerationDecisionCommand {
                    decision_id: Uuid::new_v4(),
                    subject: ModerationSubjectRef {
                        module: "forum".into(),
                        kind: ModerationSubjectKind::ForumPost,
                        id: reply.id,
                        revision: reviewed,
                    },
                    decision_kind,
                    reason_code: ModerationReasonCode::Other,
                    effect: ModerationDecisionEffect::v1(action).unwrap(),
                    decision_hash: "a".repeat(64),
                };
                if !archived && solution && expected == "hidden" {
                    // Failure happens after canonical status/solution/counter changes.
                    // No partial domain write or outbox event may survive rollback.
                    let before = reply_restore_checkpoint(db, tenant, topic, reply.id).await;
                    fail_status_publication(db, true).await;
                    let failed = adapter
                        .apply_moderation_decision(
                            context(tenant, command.decision_id),
                            command.clone(),
                        )
                        .await;
                    fail_status_publication(db, false).await;
                    assert!(
                        failed.is_err(),
                        "publication failure must fail the entire application"
                    );
                    assert_eq!(
                        reply_restore_checkpoint(db, tenant, topic, reply.id).await,
                        before
                    );
                    assert_eq!(revision(db, tenant, reply.id).await, reviewed);
                    assert_ne!(
                        receipt_status(db, tenant, command.decision_id).await,
                        "completed"
                    );
                    command.decision_id = Uuid::new_v4();
                }
                let source_before = reply_restore_checkpoint(db, tenant, topic, reply.id).await;
                let status_before = event_count(db, tenant, "forum.reply.status_changed").await;
                let topic_projection = projection_count(db, tenant, topic).await;
                let category_projection = projection_count(db, tenant, category).await;
                let first = adapter
                    .apply_moderation_decision(
                        context(tenant, command.decision_id),
                        command.clone(),
                    )
                    .await
                    .unwrap();
                assert!(first.applied_revision > reviewed);
                assert_eq!(
                    receipt_status(db, tenant, command.decision_id).await,
                    "completed"
                );
                assert_eq!(
                    forum_reply::Entity::find_by_id(reply.id)
                        .one(db)
                        .await
                        .unwrap()
                        .unwrap()
                        .status
                        .to_string(),
                    expected
                );
                assert!(
                    forum_solution::Entity::find_by_id((topic, tenant))
                        .one(db)
                        .await
                        .unwrap()
                        .is_none(),
                    "trusted hide/reject/remove must remove an accepted solution"
                );
                let topic_row = forum_topic::Entity::find_by_id(topic)
                    .one(db)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(topic_row.reply_count, 0);
                assert!(topic_row.last_reply_at.is_none());
                let category_row = forum_category::Entity::find_by_id(category)
                    .one(db)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    category_row.reply_count, 0,
                    "archived withdrawals must not double-release aggregates"
                );
                let stats = forum_user_stat::Entity::find_by_id((tenant, author.user_id.unwrap()))
                    .one(db)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!((stats.reply_count, stats.solution_count), (0, 0));
                assert_eq!(
                    event_count(db, tenant, "forum.reply.status_changed").await,
                    status_before + 1
                );
                let projected = i64::from(db.get_database_backend() == DatabaseBackend::Postgres);
                assert_eq!(
                    projection_count(db, tenant, topic).await,
                    topic_projection + projected
                );
                assert_eq!(
                    projection_count(db, tenant, category).await,
                    category_projection + projected
                );

                let checkpoint = reply_restore_checkpoint(db, tenant, topic, reply.id).await;
                assert_eq!(checkpoint["bodies"], source_before["bodies"]);
                if expected == "deleted" {
                    let old_revisions = source_before["revisions"].as_array().unwrap();
                    let new_revisions = checkpoint["revisions"].as_array().unwrap();
                    let bodies = source_before["bodies"].as_array().unwrap();
                    assert_eq!(new_revisions.len(), old_revisions.len() + bodies.len());
                    assert_eq!(
                        &new_revisions[..old_revisions.len()],
                        old_revisions.as_slice()
                    );
                    assert!(
                        new_revisions[old_revisions.len()..]
                            .iter()
                            .all(|row| row["revision_reason"] == "delete")
                    );
                    let snapshot = db.query_one_raw(Statement::from_sql_and_values(
                        db.get_database_backend(),
                        "SELECT reason FROM forum_reply_delete_snapshots WHERE tenant_id = $1 AND reply_id = $2",
                        vec![tenant.into(), reply.id.into()],
                    )).await.unwrap().unwrap();
                    assert_eq!(
                        snapshot.try_get::<String>("", "reason").unwrap(),
                        "moderation"
                    );
                    assert_eq!(checkpoint["snapshot"]["previous_status"], "approved");
                    assert_eq!(checkpoint["snapshot"]["marked_at"].is_string(), solution);
                } else {
                    assert_eq!(checkpoint["revisions"], source_before["revisions"]);
                    assert!(checkpoint["snapshot"].is_null());
                }
                let replay = adapter
                    .apply_moderation_decision(
                        context(tenant, command.decision_id),
                        command.clone(),
                    )
                    .await
                    .unwrap();
                assert_eq!(
                    replay, first,
                    "an exact decision retry returns its immutable receipt"
                );
                assert_eq!(
                    reply_restore_checkpoint(db, tenant, topic, reply.id).await,
                    checkpoint
                );
                assert_eq!(revision(db, tenant, reply.id).await, first.applied_revision);
                if expected != "deleted" {
                    command.decision_id = Uuid::new_v4();
                    command.subject.revision = first.applied_revision;
                    let no_op = adapter
                        .apply_moderation_decision(
                            context(tenant, command.decision_id),
                            command.clone(),
                        )
                        .await
                        .unwrap();
                    assert_eq!(no_op.applied_revision, first.applied_revision);
                    assert_eq!(
                        reply_restore_checkpoint(db, tenant, topic, reply.id).await,
                        checkpoint,
                        "same-target decisions append no event or aggregate change"
                    );
                    assert_eq!(
                        receipt_status(db, tenant, command.decision_id).await,
                        "completed"
                    );
                }
                assert_eq!(
                    projection_count(db, tenant, topic).await,
                    topic_projection + projected
                );
                assert_eq!(
                    projection_count(db, tenant, category).await,
                    category_projection + projected
                );
            }
        }
    }
}
