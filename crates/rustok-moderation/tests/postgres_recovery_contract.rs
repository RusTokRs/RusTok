use std::{env, error::Error, time::Duration};

use chrono::Utc;
use rustok_api::{PortActor, PortContext};
use rustok_moderation::{
    AssignModerationCaseCommand, DecideModerationCaseCommand, ModerationApplicationOperationStatus,
    ModerationCasePriority, ModerationCaseRecord, ModerationCaseStatus,
    ModerationDecisionApplication, ModerationDecisionEffect, ModerationDecisionEffectAction,
    ModerationDecisionKind, ModerationDecisionRecord, ModerationError, ModerationReasonCode,
    ModerationReporterKind, ModerationScopeRef, ModerationService, ModerationSubjectKind,
    ModerationSubjectRef, OpenModerationCaseCommand, ReconcileLegacyModerationApplicationCommand,
    RequeueModerationApplicationCommand, SubmitModerationReportCommand,
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
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Moderation PostgreSQL recovery contract"
            );
            return Ok(None);
        };

        let control = connect(&database_url, "moderation_recovery_control").await?;
        let suffix = Uuid::new_v4().simple().to_string();
        let schema_name = format!("rustok_moderation_recovery_{suffix}");
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;
        let migration = scoped_connection(
            &database_url,
            &schema_name,
            &format!("moderation_recovery_migration_{suffix}"),
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
async fn postgres_recovery_contract_preserves_requeue_denial_and_legacy_reconciliation()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };

    let outcome = run_recovery_contract(&database).await;
    let cleanup = database.cleanup().await;
    outcome?;
    cleanup
}

async fn run_recovery_contract(database: &TestDatabase) -> TestResult<()> {
    rejected_application_requeues_replay_safely_and_claims_again(database).await?;
    applied_application_cannot_be_requeued(database).await?;
    legacy_rejected_application_reconciles_to_escalated(database).await?;
    legacy_applied_application_reconciles_to_closed(database).await?;
    Ok(())
}

async fn rejected_application_requeues_replay_safely_and_claims_again(
    database: &TestDatabase,
) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let service = ModerationService::new(database.connection("recovery_requeue_owner").await?);
    let (case, decision) = seed_decided_case(&service, tenant_id, actor_id, 21, "requeue").await?;

    let claimed = service
        .claim_application_operation(tenant_id, decision.id, "postgres-requeue-first", 60)
        .await?
        .ok_or_else(|| std::io::Error::other("pending application was not claimable"))?;
    let lease_token = claimed
        .lease_token
        .ok_or_else(|| std::io::Error::other("claimed application has no lease token"))?;
    assert_eq!(
        claimed.status,
        ModerationApplicationOperationStatus::Applying
    );
    assert_eq!(claimed.attempt_count, 1);

    let rejected = service
        .mark_application_rejected(
            tenant_id,
            decision.id,
            lease_token,
            "postgres_rejected",
            "postgres recovery contract rejection",
        )
        .await?;
    assert_eq!(
        rejected.status,
        ModerationApplicationOperationStatus::Rejected
    );
    let escalated = service
        .get_case(tenant_id, case.id)
        .await?
        .ok_or_else(|| std::io::Error::other("rejected application case is missing"))?;
    assert_eq!(escalated.status, ModerationCaseStatus::Escalated);

    let command = RequeueModerationApplicationCommand {
        decision_id: decision.id,
        expected_case_revision: escalated.revision,
        reason: "operator confirmed same immutable decision should retry".to_string(),
    };
    let context = write_context(tenant_id, actor_id, "pg-recovery-requeue");
    let requeued = service
        .operator_requeue_application_replay_safe(context.clone(), command.clone())
        .await?;
    assert!(requeued.changed);
    assert_eq!(
        requeued.operation_status,
        ModerationApplicationOperationStatus::Retryable
    );
    assert_eq!(requeued.case_status, ModerationCaseStatus::ApplyingDecision);

    let replay = service
        .operator_requeue_application_replay_safe(context.clone(), command.clone())
        .await?;
    assert_eq!(replay, requeued);
    let changed_request = service
        .operator_requeue_application_replay_safe(
            context,
            RequeueModerationApplicationCommand {
                reason: "changed request under the same receipt key".to_string(),
                ..command
            },
        )
        .await;
    assert!(matches!(
        changed_request,
        Err(ModerationError::IdempotencyConflict)
    ));

    let second_claim = service
        .claim_application_operation(tenant_id, decision.id, "postgres-requeue-second", 60)
        .await?
        .ok_or_else(|| std::io::Error::other("requeued application was not claimable"))?;
    assert_eq!(
        second_claim.status,
        ModerationApplicationOperationStatus::Applying
    );
    assert_eq!(second_claim.attempt_count, 2);
    let applying_case = service
        .get_case(tenant_id, case.id)
        .await?
        .ok_or_else(|| std::io::Error::other("requeued application case is missing"))?;
    assert_eq!(applying_case.status, ModerationCaseStatus::ApplyingDecision);
    assert_eq!(applying_case.revision, requeued.case_revision);
    Ok(())
}

