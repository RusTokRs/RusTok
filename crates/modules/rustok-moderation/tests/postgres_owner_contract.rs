use std::{env, error::Error, time::Duration};

use rustok_api::{PortActor, PortContext};
use rustok_moderation::{
    AssignModerationCaseCommand, DecideModerationCaseCommand, ModerationCasePriority,
    ModerationDecisionEffect, ModerationDecisionEffectAction, ModerationDecisionKind,
    ModerationError, ModerationReasonCode, ModerationReporterKind, ModerationScopeRef,
    ModerationService, ModerationSubjectKind, ModerationSubjectRef, OpenModerationCaseCommand,
    SubmitModerationReportCommand,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::{MigrationTrait, MigratorTrait};
use serde_json::json;
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_MODERATION_TEST_DATABASE_URL";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

struct TestMigrator;

#[async_trait::async_trait]
impl MigratorTrait for TestMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        rustok_moderation::migrations::migrations()
    }
}

struct TestDatabase {
    control: DatabaseConnection,
    database_url: String,
    schema_name: String,
}

impl TestDatabase {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Moderation PostgreSQL owner contract"
            );
            return Ok(None);
        };

        let control = connect(&database_url, "moderation_contract_control").await?;
        let suffix = Uuid::new_v4().simple().to_string();
        let schema_name = format!("rustok_moderation_contract_{suffix}");
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let migration = scoped_connection(
            &database_url,
            &schema_name,
            &format!("moderation_contract_migration_{suffix}"),
        )
        .await?;
        TestMigrator::up(&migration, None).await?;
        migration.close().await?;

        Ok(Some(Self {
            control,
            database_url,
            schema_name,
        }))
    }

    async fn connection(&self, name: &str) -> TestResult<DatabaseConnection> {
        scoped_connection(&self.database_url, &self.schema_name, name).await
    }

    async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        self.control.close().await?;
        Ok(())
    }
}

#[tokio::test]
async fn postgres_owner_contract_preserves_dedup_effect_enqueue_and_revision_cas() -> TestResult<()>
{
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };

    let outcome = run_owner_contract(&database).await;
    let cleanup = database.cleanup().await;
    outcome?;
    cleanup
}

async fn run_owner_contract(database: &TestDatabase) -> TestResult<()> {
    concurrent_active_case_admission_converges(database).await?;
    decision_effect_and_pending_application_commit_together(database).await?;
    concurrent_assignment_uses_revision_cas(database).await?;
    Ok(())
}

async fn concurrent_active_case_admission_converges(database: &TestDatabase) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let subject_id = Uuid::new_v4();
    let seed = ModerationService::new(database.connection("moderation_dedup_seed").await?);

    let first_report = seed
        .submit_report_replay_safe(
            write_context(tenant_id, actor_id, "pg-dedup-report-one"),
            report_command(actor_id, subject_id, 3),
        )
        .await?;
    let second_report = seed
        .submit_report_replay_safe(
            write_context(tenant_id, actor_id, "pg-dedup-report-two"),
            report_command(actor_id, subject_id, 3),
        )
        .await?;

    let first_service = ModerationService::new(database.connection("moderation_dedup_one").await?);
    let second_service = ModerationService::new(database.connection("moderation_dedup_two").await?);
    let first = first_service.open_case_replay_safe(
        write_context(tenant_id, actor_id, "pg-dedup-case-one"),
        case_command(subject_id, 3, vec![first_report.id]),
    );
    let second = second_service.open_case_replay_safe(
        write_context(tenant_id, actor_id, "pg-dedup-case-two"),
        case_command(subject_id, 3, vec![second_report.id]),
    );
    let (first, second) = tokio::join!(first, second);
    let first = first?;
    let second = second?;

    assert_eq!(first.id, second.id);

    let observer = database.connection("moderation_dedup_observer").await?;
    assert_eq!(
        scalar_i64(
            &observer,
            "SELECT COUNT(*)::bigint AS value FROM moderation_cases WHERE tenant_id = $1 AND subject_id = $2 AND active_deduplication_key IS NOT NULL",
            vec![tenant_id.into(), subject_id.into()],
        )
        .await?,
        1
    );
    assert_eq!(
        scalar_i64(
            &observer,
            "SELECT COUNT(*)::bigint AS value FROM moderation_case_reports WHERE tenant_id = $1 AND case_id = $2",
            vec![tenant_id.into(), first.id.into()],
        )
        .await?,
        2
    );
    assert_eq!(
        scalar_i64(
            &observer,
            "SELECT COUNT(*)::bigint AS value FROM moderation_reports WHERE tenant_id = $1 AND id IN ($2, $3) AND status = 'attached'",
            vec![tenant_id.into(), first_report.id.into(), second_report.id.into()],
        )
        .await?,
        2
    );
    Ok(())
}

