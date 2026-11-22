#[path = "support/drift_repair.rs"]
mod support;

use std::sync::Arc;

use async_trait::async_trait;
use rustok_index::{
    IndexDriftAuthorizedRepairCommand, IndexDriftRepairEvidence, IndexDriftRepairEvidenceReader,
    IndexDriftRepairFailure, IndexDriftRepairFinding, IndexDriftRepairOutcome,
    IndexDriftRepairReceiptOutcome, IndexDriftRepairRecoveryAction,
    IndexDriftRepairRecoveryOutcome, IndexMutation, MutationApplyOutcome, MutationDelivery,
    PostgresMutationStore,
};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use tokio::sync::Barrier;
use uuid::Uuid;

use support::{
    FixtureRuntime, MISSING_DELIVERY_SOURCE, ORPHAN_DELIVERY_SOURCE, TestDatabase, TestResult,
    apply_record, create_missing_finding, create_orphan_finding, entity_state, exact_link_count,
    inbox_state, missing_evidence, missing_owner, orphan_evidence, orphan_owner, payload_digest,
    recovery_command, recovery_service, recovery_store, repair_command, repair_command_state,
    repair_service, replace_materialized_link_target,
};

#[derive(Clone)]
struct FailAfterOwnerEvidence {
    inner: Arc<dyn IndexDriftRepairEvidenceReader>,
}

#[async_trait]
impl IndexDriftRepairEvidenceReader for FailAfterOwnerEvidence {
    async fn capture_before(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
        finding: &IndexDriftRepairFinding,
    ) -> Result<IndexDriftRepairEvidence, IndexDriftRepairFailure> {
        self.inner.capture_before(authorized, finding).await
    }

    async fn capture_after(
        &self,
        _authorized: &IndexDriftAuthorizedRepairCommand,
        _finding: &IndexDriftRepairFinding,
        _before: &IndexDriftRepairEvidence,
    ) -> Result<IndexDriftRepairEvidence, IndexDriftRepairFailure> {
        Err(
            IndexDriftRepairFailure::retryable("repair_evidence_after_commit_crash")
                .expect("static failure code: invariant validated at startup"),
        )
    }
}

#[derive(Clone)]
struct GateBeforeOwnerEvidence {
    inner: Arc<dyn IndexDriftRepairEvidenceReader>,
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
}

#[async_trait]
impl IndexDriftRepairEvidenceReader for GateBeforeOwnerEvidence {
    async fn capture_before(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
        finding: &IndexDriftRepairFinding,
    ) -> Result<IndexDriftRepairEvidence, IndexDriftRepairFailure> {
        let evidence = self.inner.capture_before(authorized, finding).await?;
        self.entered.wait().await;
        self.release.wait().await;
        Ok(evidence)
    }

    async fn capture_after(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
        finding: &IndexDriftRepairFinding,
        before: &IndexDriftRepairEvidence,
    ) -> Result<IndexDriftRepairEvidence, IndexDriftRepairFailure> {
        self.inner.capture_after(authorized, finding, before).await
    }
}

#[derive(Clone)]
struct GateAfterOwnerEvidence {
    inner: Arc<dyn IndexDriftRepairEvidenceReader>,
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
}

#[async_trait]
impl IndexDriftRepairEvidenceReader for GateAfterOwnerEvidence {
    async fn capture_before(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
        finding: &IndexDriftRepairFinding,
    ) -> Result<IndexDriftRepairEvidence, IndexDriftRepairFailure> {
        self.inner.capture_before(authorized, finding).await
    }

    async fn capture_after(
        &self,
        authorized: &IndexDriftAuthorizedRepairCommand,
        finding: &IndexDriftRepairFinding,
        before: &IndexDriftRepairEvidence,
    ) -> Result<IndexDriftRepairEvidence, IndexDriftRepairFailure> {
        let evidence = self
            .inner
            .capture_after(authorized, finding, before)
            .await?;
        self.entered.wait().await;
        self.release.wait().await;
        Ok(evidence)
    }
}

