#![cfg(all(feature = "mod-forum", feature = "mod-moderation"))]

use std::{env, error::Error, sync::Arc, time::Duration};

use rustok_api::{HostRuntimeContext, PortActor, PortContext, RichTextDocument};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CreateReplyInput, ForumModerationSubjectAdapterFactory, ForumModule, ReplyService,
};
use rustok_moderation::{
    APPLICATION_ADAPTER_DEADLINE_SECONDS, ApplyModerationDecisionCommand,
    AssignModerationCaseCommand, DecideModerationCaseCommand, ModerationApplicationOperationStatus,
    ModerationCasePriority, ModerationCaseRecord, ModerationCaseStatus, ModerationDecisionEffect,
    ModerationDecisionEffectAction, ModerationDecisionKind, ModerationDecisionRecord,
    ModerationReasonCode, ModerationReporterKind, ModerationScopeRef, ModerationService,
    ModerationSubjectAdapterFactory, ModerationSubjectAdapterRegistry, ModerationSubjectKind,
    ModerationSubjectRef, ModerationVisibilityState, OpenModerationCaseCommand,
    SubmitModerationReportCommand,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::SchemaManager;
use serde_json::json;
use uuid::Uuid;

const FORUM_DATABASE_ENV: &str = "RUSTOK_FORUM_TEST_DATABASE_URL";
const MODERATION_DATABASE_ENV: &str = "RUSTOK_MODERATION_TEST_DATABASE_URL";
const DISPATCH_ACTOR: &str = "rustok-moderation";
const APPLY_OPERATION: &str = "apply_moderation_decision";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

struct TestDatabase {
    control: DatabaseConnection,
    database_url: String,
    db: DatabaseConnection,
    schema_name: String,
}

impl TestDatabase {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!(
                "neither {FORUM_DATABASE_ENV} nor {MODERATION_DATABASE_ENV} is set to a PostgreSQL URL; skipping Forum/Moderation lost-response contract"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_forum_moderation_lost_response_{}",
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;
        let db = connect(&database_url).await?;
        set_search_path(&db, &schema_name).await?;

        let manager = SchemaManager::new(&db);
        let migration_result = async {
            db.execute_unprepared(
                r#"
                CREATE TABLE users (
                    id UUID NOT NULL PRIMARY KEY,
                    tenant_id UUID NOT NULL
                )
                "#,
            )
            .await?;
            for migration in OutboxModule.migrations() {
                migration.up(&manager).await?;
            }
            for migration in TaxonomyModule.migrations() {
                migration.up(&manager).await?;
            }
            flex::cache_generation::create_field_definition_cache_generation_table(&manager)
                .await?;
            for migration in ForumModule.migrations() {
                migration.up(&manager).await?;
            }
            for migration in rustok_moderation::ModerationModule.migrations() {
                migration.up(&manager).await?;
            }
            Ok::<(), sea_orm::DbErr>(())
        }
        .await;

        if let Err(error) = migration_result {
            let _ = control
                .execute_unprepared(&format!(r#"DROP SCHEMA IF EXISTS "{schema_name}" CASCADE"#))
                .await;
            return Err(error.into());
        }

        Ok(Some(Self {
            control,
            database_url,
            db,
            schema_name,
        }))
    }

    async fn peer(&self) -> TestResult<DatabaseConnection> {
        let db = connect(&self.database_url).await?;
        set_search_path(&db, &self.schema_name).await?;
        Ok(db)
    }

    async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct ForumSeed {
    tenant_id: Uuid,
    category_id: Uuid,
    topic_id: Uuid,
    author_id: Uuid,
    reply_id: Uuid,
}

#[tokio::test]
async fn forum_receipt_replays_after_lost_response_before_stale_revision_check() -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };

    let outcome = run_lost_response_contract(&database).await;
    let cleanup = database.cleanup().await;
    outcome?;
    cleanup
}

async fn run_lost_response_contract(database: &TestDatabase) -> TestResult<()> {
    let seed = seed_approved_reply(&database.db).await?;
    assert_reply_state(&database.db, &seed, 1, "approved").await?;
    let reviewed_revision = reply_revision(&database.db, &seed).await?;
    assert!(reviewed_revision > 0);

    let moderation = ModerationService::new(database.peer().await?);
    let (case, decision) = seed_hide_decision(
        &moderation,
        seed.tenant_id,
        seed.author_id,
        seed.reply_id,
        reviewed_revision,
    )
    .await?;
    let command = application_command(&case, &decision)?;

    let factory = ForumModerationSubjectAdapterFactory::reply();
    let adapter = factory.build(&HostRuntimeContext::new(database.db.clone()))?;

    // Model a response loss after the producer owner has committed but before Moderation records
    // the result. Forum applies the real effect and completes the real owner-operation receipt;
    // Moderation intentionally remains pending/decided.
    let first_application = adapter
        .apply_moderation_decision(
            application_context(seed.tenant_id, decision.id, "lost-response-first"),
            command,
        )
        .await?;
    assert!(first_application.applied_revision > reviewed_revision);
    assert_reply_state(&database.db, &seed, 0, "hidden").await?;
    let revision_after_first = reply_revision(&database.db, &seed).await?;
    assert_eq!(revision_after_first, first_application.applied_revision);
    assert_eq!(count_status_events(&database.db).await?, 1);
    assert_completed_receipt(&database.db, seed.tenant_id, decision.id).await?;

    let pending = moderation
        .get_application_operation(seed.tenant_id, decision.id)
        .await?
        .ok_or_else(|| test_error("Moderation operation disappeared after Forum-only commit"))?;
    assert_eq!(
        pending.status,
        ModerationApplicationOperationStatus::Pending
    );
    assert_eq!(pending.attempt_count, 0);
    assert_eq!(
        moderation
            .get_case(seed.tenant_id, case.id)
            .await?
            .ok_or_else(|| test_error("Moderation case disappeared after Forum-only commit"))?
            .status,
        ModerationCaseStatus::Decided
    );

    // The immutable reviewed revision is now stale in Forum. A normal re-execution would hit the
    // revision fence. The dispatcher can succeed only because the completed receipt is admitted
    // and replayed before Forum touches the subject again.
    let mut registry = ModerationSubjectAdapterRegistry::default();
    registry.register_arc(adapter)?;
    let dispatched = moderation
        .dispatch_application_operation_once(
            &registry,
            seed.tenant_id,
            decision.id,
            "postgres-forum-lost-response-replay",
        )
        .await?
        .ok_or_else(|| {
            test_error("pending Moderation operation was not claimable after response loss")
        })?;

    assert_eq!(
        dispatched.status,
        ModerationApplicationOperationStatus::Applied
    );
    assert_eq!(dispatched.attempt_count, 1);
    assert_eq!(
        dispatched.applied_revision,
        Some(first_application.applied_revision)
    );
    assert_eq!(
        moderation
            .get_case(seed.tenant_id, case.id)
            .await?
            .ok_or_else(|| test_error("Moderation case disappeared after receipt replay"))?
            .status,
        ModerationCaseStatus::Closed
    );

    // Replay must not execute the producer mutation twice.
    assert_reply_state(&database.db, &seed, 0, "hidden").await?;
    assert_eq!(
        reply_revision(&database.db, &seed).await?,
        revision_after_first
    );
    assert_eq!(count_status_events(&database.db).await?, 1);
    assert_completed_receipt(&database.db, seed.tenant_id, decision.id).await?;
    Ok(())
}

async fn seed_approved_reply(db: &DatabaseConnection) -> TestResult<ForumSeed> {
    let seed = ForumSeed {
        tenant_id: Uuid::new_v4(),
        category_id: Uuid::new_v4(),
        topic_id: Uuid::new_v4(),
        author_id: Uuid::new_v4(),
        reply_id: Uuid::nil(),
    };
    db.execute_unprepared(&format!(
        r#"
        INSERT INTO forum_categories
            (id, tenant_id, position, moderated, topic_count, reply_count)
        VALUES
            ('{}', '{}', 0, FALSE, 1, 0);
        INSERT INTO forum_topics
            (id, tenant_id, category_id, status, metadata, is_pinned, is_locked, reply_count)
        VALUES
            ('{}', '{}', '{}', 'open', '{{}}', FALSE, FALSE, 0);
        "#,
        seed.category_id, seed.tenant_id, seed.topic_id, seed.tenant_id, seed.category_id,
    ))
    .await?;

    let reply = ReplyService::new(db.clone(), event_bus(db.clone()))
        .create(
            seed.tenant_id,
            SecurityContext::new(UserRole::Customer, Some(seed.author_id)),
            seed.topic_id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: RichTextDocument::single_paragraph("lost response replay fixture"),
                parent_reply_id: None,
            },
        )
        .await?;
    if reply.status != "approved" {
        return Err(test_error(format!(
            "expected approved Forum fixture reply, got `{}`",
            reply.status
        )));
    }
    Ok(ForumSeed {
        reply_id: reply.id,
        ..seed
    })
}

async fn seed_hide_decision(
    service: &ModerationService,
    tenant_id: Uuid,
    actor_id: Uuid,
    reply_id: Uuid,
    subject_revision: i64,
) -> TestResult<(ModerationCaseRecord, ModerationDecisionRecord)> {
    let subject = ModerationSubjectRef {
        module: "forum".to_string(),
        kind: ModerationSubjectKind::ForumPost,
        id: reply_id,
        revision: subject_revision,
    };
    let report = service
        .submit_report_replay_safe(
            write_context(tenant_id, actor_id, "lost-response-report"),
            SubmitModerationReportCommand {
                scope: ModerationScopeRef::platform(),
                subject: subject.clone(),
                reporter_kind: ModerationReporterKind::User,
                reporter_id: Some(actor_id),
                reason_code: ModerationReasonCode::Spam,
                description_reference: None,
                metadata: json!({"source": "forum-moderation-lost-response-postgres"}),
            },
        )
        .await?;
    let case = service
        .open_case_replay_safe(
            write_context(tenant_id, actor_id, "lost-response-case"),
            OpenModerationCaseCommand {
                scope: ModerationScopeRef::platform(),
                subject,
                queue_key: "content".to_string(),
                priority: ModerationCasePriority::Normal,
                policy_id: None,
                policy_version: 1,
                report_ids: vec![report.id],
                metadata: json!({"source": "forum-moderation-lost-response-postgres"}),
            },
        )
        .await?;
    let assigned = service
        .assign_case_replay_safe(
            write_context(tenant_id, actor_id, "lost-response-assign"),
            AssignModerationCaseCommand {
                case_id: case.id,
                expected_revision: case.revision,
                moderator_id: actor_id,
            },
        )
        .await?;
    let decision = service
        .decide_case_replay_safe(
            write_context(tenant_id, actor_id, "lost-response-decide"),
            DecideModerationCaseCommand {
                case_id: assigned.id,
                expected_revision: assigned.revision,
                decision_kind: ModerationDecisionKind::Hide,
                reason_code: ModerationReasonCode::Spam,
                effect: ModerationDecisionEffect::v1(
                    ModerationDecisionEffectAction::SetVisibility {
                        state: ModerationVisibilityState::Hidden,
                    },
                )?,
                policy_snapshot: json!({"policy": "forum-lost-response", "version": 1}),
            },
        )
        .await?;
    let decided = service
        .get_case(tenant_id, case.id)
        .await?
        .ok_or_else(|| test_error("decided lost-response case is missing"))?;
    assert_eq!(decided.status, ModerationCaseStatus::Decided);
    Ok((decided, decision))
}

fn application_command(
    case: &ModerationCaseRecord,
    decision: &ModerationDecisionRecord,
) -> TestResult<ApplyModerationDecisionCommand> {
    Ok(ApplyModerationDecisionCommand {
        decision_id: decision.id,
        subject: case.subject.clone(),
        decision_kind: decision.decision_kind,
        reason_code: decision.reason_code,
        effect: decision
            .effect
            .clone()
            .ok_or_else(|| test_error("typed Moderation decision effect is missing"))?,
        decision_hash: decision.decision_hash.clone(),
    })
}

fn application_context(tenant_id: Uuid, decision_id: Uuid, correlation: &str) -> PortContext {
    let id = decision_id.to_string();
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service(DISPATCH_ACTOR),
        "und",
        correlation,
    )
    .with_causation_id(id.clone())
    .with_idempotency_key(id)
    .with_deadline(Duration::from_secs(APPLICATION_ADAPTER_DEADLINE_SECONDS))
}

