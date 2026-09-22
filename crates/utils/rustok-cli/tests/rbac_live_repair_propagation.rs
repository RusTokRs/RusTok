use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use chrono::Utc;
use rustok_api::Permission;
use rustok_core::{UserRole, UserStatus};
use rustok_migrations::Migrator;
use rustok_rbac::RbacRoleAssignmentDbWriter;
use rustok_runtime::RuntimeComposition;
use rustok_server::common::settings::RustokSettings;
use rustok_server::models::_entities::{permissions, role_permissions, roles};
use rustok_server::models::{tenants, users};
use rustok_server::services::cache_runtime::ensure_cache_service;
use rustok_server::services::rbac_cache_invalidation::{
    RbacCacheInvalidationListenerHandle, start_rbac_cache_invalidation_listener,
};
use rustok_server::services::rbac_invalidation_generation::{
    RbacInvalidationGenerationWatchdogHandle, start_rbac_invalidation_generation_watchdog,
};
use rustok_server::services::rbac_service::RbacService;
use rustok_server::services::server_runtime_context::ServerRuntimeContext;
use rustok_telemetry::rbac_invalidation_metrics::{
    RBAC_INVALIDATION_APPLIED_GENERATION, RBAC_INVALIDATION_FULL_CLEARS_TOTAL,
    RBAC_INVALIDATION_RECOVERIES_TOTAL,
};
use rustok_test_utils::{
    assert_postgres_url, connect_postgres, create_postgres_database,
    drop_postgres_database_if_exists, postgres_database_url, unique_postgres_database_name,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use sea_orm_migration::MigratorTrait;
use serde::{Deserialize, Serialize};
use tokio::process::{Child, Command};
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const CHILD_ROLE_ENV: &str = "RUSTOK_RBAC_CLI_REPAIR_CHILD_ROLE";
const CHILD_DATABASE_URL_ENV: &str = "RUSTOK_RBAC_CLI_REPAIR_DATABASE_URL";
const CHILD_TENANT_ID_ENV: &str = "RUSTOK_RBAC_CLI_REPAIR_TENANT_ID";
const CHILD_USER_ID_ENV: &str = "RUSTOK_RBAC_CLI_REPAIR_USER_ID";
const CHILD_READY_PATH_ENV: &str = "RUSTOK_RBAC_CLI_REPAIR_READY_PATH";
const CHILD_RESULT_PATH_ENV: &str = "RUSTOK_RBAC_CLI_REPAIR_RESULT_PATH";
const CHILD_TEST_NAME: &str = "rbac_cli_live_repair_child";
const WATCHDOG_RECOVERY_BOUND: Duration = Duration::from_secs(7);

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Serialize, Deserialize)]
struct ObserverReady {
    process_id: u32,
    initial_generation: u64,
    applied_generation: u64,
    initial_allowed: bool,
    redis_configured: bool,
    listener_running: bool,
    watchdog_running: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ObserverResult {
    process_id: u32,
    durable_generation: u64,
    applied_generation: u64,
    final_allowed: bool,
    authoritative_allowed: bool,
    recovery_count_delta: u64,
    full_clear_count_delta: u64,
    recovery_elapsed_ms: u64,
    redis_configured: bool,
    listener_running: bool,
    watchdog_running: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct CliProcessResult {
    process_id: u32,
    exit_code: i32,
    stderr: String,
    applied: bool,
    changes_total: u64,
    role_permission_links_removed: u64,
    affected_users_count: usize,
    durable_generation: Option<u64>,
    runtime_restart_required_if_applied: bool,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access and subprocess execution"]
async fn live_cli_system_role_repair_reaches_two_running_replicas_without_restart() {
    run_live_cli_repair_scenario()
        .await
        .unwrap_or_else(|error| {
            panic!("RBAC live CLI repair propagation evidence failed: {error}")
        });
}

async fn run_live_cli_repair_scenario() -> TestResult<()> {
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);
    let database_name = unique_postgres_database_name("rustok_rbac_cli_live_repair");
    let target_url = postgres_database_url(&admin_url, &database_name);
    let admin = connect_postgres(&admin_url).await?;
    drop_postgres_database_if_exists(&admin, &database_name).await?;
    create_postgres_database(&admin, &database_name).await?;

    let scenario_result = async {
        let db = connect_postgres(&target_url).await?;
        Migrator::up(&db, None).await?;
        let tenant_id = insert_tenant(&db).await?;
        let first_user_id = insert_user(&db, tenant_id, "first").await?;
        let second_user_id = insert_user(&db, tenant_id, "second").await?;
        seed_manager_permission_drift(&db, tenant_id, [first_user_id, second_user_id]).await?;

        let initial_generation = rustok_rbac::read_permission_invalidation_generation(&db).await?;
        if initial_generation != 0 {
            return Err(test_error(format!(
                "expected initial durable generation 0, found {initial_generation}"
            )));
        }

        let workspace = tempfile::tempdir()?;
        let first_ready_path = workspace.path().join("observer-first-ready.json");
        let second_ready_path = workspace.path().join("observer-second-ready.json");
        let first_result_path = workspace.path().join("observer-first-result.json");
        let second_result_path = workspace.path().join("observer-second-result.json");
        let cli_result_path = workspace.path().join("cli-result.json");

        let mut first_observer = spawn_observer(
            &target_url,
            tenant_id,
            first_user_id,
            &first_ready_path,
            &first_result_path,
        )?;
        let mut second_observer = spawn_observer(
            &target_url,
            tenant_id,
            second_user_id,
            &second_ready_path,
            &second_result_path,
        )?;

        wait_for_file(&first_ready_path, Duration::from_secs(8)).await?;
        wait_for_file(&second_ready_path, Duration::from_secs(8)).await?;
        let first_ready: ObserverReady = read_json(&first_ready_path)?;
        let second_ready: ObserverReady = read_json(&second_ready_path)?;
        validate_observer_ready(&first_ready)?;
        validate_observer_ready(&second_ready)?;
        if first_ready.process_id == second_ready.process_id {
            return Err(test_error(
                "observer replicas unexpectedly share one process",
            ));
        }

        let mut cli = spawn_cli(&target_url, tenant_id, &cli_result_path)?;
        wait_for_child(&mut cli, Duration::from_secs(15), "CLI repair").await?;
        let cli_result: CliProcessResult = read_json(&cli_result_path)?;
        if cli_result.exit_code != 0
            || !cli_result.stderr.is_empty()
            || !cli_result.applied
            || cli_result.changes_total == 0
            || cli_result.role_permission_links_removed == 0
            || cli_result.affected_users_count != 2
            || cli_result.durable_generation != Some(1)
            || cli_result.runtime_restart_required_if_applied
        {
            return Err(test_error(format!(
                "invalid CLI repair result: {cli_result:?}"
            )));
        }
        if cli_result.process_id == first_ready.process_id
            || cli_result.process_id == second_ready.process_id
        {
            return Err(test_error(
                "CLI repair did not execute in an independent process",
            ));
        }

        wait_for_child(
            &mut first_observer,
            Duration::from_secs(12),
            "first observer",
        )
        .await?;
        wait_for_child(
            &mut second_observer,
            Duration::from_secs(12),
            "second observer",
        )
        .await?;

        let first_result: ObserverResult = read_json(&first_result_path)?;
        let second_result: ObserverResult = read_json(&second_result_path)?;
        validate_observer_result(&first_result, first_ready.process_id)?;
        validate_observer_result(&second_result, second_ready.process_id)?;
        if first_result.process_id == second_result.process_id {
            return Err(test_error("observer result processes are not independent"));
        }

        let final_generation = rustok_rbac::read_permission_invalidation_generation(&db).await?;
        if final_generation != 1 {
            return Err(test_error(format!(
                "CLI repair durable generation is {final_generation}, expected 1"
            )));
        }

        db.close().await?;
        Ok(())
    }
    .await;

    drop_postgres_database_if_exists(&admin, &database_name).await?;
    admin.close().await?;
    scenario_result
}

#[test]
#[ignore = "subprocess entry point for the RBAC live CLI repair harness"]
fn rbac_cli_live_repair_child() {
    let Ok(role) = std::env::var(CHILD_ROLE_ENV) else {
        return;
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("build RBAC CLI repair child runtime");
    runtime
        .block_on(run_child(role.as_str()))
        .unwrap_or_else(|error| panic!("RBAC CLI repair child {role} failed: {error}"));
}

async fn run_child(role: &str) -> TestResult<()> {
    let database_url = required_env(CHILD_DATABASE_URL_ENV)?;
    let tenant_id = Uuid::parse_str(&required_env(CHILD_TENANT_ID_ENV)?)?;
    let result_path = PathBuf::from(required_env(CHILD_RESULT_PATH_ENV)?);

    match role {
        "observer" => {
            let user_id = Uuid::parse_str(&required_env(CHILD_USER_ID_ENV)?)?;
            let ready_path = PathBuf::from(required_env(CHILD_READY_PATH_ENV)?);
            run_observer(
                database_url.as_str(),
                tenant_id,
                user_id,
                ready_path,
                result_path,
            )
            .await
        }
        "cli" => run_cli(database_url.as_str(), tenant_id, result_path).await,
        other => Err(test_error(format!(
            "unsupported RBAC CLI repair child role {other}"
        ))),
    }
}

async fn run_observer(
    database_url: &str,
    tenant_id: Uuid,
    user_id: Uuid,
    ready_path: PathBuf,
    result_path: PathBuf,
) -> TestResult<()> {
    let db = connect_postgres(database_url).await?;
    let context = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    let cache = ensure_cache_service(&context);
    if cache.redis_configuration_present() {
        return Err(test_error(
            "CLI durable-recovery observer unexpectedly configured Redis",
        ));
    }
    start_rbac_cache_invalidation_listener(&context, cache.clone()).await?;
    start_rbac_invalidation_generation_watchdog(&context).await?;

    let listener = context
        .shared_get::<RbacCacheInvalidationListenerHandle>()
        .ok_or_else(|| test_error("observer listener handle is unavailable"))?;
    let watchdog = context
        .shared_get::<RbacInvalidationGenerationWatchdogHandle>()
        .ok_or_else(|| test_error("observer watchdog handle is unavailable"))?;
    let initial_allowed =
        RbacService::has_permission(&db, &tenant_id, &user_id, &Permission::SETTINGS_MANAGE)
            .await?;
    let ready = ObserverReady {
        process_id: std::process::id(),
        initial_generation: rustok_rbac::read_permission_invalidation_generation(&db).await?,
        applied_generation: RBAC_INVALIDATION_APPLIED_GENERATION.get() as u64,
        initial_allowed,
        redis_configured: cache.redis_configuration_present(),
        listener_running: listener.is_running(),
        watchdog_running: watchdog.is_running(),
    };
    write_json(&ready_path, &ready)?;

    let recovery_before = RBAC_INVALIDATION_RECOVERIES_TOTAL
        .with_label_values(&["generation_advanced"])
        .get();
    let full_clear_before = RBAC_INVALIDATION_FULL_CLEARS_TOTAL
        .with_label_values(&["generation_advanced"])
        .get();
    let generation_seen_at = wait_for_durable_generation(&db, 1, WATCHDOG_RECOVERY_BOUND).await?;
    wait_for_applied_generation(1, WATCHDOG_RECOVERY_BOUND).await?;

    let final_allowed =
        RbacService::has_permission(&db, &tenant_id, &user_id, &Permission::SETTINGS_MANAGE)
            .await?;
    let authoritative =
        RbacService::get_user_permissions_authoritative(&db, &tenant_id, &user_id).await?;
    let result = ObserverResult {
        process_id: std::process::id(),
        durable_generation: rustok_rbac::read_permission_invalidation_generation(&db).await?,
        applied_generation: RBAC_INVALIDATION_APPLIED_GENERATION.get() as u64,
        final_allowed,
        authoritative_allowed: authoritative.contains(&Permission::SETTINGS_MANAGE),
        recovery_count_delta: RBAC_INVALIDATION_RECOVERIES_TOTAL
            .with_label_values(&["generation_advanced"])
            .get()
            .saturating_sub(recovery_before),
        full_clear_count_delta: RBAC_INVALIDATION_FULL_CLEARS_TOTAL
            .with_label_values(&["generation_advanced"])
            .get()
            .saturating_sub(full_clear_before),
        recovery_elapsed_ms: generation_seen_at.elapsed().as_millis() as u64,
        redis_configured: cache.redis_configuration_present(),
        listener_running: listener.is_running(),
        watchdog_running: watchdog.is_running(),
    };
    write_json(&result_path, &result)?;
    Ok(())
}

async fn run_cli(database_url: &str, tenant_id: Uuid, result_path: PathBuf) -> TestResult<()> {
    let db = connect_postgres(database_url).await?;
    let runtime = RuntimeComposition::from_database(db, serde_json::json!({}));
    let exit = rustok_cli::run_with_runtime(
        [
            "rustok-cli".to_string(),
            "rbac".to_string(),
            "repair-system-roles".to_string(),
            "--apply".to_string(),
            "--tenant-id".to_string(),
            tenant_id.to_string(),
        ],
        runtime,
    )
    .await;
    let data = parse_cli_data(exit.stdout.as_str())?;
    let affected_users_count = data
        .get("affected_users")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let result = CliProcessResult {
        process_id: std::process::id(),
        exit_code: exit.code,
        stderr: exit.stderr,
        applied: data
            .get("applied")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        changes_total: data
            .get("changes_total")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        role_permission_links_removed: data
            .get("role_permission_links_removed")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        affected_users_count,
        durable_generation: data
            .get("durable_generation")
            .and_then(serde_json::Value::as_u64),
        runtime_restart_required_if_applied: data
            .get("runtime_restart_required_if_applied")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
    };
    write_json(&result_path, &result)?;
    Ok(())
}

fn parse_cli_data(stdout: &str) -> TestResult<serde_json::Value> {
    let json_start = stdout
        .find('{')
        .ok_or_else(|| test_error(format!("CLI stdout did not contain JSON: {stdout}")))?;
    Ok(serde_json::from_str(&stdout[json_start..])?)
}

async fn seed_manager_permission_drift(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    user_ids: [Uuid; 2],
) -> TestResult<()> {
    let writer = RbacRoleAssignmentDbWriter::new(db.clone());
    for user_id in user_ids {
        writer
            .assign_role_permissions(tenant_id, user_id, UserRole::Manager)
            .await?;
    }

    let manager_role = roles::Entity::find()
        .filter(roles::Column::TenantId.eq(tenant_id))
        .filter(roles::Column::Slug.eq(UserRole::Manager.to_string()))
        .one(db)
        .await?
        .ok_or_else(|| test_error("canonical Manager role was not created"))?;
    let stale_permission_id = Uuid::new_v4();
    permissions::Entity::insert(permissions::ActiveModel {
        id: Set(stale_permission_id),
        tenant_id: Set(tenant_id),
        resource: Set(Permission::SETTINGS_MANAGE.resource.to_string()),
        action: Set(Permission::SETTINGS_MANAGE.action.to_string()),
        description: Set(None),
        created_at: Set(Utc::now().into()),
    })
    .exec(db)
    .await?;
    role_permissions::Entity::insert(role_permissions::ActiveModel {
        id: Set(Uuid::new_v4()),
        role_id: Set(manager_role.id),
        permission_id: Set(stale_permission_id),
    })
    .exec(db)
    .await?;
    Ok(())
}

fn validate_observer_ready(ready: &ObserverReady) -> TestResult<()> {
    if ready.initial_generation != 0
        || ready.applied_generation != 0
        || !ready.initial_allowed
        || ready.redis_configured
        || !ready.listener_running
        || !ready.watchdog_running
    {
        return Err(test_error(format!(
            "invalid live observer readiness: {ready:?}"
        )));
    }
    Ok(())
}

fn validate_observer_result(result: &ObserverResult, expected_process_id: u32) -> TestResult<()> {
    if result.process_id != expected_process_id
        || result.durable_generation != 1
        || result.applied_generation != 1
        || result.final_allowed
        || result.authoritative_allowed
        || result.recovery_count_delta == 0
        || result.full_clear_count_delta == 0
        || result.recovery_elapsed_ms > WATCHDOG_RECOVERY_BOUND.as_millis() as u64
        || result.redis_configured
        || !result.listener_running
        || !result.watchdog_running
    {
        return Err(test_error(format!(
            "invalid live observer recovery result: {result:?}"
        )));
    }
    Ok(())
}

fn spawn_observer(
    database_url: &str,
    tenant_id: Uuid,
    user_id: Uuid,
    ready_path: &Path,
    result_path: &Path,
) -> TestResult<Child> {
    let mut command = child_command("observer", database_url, tenant_id, result_path)?;
    command
        .env(CHILD_USER_ID_ENV, user_id.to_string())
        .env(CHILD_READY_PATH_ENV, ready_path);
    Ok(command.spawn()?)
}

fn spawn_cli(database_url: &str, tenant_id: Uuid, result_path: &Path) -> TestResult<Child> {
    Ok(child_command("cli", database_url, tenant_id, result_path)?.spawn()?)
}

fn child_command(
    role: &str,
    database_url: &str,
    tenant_id: Uuid,
    result_path: &Path,
) -> TestResult<Command> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("--ignored")
        .arg("--exact")
        .arg(CHILD_TEST_NAME)
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env(CHILD_ROLE_ENV, role)
        .env(CHILD_DATABASE_URL_ENV, database_url)
        .env(CHILD_TENANT_ID_ENV, tenant_id.to_string())
        .env(CHILD_RESULT_PATH_ENV, result_path)
        .env_remove("RUSTOK_REDIS_URL")
        .env_remove("REDIS_URL")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    Ok(command)
}

async fn wait_for_applied_generation(expected: u64, timeout: Duration) -> TestResult<()> {
    tokio::time::timeout(timeout, async {
        loop {
            if RBAC_INVALIDATION_APPLIED_GENERATION.get() == expected as i64 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| test_error(format!("applied generation did not reach {expected}")))?;
    Ok(())
}

async fn wait_for_durable_generation(
    db: &DatabaseConnection,
    expected: u64,
    timeout: Duration,
) -> TestResult<Instant> {
    tokio::time::timeout(timeout, async {
        loop {
            let generation = rustok_rbac::read_permission_invalidation_generation(db).await?;
            if generation >= expected {
                return Ok::<Instant, Box<dyn std::error::Error + Send + Sync>>(Instant::now());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| test_error(format!("durable generation did not reach {expected}")))?
}

async fn wait_for_child(child: &mut Child, timeout: Duration, label: &str) -> TestResult<()> {
    match tokio::time::timeout(timeout, child.wait()).await {
        Ok(status) => {
            let status = status?;
            if status.success() {
                Ok(())
            } else {
                Err(test_error(format!("{label} exited with {status}")))
            }
        }
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            Err(test_error(format!("{label} exceeded {timeout:?}")))
        }
    }
}

async fn wait_for_file(path: &Path, timeout: Duration) -> TestResult<()> {
    tokio::time::timeout(timeout, async {
        loop {
            if path.is_file() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .map_err(|_| test_error(format!("timed out waiting for {}", path.display())))?;
    Ok(())
}

fn required_env(name: &str) -> TestResult<String> {
    std::env::var(name)
        .map_err(|_| test_error(format!("required environment variable {name} is missing")))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> TestResult<()> {
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> TestResult<T> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

async fn insert_tenant(db: &DatabaseConnection) -> TestResult<Uuid> {
    let tenant_id = Uuid::new_v4();
    tenants::Entity::insert(tenants::ActiveModel {
        id: Set(tenant_id),
        name: Set("RBAC live CLI repair".to_string()),
        slug: Set(format!("rbac-live-cli-repair-{tenant_id}")),
        domain: Set(None),
        settings: Set(serde_json::json!({})),
        default_locale: Set("en".to_string()),
        is_active: Set(true),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
    })
    .exec(db)
    .await?;
    Ok(tenant_id)
}

async fn insert_user(db: &DatabaseConnection, tenant_id: Uuid, suffix: &str) -> TestResult<Uuid> {
    let user_id = Uuid::new_v4();
    users::Entity::insert(users::ActiveModel {
        id: Set(user_id),
        tenant_id: Set(tenant_id),
        email: Set(format!("rbac-cli-repair-{suffix}-{user_id}@example.com")),
        password_hash: Set("hash".to_string()),
        name: Set(None),
        status: Set(UserStatus::Active),
        email_verified_at: Set(None),
        last_login_at: Set(None),
        metadata: Set(serde_json::json!({})),
        created_at: Set(Utc::now().into()),
        updated_at: Set(Utc::now().into()),
    })
    .exec(db)
    .await?;
    Ok(user_id)
}

fn test_error(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::other(message.into()))
}