async fn decision_effect_and_pending_application_commit_together(
    database: &TestDatabase,
) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let subject_id = Uuid::new_v4();
    let service = ModerationService::new(database.connection("moderation_effect_owner").await?);

    let report = service
        .submit_report_replay_safe(
            write_context(tenant_id, actor_id, "pg-effect-report"),
            report_command(actor_id, subject_id, 7),
        )
        .await?;
    let case = service
        .open_case_replay_safe(
            write_context(tenant_id, actor_id, "pg-effect-case"),
            case_command(subject_id, 7, vec![report.id]),
        )
        .await?;
    let assigned = service
        .assign_case_replay_safe(
            write_context(tenant_id, actor_id, "pg-effect-assign"),
            AssignModerationCaseCommand {
                case_id: case.id,
                expected_revision: case.revision,
                moderator_id: actor_id,
            },
        )
        .await?;
    let effect = ModerationDecisionEffect::v1(ModerationDecisionEffectAction::NoDomainMutation)?;
    let decision = service
        .decide_case_replay_safe(
            write_context(tenant_id, actor_id, "pg-effect-decide"),
            DecideModerationCaseCommand {
                case_id: assigned.id,
                expected_revision: assigned.revision,
                decision_kind: ModerationDecisionKind::Warning,
                reason_code: ModerationReasonCode::Other,
                effect: effect.clone(),
                policy_snapshot: json!({"policy": "postgres-owner-contract", "version": 1}),
            },
        )
        .await?;

    let observer = database.connection("moderation_effect_observer").await?;
    let effect_row = observer
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT schema_version, effect_kind, effect_payload FROM moderation_decision_effects WHERE tenant_id = $1 AND decision_id = $2",
            vec![tenant_id.into(), decision.id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("moderation decision effect row is missing"))?;
    assert_eq!(effect_row.try_get::<i32>("", "schema_version")?, 1);
    assert_eq!(effect_row.try_get::<String>("", "effect_kind")?, "warning");
    assert_eq!(
        effect_row.try_get::<serde_json::Value>("", "effect_payload")?,
        serde_json::to_value(&effect)?
    );

    let operation = observer
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT case_id, decision_hash, subject_revision, status, attempt_count FROM moderation_application_operations WHERE tenant_id = $1 AND decision_id = $2",
            vec![tenant_id.into(), decision.id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("moderation application operation row is missing"))?;
    assert_eq!(operation.try_get::<Uuid>("", "case_id")?, assigned.id);
    assert_eq!(
        operation.try_get::<String>("", "decision_hash")?,
        decision.decision_hash
    );
    assert_eq!(operation.try_get::<i64>("", "subject_revision")?, 7);
    assert_eq!(operation.try_get::<String>("", "status")?, "pending");
    assert_eq!(operation.try_get::<i32>("", "attempt_count")?, 0);
    Ok(())
}

