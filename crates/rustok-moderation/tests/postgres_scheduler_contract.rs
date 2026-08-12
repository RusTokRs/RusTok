use std::{
    env,
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{HostRuntimeContext, PortActor, PortContext, PortError};
use rustok_core::{ModuleRuntimeExtensions, RusToKModule};
use rustok_moderation::{
    AssignModerationCaseCommand, DecideModerationCaseCommand, ModerationApplicationOperationStatus,
    ModerationCasePriority, ModerationCaseRecord, ModerationCaseStatus, ModerationDecisionEffect,
    ModerationDecisionEffectAction, ModerationDecisionKind, ModerationDecisionRecord,
    ModerationModule, ModerationReasonCode, ModerationReporterKind, ModerationScopeRef,
    ModerationService, ModerationSubjectAdapterKey, ModerationSubjectAdapterRegistry,
    ModerationSubjectKind, ModerationSubjectRef, OpenModerationCaseCommand,
    SubmitModerationReportCommand,
};
use rustok_moderation_api::{
    ApplyModerationDecisionCommand, ModerationDecisionApplication, ModerationSubjectCommandPort,
};
use rustok_runtime::{ModuleWorkRegistrations, ModuleWorkScheduler};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use sea_orm_migration::{MigrationTrait, MigratorTrait};
use serde_json::json;
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_MODERATION_TEST_DATABASE_URL";
type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

struct TestMigrator;

#[async_trait]
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
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Moderation PostgreSQL scheduler contract"
            );
            return Ok(None);
        };
        let control = connect(&database_url, "moderation_scheduler_control").await?;
        let suffix = Uuid::new_v4().simple().to_string();
        let schema_name = format!("rustok_moderation_scheduler_{suffix}");
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;
        let migration = scoped_connection(
            &database_url,
            &schema_name,
            &format!("moderation_scheduler_migration_{suffix}"),
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

#[derive(Clone)]
struct CountingAdapter {
    key: ModerationSubjectAdapterKey,
    calls: Arc<AtomicUsize>,
}

impl CountingAdapter {
    fn forum_post() -> Self {
        Self {
            key: ModerationSubjectAdapterKey::new("forum", ModerationSubjectKind::ForumPost)
                .expect("valid scheduler test adapter key"),
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ModerationSubjectCommandPort for CountingAdapter {
    fn key(&self) -> ModerationSubjectAdapterKey {
        self.key.clone()
    }

    async fn apply_moderation_decision(
        &self,
        _context: PortContext,
        command: ApplyModerationDecisionCommand,
    ) -> Result<ModerationDecisionApplication, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ModerationDecisionApplication {
            decision_id: command.decision_id,
            subject: command.subject.clone(),
            applied_revision: command.subject.revision,
            applied_at: Utc::now(),
        })
    }
}

#[tokio::test]
async fn postgres_scheduler_contract_preserves_multi_host_stop_and_crash_recovery() -> TestResult<()>
{
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let outcome = run_scheduler_contract(&database).await;
    let cleanup = database.cleanup().await;
    outcome?;
    cleanup
}

async fn run_scheduler_contract(database: &TestDatabase) -> TestResult<()> {
    two_schedulers_converge_on_one_domain_call(database).await?;
    expired_claim_is_recovered_by_scheduler_without_duplicate_start_transition(database).await?;
    // Stop intentionally leaves one pending due operation untouched, so it must be the final
    // scenario in the shared schema rather than a candidate for a later scheduler run.
    stop_signal_prevents_new_moderation_claim(database).await?;
    Ok(())
}

async fn two_schedulers_converge_on_one_domain_call(database: &TestDatabase) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let owner = ModerationService::new(database.connection("scheduler_race_seed").await?);
    let (case, decision) =
        seed_decided_case(&owner, tenant_id, actor_id, 11, "scheduler-race").await?;

    let adapter = CountingAdapter::forum_post();
    let registry = registry_with(adapter.clone())?;
    let first = scheduler(
        database.connection("scheduler_race_one").await?,
        registry.clone(),
    )
    .await?;
    let second = scheduler(database.connection("scheduler_race_two").await?, registry).await?;
    let (first_run, second_run) = tokio::join!(first.run_once(), second.run_once());
    let executed = first_run? + second_run?;

    assert!((1..=2).contains(&executed));
    assert_eq!(adapter.call_count(), 1);
    let operation = owner
        .get_application_operation(tenant_id, decision.id)
        .await?
        .ok_or_else(|| test_error("scheduler race operation disappeared"))?;
    assert_eq!(
        operation.status,
        ModerationApplicationOperationStatus::Applied
    );
    assert_eq!(operation.attempt_count, 1);
    assert_eq!(
        owner
            .get_case(tenant_id, case.id)
            .await?
            .ok_or_else(|| test_error("scheduler race case disappeared"))?
            .status,
        ModerationCaseStatus::Closed
    );

    let observer = database.connection("scheduler_race_observer").await?;
    assert_eq!(
        count_events(
            &observer,
            tenant_id,
            "application",
            decision.id,
            "application_attempt_claimed"
        )
        .await?,
        1
    );
    assert_eq!(
        count_events(
            &observer,
            tenant_id,
            "case",
            case.id,
            "case_application_started"
        )
        .await?,
        1
    );
    assert_eq!(
        count_events(&observer, tenant_id, "case", case.id, "case_closed").await?,
        1
    );
    Ok(())
}

async fn expired_claim_is_recovered_by_scheduler_without_duplicate_start_transition(
    database: &TestDatabase,
) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let owner = ModerationService::new(database.connection("scheduler_crash_owner").await?);
    let (case, decision) =
        seed_decided_case(&owner, tenant_id, actor_id, 31, "scheduler-crash").await?;

    // Simulate a process crash after the authoritative owner claim but before adapter execution.
    let claimed = owner
        .claim_application_operation(tenant_id, decision.id, "crashed-host", 60)
        .await?
        .ok_or_else(|| test_error("crash fixture was not claimable"))?;
    assert_eq!(
        claimed.status,
        ModerationApplicationOperationStatus::Applying
    );
    assert_eq!(claimed.attempt_count, 1);
    let after_first_claim = owner
        .get_case(tenant_id, case.id)
        .await?
        .ok_or_else(|| test_error("case after crashed claim disappeared"))?;
    assert_eq!(
        after_first_claim.status,
        ModerationCaseStatus::ApplyingDecision
    );
    assert_eq!(after_first_claim.revision, case.revision + 1);

    let storage = database.connection("scheduler_crash_storage").await?;
    storage
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE moderation_application_operations SET lease_expires_at = NOW() - INTERVAL '1 second' WHERE tenant_id = $1 AND decision_id = $2",
            vec![tenant_id.into(), decision.id.into()],
        ))
        .await?;

    let adapter = CountingAdapter::forum_post();
    let scheduler = scheduler(
        database.connection("scheduler_crash_recovery").await?,
        registry_with(adapter.clone())?,
    )
    .await?;
    assert_eq!(scheduler.run_once().await?, 1);
    assert_eq!(adapter.call_count(), 1);

    let operation = owner
        .get_application_operation(tenant_id, decision.id)
        .await?
        .ok_or_else(|| test_error("recovered scheduler operation disappeared"))?;
    assert_eq!(
        operation.status,
        ModerationApplicationOperationStatus::Applied
    );
    assert_eq!(operation.attempt_count, 2);
    let closed = owner
        .get_case(tenant_id, case.id)
        .await?
        .ok_or_else(|| test_error("recovered scheduler case disappeared"))?;
    assert_eq!(closed.status, ModerationCaseStatus::Closed);
    assert_eq!(closed.revision, after_first_claim.revision + 1);

    let observer = database.connection("scheduler_crash_observer").await?;
    assert_eq!(
        count_events(
            &observer,
            tenant_id,
            "application",
            decision.id,
            "application_attempt_claimed"
        )
        .await?,
        2
    );
    assert_eq!(
        count_events(
            &observer,
            tenant_id,
            "case",
            case.id,
            "case_application_started"
        )
        .await?,
        1
    );
    assert_eq!(
        count_events(&observer, tenant_id, "case", case.id, "case_closed").await?,
        1
    );
    Ok(())
}

