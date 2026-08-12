use std::{env, error::Error, time::Duration};

use rustok_api::{PortActor, PortContext};
use rustok_moderation::{
    AssignModerationCaseCommand, DecideModerationCaseCommand, MAX_DUE_APPLICATION_OPERATIONS,
    ModerationApplicationOperationStatus, ModerationCasePriority, ModerationCaseRecord,
    ModerationCaseStatus, ModerationDecisionEffect, ModerationDecisionEffectAction,
    ModerationDecisionKind, ModerationDecisionRecord, ModerationError, ModerationReasonCode,
    ModerationReporterKind, ModerationScopeRef, ModerationService, ModerationSubjectKind,
    ModerationSubjectRef, OpenModerationCaseCommand, SubmitModerationReportCommand,
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
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Moderation PostgreSQL application-operation contract"
            );
            return Ok(None);
        };

        let control = connect(&database_url, "moderation_application_contract_control").await?;
        let suffix = Uuid::new_v4().simple().to_string();
        let schema_name = format!("rustok_moderation_application_{suffix}");
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let migration = scoped_connection(
            &database_url,
            &schema_name,
            &format!("moderation_application_migration_{suffix}"),
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
async fn postgres_application_operation_contract_preserves_due_claim_and_lease_fences()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };

    let outcome = run_application_operation_contract(&database).await;
    let cleanup = database.cleanup().await;
    outcome?;
    cleanup
}

async fn run_application_operation_contract(database: &TestDatabase) -> TestResult<()> {
    due_reads_are_ordered_and_bounded(database).await?;
    concurrent_claim_has_exactly_one_winner(database).await?;
    expired_lease_reclaims_without_second_case_revision_and_fences_stale_worker(database).await?;
    retryable_deadline_controls_due_visibility(database).await?;
    Ok(())
}

async fn due_reads_are_ordered_and_bounded(database: &TestDatabase) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let service = ModerationService::new(database.connection("application_due_owner").await?);
    let (_, oldest) = seed_decided_case(&service, tenant_id, actor_id, 11, "due-oldest").await?;
    let (_, newer) = seed_decided_case(&service, tenant_id, actor_id, 12, "due-newer").await?;
    let (_, future) = seed_decided_case(&service, tenant_id, actor_id, 13, "due-future").await?;
    let storage = database.connection("application_due_storage").await?;

    set_next_attempt_offset(&storage, tenant_id, oldest.id, -30).await?;
    set_next_attempt_offset(&storage, tenant_id, newer.id, -10).await?;
    set_next_attempt_offset(&storage, tenant_id, future.id, 30).await?;

    assert_eq!(MAX_DUE_APPLICATION_OPERATIONS, 100);
    let minimum_bounded = service
        .list_due_application_operations(tenant_id, 0)
        .await?;
    assert_eq!(minimum_bounded.len(), 1);
    assert_eq!(minimum_bounded[0].decision_id, oldest.id);

    let ordered = service
        .list_due_application_operations(tenant_id, 2)
        .await?;
    assert_eq!(ordered.len(), 2);
    assert_eq!(ordered[0].decision_id, oldest.id);
    assert_eq!(ordered[1].decision_id, newer.id);
    assert!(ordered.iter().all(|row| row.decision_id != future.id));

    let oversized = service
        .list_due_application_operations(tenant_id, MAX_DUE_APPLICATION_OPERATIONS + 1)
        .await?;
    assert_eq!(oversized.len(), 2);
    assert_eq!(oversized[0].decision_id, oldest.id);
    assert_eq!(oversized[1].decision_id, newer.id);
    Ok(())
}