#[tokio::test]
async fn missing_and_orphan_crash_windows_resume_exactly() -> TestResult<()> {
    let Some(database) = TestDatabase::setup("repair_crash_windows").await? else {
        return Ok(());
    };
    let runtime = FixtureRuntime::setup(&database).await?;

    let missing_key = runtime.missing_key(database.tenant_id, Uuid::new_v4());
    apply_record(
        &database,
        &runtime,
        runtime.missing_record(missing_key.clone(), 7),
        "repair-evidence-missing-crash-seed",
    )
    .await?;
    runtime.authority.clear_mutation(&missing_key).await;
    runtime.authority.set_absence(missing_key.clone(), 9).await;
    let (missing_finding, missing_target) =
        create_missing_finding(&database, &runtime, missing_key.clone(), 7, 9).await?;
    let missing_command_id = Uuid::new_v4();
    let missing_command = repair_command(
        database.tenant_id,
        missing_finding,
        missing_command_id,
        missing_target,
        "missing crash window",
    )?;

    let missing_crash = repair_service(
        Arc::new(FailAfterOwnerEvidence {
            inner: missing_evidence(&database, &runtime).await?,
        }),
        missing_owner(&database, &runtime).await?,
        recovery_store(&database).await?,
    )?;
    let error = missing_crash
        .execute(&missing_command)
        .await
        .expect_err("fixture must stop after the owner commit");
    assert_eq!(error.code(), "repair_evidence_after_commit_crash");
    assert_eq!(
        entity_state(&database, &missing_key).await?,
        Some((9, true))
    );
    assert_eq!(
        inbox_state(&database, MISSING_DELIVERY_SOURCE, missing_command_id).await?,
        Some("applied".to_owned())
    );
    assert_eq!(
        repair_command_state(&database, missing_command_id).await?,
        "prepared"
    );

    let missing_retry = repair_service(
        missing_evidence(&database, &runtime).await?,
        missing_owner(&database, &runtime).await?,
        recovery_store(&database).await?,
    )?;
    assert!(matches!(
        missing_retry.execute(&missing_command).await?,
        IndexDriftRepairOutcome::Repaired(_)
    ));
    assert!(matches!(
        missing_retry.execute(&missing_command).await?,
        IndexDriftRepairOutcome::AlreadyCompleted(_)
    ));

    let source_key = runtime.source_key(database.tenant_id, Uuid::new_v4());
    let target_zero = runtime.linked_target(Uuid::new_v4());
    let target_one = runtime.linked_target(Uuid::new_v4());
    apply_record(
        &database,
        &runtime,
        runtime.source_record(
            source_key.clone(),
            7,
            vec![target_zero.clone(), target_one.clone()],
        ),
        "repair-evidence-orphan-crash-seed",
    )
    .await?;
    runtime
        .authority
        .set_absence(
            runtime.target_key(database.tenant_id, target_zero.entity_id),
            9,
        )
        .await;
    let (orphan_finding, orphan_target) = create_orphan_finding(
        &database,
        &runtime,
        source_key.clone(),
        7,
        0,
        target_zero.clone(),
        9,
    )
    .await?;
    let orphan_command_id = Uuid::new_v4();
    let orphan_command = repair_command(
        database.tenant_id,
        orphan_finding,
        orphan_command_id,
        orphan_target,
        "orphan crash window",
    )?;

    let orphan_crash = repair_service(
        Arc::new(FailAfterOwnerEvidence {
            inner: orphan_evidence(&database, &runtime).await?,
        }),
        orphan_owner(&database, &runtime).await?,
        recovery_store(&database).await?,
    )?;
    let error = orphan_crash
        .execute(&orphan_command)
        .await
        .expect_err("fixture must stop after exact edge commit");
    assert_eq!(error.code(), "repair_evidence_after_commit_crash");
    assert_eq!(
        entity_state(&database, &source_key).await?,
        Some((7, false))
    );
    assert_eq!(
        exact_link_count(
            &database,
            &source_key,
            7,
            &runtime.contracts.link_name,
            0,
            &target_zero,
        )
        .await?,
        0
    );
    assert_eq!(
        exact_link_count(
            &database,
            &source_key,
            7,
            &runtime.contracts.link_name,
            1,
            &target_one,
        )
        .await?,
        1
    );
    assert_eq!(
        inbox_state(&database, ORPHAN_DELIVERY_SOURCE, orphan_command_id).await?,
        Some("applied".to_owned())
    );
    assert_eq!(
        repair_command_state(&database, orphan_command_id).await?,
        "prepared"
    );

    let orphan_retry = repair_service(
        orphan_evidence(&database, &runtime).await?,
        orphan_owner(&database, &runtime).await?,
        recovery_store(&database).await?,
    )?;
    assert!(matches!(
        orphan_retry.execute(&orphan_command).await?,
        IndexDriftRepairOutcome::Repaired(_)
    ));
    assert!(matches!(
        orphan_retry.execute(&orphan_command).await?,
        IndexDriftRepairOutcome::AlreadyCompleted(_)
    ));

    database.cleanup().await
}