async fn stop_signal_prevents_new_moderation_claim(database: &TestDatabase) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let owner = ModerationService::new(database.connection("scheduler_stop_owner").await?);
    let (case, decision) =
        seed_decided_case(&owner, tenant_id, actor_id, 21, "scheduler-stop").await?;

    let adapter = CountingAdapter::forum_post();
    let scheduler = scheduler(
        database.connection("scheduler_stop_runtime").await?,
        registry_with(adapter.clone())?,
    )
    .await?;
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    stop_tx
        .send(true)
        .map_err(|_| test_error("scheduler stop receiver unexpectedly closed"))?;
    scheduler
        .run_until_stopped(stop_rx, Duration::from_millis(1))
        .await;

    assert_eq!(adapter.call_count(), 0);
    let operation = owner
        .get_application_operation(tenant_id, decision.id)
        .await?
        .ok_or_else(|| test_error("stopped scheduler operation disappeared"))?;
    assert_eq!(
        operation.status,
        ModerationApplicationOperationStatus::Pending
    );
    assert_eq!(operation.attempt_count, 0);
    assert!(operation.lease_token.is_none());
    let stored_case = owner
        .get_case(tenant_id, case.id)
        .await?
        .ok_or_else(|| test_error("stopped scheduler case disappeared"))?;
    assert_eq!(stored_case.status, ModerationCaseStatus::Decided);
    assert_eq!(stored_case.revision, case.revision);
    Ok(())
}

