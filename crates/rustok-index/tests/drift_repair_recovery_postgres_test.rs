#[path = "support/drift_repair.rs"]
mod support;

use std::sync::Arc;

use async_trait::async_trait;
use rustok_index::{
    IndexDriftAuthorizedRepairCommand, IndexDriftRepairEvidence, IndexDriftRepairEvidenceReader,
    IndexDriftRepairFailure, IndexDriftRepairFinding, IndexDriftRepairNotStartedReason,
    IndexDriftRepairOutcome, IndexDriftRepairRecoveryAction, IndexDriftRepairRecoveryOutcome,
};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use uuid::Uuid;

use support::{
    FixtureRuntime, TestDatabase, TestResult, count_recovery_decisions, count_repair_commands,
    create_missing_finding, missing_owner, payload_digest, recovery_command, recovery_service,
    recovery_store, repair_command, repair_service, table_exists,
};

#[derive(Clone)]
struct StopBeforeEvidence;

#[async_trait]
impl IndexDriftRepairEvidenceReader for StopBeforeEvidence {
    async fn capture_before(
        &self,
        _authorized: &IndexDriftAuthorizedRepairCommand,
        _finding: &IndexDriftRepairFinding,
    ) -> Result<IndexDriftRepairEvidence, IndexDriftRepairFailure> {
        Err(
            IndexDriftRepairFailure::retryable("repair_evidence_before_stop")
                .expect("static failure code"),
        )
    }

    async fn capture_after(
        &self,
        _authorized: &IndexDriftAuthorizedRepairCommand,
        _finding: &IndexDriftRepairFinding,
        _before: &IndexDriftRepairEvidence,
    ) -> Result<IndexDriftRepairEvidence, IndexDriftRepairFailure> {
        unreachable!("before evidence always stops the fixture")
    }
}