#[tokio::test]
async fn recovery_admission_fences_side_effect_and_completion() -> TestResult<()> {
    let Some(database) = TestDatabase::setup("repair_recovery_fences").await? else {
        return Ok(());
    };
    let runtime = FixtureRuntime::setup(&database).await?;

    let before_key = runtime.missing_key(database.tenant_id, Uuid::new_v4());
    apply_record(
        &database,
        &runtime,
        runtime.missing_record(before_key.clone(), 7),
        "repair-evidence-before-gate-seed",
    )
    .await?;
    runtime.authority.clear_mutation(&before_key).await;
    runtime.authority.set_absence(before_key.clone(), 9).await;
    let (before_finding, before_target) =
        create_missing_finding(&database, &runtime, before_key.clone(), 7, 9).await?;
    let before_command_id = Uuid::new_v4();
    let before_command = repair_command(
        database.tenant_id,
        before_finding,
        before_command_id,
        before_target,
        "pause before owner",
    )?;

    let before_entered = Arc::new(Barrier::new(2));
    let before_release = Arc::new(Barrier::new(2));
    let before_service = repair_service(
        Arc::new(GateBeforeOwnerEvidence {
            inner: missing_evidence(&database, &runtime).await?,
            entered: before_entered.clone(),
            release: before_release.clone(),
        }),
        missing_owner(&database, &runtime).await?,
        recovery_store(&database).await?,
    )?;
    let before_task = tokio::spawn({
        let service = before_service.clone();
        let command = before_command.clone();
        async move { service.execute(&command).await }
    });
    before_entered.wait().await;

    let before_payload = payload_digest(&database, before_command_id).await?;
    let recovery = recovery_service(&database).await?;
    let pause = recovery_command(
        database.tenant_id,
        before_finding,
        before_command_id,
        before_payload.clone(),
        Uuid::new_v4(),
        Some(0),
        IndexDriftRepairRecoveryAction::Pause,
        "pause wins before owner",
    )?;
    assert!(matches!(
        recovery.execute(&pause).await?,
        IndexDriftRepairRecoveryOutcome::Applied(_)
    ));
    before_release.wait().await;
    let before_result = before_task.await?;
    let before_error = before_result.expect_err("paused owner admission must fail closed");
    assert_eq!(before_error.code(), "index_drift_repair_recovery_paused");
    assert_eq!(
        entity_state(&database, &before_key).await?,
        Some((7, false))
    );
    assert_eq!(
        inbox_state(&database, MISSING_DELIVERY_SOURCE, before_command_id).await?,
        None
    );

    let resume = recovery_command(
        database.tenant_id,
        before_finding,
        before_command_id,
        before_payload,
        Uuid::new_v4(),
        Some(1),
        IndexDriftRepairRecoveryAction::Resume,
        "resume exact command",
    )?;
    assert!(matches!(
        recovery.execute(&resume).await?,
        IndexDriftRepairRecoveryOutcome::Applied(_)
    ));
    let resumed_service = repair_service(
        missing_evidence(&database, &runtime).await?,
        missing_owner(&database, &runtime).await?,
        recovery_store(&database).await?,
    )?;
    assert!(matches!(
        resumed_service.execute(&before_command).await?,
        IndexDriftRepairOutcome::Repaired(_)
    ));

    let after_key = runtime.missing_key(database.tenant_id, Uuid::new_v4());
    apply_record(
        &database,
        &runtime,
        runtime.missing_record(after_key.clone(), 11),
        "repair-evidence-after-gate-seed",
    )
    .await?;
    runtime.authority.clear_mutation(&after_key).await;
    runtime.authority.set_absence(after_key.clone(), 13).await;
    let (after_finding, after_target) =
        create_missing_finding(&database, &runtime, after_key.clone(), 11, 13).await?;
    let after_command_id = Uuid::new_v4();
    let after_command = repair_command(
        database.tenant_id,
        after_finding,
        after_command_id,
        after_target,
        "abandon after owner",
    )?;

    let after_entered = Arc::new(Barrier::new(2));
    let after_release = Arc::new(Barrier::new(2));
    let after_service = repair_service(
        Arc::new(GateAfterOwnerEvidence {
            inner: missing_evidence(&database, &runtime).await?,
            entered: after_entered.clone(),
            release: after_release.clone(),
        }),
        missing_owner(&database, &runtime).await?,
        recovery_store(&database).await?,
    )?;
    let after_task = tokio::spawn({
        let service = after_service.clone();
        let command = after_command.clone();
        async move { service.execute(&command).await }
    });
    after_entered.wait().await;

    assert_eq!(entity_state(&database, &after_key).await?, Some((13, true)));
    assert_eq!(
        inbox_state(&database, MISSING_DELIVERY_SOURCE, after_command_id).await?,
        Some("applied".to_owned())
    );
    let after_payload = payload_digest(&database, after_command_id).await?;
    let abandon = recovery_command(
        database.tenant_id,
        after_finding,
        after_command_id,
        after_payload,
        Uuid::new_v4(),
        Some(0),
        IndexDriftRepairRecoveryAction::Abandon,
        "abandon wins before completion",
    )?;
    assert!(matches!(
        recovery.execute(&abandon).await?,
        IndexDriftRepairRecoveryOutcome::Applied(_)
    ));
    after_release.wait().await;
    let after_result = after_task.await?;
    let after_error = after_result.expect_err("completion must fail after terminal abandon");
    assert_eq!(after_error.code(), "index_drift_repair_recovery_abandoned");
    assert_eq!(
        repair_command_state(&database, after_command_id).await?,
        "prepared"
    );

    let abandoned_retry = repair_service(
        missing_evidence(&database, &runtime).await?,
        missing_owner(&database, &runtime).await?,
        recovery_store(&database).await?,
    )?;
    let retry_error = abandoned_retry
        .execute(&after_command)
        .await
        .expect_err("abandoned repair must remain terminally fenced");
    assert_eq!(retry_error.code(), "index_drift_repair_recovery_abandoned");
    assert!(
        force_complete_repair(&database, after_command_id)
            .await
            .is_err(),
        "database completion trigger must reject abandoned state",
    );

    database.cleanup().await
}