fn write_context(tenant_id: Uuid, actor_id: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("forum-moderation-lost-response-{key}"),
    )
    .with_idempotency_key(key)
    .with_deadline(Duration::from_secs(5))
}

fn event_bus(db: DatabaseConnection) -> TransactionalEventBus {
    TransactionalEventBus::new(Arc::new(OutboxTransport::new(db)))
}

async fn assert_reply_state(
    db: &DatabaseConnection,
    seed: &ForumSeed,
    expected_count: i64,
    expected_status: &str,
) -> TestResult<()> {
    let topic_count = scalar_i64(
        db,
        "SELECT reply_count::bigint AS value FROM forum_topics WHERE tenant_id = $1 AND id = $2",
        vec![seed.tenant_id.into(), seed.topic_id.into()],
    )
    .await?;
    let category_count = scalar_i64(
        db,
        "SELECT reply_count::bigint AS value FROM forum_categories WHERE tenant_id = $1 AND id = $2",
        vec![seed.tenant_id.into(), seed.category_id.into()],
    )
    .await?;
    let user_count = scalar_i64(
        db,
        "SELECT reply_count::bigint AS value FROM forum_user_stats WHERE tenant_id = $1 AND user_id = $2",
        vec![seed.tenant_id.into(), seed.author_id.into()],
    )
    .await?;
    let status = scalar_string(
        db,
        "SELECT status AS value FROM forum_replies WHERE tenant_id = $1 AND id = $2",
        vec![seed.tenant_id.into(), seed.reply_id.into()],
    )
    .await?;
    if topic_count != expected_count
        || category_count != expected_count
        || user_count != expected_count
        || status != expected_status
    {
        return Err(test_error(format!(
            "unexpected Forum reply state: topic={topic_count}, category={category_count}, user={user_count}, status={status}; expected count={expected_count}, status={expected_status}"
        )));
    }
    Ok(())
}