async fn concurrent_claim_has_exactly_one_winner(database: &TestDatabase) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let seed = ModerationService::new(database.connection("application_claim_seed").await?);
    let (case, decision) = seed_decided_case(&seed, tenant_id, actor_id, 21, "claim-race").await?;

    let first_service = ModerationService::new(database.connection("application_claim_one").await?);
    let second_service =
        ModerationService::new(database.connection("application_claim_two").await?);
    let first =
        first_service.claim_application_operation(tenant_id, decision.id, "postgres-claim-one", 60);
    let second = second_service.claim_application_operation(
        tenant_id,
        decision.id,
        "postgres-claim-two",
        60,
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
    let winner = &winners[0];
    assert_eq!(
        winner.status,
        ModerationApplicationOperationStatus::Applying
    );
    assert_eq!(winner.attempt_count, 1);
    assert!(winner.lease_token.is_some());
    assert!(winner.lease_expires_at.is_some());
    assert!(matches!(
        winner.lease_owner.as_deref(),
        Some("postgres-claim-one") | Some("postgres-claim-two")
    ));

    let stored_case = seed
        .get_case(tenant_id, case.id)
        .await?
        .ok_or_else(|| std::io::Error::other("claimed moderation case is missing"))?;
    assert_eq!(stored_case.status, ModerationCaseStatus::ApplyingDecision);
    assert_eq!(stored_case.revision, case.revision + 1);

    let observer = database.connection("application_claim_observer").await?;
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
        count_events(
            &observer,
            tenant_id,
            "case",
            case.id,
            "case_application_started",
        )
        .await?,
        1
    );
    Ok(())
}

async fn expired_lease_reclaims_without_second_case_revision_and_fences_stale_worker(
    database: &TestDatabase,
) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let service = ModerationService::new(database.connection("application_reclaim_owner").await?);
    let (case, decision) =
        seed_decided_case(&service, tenant_id, actor_id, 31, "lease-reclaim").await?;

    let first = service
        .claim_application_operation(tenant_id, decision.id, "postgres-stale-worker", 60)
        .await?
        .ok_or_else(|| std::io::Error::other("pending application was not claimable"))?;
    let stale_token = first
        .lease_token
        .ok_or_else(|| std::io::Error::other("first application claim has no lease token"))?;
    assert_eq!(first.attempt_count, 1);

    let after_first_claim = service
        .get_case(tenant_id, case.id)
        .await?
        .ok_or_else(|| std::io::Error::other("case after first application claim is missing"))?;
    assert_eq!(
        after_first_claim.status,
        ModerationCaseStatus::ApplyingDecision
    );
    assert_eq!(after_first_claim.revision, case.revision + 1);

    let storage = database.connection("application_reclaim_storage").await?;
    storage
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE moderation_application_operations SET lease_expires_at = NOW() - INTERVAL '1 second' WHERE tenant_id = $1 AND decision_id = $2",
            vec![tenant_id.into(), decision.id.into()],
        ))
        .await?;

    let due_after_expiry = service
        .list_due_application_operations(tenant_id, 10)
        .await?;
    assert!(
        due_after_expiry
            .iter()
            .any(|operation| operation.decision_id == decision.id)
    );

    let reclaimer = ModerationService::new(database.connection("application_reclaimer").await?);
    let reclaimed = reclaimer
        .claim_application_operation(tenant_id, decision.id, "postgres-reclaimer", 60)
        .await?
        .ok_or_else(|| std::io::Error::other("expired application lease was not reclaimable"))?;
    let live_token = reclaimed
        .lease_token
        .ok_or_else(|| std::io::Error::other("reclaimed application has no lease token"))?;
    assert_ne!(live_token, stale_token);
    assert_eq!(
        reclaimed.status,
        ModerationApplicationOperationStatus::Applying
    );
    assert_eq!(reclaimed.attempt_count, 2);
    assert_eq!(reclaimed.lease_owner.as_deref(), Some("postgres-reclaimer"));

    let after_reclaim = reclaimer
        .get_case(tenant_id, case.id)
        .await?
        .ok_or_else(|| std::io::Error::other("case after application reclaim is missing"))?;
    assert_eq!(after_reclaim.status, ModerationCaseStatus::ApplyingDecision);
    assert_eq!(after_reclaim.revision, after_first_claim.revision);

    let stale_finish = service
        .mark_application_retryable(
            tenant_id,
            decision.id,
            stale_token,
            "stale_worker",
            "stale worker must not finish reclaimed attempt",
            5,
        )
        .await;
    assert!(matches!(
        stale_finish,
        Err(ModerationError::ApplicationLeaseConflict(id)) if id == decision.id
    ));

    let still_owned = reclaimer
        .get_application_operation(tenant_id, decision.id)
        .await?
        .ok_or_else(|| std::io::Error::other("reclaimed application operation disappeared"))?;
    assert_eq!(
        still_owned.status,
        ModerationApplicationOperationStatus::Applying
    );
    assert_eq!(still_owned.attempt_count, 2);
    assert_eq!(still_owned.lease_token, Some(live_token));
    assert_eq!(
        still_owned.lease_owner.as_deref(),
        Some("postgres-reclaimer")
    );

    let retryable = reclaimer
        .mark_application_retryable(
            tenant_id,
            decision.id,
            live_token,
            "retry_current_worker",
            "current reclaimed worker schedules retry",
            60,
        )
        .await?;
    assert_eq!(
        retryable.status,
        ModerationApplicationOperationStatus::Retryable
    );
    assert_eq!(retryable.attempt_count, 2);
    assert!(retryable.lease_token.is_none());
    assert!(retryable.lease_owner.is_none());
    assert!(retryable.lease_expires_at.is_none());
    assert_eq!(
        reclaimer
            .get_case(tenant_id, case.id)
            .await?
            .ok_or_else(|| std::io::Error::other("retryable application case is missing"))?
            .revision,
        after_first_claim.revision
    );
    Ok(())
}