#[tokio::test]
async fn orphan_commitments_and_normal_mutations_fail_closed() -> TestResult<()> {
    let Some(database) = TestDatabase::setup("orphan_commitments").await? else {
        return Ok(());
    };
    let runtime = FixtureRuntime::setup(&database).await?;

    let moved = prepare_orphan_case(&database, &runtime, "source-moved").await?;
    apply_record(
        &database,
        &runtime,
        runtime.source_record(moved.source_key.clone(), 8, vec![moved.target.clone()]),
        "repair-evidence-source-moved",
    )
    .await?;
    assert_orphan_not_repaired(&database, &runtime, &moved).await?;

    let substituted = prepare_orphan_case(&database, &runtime, "link-substituted").await?;
    let replacement = runtime.linked_target(Uuid::new_v4());
    replace_materialized_link_target(
        &database,
        &substituted.source_key,
        7,
        &runtime.contracts.link_name,
        0,
        &replacement,
    )
    .await?;
    runtime
        .authority
        .set_mutation(IndexMutation::Upsert {
            event_id: Uuid::new_v4(),
            record: runtime.source_record(substituted.source_key.clone(), 7, vec![replacement]),
        })
        .await;
    assert_orphan_not_repaired(&database, &runtime, &substituted).await?;

    let restored = prepare_orphan_case(&database, &runtime, "target-restored").await?;
    let restored_key = runtime.target_key(database.tenant_id, restored.target.entity_id);
    runtime
        .authority
        .set_mutation(IndexMutation::Upsert {
            event_id: Uuid::new_v4(),
            record: runtime.target_record(restored_key.clone(), 10),
        })
        .await;
    runtime.authority.clear_absence(&restored_key).await;
    assert_orphan_not_repaired(&database, &runtime, &restored).await?;

    let absence_moved = prepare_orphan_case(&database, &runtime, "absence-moved").await?;
    runtime
        .authority
        .set_absence(
            runtime.target_key(database.tenant_id, absence_moved.target.entity_id),
            10,
        )
        .await;
    assert_orphan_not_repaired(&database, &runtime, &absence_moved).await?;

    let concurrent = prepare_orphan_case(&database, &runtime, "normal-mutation").await?;
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let service = repair_service(
        Arc::new(GateBeforeOwnerEvidence {
            inner: orphan_evidence(&database, &runtime).await?,
            entered: entered.clone(),
            release: release.clone(),
        }),
        orphan_owner(&database, &runtime).await?,
        recovery_store(&database).await?,
    )?;
    let task = tokio::spawn({
        let service = service.clone();
        let command = concurrent.command.clone();
        async move { service.execute(&command).await }
    });
    entered.wait().await;
    let replacement = runtime.linked_target(Uuid::new_v4());
    apply_record(
        &database,
        &runtime,
        runtime.source_record(concurrent.source_key.clone(), 8, vec![replacement.clone()]),
        "repair-evidence-normal-mutation",
    )
    .await?;
    release.wait().await;
    let result = task.await?;
    let error = result.expect_err("normal mutation must invalidate the admitted orphan target");
    assert_eq!(error.code(), "index_drift_repair_source_changed");
    assert_eq!(
        entity_state(&database, &concurrent.source_key).await?,
        Some((8, false))
    );
    assert_eq!(
        inbox_state(
            &database,
            ORPHAN_DELIVERY_SOURCE,
            concurrent.command.command_id()
        )
        .await?,
        None
    );
    assert_eq!(
        exact_link_count(
            &database,
            &concurrent.source_key,
            8,
            &runtime.contracts.link_name,
            0,
            &replacement,
        )
        .await?,
        1
    );
    assert_eq!(
        repair_command_state(&database, concurrent.command.command_id()).await?,
        "prepared"
    );

    database.cleanup().await
}