#[tokio::test]
async fn migrations_recovery_guard_and_concurrent_reservation_are_executable() -> TestResult<()> {
    let Some(database) = TestDatabase::setup("repair_recovery").await? else {
        return Ok(());
    };
    let runtime = FixtureRuntime::setup(&database).await?;

    let key = runtime.missing_key(database.tenant_id, Uuid::new_v4());
    support::apply_record(
        &database,
        &runtime,
        runtime.missing_record(key.clone(), 7),
        "repair-evidence-missing-seed",
    )
    .await?;
    runtime.authority.clear_mutation(&key);
    runtime.authority.set_absence(key.clone(), 9);
    let (finding_id, target) = create_missing_finding(&database, &runtime, key, 7, 9).await?;

    let command_a = repair_command(
        database.tenant_id,
        finding_id,
        Uuid::new_v4(),
        target.clone(),
        "reserve command a",
    )?;
    let command_b = repair_command(
        database.tenant_id,
        finding_id,
        Uuid::new_v4(),
        target.clone(),
        "reserve command b",
    )?;

    let service_a = repair_service(
        Arc::new(StopBeforeEvidence),
        missing_owner(&database, &runtime).await?,
        recovery_store(&database).await?,
    )?;
    let service_b = repair_service(
        Arc::new(StopBeforeEvidence),
        missing_owner(&database, &runtime).await?,
        recovery_store(&database).await?,
    )?;

    let (outcome_a, outcome_b) =
        tokio::join!(service_a.execute(&command_a), service_b.execute(&command_b),);
    assert_eq!(
        [&outcome_a, &outcome_b]
            .into_iter()
            .filter(|value| matches!(
                value,
                Err(error) if error.code() == "repair_evidence_before_stop"
            ))
            .count(),
        1,
    );
    assert_eq!(
        [&outcome_a, &outcome_b]
            .into_iter()
            .filter(|value| matches!(
                value,
                Ok(IndexDriftRepairOutcome::NotStarted(
                    IndexDriftRepairNotStartedReason::FindingBusy
                ))
            ))
            .count(),
        1,
    );
    assert_eq!(count_repair_commands(&database, finding_id).await?, 1);

    let winner_command_id = reserved_command_id(&database, finding_id).await?;
    assert_eq!(
        count_recovery_decisions(&database, winner_command_id).await?,
        1
    );

    let conflicting = repair_command(
        database.tenant_id,
        finding_id,
        winner_command_id,
        target,
        "changed command payload",
    )?;
    let conflict_service = repair_service(
        Arc::new(StopBeforeEvidence),
        missing_owner(&database, &runtime).await?,
        recovery_store(&database).await?,
    )?;
    let conflict = conflict_service
        .execute(&conflicting)
        .await
        .expect_err("command UUID payload reuse must fail");
    assert_eq!(conflict.code(), "index_drift_repair_command_id_conflict");

    let payload = payload_digest(&database, winner_command_id).await?;
    let recovery = recovery_service(&database).await?;
    let pause_decision = Uuid::new_v4();
    let pause = recovery_command(
        database.tenant_id,
        finding_id,
        winner_command_id,
        payload.clone(),
        pause_decision,
        Some(0),
        IndexDriftRepairRecoveryAction::Pause,
        "pause before completion",
    )?;
    let paused = recovery.execute(&pause).await?;
    assert!(matches!(
        paused,
        IndexDriftRepairRecoveryOutcome::Applied(ref receipt)
            if receipt.revision() == 1
    ));

    let duplicate = recovery.execute(&pause).await?;
    assert!(matches!(
        duplicate,
        IndexDriftRepairRecoveryOutcome::AlreadyApplied(ref receipt)
            if receipt.decision_id() == pause_decision && receipt.revision() == 1
    ));

    let stale = recovery_command(
        database.tenant_id,
        finding_id,
        winner_command_id,
        payload.clone(),
        Uuid::new_v4(),
        Some(0),
        IndexDriftRepairRecoveryAction::Resume,
        "stale resume",
    )?;
    assert!(matches!(
        recovery.execute(&stale).await?,
        IndexDriftRepairRecoveryOutcome::StaleRevision {
            current_revision: Some(1)
        }
    ));

    assert!(
        force_complete_repair(&database, winner_command_id)
            .await
            .is_err(),
        "completion trigger must reject a paused command",
    );

    let resume = recovery_command(
        database.tenant_id,
        finding_id,
        winner_command_id,
        payload,
        Uuid::new_v4(),
        Some(1),
        IndexDriftRepairRecoveryAction::Resume,
        "resume for guarded completion",
    )?;
    assert!(matches!(
        recovery.execute(&resume).await?,
        IndexDriftRepairRecoveryOutcome::Applied(ref receipt)
            if receipt.revision() == 2
    ));
    assert_eq!(
        force_complete_repair(&database, winner_command_id).await?,
        1
    );
    assert!(
        mutate_completed_command(&database, winner_command_id)
            .await
            .is_err(),
        "completed repair identity must remain immutable",
    );

    database.migrate_down().await?;
    assert!(
        !table_exists(
            &database,
            "index_consistency_finding_repair_recovery_decisions"
        )
        .await?
    );
    assert!(!table_exists(&database, "index_consistency_finding_repair_commands").await?);
    database.cleanup().await
}

async fn reserved_command_id(database: &TestDatabase, finding_id: Uuid) -> TestResult<Uuid> {
    let db = database.connection().await?;
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT command_id FROM index_consistency_finding_repair_commands WHERE tenant_id = $1 AND finding_id = $2",
            vec![database.tenant_id.into(), finding_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("reserved command is missing"))?;
    Ok(row.try_get("", "command_id")?)
}

async fn force_complete_repair(database: &TestDatabase, command_id: Uuid) -> TestResult<u64> {
    let db = database.connection().await?;
    let updated = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE index_consistency_finding_repair_commands SET state = 'completed', outcome = 'repaired', owner_name = 'repair_evidence_owner', before_digest = $3, after_digest = $4, owner_receipt_digest = $5, completed_at = CURRENT_TIMESTAMP WHERE tenant_id = $1 AND command_id = $2 AND state = 'prepared'",
            vec![
                database.tenant_id.into(),
                command_id.into(),
                "a".repeat(64).into(),
                "b".repeat(64).into(),
                "c".repeat(64).into(),
            ],
        ))
        .await?;
    Ok(updated.rows_affected())
}

async fn mutate_completed_command(database: &TestDatabase, command_id: Uuid) -> TestResult<()> {
    let db = database.connection().await?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE index_consistency_finding_repair_commands SET reason = reason || '-changed' WHERE tenant_id = $1 AND command_id = $2",
        vec![database.tenant_id.into(), command_id.into()],
    ))
    .await?;
    Ok(())
}