async fn applied_application_cannot_be_requeued(database: &TestDatabase) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let service = ModerationService::new(database.connection("recovery_applied_owner").await?);
    let (case, decision) = seed_decided_case(&service, tenant_id, actor_id, 31, "applied").await?;

    let claimed = service
        .claim_application_operation(tenant_id, decision.id, "postgres-applied", 60)
        .await?
        .ok_or_else(|| std::io::Error::other("application was not claimable"))?;
    let lease_token = claimed
        .lease_token
        .ok_or_else(|| std::io::Error::other("claimed application has no lease token"))?;
    let applied = service
        .mark_application_applied(
            tenant_id,
            decision.id,
            lease_token,
            ModerationDecisionApplication {
                decision_id: decision.id,
                subject: case.subject.clone(),
                applied_revision: case.subject.revision,
                applied_at: Utc::now(),
            },
        )
        .await?;
    assert_eq!(
        applied.status,
        ModerationApplicationOperationStatus::Applied
    );
    let closed = service
        .get_case(tenant_id, case.id)
        .await?
        .ok_or_else(|| std::io::Error::other("applied application case is missing"))?;
    assert_eq!(closed.status, ModerationCaseStatus::Closed);

    let result = service
        .operator_requeue_application_replay_safe(
            write_context(tenant_id, actor_id, "pg-recovery-applied-denial"),
            RequeueModerationApplicationCommand {
                decision_id: decision.id,
                expected_case_revision: closed.revision,
                reason: "must remain applied".to_string(),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(ModerationError::Validation(message)) if message.contains("applied decisions must never be requeued")
    ));
    assert_eq!(
        service
            .get_application_operation(tenant_id, decision.id)
            .await?
            .ok_or_else(|| std::io::Error::other("applied operation disappeared"))?
            .status,
        ModerationApplicationOperationStatus::Applied
    );
    assert_eq!(
        service
            .get_case(tenant_id, case.id)
            .await?
            .ok_or_else(|| std::io::Error::other("closed case disappeared"))?
            .status,
        ModerationCaseStatus::Closed
    );
    Ok(())
}

async fn legacy_rejected_application_reconciles_to_escalated(
    database: &TestDatabase,
) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let service = ModerationService::new(database.connection("legacy_rejected_owner").await?);
    let (case, decision) =
        seed_decided_case(&service, tenant_id, actor_id, 41, "legacy-rejected").await?;
    let storage = database.connection("legacy_rejected_storage").await?;
    storage
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE moderation_application_operations SET status = 'rejected', last_error_code = 'legacy_rejected', last_error_message = 'legacy terminal row', updated_at = NOW() WHERE tenant_id = $1 AND decision_id = $2",
            vec![tenant_id.into(), decision.id.into()],
        ))
        .await?;

    let reconciled = service
        .operator_reconcile_legacy_application_replay_safe(
            write_context(tenant_id, actor_id, "pg-reconcile-legacy-rejected"),
            ReconcileLegacyModerationApplicationCommand {
                decision_id: decision.id,
                expected_case_revision: case.revision,
                reason: "align legacy rejected terminal row".to_string(),
            },
        )
        .await?;
    assert!(reconciled.changed);
    assert_eq!(
        reconciled.operation_status,
        ModerationApplicationOperationStatus::Rejected
    );
    assert_eq!(reconciled.case_status, ModerationCaseStatus::Escalated);

    let no_op = service
        .operator_reconcile_legacy_application_replay_safe(
            write_context(tenant_id, actor_id, "pg-reconcile-legacy-rejected-noop"),
            ReconcileLegacyModerationApplicationCommand {
                decision_id: decision.id,
                expected_case_revision: reconciled.case_revision,
                reason: "confirm already reconciled rejected row".to_string(),
            },
        )
        .await?;
    assert!(!no_op.changed);
    assert_eq!(no_op.case_revision, reconciled.case_revision);
    Ok(())
}

async fn legacy_applied_application_reconciles_to_closed(
    database: &TestDatabase,
) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let service = ModerationService::new(database.connection("legacy_applied_owner").await?);
    let (case, decision) =
        seed_decided_case(&service, tenant_id, actor_id, 51, "legacy-applied").await?;
    let storage = database.connection("legacy_applied_storage").await?;
    storage
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE moderation_application_operations SET status = 'applied', applied_revision = subject_revision, applied_at = NOW(), updated_at = NOW() WHERE tenant_id = $1 AND decision_id = $2",
            vec![tenant_id.into(), decision.id.into()],
        ))
        .await?;

    let reconciled = service
        .operator_reconcile_legacy_application_replay_safe(
            write_context(tenant_id, actor_id, "pg-reconcile-legacy-applied"),
            ReconcileLegacyModerationApplicationCommand {
                decision_id: decision.id,
                expected_case_revision: case.revision,
                reason: "align legacy applied terminal row".to_string(),
            },
        )
        .await?;
    assert!(reconciled.changed);
    assert_eq!(
        reconciled.operation_status,
        ModerationApplicationOperationStatus::Applied
    );
    assert_eq!(reconciled.case_status, ModerationCaseStatus::Closed);

    let row = storage
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT closed_at IS NOT NULL AS has_closed_at, active_deduplication_key IS NULL AS released_active_key FROM moderation_cases WHERE tenant_id = $1 AND id = $2",
            vec![tenant_id.into(), case.id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("reconciled legacy applied case is missing"))?;
    assert!(row.try_get::<bool>("", "has_closed_at")?);
    assert!(row.try_get::<bool>("", "released_active_key")?);
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
                policy_snapshot: json!({"policy": "postgres-recovery-contract", "version": 1}),
            },
        )
        .await?;
    let decided = service
        .get_case(tenant_id, case.id)
        .await?
        .ok_or_else(|| std::io::Error::other("decided case is missing"))?;
    assert_eq!(decided.status, ModerationCaseStatus::Decided);
    Ok((decided, decision))
}

fn write_context(tenant_id: Uuid, actor_id: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        format!("postgres-moderation-recovery-{key}"),
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
        metadata: json!({"source": "postgres-recovery-contract"}),
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
        metadata: json!({"source": "postgres-recovery-contract"}),
    }
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