struct OrphanCase {
    source_key: rustok_index::EntityKey,
    target: rustok_index::LinkedEntityKey,
    command: rustok_index::IndexDriftRepairCommand,
}

async fn prepare_orphan_case(
    database: &TestDatabase,
    runtime: &FixtureRuntime,
    label: &str,
) -> TestResult<OrphanCase> {
    let source_key = runtime.source_key(database.tenant_id, Uuid::new_v4());
    let target = runtime.linked_target(Uuid::new_v4());
    apply_record(
        database,
        runtime,
        runtime.source_record(source_key.clone(), 7, vec![target.clone()]),
        &format!("repair-evidence-{label}"),
    )
    .await?;
    runtime
        .authority
        .set_absence(runtime.target_key(database.tenant_id, target.entity_id), 9)
        .await;
    let (finding_id, repair_target) = create_orphan_finding(
        database,
        runtime,
        source_key.clone(),
        7,
        0,
        target.clone(),
        9,
    )
    .await?;
    let command = repair_command(
        database.tenant_id,
        finding_id,
        Uuid::new_v4(),
        repair_target,
        label,
    )?;
    Ok(OrphanCase {
        source_key,
        target,
        command,
    })
}

async fn assert_orphan_not_repaired(
    database: &TestDatabase,
    runtime: &FixtureRuntime,
    case: &OrphanCase,
) -> TestResult<()> {
    let service = repair_service(
        orphan_evidence(database, runtime).await?,
        orphan_owner(database, runtime).await?,
        recovery_store(database).await?,
    )?;
    let outcome = service.execute(&case.command).await?;
    match outcome {
        IndexDriftRepairOutcome::NotRepaired(receipt) => match receipt.outcome() {
            IndexDriftRepairReceiptOutcome::NotRepaired { code } => {
                assert_eq!(code, "before_not_repairable");
            }
            other => panic!("unexpected receipt outcome: {other:?}"),
        },
        other => panic!("changed orphan commitment was not rejected: {other:?}"),
    }
    assert_eq!(
        inbox_state(database, ORPHAN_DELIVERY_SOURCE, case.command.command_id()).await?,
        None
    );
    Ok(())
}

