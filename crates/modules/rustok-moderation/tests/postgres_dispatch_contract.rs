use std::{
    env,
    error::Error,
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::Utc;
use rustok_api::{PortActor, PortActorKind, PortContext, PortError};
use rustok_moderation::{
    APPLICATION_ADAPTER_DEADLINE_SECONDS, AssignModerationCaseCommand, DecideModerationCaseCommand,
    ModerationApplicationOperationStatus, ModerationCasePriority, ModerationCaseRecord,
    ModerationCaseStatus, ModerationDecisionEffect, ModerationDecisionEffectAction,
    ModerationDecisionKind, ModerationDecisionRecord, ModerationReasonCode, ModerationReporterKind,
    ModerationScopeRef, ModerationService, ModerationSubjectKind, ModerationSubjectRef,
    OpenModerationCaseCommand, SubmitModerationReportCommand,
};
use rustok_moderation_api::{
    ApplyModerationDecisionCommand, ModerationDecisionApplication, ModerationSubjectAdapterKey,
    ModerationSubjectAdapterRegistry, ModerationSubjectCommandPort,
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
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Moderation PostgreSQL dispatcher contract"
            );
            return Ok(None);
        };

        let control = connect(&database_url, "moderation_dispatch_contract_control").await?;
        let suffix = Uuid::new_v4().simple().to_string();
        let schema_name = format!("rustok_moderation_dispatch_{suffix}");
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;
        let migration = scoped_connection(
            &database_url,
            &schema_name,
            &format!("moderation_dispatch_migration_{suffix}"),
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

#[derive(Clone, Copy)]
enum AdapterBehavior {
    Success,
    UnavailableThenSuccess,
    Conflict,
    Validation,
    InvalidEvidence,
}

#[derive(Clone, Debug)]
struct AdapterCall {
    actor_kind: PortActorKind,
    actor_id: String,
    idempotency_key: Option<String>,
    causation_id: Option<String>,
    correlation_id: String,
    deadline_ms: Option<u64>,
    command: ApplyModerationDecisionCommand,
}

#[derive(Clone)]
struct RecordingAdapter {
    key: ModerationSubjectAdapterKey,
    behavior: AdapterBehavior,
    calls: Arc<Mutex<Vec<AdapterCall>>>,
}

impl RecordingAdapter {
    fn new(module: &str, kind: ModerationSubjectKind, behavior: AdapterBehavior) -> Self {
        Self {
            key: ModerationSubjectAdapterKey::new(module, kind).expect("valid test adapter key"),
            behavior,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn calls(&self) -> Vec<AdapterCall> {
        self.calls.lock().expect("adapter call lock").clone()
    }
}

#[async_trait::async_trait]
impl ModerationSubjectCommandPort for RecordingAdapter {
    fn key(&self) -> ModerationSubjectAdapterKey {
        self.key.clone()
    }

    async fn apply_moderation_decision(
        &self,
        context: PortContext,
        command: ApplyModerationDecisionCommand,
    ) -> Result<ModerationDecisionApplication, PortError> {
        let call_number = {
            let mut calls = self.calls.lock().expect("adapter call lock");
            calls.push(AdapterCall {
                actor_kind: context.actor.kind.clone(),
                actor_id: context.actor.id.clone(),
                idempotency_key: context.idempotency_key.clone(),
                causation_id: context.causation_id.clone(),
                correlation_id: context.correlation_id.clone(),
                deadline_ms: context.deadline_ms,
                command: command.clone(),
            });
            calls.len()
        };

        match self.behavior {
            AdapterBehavior::Success => Ok(success_application(&command)),
            AdapterBehavior::UnavailableThenSuccess if call_number == 1 => Err(
                PortError::unavailable("test.adapter_unavailable", "temporary test adapter outage"),
            ),
            AdapterBehavior::UnavailableThenSuccess => Ok(success_application(&command)),
            AdapterBehavior::Conflict => Err(PortError::conflict(
                "test.adapter_conflict",
                "test adapter observed a stale subject revision",
            )),
            AdapterBehavior::Validation => Err(PortError::validation(
                "test.adapter_validation",
                "test adapter rejected the command",
            )),
            AdapterBehavior::InvalidEvidence => {
                let mut subject = command.subject.clone();
                subject.revision += 1;
                Ok(ModerationDecisionApplication {
                    decision_id: command.decision_id,
                    applied_revision: subject.revision,
                    subject,
                    applied_at: Utc::now(),
                })
            }
        }
    }
}

fn success_application(command: &ApplyModerationDecisionCommand) -> ModerationDecisionApplication {
    ModerationDecisionApplication {
        decision_id: command.decision_id,
        subject: command.subject.clone(),
        applied_revision: command.subject.revision,
        applied_at: Utc::now(),
    }
}

#[tokio::test]
async fn postgres_dispatch_contract_preserves_exact_routing_cas_and_fail_closed_outcomes()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };

    let outcome = run_dispatch_contract(&database).await;
    let cleanup = database.cleanup().await;
    outcome?;
    cleanup
}

async fn run_dispatch_contract(database: &TestDatabase) -> TestResult<()> {
    concurrent_dispatch_calls_exact_adapter_once(database).await?;
    missing_exact_adapter_never_falls_back(database).await?;
    retryable_attempt_reuses_decision_idempotency_on_next_attempt(database).await?;
    adapter_errors_and_invalid_success_are_classified_fail_closed(database).await?;
    Ok(())
}

async fn concurrent_dispatch_calls_exact_adapter_once(database: &TestDatabase) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let seed = ModerationService::new(database.connection("dispatch_race_seed").await?);
    let (case, decision) =
        seed_decided_case(&seed, tenant_id, actor_id, 11, "dispatch-race").await?;

    let exact = RecordingAdapter::new(
        "forum",
        ModerationSubjectKind::ForumPost,
        AdapterBehavior::Success,
    );
    let wrong = RecordingAdapter::new(
        "forum",
        ModerationSubjectKind::ForumTopic,
        AdapterBehavior::Success,
    );
    let mut registry = ModerationSubjectAdapterRegistry::default();
    registry.register(wrong.clone())?;
    registry.register(exact.clone())?;

    let first_service = ModerationService::new(database.connection("dispatch_race_one").await?);
    let second_service = ModerationService::new(database.connection("dispatch_race_two").await?);
    let first = first_service.dispatch_application_operation_once(
        &registry,
        tenant_id,
        decision.id,
        "postgres-dispatch-one",
    );
    let second = second_service.dispatch_application_operation_once(
        &registry,
        tenant_id,
        decision.id,
        "postgres-dispatch-two",
    );
    let (first, second) = tokio::join!(first, second);

    let mut winners = Vec::new();
    let mut losers = 0;
    for result in [first?, second?] {
        match result {
            Some(operation) => winners.push(operation),
            None => losers += 1,
        }
    }
    assert_eq!(winners.len(), 1);
    assert_eq!(losers, 1);
    assert_eq!(
        winners[0].status,
        ModerationApplicationOperationStatus::Applied
    );
    assert_eq!(winners[0].attempt_count, 1);
    assert!(winners[0].lease_token.is_none());

    let exact_calls = exact.calls();
    assert_eq!(exact_calls.len(), 1);
    assert!(wrong.calls().is_empty());
    let call = &exact_calls[0];
    assert_eq!(call.actor_kind, PortActorKind::Service);
    assert_eq!(call.actor_id, "rustok-moderation");
    assert_eq!(
        call.idempotency_key.as_deref(),
        Some(decision.id.to_string().as_str())
    );
    assert_eq!(
        call.causation_id.as_deref(),
        Some(decision.id.to_string().as_str())
    );
    assert_eq!(
        call.deadline_ms,
        Some(APPLICATION_ADAPTER_DEADLINE_SECONDS * 1_000)
    );
    assert_eq!(call.command.decision_id, decision.id);
    assert_eq!(call.command.subject, case.subject);
    assert_eq!(call.command.decision_hash, decision.decision_hash);

    let closed = seed
        .get_case(tenant_id, case.id)
        .await?
        .ok_or_else(|| std::io::Error::other("dispatched moderation case is missing"))?;
    assert_eq!(closed.status, ModerationCaseStatus::Closed);

    let observer = database.connection("dispatch_race_observer").await?;
    assert_eq!(
        count_events(
            &observer,
            tenant_id,
            "application",
            decision.id,
            "application_attempt_claimed",
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

async fn missing_exact_adapter_never_falls_back(database: &TestDatabase) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let service = ModerationService::new(database.connection("dispatch_missing_owner").await?);
    let (case, decision) =
        seed_decided_case(&service, tenant_id, actor_id, 21, "dispatch-missing").await?;

    let wrong = RecordingAdapter::new(
        "forum",
        ModerationSubjectKind::ForumTopic,
        AdapterBehavior::Success,
    );
    let mut registry = ModerationSubjectAdapterRegistry::default();
    registry.register(wrong.clone())?;

    let operation = service
        .dispatch_application_operation_once(
            &registry,
            tenant_id,
            decision.id,
            "postgres-dispatch-missing",
        )
        .await?
        .ok_or_else(|| std::io::Error::other("missing-adapter operation was not claimed"))?;
    assert!(wrong.calls().is_empty());
    assert_eq!(
        operation.status,
        ModerationApplicationOperationStatus::Retryable
    );
    assert_eq!(
        operation.last_error_code.as_deref(),
        Some("moderation.application_adapter_missing")
    );
    assert_eq!(
        service
            .get_case(tenant_id, case.id)
            .await?
            .ok_or_else(|| std::io::Error::other("missing-adapter case disappeared"))?
            .status,
        ModerationCaseStatus::ApplyingDecision
    );
    Ok(())
}

async fn retryable_attempt_reuses_decision_idempotency_on_next_attempt(
    database: &TestDatabase,
) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let service = ModerationService::new(database.connection("dispatch_retry_owner").await?);
    let (case, decision) =
        seed_decided_case(&service, tenant_id, actor_id, 31, "dispatch-retry").await?;

    let adapter = RecordingAdapter::new(
        "forum",
        ModerationSubjectKind::ForumPost,
        AdapterBehavior::UnavailableThenSuccess,
    );
    let mut registry = ModerationSubjectAdapterRegistry::default();
    registry.register(adapter.clone())?;

    let first = service
        .dispatch_application_operation_once(
            &registry,
            tenant_id,
            decision.id,
            "postgres-dispatch-retry-one",
        )
        .await?
        .ok_or_else(|| std::io::Error::other("retry fixture was not claimed"))?;
    assert_eq!(
        first.status,
        ModerationApplicationOperationStatus::Retryable
    );
    assert_eq!(first.attempt_count, 1);
    assert_eq!(
        first.last_error_code.as_deref(),
        Some("test.adapter_unavailable")
    );

    let storage = database.connection("dispatch_retry_storage").await?;
    make_operation_due(&storage, tenant_id, decision.id).await?;

    let second = service
        .dispatch_application_operation_once(
            &registry,
            tenant_id,
            decision.id,
            "postgres-dispatch-retry-two",
        )
        .await?
        .ok_or_else(|| std::io::Error::other("retryable fixture was not claimed again"))?;
    assert_eq!(second.status, ModerationApplicationOperationStatus::Applied);
    assert_eq!(second.attempt_count, 2);

    let calls = adapter.calls();
    assert_eq!(calls.len(), 2);
    let expected_key = decision.id.to_string();
    assert_eq!(
        calls[0].idempotency_key.as_deref(),
        Some(expected_key.as_str())
    );
    assert_eq!(
        calls[1].idempotency_key.as_deref(),
        Some(expected_key.as_str())
    );
    assert_eq!(
        calls[0].causation_id.as_deref(),
        Some(expected_key.as_str())
    );
    assert_eq!(
        calls[1].causation_id.as_deref(),
        Some(expected_key.as_str())
    );
    assert_ne!(calls[0].correlation_id, calls[1].correlation_id);
    assert_eq!(calls[0].command, calls[1].command);

    assert_eq!(
        service
            .get_case(tenant_id, case.id)
            .await?
            .ok_or_else(|| std::io::Error::other("retried dispatch case disappeared"))?
            .status,
        ModerationCaseStatus::Closed
    );
    Ok(())
}

async fn adapter_errors_and_invalid_success_are_classified_fail_closed(
    database: &TestDatabase,
) -> TestResult<()> {
    let conflict =
        dispatch_behavior_fixture(database, AdapterBehavior::Conflict, 41, "dispatch-conflict")
            .await?;
    assert_eq!(
        conflict.operation_status,
        ModerationApplicationOperationStatus::OperatorReview
    );
    assert_eq!(conflict.case_status, ModerationCaseStatus::Escalated);
    assert_eq!(
        conflict.error_code.as_deref(),
        Some("test.adapter_conflict")
    );

    let validation = dispatch_behavior_fixture(
        database,
        AdapterBehavior::Validation,
        42,
        "dispatch-validation",
    )
    .await?;
    assert_eq!(
        validation.operation_status,
        ModerationApplicationOperationStatus::Rejected
    );
    assert_eq!(validation.case_status, ModerationCaseStatus::Escalated);
    assert_eq!(
        validation.error_code.as_deref(),
        Some("test.adapter_validation")
    );

    let invalid = dispatch_behavior_fixture(
        database,
        AdapterBehavior::InvalidEvidence,
        43,
        "dispatch-invalid-evidence",
    )
    .await?;
    assert_eq!(
        invalid.operation_status,
        ModerationApplicationOperationStatus::OperatorReview
    );
    assert_eq!(invalid.case_status, ModerationCaseStatus::Escalated);
    assert_eq!(
        invalid.error_code.as_deref(),
        Some("moderation.application_evidence_invalid")
    );
    Ok(())
}

struct DispatchFixtureOutcome {
    operation_status: ModerationApplicationOperationStatus,
    case_status: ModerationCaseStatus,
    error_code: Option<String>,
}

async fn dispatch_behavior_fixture(
    database: &TestDatabase,
    behavior: AdapterBehavior,
    subject_revision: i64,
    key_prefix: &str,
) -> TestResult<DispatchFixtureOutcome> {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let service = ModerationService::new(database.connection(key_prefix).await?);
    let (case, decision) =
        seed_decided_case(&service, tenant_id, actor_id, subject_revision, key_prefix).await?;
    let adapter = RecordingAdapter::new("forum", ModerationSubjectKind::ForumPost, behavior);
    let mut registry = ModerationSubjectAdapterRegistry::default();
    registry.register(adapter.clone())?;

    let operation = service
        .dispatch_application_operation_once(
            &registry,
            tenant_id,
            decision.id,
            format!("postgres-{key_prefix}"),
        )
        .await?
        .ok_or_else(|| std::io::Error::other("dispatch fixture was not claimed"))?;
    assert_eq!(adapter.calls().len(), 1);
    let stored_case = service
        .get_case(tenant_id, case.id)
        .await?
        .ok_or_else(|| std::io::Error::other("dispatch fixture case disappeared"))?;
    Ok(DispatchFixtureOutcome {
        operation_status: operation.status,
        case_status: stored_case.status,
        error_code: operation.last_error_code,
    })
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
                policy_snapshot: json!({"policy": "postgres-dispatch-contract", "version": 1}),
            },
        )
        .await?;
    let decided = service
        .get_case(tenant_id, case.id)
        .await?
        .ok_or_else(|| std::io::Error::other("decided dispatch case is missing"))?;
    assert_eq!(decided.status, ModerationCaseStatus::Decided);
    Ok((decided, decision))
}

fn write_context(tenant_id: Uuid, actor_id: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("postgres-moderation-dispatch-{key}"),
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
        metadata: json!({"source": "postgres-dispatch-contract"}),
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
        metadata: json!({"source": "postgres-dispatch-contract"}),
    }
}

async fn make_operation_due(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    decision_id: Uuid,
) -> TestResult<()> {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE moderation_application_operations SET next_attempt_at = NOW() - INTERVAL '1 second', updated_at = NOW() WHERE tenant_id = $1 AND decision_id = $2",
        vec![tenant_id.into(), decision_id.into()],
    ))
    .await?;
    Ok(())
}

async fn count_events(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    aggregate_kind: &str,
    aggregate_id: Uuid,
    event_type: &str,
) -> TestResult<i64> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS value FROM moderation_events WHERE tenant_id = $1 AND aggregate_kind = $2 AND aggregate_id = $3 AND event_type = $4",
            vec![
                tenant_id.into(),
                aggregate_kind.to_owned().into(),
                aggregate_id.into(),
                event_type.to_owned().into(),
            ],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("moderation event count returned no row"))?;
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
