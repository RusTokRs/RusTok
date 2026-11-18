use chrono::Utc;
use rustok_installer::{
    AdminBootstrap, DatabaseConfig, DatabaseEngine, InstallEnvironment, InstallPlan,
    InstallProfile, InstallReceipt, InstallSessionRecord, InstallState, InstallStep,
    InstallTopology, InstallTopologyMode, ModuleSelection, ReceiptOutcome, SecretMode, SecretValue,
    TenantBootstrap, MAX_INSTALL_LOCK_TTL_SECS,
};
use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, Set};
use uuid::Uuid;

use crate::entities::install_session;
use crate::{InstallStepReceiptItem, InstallerPersistenceService};

async fn setup_contract_test_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("sqlite database");
    db.execute_unprepared(
        r#"
        CREATE TABLE install_http_jobs (
            id TEXT PRIMARY KEY NOT NULL,
            status TEXT NOT NULL,
            submitted_at TEXT NOT NULL,
            started_at TEXT NOT NULL,
            finished_at TEXT NULL,
            session_id TEXT NULL,
            tenant_id TEXT NULL,
            output TEXT NULL,
            error_message TEXT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE install_sessions (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NULL,
            status TEXT NOT NULL,
            attempt_generation BIGINT NOT NULL DEFAULT 0,
            profile TEXT NOT NULL,
            environment TEXT NOT NULL,
            database_engine TEXT NOT NULL,
            seed_profile TEXT NOT NULL,
            plan_snapshot TEXT NOT NULL,
            lock_owner TEXT NULL,
            lock_expires_at TEXT NULL,
            error_message TEXT NULL,
            created_by TEXT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            completed_at TEXT NULL
        );
        CREATE TABLE install_step_receipts (
            id TEXT PRIMARY KEY NOT NULL,
            session_id TEXT NOT NULL,
            attempt_generation BIGINT NOT NULL DEFAULT 0,
            step TEXT NOT NULL,
            outcome TEXT NOT NULL,
            input_checksum TEXT NOT NULL,
            diagnostics TEXT NOT NULL,
            installer_version TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .await
    .expect("setup contract test tables");
    db
}

fn sample_plan() -> InstallPlan {
    InstallPlan {
        placement: rustok_installer::InstancePlacement::new("."),
        environment: InstallEnvironment::Local,
        profile: InstallProfile::Monolith,
        database: DatabaseConfig {
            engine: DatabaseEngine::Sqlite,
            url: SecretValue::Plaintext {
                value: "sqlite::memory:".to_string(),
            },
            create_if_missing: false,
        },
        tenant: TenantBootstrap {
            slug: "test-tenant".to_string(),
            name: "Test Tenant".to_string(),
        },
        admin: AdminBootstrap {
            email: "admin@example.com".to_string(),
            password: SecretValue::Plaintext {
                value: "test-password-12345".to_string(),
            },
        },
        modules: ModuleSelection::default(),
        topology: InstallTopology::for_mode(InstallTopologyMode::Monolith)
            .bind_composition("test-rev".to_string(), "0".repeat(64)),
        seed_profile: rustok_installer::SeedProfile::Minimal,
        secrets_mode: SecretMode::Env,
    }
}

fn session_record(model: &install_session::Model) -> InstallSessionRecord {
    InstallSessionRecord {
        id: model.id,
        tenant_id: model.tenant_id,
        status: model.status.clone(),
        attempt_generation: model.attempt_generation,
        lock_owner: model.lock_owner.clone(),
        lock_expires_at: model.lock_expires_at.clone(),
    }
}

async fn record_successful_step(
    service: &InstallerPersistenceService,
    session: &InstallSessionRecord,
    step: InstallStep,
) {
    let receipt = InstallReceipt {
        session_id: session.id.to_string(),
        step,
        outcome: ReceiptOutcome::Success,
        input_checksum: format!("sha256:{}", Uuid::new_v4().simple()),
        diagnostics: serde_json::json!({ "step": "completed" }),
        installer_version: "0.1.0".to_string(),
        created_at: Utc::now(),
    };
    service
        .record_receipt(session, &receipt)
        .await
        .expect("record successful installer step");
}

#[tokio::test]
async fn session_persistence_lifecycle_and_state_transitions() {
    let db = setup_contract_test_db().await;
    let service = InstallerPersistenceService::new(db.clone());
    let plan = sample_plan();

    let session = service
        .create_session(&plan, None, None)
        .await
        .expect("session created");
    assert_eq!(session.status, "draft");
    assert_eq!(session.attempt_generation, 0);
    assert_eq!(session.environment, "local");
    assert_eq!(session.profile, "monolith");
    assert!(session.lock_owner.is_none());

    let latest = service
        .latest_session()
        .await
        .expect("latest query")
        .expect("session exists");
    assert_eq!(latest.id, session.id);

    let locked = service
        .acquire_lock(
            session.clone(),
            "worker-node-1",
            chrono::Duration::minutes(5),
        )
        .await
        .expect("acquire lock");
    assert_eq!(locked.lock_owner.as_deref(), Some("worker-node-1"));
    assert_eq!(locked.attempt_generation, 1);
    assert!(locked.lock_expires_at.is_some());

    let same_session_err = service
        .acquire_lock(
            locked.clone(),
            "worker-node-retry",
            chrono::Duration::minutes(5),
        )
        .await
        .expect_err("a session can only be claimed once");
    assert!(same_session_err.to_string().contains("already has an execution attempt"));

    let session2 = service
        .create_session(&plan, None, None)
        .await
        .expect("second session created");
    let err = service
        .acquire_lock(
            session2.clone(),
            "worker-node-2",
            chrono::Duration::minutes(5),
        )
        .await
        .expect_err("competing lock must be rejected");
    assert!(err.to_string().contains("unfinished execution"));

    let mut expired_session: install_session::ActiveModel = locked.clone().into();
    expired_session.lock_expires_at =
        Set(Some(Utc::now() - chrono::Duration::seconds(1)));
    expired_session
        .update(&db)
        .await
        .expect("expire the first session lease");
    let expired_lock_err = service
        .acquire_lock(
            session2.clone(),
            "worker-node-2",
            chrono::Duration::minutes(5),
        )
        .await
        .expect_err("expired lease must still block an unfinished session");
    assert!(expired_lock_err.to_string().contains("unfinished execution"));

    let mut wrong_owner = session_record(&locked);
    wrong_owner.lock_owner = Some("stale-worker".to_string());
    let stale_owner_error = service
        .set_state(&wrong_owner, InstallState::PreflightPassed)
        .await
        .expect_err("a stale owner must not mutate the session");
    assert!(stale_owner_error.to_string().contains("attempt fence"));

    let mut wrong_generation = session_record(&locked);
    wrong_generation.attempt_generation = locked.attempt_generation + 1;
    let stale_generation_error = service
        .set_state(&wrong_generation, InstallState::PreflightPassed)
        .await
        .expect_err("a stale generation must not mutate the session");
    assert!(stale_generation_error.to_string().contains("attempt fence"));

    let missing_preflight = service
        .set_state(&session_record(&locked), InstallState::PreflightPassed)
        .await
        .expect_err("state cannot advance without a successful step receipt");
    assert!(missing_preflight.to_string().contains("Preflight receipt"));
    record_successful_step(&service, &session_record(&locked), InstallStep::Preflight).await;
    let preflight = service
        .set_state(&session_record(&locked), InstallState::PreflightPassed)
        .await
        .expect("set preflight state");
    assert_eq!(preflight.status, "preflight_passed");

    record_successful_step(&service, &session_record(&preflight), InstallStep::Config).await;
    let config = service
        .set_state(&session_record(&preflight), InstallState::ConfigPrepared)
        .await
        .expect("set config state");
    record_successful_step(&service, &session_record(&config), InstallStep::Database).await;
    let database_ready = service
        .set_state(&session_record(&config), InstallState::DatabaseReady)
        .await
        .expect("set database state");
    record_successful_step(&service, &session_record(&database_ready), InstallStep::Migrate).await;
    let schema_applied = service
        .set_state(&session_record(&database_ready), InstallState::SchemaApplied)
        .await
        .expect("set schema state");

    let tenant_id = Uuid::new_v4();
    let mut stale_tenant_owner = session_record(&schema_applied);
    stale_tenant_owner.lock_owner = Some("stale-worker".to_string());
    assert!(
        service
            .set_tenant_id(&stale_tenant_owner, tenant_id)
            .await
            .expect_err("stale owner cannot change the tenant")
            .to_string()
            .contains("attempt fence")
    );
    let mut stale_tenant_generation = session_record(&schema_applied);
    stale_tenant_generation.attempt_generation += 1;
    assert!(
        service
            .set_tenant_id(&stale_tenant_generation, tenant_id)
            .await
            .expect_err("stale generation cannot change the tenant")
            .to_string()
            .contains("attempt fence")
    );
    let updated_tenant = service
        .set_tenant_id(&session_record(&schema_applied), tenant_id)
        .await
        .expect("set tenant id");
    assert_eq!(updated_tenant.tenant_id, Some(tenant_id));

    let recovery_required = service
        .set_state(
            &session_record(&updated_tenant),
            InstallState::RecoveryRequired,
        )
        .await
        .expect("mark recovery required");
    assert_eq!(recovery_required.status, "recovery_required");

    // Even an operator-cleared owner does not make an unresolved recovery
    // session safe to bypass: the admission gate also checks durable state.
    let mut ownerless_recovery_session: install_session::ActiveModel =
        recovery_required.clone().into();
    ownerless_recovery_session.lock_owner = Set(None);
    ownerless_recovery_session
        .update(&db)
        .await
        .expect("clear owner only as part of the manual-reconciliation fixture");
    let recovery_gate_err = service
        .acquire_lock(
            session2,
            "worker-node-2",
            chrono::Duration::minutes(5),
        )
        .await
        .expect_err("recovery-required session must keep the admission gate closed");
    assert!(recovery_gate_err.to_string().contains("unfinished execution"));
    assert!(!service.has_completed_session().await.expect("query"));
}

#[tokio::test]
async fn reclaimed_session_advances_generation_and_fences_the_previous_attempt() {
    let db = setup_contract_test_db().await;
    let service = InstallerPersistenceService::new(db.clone());
    let session = service
        .create_session(&sample_plan(), None, None)
        .await
        .expect("session created");
    let first_attempt = service
        .acquire_lock(
            session,
            "worker-attempt-one",
            chrono::Duration::minutes(5),
        )
        .await
        .expect("first attempt claims the session");
    assert_eq!(first_attempt.attempt_generation, 1);

    // Simulate an explicit operator reconciliation that has proven the first
    // worker fenced and returned the session to its resumable draft boundary.
    let mut reconciled: install_session::ActiveModel = first_attempt.clone().into();
    reconciled.lock_owner = Set(None);
    reconciled.lock_expires_at = Set(None);
    reconciled.status = Set("draft".to_string());
    reconciled
        .update(&db)
        .await
        .expect("reconciliation fixture releases the old claim");

    let current = service
        .get_session(first_attempt.id)
        .await
        .expect("read reconciled session")
        .expect("session exists");
    let second_attempt = service
        .acquire_lock(
            current,
            "worker-attempt-two",
            chrono::Duration::minutes(5),
        )
        .await
        .expect("second attempt claims the reconciled session");
    assert_eq!(second_attempt.attempt_generation, 2);
    assert_eq!(
        second_attempt.lock_owner.as_deref(),
        Some("worker-attempt-two")
    );

    let stale_state_error = service
        .set_state(
            &session_record(&first_attempt),
            InstallState::PreflightPassed,
        )
        .await
        .expect_err("the first worker cannot mutate after generation advances");
    assert!(stale_state_error.to_string().contains("attempt fence"));

    let stale_receipt = InstallReceipt {
        session_id: first_attempt.id.to_string(),
        step: InstallStep::Preflight,
        outcome: ReceiptOutcome::Success,
        input_checksum: "sha256:old-attempt".to_string(),
        diagnostics: serde_json::json!({ "attempt": 1 }),
        installer_version: "0.1.0".to_string(),
        created_at: Utc::now(),
    };
    let stale_receipt_error = service
        .record_receipt(&session_record(&first_attempt), &stale_receipt)
        .await
        .expect_err("the first worker cannot append a receipt after takeover");
    assert!(stale_receipt_error.to_string().contains("attempt fence"));

    record_successful_step(
        &service,
        &session_record(&second_attempt),
        InstallStep::Preflight,
    )
    .await;
    let accepted = service
        .set_state(
            &session_record(&second_attempt),
            InstallState::PreflightPassed,
        )
        .await
        .expect("the current attempt can advance the state");
    assert_eq!(accepted.status, "preflight_passed");
}

#[tokio::test]
async fn fresh_install_cleaned_requires_successful_rollback_receipt() {
    let db = setup_contract_test_db().await;
    let service = InstallerPersistenceService::new(db);
    let session = service
        .create_session(&sample_plan(), None, None)
        .await
        .expect("session created");
    let locked = service
        .acquire_lock(
            session,
            "cleanup-worker",
            chrono::Duration::minutes(5),
        )
        .await
        .expect("acquire lock");

    let missing_receipt = service
        .set_state(
            &session_record(&locked),
            InstallState::FreshInstallCleaned,
        )
        .await
        .expect_err("cleanup state cannot release admission without rollback evidence");
    assert!(missing_receipt.to_string().contains("successful rollback receipt"));

    let rollback = InstallReceipt {
        session_id: locked.id.to_string(),
        step: InstallStep::Rollback,
        outcome: ReceiptOutcome::Success,
        input_checksum: "sha256:cleanup-complete".to_string(),
        diagnostics: serde_json::json!({ "cleanup": "completed" }),
        installer_version: "0.1.0".to_string(),
        created_at: Utc::now(),
    };
    service
        .record_receipt(&session_record(&locked), &rollback)
        .await
        .expect("persist successful rollback evidence");

    let cleaned = service
        .set_state(
            &session_record(&locked),
            InstallState::FreshInstallCleaned,
        )
        .await
        .expect("successful cleanup receipt allows release");
    assert_eq!(cleaned.status, "fresh_install_cleaned");

    let next_session = service
        .create_session(&sample_plan(), None, None)
        .await
        .expect("new session created after verified cleanup");
    service
        .acquire_lock(
            next_session,
            "next-clean-install-worker",
            chrono::Duration::minutes(5),
        )
        .await
        .expect("verified cleanup releases global admission");
}

#[tokio::test]
async fn failed_state_requires_recovery_and_successful_rollback_receipt() {
    let db = setup_contract_test_db().await;
    let service = InstallerPersistenceService::new(db);
    let session = service
        .create_session(&sample_plan(), None, None)
        .await
        .expect("session created");
    let locked = service
        .acquire_lock(
            session,
            "recovery-worker",
            chrono::Duration::minutes(5),
        )
        .await
        .expect("acquire lock");
    let recovery = service
        .set_state(
            &session_record(&locked),
            InstallState::RecoveryRequired,
        )
        .await
        .expect("mark recovery required");

    let non_rollback = InstallReceipt {
        session_id: recovery.id.to_string(),
        step: InstallStep::Preflight,
        outcome: ReceiptOutcome::Success,
        input_checksum: "sha256:not-rollback".to_string(),
        diagnostics: serde_json::json!({}),
        installer_version: "0.1.0".to_string(),
        created_at: Utc::now(),
    };
    assert!(
        service
            .record_receipt(&session_record(&recovery), &non_rollback)
            .await
            .expect_err("recovery state only accepts rollback evidence")
            .to_string()
            .contains("cannot accept this receipt")
    );

    let rollback_failed = InstallReceipt {
        session_id: recovery.id.to_string(),
        step: InstallStep::Rollback,
        outcome: ReceiptOutcome::Failed,
        input_checksum: "sha256:rollback-failed".to_string(),
        diagnostics: serde_json::json!({ "cleanup": "failed" }),
        installer_version: "0.1.0".to_string(),
        created_at: Utc::now(),
    };
    service
        .record_receipt(&session_record(&recovery), &rollback_failed)
        .await
        .expect("failed rollback attempt is retained as evidence");
    assert!(
        service
            .set_state(&session_record(&recovery), InstallState::Failed)
            .await
            .expect_err("failed rollback cannot release admission")
            .to_string()
            .contains("successful rollback receipt")
    );

    let rollback_succeeded = InstallReceipt {
        session_id: recovery.id.to_string(),
        step: InstallStep::Rollback,
        outcome: ReceiptOutcome::Success,
        input_checksum: "sha256:rollback-succeeded".to_string(),
        diagnostics: serde_json::json!({ "cleanup": "completed" }),
        installer_version: "0.1.0".to_string(),
        created_at: Utc::now(),
    };
    service
        .record_receipt(&session_record(&recovery), &rollback_succeeded)
        .await
        .expect("successful rollback evidence is retained");

    let failed = service
        .set_state(&session_record(&recovery), InstallState::Failed)
        .await
        .expect("successful rollback permits terminal failure");
    assert_eq!(failed.status, "failed");

    let next_session = service
        .create_session(&sample_plan(), None, None)
        .await
        .expect("new session created after verified rollback");
    service
        .acquire_lock(
            next_session,
            "next-recovery-worker",
            chrono::Duration::minutes(5),
        )
        .await
        .expect("verified rollback releases global admission");
}

#[tokio::test]
async fn completed_session_releases_admission_gate() {
    let db = setup_contract_test_db().await;
    let service = InstallerPersistenceService::new(db);
    let plan = sample_plan();
    let session = service
        .create_session(&plan, None, None)
        .await
        .expect("session created");
    let locked = service
        .acquire_lock(
            session,
            "worker-node-1",
            chrono::Duration::minutes(5),
        )
        .await
        .expect("acquire lock");

    record_successful_step(&service, &session_record(&locked), InstallStep::Preflight).await;
    let preflight = service
        .set_state(&session_record(&locked), InstallState::PreflightPassed)
        .await
        .expect("preflight");
    record_successful_step(&service, &session_record(&preflight), InstallStep::Config).await;
    let config = service
        .set_state(&session_record(&preflight), InstallState::ConfigPrepared)
        .await
        .expect("config");
    record_successful_step(&service, &session_record(&config), InstallStep::Database).await;
    let database_ready = service
        .set_state(&session_record(&config), InstallState::DatabaseReady)
        .await
        .expect("database");
    record_successful_step(&service, &session_record(&database_ready), InstallStep::Migrate).await;
    let schema_applied = service
        .set_state(&session_record(&database_ready), InstallState::SchemaApplied)
        .await
        .expect("schema");
    record_successful_step(&service, &session_record(&schema_applied), InstallStep::Seed).await;
    let seed_applied = service
        .set_state(&session_record(&schema_applied), InstallState::SeedApplied)
        .await
        .expect("seed");
    record_successful_step(&service, &session_record(&seed_applied), InstallStep::Admin).await;
    let admin = service
        .set_state(&session_record(&seed_applied), InstallState::AdminProvisioned)
        .await
        .expect("admin");
    let deploying = service
        .set_state(&session_record(&admin), InstallState::Deploying)
        .await
        .expect("deploying");
    record_successful_step(&service, &session_record(&deploying), InstallStep::Verify).await;
    let missing_deploy = service
        .set_state(&session_record(&deploying), InstallState::Verified)
        .await
        .expect_err("verified state requires deploy and verify receipts");
    assert!(missing_deploy.to_string().contains("Deploy receipt"));
    record_successful_step(&service, &session_record(&deploying), InstallStep::Deploy).await;
    let verified = service
        .set_state(&session_record(&deploying), InstallState::Verified)
        .await
        .expect("verified");
    let missing_finalize = service
        .set_state(&session_record(&verified), InstallState::Completed)
        .await
        .expect_err("completed state requires finalize receipt");
    assert!(missing_finalize.to_string().contains("Finalize receipt"));
    record_successful_step(&service, &session_record(&verified), InstallStep::Finalize).await;
    let completed = service
        .set_state(&session_record(&verified), InstallState::Completed)
        .await
        .expect("completed");
    assert_eq!(completed.status, "completed");
    assert!(completed.completed_at.is_some());
    assert!(service.has_completed_session().await.expect("query"));

    let next_session = service
        .create_session(&plan, None, None)
        .await
        .expect("next session created");
    let next_lock = service
        .acquire_lock(
            next_session,
            "worker-node-2",
            chrono::Duration::minutes(5),
        )
        .await
        .expect("completed session releases the admission gate");
    assert_eq!(next_lock.lock_owner.as_deref(), Some("worker-node-2"));
    assert_eq!(next_lock.attempt_generation, 1);
}

#[tokio::test]
async fn acquire_lock_rejects_owner_exceeding_the_storage_bound() {
    let db = setup_contract_test_db().await;
    let service = InstallerPersistenceService::new(db.clone());
    let session = service
        .create_session(&sample_plan(), None, None)
        .await
        .expect("session created");

    let error = service
        .acquire_lock(
            session.clone(),
            "x".repeat(129),
            chrono::Duration::minutes(5),
        )
        .await
        .expect_err("oversized owner must be rejected before claim");
    assert!(error.to_string().contains("128-byte"));

    let unchanged = service
        .get_session(session.id)
        .await
        .expect("read session")
        .expect("session exists");
    assert!(unchanged.lock_owner.is_none());
    assert_eq!(unchanged.attempt_generation, 0);
}

#[tokio::test]
async fn acquire_lock_rejects_corrupt_or_exhausted_attempt_generation() {
    for generation in [-1, i64::MAX] {
        let db = setup_contract_test_db().await;
        let service = InstallerPersistenceService::new(db.clone());
        let session = service
            .create_session(&sample_plan(), None, None)
            .await
            .expect("session created");

        let mut corrupted: install_session::ActiveModel = session.clone().into();
        corrupted.attempt_generation = Set(generation);
        corrupted
            .update(&db)
            .await
            .expect("inject invalid generation fixture");

        let current = service
            .get_session(session.id)
            .await
            .expect("read session")
            .expect("session exists");
        let error = service
            .acquire_lock(
                current,
                "worker-node-invalid-generation",
                chrono::Duration::minutes(5),
            )
            .await
            .expect_err("invalid generation must fail before claiming");
        assert!(
            error.to_string().contains("generation"),
            "unexpected error for generation {generation}: {error}"
        );

        let unchanged = service
            .get_session(session.id)
            .await
            .expect("read unchanged session")
            .expect("session exists");
        assert!(unchanged.lock_owner.is_none());
        assert_eq!(unchanged.attempt_generation, generation);
    }
}

#[tokio::test]
async fn acquire_lock_rejects_out_of_range_ttl_before_claiming_session() {
    let db = setup_contract_test_db().await;
    let service = InstallerPersistenceService::new(db);
    let session = service
        .create_session(&sample_plan(), None, None)
        .await
        .expect("session created");

    let zero = service
        .acquire_lock(
            session.clone(),
            "worker",
            chrono::Duration::seconds(0),
        )
        .await
        .expect_err("zero TTL must be rejected");
    assert!(zero.to_string().contains("between 1 and"));

    let too_large = service
        .acquire_lock(
            session.clone(),
            "worker",
            chrono::Duration::seconds(MAX_INSTALL_LOCK_TTL_SECS + 1),
        )
        .await
        .expect_err("over-limit TTL must be rejected");
    assert!(too_large.to_string().contains("between 1 and"));

    let accepted = service
        .acquire_lock(
            session,
            "worker",
            chrono::Duration::seconds(MAX_INSTALL_LOCK_TTL_SECS),
        )
        .await
        .expect("maximum supported TTL is accepted");
    assert!(accepted.lock_expires_at.is_some());
    assert_eq!(accepted.attempt_generation, 1);
}

#[tokio::test]
async fn reserved_session_id_is_used_by_the_durable_session_row() {
    let db = setup_contract_test_db().await;
    let service = InstallerPersistenceService::new(db);
    let expected_id = Uuid::new_v4();
    let plan = sample_plan();
    let session = service
        .create_session_with_id(expected_id, &plan, None, None)
        .await
        .expect("reserved session should be persisted");

    assert_eq!(session.id, expected_id);
    assert_eq!(session.status, "draft");
    assert_eq!(session.attempt_generation, 0);
}

#[tokio::test]
async fn receipt_persistence_and_conversion_is_bound_to_the_current_attempt() {
    let db = setup_contract_test_db().await;
    let service = InstallerPersistenceService::new(db);
    let session = service
        .create_session(&sample_plan(), None, None)
        .await
        .expect("session created");
    let locked = service
        .acquire_lock(
            session,
            "receipt-worker",
            chrono::Duration::minutes(5),
        )
        .await
        .expect("acquire lock");

    let receipt = InstallReceipt {
        session_id: locked.id.to_string(),
        step: InstallStep::Preflight,
        outcome: ReceiptOutcome::Success,
        input_checksum: "sha256:abc123def456".to_string(),
        diagnostics: serde_json::json!({ "checked_paths": 4 }),
        installer_version: "0.1.0".to_string(),
        created_at: Utc::now(),
    };

    let mut stale_owner = session_record(&locked);
    stale_owner.lock_owner = Some("replaced-worker".to_string());
    assert!(
        service
            .record_receipt(&stale_owner, &receipt)
            .await
            .expect_err("stale owner cannot append a receipt")
            .to_string()
            .contains("attempt fence")
    );

    let mut stale_generation = session_record(&locked);
    stale_generation.attempt_generation += 1;
    assert!(
        service
            .record_receipt(&stale_generation, &receipt)
            .await
            .expect_err("stale generation cannot append a receipt")
            .to_string()
            .contains("attempt fence")
    );

    let model = service
        .record_receipt(&session_record(&locked), &receipt)
        .await
        .expect("record receipt");
    assert_eq!(model.session_id, locked.id);
    assert_eq!(model.attempt_generation, locked.attempt_generation);
    assert_eq!(model.step, "preflight");
    assert_eq!(model.outcome, "success");
    assert_eq!(model.input_checksum, "sha256:abc123def456");

    let items = service
        .list_receipts(locked.id)
        .await
        .expect("list receipts");
    assert_eq!(items.len(), 1);
    let item: &InstallStepReceiptItem = &items[0];
    assert_eq!(item.session_id, locked.id);
    assert_eq!(item.attempt_generation, locked.attempt_generation);
    assert_eq!(item.step, "preflight");
    assert_eq!(item.outcome, "success");
    assert_eq!(item.input_checksum, "sha256:abc123def456");
    assert_eq!(item.installer_version, "0.1.0");
}

#[tokio::test]
async fn empty_or_whitespace_lock_owner_is_rejected() {
    let db = setup_contract_test_db().await;
    let service = InstallerPersistenceService::new(db);
    let plan = sample_plan();
    let session = service
        .create_session(&plan, None, None)
        .await
        .expect("session created");

    let err = service
        .acquire_lock(session, "   ", chrono::Duration::seconds(10))
        .await
        .expect_err("empty lock owner must be rejected");
    assert!(err.to_string().contains("must not be empty"));
}