async fn force_complete_repair(database: &TestDatabase, command_id: Uuid) -> TestResult<()> {
    let db = database.connection().await?;
    db.execute_raw(Statement::from_sql_and_values(
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
    Ok(())
}

#[tokio::test]
async fn owner_target_revived_after_evidence_prevents_orphan_deletion_without_index_redelivery()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup("orphan_owner_write_fence").await? else {
        return Ok(());
    };
    let runtime = FixtureRuntime::setup(&database).await?;
    let case = prepare_orphan_case(&database, &runtime, "owner-target-revived").await?;
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let service = repair_service(
        Arc::new(GateBeforeOwnerEvidence {
            inner: orphan_evidence(&database, &runtime).await?,
            entered: entered.clone(),
            release: release.clone(),
        }),
        orphan_owner(&database, &runtime).await?,
        recovery_store(&database).await?,
    )?;
    let command = case.command.clone();
    let task = tokio::spawn(async move { service.execute(&command).await });
    entered.wait().await;
    let target_key = runtime.target_key(database.tenant_id, case.target.entity_id);
    runtime
        .authority
        .set_mutation(IndexMutation::Upsert {
            event_id: Uuid::new_v4(),
            record: runtime.target_record(target_key, 10),
        })
        .await;
    release.wait().await;
    let error = task
        .await?
        .expect_err("owner authority changed without an Index write");
    assert_eq!(error.code(), "index_drift_repair_source_changed");
    assert_eq!(
        inbox_state(&database, ORPHAN_DELIVERY_SOURCE, case.command.command_id()).await?,
        None
    );
    assert_eq!(
        exact_link_count(
            &database,
            &case.source_key,
            7,
            &runtime.contracts.link_name,
            0,
            &case.target
        )
        .await?,
        1
    );
    database.cleanup().await
}

#[tokio::test]
async fn orphan_fence_survives_commit_and_crash_recovery_preserves_restored_edge() -> TestResult<()>
{
    let Some(database) = TestDatabase::setup("orphan_commit_fence").await? else {
        return Ok(());
    };
    let runtime = FixtureRuntime::setup(&database).await?;
    let case = prepare_orphan_case(&database, &runtime, "orphan-commit-fence").await?;
    let db = database.connection().await?;
    // A deferred constraint trigger runs at the repair transaction's commit, after the link
    // is removed and the inbox is written. Checking the lock's PID also rejects a fence taken
    // on another connection, even if that connection was kept alive by the fixture.
    db.execute_unprepared(
        r#"
        CREATE FUNCTION require_fixture_owner_fence() RETURNS trigger LANGUAGE plpgsql AS $$
        DECLARE fence_key bigint := hashtextextended(current_schema() || ':fixture-owner', 0);
        BEGIN
            IF NOT EXISTS (
                SELECT 1 FROM pg_catalog.pg_locks
                WHERE pid = pg_backend_pid() AND locktype = 'advisory' AND granted
                  AND mode = 'ShareLock' AND objsubid = 1
                  AND classid::bigint = ((fence_key >> 32) & 4294967295)
                  AND objid::bigint = (fence_key & 4294967295)
            ) THEN
                RAISE EXCEPTION 'authoritative owner lock was not held through repair commit';
            END IF;
            RETURN OLD;
        END; $$;
        CREATE CONSTRAINT TRIGGER check_owner_lock_at_repair_commit
            AFTER DELETE ON index_links DEFERRABLE INITIALLY DEFERRED
            FOR EACH ROW EXECUTE FUNCTION require_fixture_owner_fence();
        "#,
    )
    .await?;
    let crash = repair_service(
        Arc::new(FailAfterOwnerEvidence {
            inner: orphan_evidence(&database, &runtime).await?,
        }),
        orphan_owner(&database, &runtime).await?,
        recovery_store(&database).await?,
    )?;
    assert_eq!(
        crash
            .execute(&case.command)
            .await
            .expect_err("interruption after commit")
            .code(),
        "repair_evidence_after_commit_crash",
    );
    assert_eq!(
        repair_command_state(&database, case.command.command_id()).await?,
        "prepared"
    );
    assert_eq!(
        inbox_state(&database, ORPHAN_DELIVERY_SOURCE, case.command.command_id())
            .await?
            .as_deref(),
        Some("applied")
    );
    db.execute_unprepared("DROP TRIGGER check_owner_lock_at_repair_commit ON index_links")
        .await?;
    let store = PostgresMutationStore::new(db);
    let redelivery = MutationDelivery::from_event(
        "repair-evidence-equal-revision",
        IndexMutation::Upsert {
            event_id: Uuid::new_v4(),
            record: runtime.source_record(case.source_key.clone(), 7, vec![case.target.clone()]),
        },
    )?;
    assert_eq!(
        store.apply(runtime.schemas.as_ref(), &redelivery).await?,
        MutationApplyOutcome::Duplicate { source_version: 7 }
    );
    let count = |version| {
        exact_link_count(
            &database,
            &case.source_key,
            version,
            &runtime.contracts.link_name,
            0,
            &case.target,
        )
    };
    assert_eq!(count(7).await?, 0);

    // An owner allowing resurrection must publish the new target AND advance the source revision
    // for restored edges. Index's duplicate check intentionally does not undo a local repair.
    for record in [
        runtime.target_record(
            runtime.target_key(database.tenant_id, case.target.entity_id),
            10,
        ),
        runtime.source_record(case.source_key.clone(), 8, vec![case.target.clone()]),
    ] {
        let mutation = IndexMutation::Upsert {
            event_id: Uuid::new_v4(),
            record,
        };
        runtime.authority.set_mutation(mutation.clone()).await;
        let delivery = MutationDelivery::from_event("repair-evidence-restoration", mutation)?;
        assert_eq!(
            store.apply(runtime.schemas.as_ref(), &delivery).await?,
            MutationApplyOutcome::Applied {
                source_version: delivery.mutation().source_version()
            }
        );
    }
    assert_eq!(count(7).await?, 0);
    assert_eq!(count(8).await?, 1);

    // An old command can record its exact applied receipt, but cannot claim current convergence
    // or run a second delete once the source/target authority has advanced.
    let retry = repair_service(
        orphan_evidence(&database, &runtime).await?,
        orphan_owner(&database, &runtime).await?,
        recovery_store(&database).await?,
    )?;
    let result = retry.execute(&case.command).await?;
    let IndexDriftRepairOutcome::NotRepaired(receipt) = result else {
        panic!("old command must not claim convergence: {result:?}");
    };
    assert_eq!(
        receipt.outcome(),
        &IndexDriftRepairReceiptOutcome::NotRepaired {
            code: "after_not_converged".into()
        }
    );
    assert!(receipt.owner_receipt_digest().is_some());
    assert!(receipt.after_digest().is_some());
    assert_eq!(count(8).await?, 1);
    assert_eq!(
        retry.execute(&case.command).await?,
        IndexDriftRepairOutcome::AlreadyCompleted(receipt)
    );
    database.cleanup().await
}