async fn concurrent_assignment_uses_revision_cas(database: &TestDatabase) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let subject_id = Uuid::new_v4();
    let seed = ModerationService::new(database.connection("moderation_cas_seed").await?);
    let report = seed
        .submit_report_replay_safe(
            write_context(tenant_id, actor_id, "pg-cas-report"),
            report_command(actor_id, subject_id, 11),
        )
        .await?;
    let case = seed
        .open_case_replay_safe(
            write_context(tenant_id, actor_id, "pg-cas-case"),
            case_command(subject_id, 11, vec![report.id]),
        )
        .await?;

    let first_moderator = Uuid::new_v4();
    let second_moderator = Uuid::new_v4();
    let first_service = ModerationService::new(database.connection("moderation_cas_one").await?);
    let second_service = ModerationService::new(database.connection("moderation_cas_two").await?);
    let first = first_service.assign_case_replay_safe(
        write_context(tenant_id, actor_id, "pg-cas-assign-one"),
        AssignModerationCaseCommand {
            case_id: case.id,
            expected_revision: case.revision,
            moderator_id: first_moderator,
        },
    );
    let second = second_service.assign_case_replay_safe(
        write_context(tenant_id, actor_id, "pg-cas-assign-two"),
        AssignModerationCaseCommand {
            case_id: case.id,
            expected_revision: case.revision,
            moderator_id: second_moderator,
        },
    );
    let (first, second) = tokio::join!(first, second);

    let mut winners = Vec::new();
    let mut revision_conflicts = 0;
    for result in [first, second] {
        match result {
            Ok(record) => winners.push(record),
            Err(ModerationError::RevisionConflict) => revision_conflicts += 1,
            Err(error) => return Err(error.into()),
        }
    }
    assert_eq!(winners.len(), 1);
    assert_eq!(revision_conflicts, 1);
    assert_eq!(winners[0].revision, case.revision + 1);
    assert!(
        winners[0].assigned_moderator_id == Some(first_moderator)
            || winners[0].assigned_moderator_id == Some(second_moderator)
    );

    let observer = database.connection("moderation_cas_observer").await?;
    let row = observer
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT revision, assigned_moderator_id FROM moderation_cases WHERE tenant_id = $1 AND id = $2",
            vec![tenant_id.into(), case.id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("assigned moderation case is missing"))?;
    assert_eq!(row.try_get::<i64>("", "revision")?, case.revision + 1);
    let stored_moderator = row.try_get::<Uuid>("", "assigned_moderator_id")?;
    assert_eq!(Some(stored_moderator), winners[0].assigned_moderator_id);
    Ok(())
}

fn write_context(tenant_id: Uuid, actor_id: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("postgres-moderation-contract-{key}"),
    )
    .with_idempotency_key(key)
    .with_deadline(Duration::from_secs(5))
}

fn report_command(
    actor_id: Uuid,
    subject_id: Uuid,
    revision: i64,
) -> SubmitModerationReportCommand {
    SubmitModerationReportCommand {
        scope: ModerationScopeRef::platform(),
        subject: ModerationSubjectRef {
            module: "forum".to_string(),
            kind: ModerationSubjectKind::ForumPost,
            id: subject_id,
            revision,
        },
        reporter_kind: ModerationReporterKind::User,
        reporter_id: Some(actor_id),
        reason_code: ModerationReasonCode::Spam,
        description_reference: None,
        metadata: json!({"source": "postgres-owner-contract"}),
    }
}

fn case_command(
    subject_id: Uuid,
    revision: i64,
    report_ids: Vec<Uuid>,
) -> OpenModerationCaseCommand {
    OpenModerationCaseCommand {
        scope: ModerationScopeRef::platform(),
        subject: ModerationSubjectRef {
            module: "forum".to_string(),
            kind: ModerationSubjectKind::ForumPost,
            id: subject_id,
            revision,
        },
        queue_key: "content".to_string(),
        priority: ModerationCasePriority::Normal,
        policy_id: None,
        policy_version: 1,
        report_ids,
        metadata: json!({"source": "postgres-owner-contract"}),
    }
}

async fn scalar_i64(
    db: &DatabaseConnection,
    sql: &str,
    values: Vec<sea_orm::Value>,
) -> TestResult<i64> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            values,
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("scalar PostgreSQL query returned no row"))?;
    Ok(row.try_get("", "value")?)
}

fn database_url() -> Option<String> {
    env::var(DATABASE_ENV)
        .or_else(|_| env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect(database_url: &str, application_name: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    db.execute_unprepared(&format!("SET application_name TO '{application_name}'"))
        .await?;
    Ok(db)
}

async fn scoped_connection(
    database_url: &str,
    schema_name: &str,
    application_name: &str,
) -> TestResult<DatabaseConnection> {
    let db = connect(database_url, application_name).await?;
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}""#))
        .await?;
    Ok(db)
}
