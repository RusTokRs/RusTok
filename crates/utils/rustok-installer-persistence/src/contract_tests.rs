use chrono::Utc;
use rustok_installer::{
    AdminBootstrap, DatabaseConfig, DatabaseEngine, InstallEnvironment, InstallPlan,
    InstallProfile, InstallReceipt, InstallSessionRecord, InstallState, InstallStep,
    InstallTopology, InstallTopologyMode, MAX_INSTALL_LOCK_TTL_SECS, ModuleSelection,
    ReceiptOutcome, SecretMode, SecretValue, TenantBootstrap,
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
        lock_expires_at: model.lock_expires_at,
    }
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
    assert!(
        same_session_err
            .to_string()
            .contains("already has an execution attempt")
    );

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
    expired_session.lock_expires_at = Set(Some(Utc::now() - chrono::Duration::seconds(1)));
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
    assert!(
        expired_lock_err
            .to_string()
            .contains("unfinished execution")
    );

    let mut wrong_owner = session_record(&locked);
    wrong_owner.lock_owner = Some("stale-worker".to_string());
    let stale_owner_error = service
        .set_state(&wrong_owner, InstallState::PreflightPassed)
        .await
        .expect_err("a stale owner must not mutate the session");
    assert!(stale_owner_error.to_string().contains("attempt fence"));

    let mut wrong_generation = session_record(&locked);
    wrong_generation.attempt_generation += 1;
    let generation_error = service
        .set_state(&wrong_generation, InstallState::PreflightPassed)
        .await
        .expect_err("a mismatched generation must not mutate the session");
    assert!(generation_error.to_string().contains("attempt fence"));

    let preflight = service
        .set_state(&session_record(&locked), InstallState::PreflightPassed)
        .await
        .expect("set preflight state");
    assert_eq!(preflight.status, "preflight_passed");

    let config = service
        .set_state(&session_record(&preflight), InstallState::ConfigPrepared)
        .await
        .expect("set config state");
    let database_ready = service
        .set_state(&session_record(&config), InstallState::DatabaseReady)
        .await
        .expect("set database state");
    let schema_applied = service
        .set_state(
            &session_record(&database_ready),
            InstallState::SchemaApplied,
        )
        .await
        .expect("set schema state");

    let tenant_id = Uuid::new_v4();
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
        .acquire_lock(session2, "worker-node-2", chrono::Duration::minutes(5))
        .await
        .expect_err("recovery-required session must keep the admission gate closed");
    assert!(
        recovery_gate_err
            .to_string()
            .contains("unfinished execution")
    );
    assert!(!service.has_completed_session().await.expect("query"));
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
        .acquire_lock(session, "worker-node-1", chrono::Duration::minutes(5))
        .await
        .expect("acquire lock");

    let preflight = service
        .set_state(&session_record(&locked), InstallState::PreflightPassed)
        .await
        .expect("preflight");
    let config = service
        .set_state(&session_record(&preflight), InstallState::ConfigPrepared)
        .await
        .expect("config");
    let database_ready = service
        .set_state(&session_record(&config), InstallState::DatabaseReady)
        .await
        .expect("database");
    let schema_applied = service
        .set_state(
            &session_record(&database_ready),
            InstallState::SchemaApplied,
        )
        .await
        .expect("schema");
    let seed_applied = service
        .set_state(&session_record(&schema_applied), InstallState::SeedApplied)
        .await
        .expect("seed");
    let admin = service
        .set_state(
            &session_record(&seed_applied),
            InstallState::AdminProvisioned,
        )
        .await
        .expect("admin");
    let deploying = service
        .set_state(&session_record(&admin), InstallState::Deploying)
        .await
        .expect("deploying");
    let verified = service
        .set_state(&session_record(&deploying), InstallState::Verified)
        .await
        .expect("verified");
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
        .acquire_lock(next_session, "worker-node-2", chrono::Duration::minutes(5))
        .await
        .expect("completed session releases the admission gate");
    assert_eq!(next_lock.lock_owner.as_deref(), Some("worker-node-2"));
    assert_eq!(next_lock.attempt_generation, 1);
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
        .acquire_lock(session.clone(), "worker", chrono::Duration::seconds(0))
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
        .acquire_lock(session, "receipt-worker", chrono::Duration::minutes(5))
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

    let mut wrong_generation = session_record(&locked);
    wrong_generation.attempt_generation += 1;
    assert!(
        service
            .record_receipt(&wrong_generation, &receipt)
            .await
            .expect_err("mismatched generation cannot append a receipt")
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

#[tokio::test]
async fn unstarted_http_job_cleanup_is_identity_fenced_and_terminal() {
    let db = setup_contract_test_db().await;
    let service = InstallerPersistenceService::new(db);
    let job_id = Uuid::new_v4();
    let submitted_at = Utc::now();

    service
        .create_http_job(job_id, job_id, submitted_at.clone())
        .await
        .expect("persist the job whose insert acknowledgement could be lost");

    assert!(
        !service
            .finish_unstarted_http_job_failed(job_id, Uuid::new_v4(), submitted_at.clone(),)
            .await
            .expect("wrong session is a no-op"),
        "a different session must not be finalized"
    );
    assert!(
        !service
            .finish_unstarted_http_job_failed(
                job_id,
                job_id,
                submitted_at.clone() + chrono::Duration::seconds(1),
            )
            .await
            .expect("wrong submission time is a no-op"),
        "an unrelated HTTP submission must not be finalized"
    );
    assert_eq!(
        service
            .get_http_job(job_id)
            .await
            .expect("job read")
            .expect("job exists")
            .status,
        "running"
    );

    assert!(
        service
            .finish_unstarted_http_job_failed(job_id, job_id, submitted_at.clone())
            .await
            .expect("exact claim may be safely closed before execution")
    );
    let finished = service
        .get_http_job(job_id)
        .await
        .expect("job read")
        .expect("job exists");
    assert_eq!(finished.status, "failed");
    assert!(finished.finished_at.is_some());
    assert!(finished.error_message.is_some());
    assert!(finished.output.is_none());
    assert!(
        !service
            .finish_unstarted_http_job_failed(job_id, job_id, submitted_at)
            .await
            .expect("terminal cleanup is idempotent"),
        "a terminal receipt must never be overwritten"
    );
}