fn registry_with(adapter: CountingAdapter) -> TestResult<Arc<ModerationSubjectAdapterRegistry>> {
    let mut registry = ModerationSubjectAdapterRegistry::default();
    registry.register(adapter)?;
    Ok(Arc::new(registry))
}

async fn scheduler(
    database: DatabaseConnection,
    registry: Arc<ModerationSubjectAdapterRegistry>,
) -> TestResult<ModuleWorkScheduler> {
    let mut extensions = ModuleRuntimeExtensions::default();
    ModerationModule.register_runtime_extensions(&mut extensions)?;
    let registrations = extensions
        .get::<ModuleWorkRegistrations>()
        .cloned()
        .ok_or_else(|| test_error("Moderation module work registration is missing"))?;
    let host = HostRuntimeContext::new(database).with_shared_value(registry);
    let scheduler = ModuleWorkScheduler::new();
    registrations.register_all(&host, &scheduler).await?;
    Ok(scheduler)
}

async fn seed_decided_case(
    service: &ModerationService,
    tenant_id: Uuid,
    actor_id: Uuid,
    subject_revision: i64,
    key_prefix: &str,
) -> TestResult<(ModerationCaseRecord, ModerationDecisionRecord)> {
    let subject_id = Uuid::new_v4();
    let report = service
        .submit_report_replay_safe(
            write_context(tenant_id, actor_id, &format!("{key_prefix}-report")),
            report_command(actor_id, subject_id, subject_revision),
        )
        .await?;
    let case = service
        .open_case_replay_safe(
            write_context(tenant_id, actor_id, &format!("{key_prefix}-case")),
            case_command(subject_id, subject_revision, vec![report.id]),
        )
        .await?;
    let assigned = service
        .assign_case_replay_safe(
            write_context(tenant_id, actor_id, &format!("{key_prefix}-assign")),
            AssignModerationCaseCommand {
                case_id: case.id,
                expected_revision: case.revision,
                moderator_id: actor_id,
            },
        )
        .await?;
    let decision = service
        .decide_case_replay_safe(
            write_context(tenant_id, actor_id, &format!("{key_prefix}-decide")),
            DecideModerationCaseCommand {
                case_id: assigned.id,
                expected_revision: assigned.revision,
                decision_kind: ModerationDecisionKind::Warning,
                reason_code: ModerationReasonCode::Other,
                effect: ModerationDecisionEffect::v1(
                    ModerationDecisionEffectAction::NoDomainMutation,
                )?,
                policy_snapshot: json!({"policy": "postgres-scheduler-contract", "version": 1}),
            },
        )
        .await?;
    let decided = service
        .get_case(tenant_id, case.id)
        .await?
        .ok_or_else(|| test_error("decided scheduler case is missing"))?;
    assert_eq!(decided.status, ModerationCaseStatus::Decided);
    Ok((decided, decision))
}

fn write_context(tenant_id: Uuid, actor_id: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("postgres-moderation-scheduler-{key}"),
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
        metadata: json!({"source": "postgres-scheduler-contract"}),
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
        metadata: json!({"source": "postgres-scheduler-contract"}),
    }
}

async fn count_events(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    aggregate_kind: &str,
    aggregate_id: Uuid,
    event_type: &str,
) -> TestResult<i64> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT COUNT(*)::bigint AS value FROM moderation_events WHERE tenant_id = $1 AND aggregate_kind = $2 AND aggregate_id = $3 AND event_type = $4",
            vec![
                tenant_id.into(),
                aggregate_kind.to_owned().into(),
                aggregate_id.into(),
                event_type.to_owned().into(),
            ],
        ))
        .await?
        .ok_or_else(|| test_error("moderation event count returned no row"))?;
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

fn test_error(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    std::io::Error::other(message.into()).into()
}
