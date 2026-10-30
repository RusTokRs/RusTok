use chrono::Utc;
use rustok_installer::{
    AdminBootstrap, DatabaseConfig, DatabaseEngine, InstallEnvironment, InstallPlan,
    InstallProfile, InstallReceipt, InstallState, InstallStep, InstallTopology,
    InstallTopologyMode, ModuleSelection, ReceiptOutcome, SecretMode, SecretValue, TenantBootstrap,
};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection};
use uuid::Uuid;

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

#[tokio::test]
async fn session_persistence_lifecycle_and_state_transitions() {
    let db = setup_contract_test_db().await;
    let service = InstallerPersistenceService::new(db);
    let plan = sample_plan();

    // 1. Create session
    let session = service
        .create_session(&plan, None, None)
        .await
        .expect("session created");
    assert_eq!(session.status, "draft");
    assert_eq!(session.environment, "local");
    assert_eq!(session.profile, "monolith");
    assert!(session.lock_owner.is_none());

    // 2. Fetch latest session
    let latest = service
        .latest_session()
        .await
        .expect("latest query")
        .expect("session exists");
    assert_eq!(latest.id, session.id);

    // 3. Acquire lock
    let locked = service
        .acquire_lock(session.clone(), "worker-node-1", chrono::Duration::minutes(5))
        .await
        .expect("acquire lock");
    assert_eq!(locked.lock_owner.as_deref(), Some("worker-node-1"));
    assert!(locked.lock_expires_at.is_some());

    // 4. Competing lock attempt from another session while active lock is unexpired must fail
    let session2 = service
        .create_session(&plan, None, None)
        .await
        .expect("second session created");
    let err = service
        .acquire_lock(session2.clone(), "worker-node-2", chrono::Duration::minutes(5))
        .await
        .expect_err("competing lock must be rejected");
    assert!(err.to_string().contains("already held"));

    // 5. State transitions
    let updated = service
        .set_state(session.id, InstallState::PreflightPassed)
        .await
        .expect("set state");
    assert_eq!(updated.status, "preflight_passed");

    // 6. Set tenant ID
    let tenant_id = Uuid::new_v4();
    let updated_tenant = service
        .set_tenant_id(session.id, tenant_id)
        .await
        .expect("set tenant id");
    assert_eq!(updated_tenant.tenant_id, Some(tenant_id));

    // 7. Verify has_completed_session before and after completion
    assert!(!service.has_completed_session().await.expect("query"));
    let completed = service
        .set_state(session.id, InstallState::Completed)
        .await
        .expect("set completed");
    assert_eq!(completed.status, "completed");
    assert!(completed.completed_at.is_some());
    assert!(service.has_completed_session().await.expect("query"));
}

#[tokio::test]
async fn receipt_persistence_and_conversion() {
    let db = setup_contract_test_db().await;
    let service = InstallerPersistenceService::new(db);
    let session_id = Uuid::new_v4();

    let receipt = InstallReceipt {
        session_id: session_id.to_string(),
        step: InstallStep::Preflight,
        outcome: ReceiptOutcome::Success,
        input_checksum: "sha256:abc123def456".to_string(),
        diagnostics: serde_json::json!({ "checked_paths": 4 }),
        installer_version: "0.1.0".to_string(),
        created_at: Utc::now(),
    };

    let model = service
        .record_receipt(&receipt)
        .await
        .expect("record receipt");
    assert_eq!(model.session_id, session_id);
    assert_eq!(model.step, "preflight");
    assert_eq!(model.outcome, "success");
    assert_eq!(model.input_checksum, "sha256:abc123def456");

    let items = service
        .list_receipts(session_id)
        .await
        .expect("list receipts");
    assert_eq!(items.len(), 1);
    let item: &InstallStepReceiptItem = &items[0];
    assert_eq!(item.session_id, session_id);
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