async fn retryable_deadline_controls_due_visibility(database: &TestDatabase) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let service = ModerationService::new(database.connection("application_retry_owner").await?);
    let (_, decision) =
        seed_decided_case(&service, tenant_id, actor_id, 41, "retry-deadline").await?;

    let claimed = service
        .claim_application_operation(tenant_id, decision.id, "postgres-retry-worker", 60)
        .await?
        .ok_or_else(|| std::io::Error::other("retry fixture application was not claimable"))?;
    let token = claimed
        .lease_token
        .ok_or_else(|| std::io::Error::other("retry fixture claim has no lease token"))?;
    let retryable = service
        .mark_application_retryable(
            tenant_id,
            decision.id,
            token,
            "temporary_owner_outage",
            "retry after owner outage",
            60,
        )
        .await?;
    assert_eq!(
        retryable.status,
        ModerationApplicationOperationStatus::Retryable
    );

    let before_deadline = service
        .list_due_application_operations(tenant_id, 10)
        .await?;
    assert!(
        before_deadline
            .iter()
            .all(|operation| operation.decision_id != decision.id)
    );

    let storage = database.connection("application_retry_storage").await?;
    set_next_attempt_offset(&storage, tenant_id, decision.id, -1).await?;
    let after_deadline = service
        .list_due_application_operations(tenant_id, 10)
        .await?;
    assert!(
        after_deadline
            .iter()
            .any(|operation| operation.decision_id == decision.id)
    );
    Ok(())
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
                policy_snapshot: json!({"policy": "postgres-application-contract", "version": 1}),
            },
        )
        .await?;
    let decided = service
        .get_case(tenant_id, case.id)
        .await?
        .ok_or_else(|| std::io::Error::other("decided moderation case is missing"))?;
    assert_eq!(decided.status, ModerationCaseStatus::Decided);
    Ok((decided, decision))
}

fn write_context(tenant_id: Uuid, actor_id: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("postgres-moderation-application-{key}"),
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
        metadata: json!({"source": "postgres-application-contract"}),
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
        metadata: json!({"source": "postgres-application-contract"}),
    }
}

async fn set_next_attempt_offset(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    decision_id: Uuid,
    seconds: i64,
) -> TestResult<()> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE moderation_application_operations SET next_attempt_at = NOW() + ($3::bigint * INTERVAL '1 second'), updated_at = NOW() WHERE tenant_id = $1 AND decision_id = $2",
        vec![tenant_id.into(), decision_id.into(), seconds.into()],
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
        .query_one(Statement::from_sql_and_values(
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