async fn reply_revision(db: &DatabaseConnection, seed: &ForumSeed) -> TestResult<i64> {
    scalar_i64(
        db,
        "SELECT revision::bigint AS value FROM forum_reply_moderation_subject_revisions WHERE tenant_id = $1 AND reply_id = $2",
        vec![seed.tenant_id.into(), seed.reply_id.into()],
    )
    .await
}

async fn count_status_events(db: &DatabaseConnection) -> TestResult<i64> {
    scalar_i64(
        db,
        "SELECT COUNT(*)::bigint AS value FROM sys_events WHERE event_type = 'forum.reply.status_changed'",
        Vec::new(),
    )
    .await
}

async fn assert_completed_receipt(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    decision_id: Uuid,
) -> TestResult<()> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT status, response_json IS NOT NULL AS has_response, completed_at IS NOT NULL AS has_completed_at FROM owner_operation_receipts WHERE tenant_id = $1 AND owner_slug = 'forum' AND idempotency_key = $2 AND operation = $3",
            vec![
                tenant_id.into(),
                decision_id.to_string().into(),
                APPLY_OPERATION.to_string().into(),
            ],
        ))
        .await?
        .ok_or_else(|| test_error("completed Forum moderation application receipt is missing"))?;
    assert_eq!(row.try_get::<String>("", "status")?, "completed");
    assert!(row.try_get::<bool>("", "has_response")?);
    assert!(row.try_get::<bool>("", "has_completed_at")?);
    assert_eq!(
        scalar_i64(
            db,
            "SELECT COUNT(*)::bigint AS value FROM owner_operation_receipts WHERE tenant_id = $1 AND owner_slug = 'forum' AND idempotency_key = $2",
            vec![tenant_id.into(), decision_id.to_string().into()],
        )
        .await?,
        1
    );
    Ok(())
}

async fn scalar_i64(
    db: &DatabaseConnection,
    sql: &str,
    values: Vec<sea_orm::Value>,
) -> TestResult<i64> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            values,
        ))
        .await?
        .ok_or_else(|| test_error("scalar PostgreSQL query returned no row"))?;
    Ok(row.try_get("", "value")?)
}

async fn scalar_string(
    db: &DatabaseConnection,
    sql: &str,
    values: Vec<sea_orm::Value>,
) -> TestResult<String> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            values,
        ))
        .await?
        .ok_or_else(|| test_error("scalar PostgreSQL query returned no row"))?;
    Ok(row.try_get("", "value")?)
}

fn database_url() -> Option<String> {
    env::var(FORUM_DATABASE_ENV)
        .or_else(|_| env::var(MODERATION_DATABASE_ENV))
        .or_else(|_| env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

async fn set_search_path(db: &DatabaseConnection, schema_name: &str) -> TestResult<()> {
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}""#))
        .await?;
    Ok(())
}

fn test_error(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    std::io::Error::other(message.into()).into()
}
