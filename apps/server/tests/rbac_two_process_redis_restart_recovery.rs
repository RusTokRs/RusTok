use std::future::Future;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child as ProcessChild, Command as ProcessCommand, Stdio};
use std::time::{Duration, Instant};

use chrono::Utc;
use rustok_api::Permission;
use rustok_core::{UserRole, UserStatus};
use rustok_migrations::Migrator;
use rustok_rbac::RbacRoleAssignmentDbWriter;
use rustok_server::common::settings::RustokSettings;
use rustok_server::models::{tenants, users};
use rustok_server::services::cache_runtime::ensure_cache_service;
use rustok_server::services::rbac_cache_invalidation::{
    RBAC_PERMISSION_INVALIDATION_CHANNEL, start_rbac_cache_invalidation_listener,
};
use rustok_server::services::rbac_invalidation_generation::start_rbac_invalidation_generation_watchdog;
use rustok_server::services::rbac_service::RbacService;
use rustok_server::services::server_runtime_context::ServerRuntimeContext;
use rustok_test_utils::{
    assert_postgres_url, connect_postgres, create_postgres_database,
    drop_postgres_database_if_exists, postgres_database_url, unique_postgres_database_name,
};
use sea_orm::{DatabaseConnection, EntityTrait, Set};
use sea_orm_migration::MigratorTrait;
use serde::{Deserialize, Serialize};
use tokio::process::{Child as RedisChild, Command as RedisCommand};
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const REDIS_SERVER_BIN_ENV: &str = "RUSTOK_CACHE_REDIS_SERVER_BIN";
const CHILD_ROLE_ENV: &str = "RUSTOK_RBAC_REDIS_CHILD_ROLE";
const CHILD_SCENARIO_ENV: &str = "RUSTOK_RBAC_REDIS_CHILD_SCENARIO";
const CHILD_DATABASE_URL_ENV: &str = "RUSTOK_RBAC_REDIS_CHILD_DATABASE_URL";
const CHILD_REDIS_URL_ENV: &str = "RUSTOK_RBAC_REDIS_CHILD_REDIS_URL";
const CHILD_TENANT_ID_ENV: &str = "RUSTOK_RBAC_REDIS_CHILD_TENANT_ID";
const CHILD_USER_ID_ENV: &str = "RUSTOK_RBAC_REDIS_CHILD_USER_ID";
const CHILD_READY_PATH_ENV: &str = "RUSTOK_RBAC_REDIS_CHILD_READY_PATH";
const CHILD_STALE_PATH_ENV: &str = "RUSTOK_RBAC_REDIS_CHILD_STALE_PATH";
const CHILD_RESTART_MARKER_PATH_ENV: &str = "RUSTOK_RBAC_REDIS_CHILD_RESTART_MARKER_PATH";
const CHILD_RESULT_PATH_ENV: &str = "RUSTOK_RBAC_REDIS_CHILD_RESULT_PATH";
const CHILD_TEST_NAME: &str = "rbac_redis_replica_child";
const SCENARIO_AVAILABLE: &str = "available";
const SCENARIO_RESTART: &str = "restart";
const AVAILABLE_REDIS_BOUND: Duration = Duration::from_secs(3);
const REDIS_RESTART_BOUND: Duration = Duration::from_secs(4);

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Serialize, Deserialize)]
struct ObserverReady {
    initial_generation: u64,
    initial_allowed: bool,
    redis_configured: bool,
    redis_healthy: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct StaleObservation {
    durable_generation: u64,
    allowed_before_restart: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ObserverResult {
    initial_generation: u64,
    durable_generation: u64,
    final_allowed: bool,
    authoritative_allowed: bool,
    redis_healthy_after_recovery: bool,
    recovery_elapsed_ms: u64,
    permission_checks: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct MutatorResult {
    committed_generation: u64,
    redis_configured: bool,
    redis_publish_success_total: u64,
    redis_publish_failure_total: u64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access, redis-server and subprocess execution"]
async fn separate_process_replica_applies_available_redis_invalidation() {
    let binary = required_env(REDIS_SERVER_BIN_ENV).unwrap();
    let port = reserve_loopback_port();
    let redis_url = format!("redis://127.0.0.1:{port}/");
    let mut redis_process = spawn_redis(&binary, port).await;

    let result = with_rbac_postgres_database("rustok_rbac_redis_available", |db, target_url| {
        let redis_url = redis_url.clone();
        async move {
            let tenant_id = insert_tenant(&db, "available").await?;
            let user_id = insert_user(&db, tenant_id, "available").await?;
            RbacRoleAssignmentDbWriter::new(db.clone())
                .assign_role_permissions(tenant_id, user_id, UserRole::Admin)
                .await?;

            let workspace = tempfile::tempdir()?;
            let ready_path = workspace.path().join("observer-ready.json");
            let observer_result_path = workspace.path().join("observer-result.json");
            let mutator_result_path = workspace.path().join("mutator-result.json");

            let observer = spawn_child(
                "observer",
                SCENARIO_AVAILABLE,
                &target_url,
                &redis_url,
                tenant_id,
                user_id,
                Some(&ready_path),
                None,
                None,
                &observer_result_path,
            )?;
            wait_for_file(&ready_path, Duration::from_secs(8)).await?;
            wait_for_redis_subscribers(
                &redis_url,
                RBAC_PERMISSION_INVALIDATION_CHANNEL,
                1,
                Duration::from_secs(5),
            )
            .await?;

            let mutator = spawn_child(
                "mutator",
                SCENARIO_AVAILABLE,
                &target_url,
                &redis_url,
                tenant_id,
                user_id,
                None,
                None,
                None,
                &mutator_result_path,
            )?;
            wait_for_child(mutator, Duration::from_secs(12), "available Redis mutator").await?;
            wait_for_child(observer, Duration::from_secs(12), "available Redis observer").await?;

            let ready: ObserverReady = read_json(&ready_path)?;
            let mutation: MutatorResult = read_json(&mutator_result_path)?;
            let observation: ObserverResult = read_json(&observer_result_path)?;

            if !ready.initial_allowed || !ready.redis_configured || !ready.redis_healthy {
                return Err(test_error("observer did not start with a healthy configured Redis and an allowed snapshot"));
            }
            if mutation.committed_generation != ready.initial_generation + 1 {
                return Err(test_error("available Redis mutation did not commit exactly one durable generation"));
            }
            if !mutation.redis_configured
                || mutation.redis_publish_success_total == 0
                || mutation.redis_publish_failure_total != 0
            {
                return Err(test_error("available Redis mutation did not record successful canonical publication"));
            }
            assert_terminal_deny(&observation, mutation.committed_generation)?;
            if observation.recovery_elapsed_ms > AVAILABLE_REDIS_BOUND.as_millis() as u64 {
                return Err(test_error(format!(
                    "available Redis propagation took {} ms, beyond {} ms",
                    observation.recovery_elapsed_ms,
                    AVAILABLE_REDIS_BOUND.as_millis()
                )));
            }
            Ok(())
        }
    })
    .await;

    stop_redis(&mut redis_process).await;
    result.unwrap_or_else(|error| panic!("RBAC available Redis evidence failed: {error}"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access, redis-server and subprocess execution"]
async fn separate_process_replica_recovers_after_redis_restart_and_resubscribe() {
    let binary = required_env(REDIS_SERVER_BIN_ENV).unwrap();
    let port = reserve_loopback_port();
    let redis_url = format!("redis://127.0.0.1:{port}/");
    let mut redis_process = spawn_redis(&binary, port).await;

    let result = with_rbac_postgres_database("rustok_rbac_redis_restart", |db, target_url| {
        let redis_url = redis_url.clone();
        let binary = binary.clone();
        async move {
            let tenant_id = insert_tenant(&db, "restart").await?;
            let user_id = insert_user(&db, tenant_id, "restart").await?;
            RbacRoleAssignmentDbWriter::new(db.clone())
                .assign_role_permissions(tenant_id, user_id, UserRole::Admin)
                .await?;

            let workspace = tempfile::tempdir()?;
            let ready_path = workspace.path().join("observer-ready.json");
            let stale_path = workspace.path().join("observer-stale.json");
            let restart_marker_path = workspace.path().join("redis-restart-started");
            let observer_result_path = workspace.path().join("observer-result.json");
            let mutator_result_path = workspace.path().join("mutator-result.json");

            let observer = spawn_child(
                "observer",
                SCENARIO_RESTART,
                &target_url,
                &redis_url,
                tenant_id,
                user_id,
                Some(&ready_path),
                Some(&stale_path),
                Some(&restart_marker_path),
                &observer_result_path,
            )?;
            wait_for_file(&ready_path, Duration::from_secs(8)).await?;
            wait_for_redis_subscribers(
                &redis_url,
                RBAC_PERMISSION_INVALIDATION_CHANNEL,
                1,
                Duration::from_secs(5),
            )
            .await?;

            stop_redis(&mut redis_process).await;

            let mutator = spawn_child(
                "mutator",
                SCENARIO_RESTART,
                &target_url,
                &redis_url,
                tenant_id,
                user_id,
                None,
                None,
                None,
                &mutator_result_path,
            )?;
            wait_for_child(mutator, Duration::from_secs(14), "stopped Redis mutator").await?;
            wait_for_file(&stale_path, Duration::from_secs(7)).await?;

            std::fs::write(&restart_marker_path, b"restart")?;
            redis_process = spawn_redis(&binary, port).await;
            wait_for_child(observer, Duration::from_secs(14), "restarted Redis observer").await?;

            let ready: ObserverReady = read_json(&ready_path)?;
            let stale: StaleObservation = read_json(&stale_path)?;
            let mutation: MutatorResult = read_json(&mutator_result_path)?;
            let observation: ObserverResult = read_json(&observer_result_path)?;

            if !ready.initial_allowed || !ready.redis_configured || !ready.redis_healthy {
                return Err(test_error("restart observer did not start from a healthy configured Redis snapshot"));
            }
            if mutation.committed_generation != ready.initial_generation + 1 {
                return Err(test_error("stopped Redis mutation did not commit exactly one durable generation"));
            }
            if !mutation.redis_configured
                || mutation.redis_publish_failure_total == 0
                || mutation.redis_publish_success_total != 0
            {
                return Err(test_error("stopped Redis mutation did not record deferred canonical publication"));
            }
            if stale.durable_generation != mutation.committed_generation
                || !stale.allowed_before_restart
            {
                return Err(test_error("observer did not retain the intentionally stale allow before Redis restart"));
            }
            assert_terminal_deny(&observation, mutation.committed_generation)?;
            if !observation.redis_healthy_after_recovery {
                return Err(test_error("observer did not report healthy Redis after resubscribe recovery"));
            }
            if observation.recovery_elapsed_ms > REDIS_RESTART_BOUND.as_millis() as u64 {
                return Err(test_error(format!(
                    "Redis restart recovery took {} ms, beyond {} ms",
                    observation.recovery_elapsed_ms,
                    REDIS_RESTART_BOUND.as_millis()
                )));
            }
            Ok(())
        }
    })
    .await;

    stop_redis(&mut redis_process).await;
    result.unwrap_or_else(|error| panic!("RBAC Redis restart evidence failed: {error}"));
}

#[test]
#[ignore = "subprocess entry point for RBAC Redis recovery harness"]
fn rbac_redis_replica_child() {
    let Ok(role) = std::env::var(CHILD_ROLE_ENV) else {
        return;
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("build RBAC Redis replica child runtime");
    runtime
        .block_on(run_child(&role))
        .unwrap_or_else(|error| panic!("RBAC Redis replica child {role} failed: {error}"));
}

async fn run_child(role: &str) -> TestResult<()> {
    let scenario = required_env(CHILD_SCENARIO_ENV)?;
    let database_url = required_env(CHILD_DATABASE_URL_ENV)?;
    let redis_url = required_env(CHILD_REDIS_URL_ENV)?;
    let tenant_id = Uuid::parse_str(&required_env(CHILD_TENANT_ID_ENV)?)?;
    let user_id = Uuid::parse_str(&required_env(CHILD_USER_ID_ENV)?)?;
    let result_path = PathBuf::from(required_env(CHILD_RESULT_PATH_ENV)?);
    let db = connect_postgres(&database_url).await?;
    let mut settings = RustokSettings::default();
    settings.cache.redis_url = Some(redis_url);
    let context = ServerRuntimeContext::new(db.clone(), settings);
    let cache = ensure_cache_service(&context);
    start_rbac_cache_invalidation_listener(&context, cache.clone()).await?;

    if scenario == SCENARIO_AVAILABLE && role == "observer" {
        start_rbac_invalidation_generation_watchdog(&context).await?;
    }

    match role {
        "observer" => {
            let ready_path = PathBuf::from(required_env(CHILD_READY_PATH_ENV)?);
            let stale_path = optional_path(CHILD_STALE_PATH_ENV);
            let restart_marker_path = optional_path(CHILD_RESTART_MARKER_PATH_ENV);
            run_observer(
                db,
                cache,
                tenant_id,
                user_id,
                &scenario,
                ready_path,
                stale_path,
                restart_marker_path,
                result_path,
            )
            .await
        }
        "mutator" => run_mutator(db, cache, tenant_id, user_id, result_path).await,
        other => Err(test_error(format!("unsupported Redis replica child role {other}"))),
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_observer(
    db: DatabaseConnection,
    cache: rustok_cache::CacheService,
    tenant_id: Uuid,
    user_id: Uuid,
    scenario: &str,
    ready_path: PathBuf,
    stale_path: Option<PathBuf>,
    restart_marker_path: Option<PathBuf>,
    result_path: PathBuf,
) -> TestResult<()> {
    let initial_generation = rustok_rbac::read_permission_invalidation_generation(&db).await?;
    let initial_allowed = RbacService::has_permission(
        &db,
        &tenant_id,
        &user_id,
        &Permission::SETTINGS_MANAGE,
    )
    .await?;
    let initial_health = cache.health().await;
    write_json(
        &ready_path,
        &ObserverReady {
            initial_generation,
            initial_allowed,
            redis_configured: initial_health.redis_configured,
            redis_healthy: initial_health.redis_healthy,
        },
    )?;

    let deadline = Instant::now() + Duration::from_secs(16);
    let mut durable_generation = initial_generation;
    let mut generation_seen_at = None;
    let mut restart_seen_at = None;
    let mut permission_checks = 1_u64;
    let mut stale_written = false;

    while Instant::now() < deadline {
        durable_generation = rustok_rbac::read_permission_invalidation_generation(&db).await?;
        if durable_generation > initial_generation && generation_seen_at.is_none() {
            generation_seen_at = Some(Instant::now());
        }

        let allowed = RbacService::has_permission(
            &db,
            &tenant_id,
            &user_id,
            &Permission::SETTINGS_MANAGE,
        )
        .await?;
        permission_checks += 1;

        if scenario == SCENARIO_RESTART
            && durable_generation > initial_generation
            && allowed
            && !stale_written
        {
            let stale_path = stale_path
                .as_ref()
                .ok_or_else(|| test_error("restart observer stale path is missing"))?;
            write_json(
                stale_path,
                &StaleObservation {
                    durable_generation,
                    allowed_before_restart: true,
                },
            )?;
            stale_written = true;
        }

        if scenario == SCENARIO_RESTART && restart_seen_at.is_none() {
            if restart_marker_path.as_ref().is_some_and(|path| path.is_file()) {
                restart_seen_at = Some(Instant::now());
            }
        }

        let recovery_origin = if scenario == SCENARIO_RESTART {
            restart_seen_at
        } else {
            generation_seen_at
        };
        if !allowed && durable_generation > initial_generation {
            if scenario == SCENARIO_RESTART && recovery_origin.is_none() {
                return Err(test_error("restart observer recovered before the Redis restart marker"));
            }
            let authoritative = RbacService::get_user_permissions_authoritative(
                &db,
                &tenant_id,
                &user_id,
            )
            .await?;
            let health = cache.health().await;
            write_json(
                &result_path,
                &ObserverResult {
                    initial_generation,
                    durable_generation,
                    final_allowed: allowed,
                    authoritative_allowed: authoritative.contains(&Permission::SETTINGS_MANAGE),
                    redis_healthy_after_recovery: health.redis_healthy,
                    recovery_elapsed_ms: recovery_origin
                        .unwrap_or_else(Instant::now)
                        .elapsed()
                        .as_millis() as u64,
                    permission_checks,
                },
            )?;
            return Ok(());
        }

        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    Err(test_error(format!(
        "observer did not converge after durable generation advanced from {initial_generation} to {durable_generation}"
    )))
}

async fn run_mutator(
    db: DatabaseConnection,
    cache: rustok_cache::CacheService,
    tenant_id: Uuid,
    user_id: Uuid,
    result_path: PathBuf,
) -> TestResult<()> {
    RbacService::replace_user_role_committed(&db, &user_id, &tenant_id, UserRole::Customer).await?;
    let committed_generation = rustok_rbac::read_permission_invalidation_generation(&db).await?;
    let stats = cache.invalidations().stats();
    write_json(
        &result_path,
        &MutatorResult {
            committed_generation,
            redis_configured: cache.redis_url().is_some(),
            redis_publish_success_total: stats.redis_publish_success_total,
            redis_publish_failure_total: stats.redis_publish_failure_total,
        },
    )?;
    Ok(())
}

fn assert_terminal_deny(
    observation: &ObserverResult,
    committed_generation: u64,
) -> TestResult<()> {
    if observation.durable_generation != committed_generation
        || observation.final_allowed
        || observation.authoritative_allowed
        || observation.permission_checks < 2
    {
        return Err(test_error("observer did not converge to the authoritative deny at the committed generation"));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn spawn_child(
    role: &str,
    scenario: &str,
    database_url: &str,
    redis_url: &str,
    tenant_id: Uuid,
    user_id: Uuid,
    ready_path: Option<&Path>,
    stale_path: Option<&Path>,
    restart_marker_path: Option<&Path>,
    result_path: &Path,
) -> TestResult<ProcessChild> {
    let executable = std::env::current_exe()?;
    let mut command = ProcessCommand::new(executable);
    command
        .arg("--ignored")
        .arg("--exact")
        .arg(CHILD_TEST_NAME)
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env(CHILD_ROLE_ENV, role)
        .env(CHILD_SCENARIO_ENV, scenario)
        .env(CHILD_DATABASE_URL_ENV, database_url)
        .env(CHILD_REDIS_URL_ENV, redis_url)
        .env(CHILD_TENANT_ID_ENV, tenant_id.to_string())
        .env(CHILD_USER_ID_ENV, user_id.to_string())
        .env(CHILD_RESULT_PATH_ENV, result_path)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if let Some(path) = ready_path {
        command.env(CHILD_READY_PATH_ENV, path);
    }
    if let Some(path) = stale_path {
        command.env(CHILD_STALE_PATH_ENV, path);
    }
    if let Some(path) = restart_marker_path {
        command.env(CHILD_RESTART_MARKER_PATH_ENV, path);
    }
    Ok(command.spawn()?)
}

async fn wait_for_child(
    mut child: ProcessChild,
    timeout: Duration,
    label: &str,
) -> TestResult<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            if status.success() {
                return Ok(());
            }
            return Err(test_error(format!("{label} exited with {status}")));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(test_error(format!("{label} exceeded {timeout:?}")));
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

fn reserve_loopback_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("loopback port should be reservable")
        .local_addr()
        .expect("reserved loopback address")
        .port()
}

async fn spawn_redis(binary: &str, port: u16) -> RedisChild {
    let child = RedisCommand::new(binary)
        .arg("--bind")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string())
        .arg("--save")
        .arg("")
        .arg("--appendonly")
        .arg("no")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("redis-server should start");
    wait_for_redis(&format!("redis://127.0.0.1:{port}/"))
        .await
        .expect("spawned Redis should become ready");
    child
}

async fn stop_redis(child: &mut RedisChild) {
    if child.id().is_some() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}

async fn wait_for_redis(url: &str) -> TestResult<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(client) = redis::Client::open(url)
                && let Ok(mut connection) = client.get_multiplexed_async_connection().await
            {
                let pong = redis::cmd("PING")
                    .query_async::<String>(&mut connection)
                    .await;
                if pong.as_deref() == Ok("PONG") {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .map_err(|_| test_error("Redis did not become ready"))?;
    Ok(())
}

async fn wait_for_redis_subscribers(
    url: &str,
    channel: &str,
    minimum: u64,
    timeout: Duration,
) -> TestResult<()> {
    let client = redis::Client::open(url)?;
    tokio::time::timeout(timeout, async {
        loop {
            if let Ok(mut connection) = client.get_multiplexed_async_connection().await {
                let response = redis::cmd("PUBSUB")
                    .arg("NUMSUB")
                    .arg(channel)
                    .query_async::<(String, u64)>(&mut connection)
                    .await;
                if response.is_ok_and(|(_, subscribers)| subscribers >= minimum) {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .map_err(|_| test_error(format!("Redis subscriber count for {channel} did not reach {minimum}")))?;
    Ok(())
}

async fn wait_for_file(path: &Path, timeout: Duration) -> TestResult<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.is_file() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Err(test_error(format!(
        "timed out waiting for child evidence file {}",
        path.display()
    )))
}

fn required_env(name: &str) -> TestResult<String> {
    std::env::var(name)
        .map_err(|_| test_error(format!("required environment variable {name} is missing")))
}

fn optional_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from)
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

async fn with_rbac_postgres_database<T, F, Fut>(prefix: &str, test: F) -> TestResult<T>
where
    F: FnOnce(DatabaseConnection, String) -> Fut,
    Fut: Future<Output = TestResult<T>>,
{
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);
    let database_name = unique_postgres_database_name(prefix);
    let target_url = postgres_database_url(&admin_url, &database_name);
    let admin = connect_postgres(&admin_url).await?;
    drop_postgres_database_if_exists(&admin, &database_name).await?;
    create_postgres_database(&admin, &database_name).await?;

    let result = async {
        let db = connect_postgres(&target_url).await?;
        Migrator::up(&db, None).await?;
        let result = test(db.clone(), target_url.clone()).await;
        db.close().await?;
        result
    }
    .await;

    drop_postgres_database_if_exists(&admin, &database_name).await?;
    admin.close().await?;
    result
}

async fn insert_tenant(
    db: &DatabaseConnection,
    scenario: &str,
) -> TestResult<Uuid> {
    let tenant_id = Uuid::new_v4();
    tenants::Entity::insert(tenants::ActiveModel {
        id: Set(tenant_id),
        name: Set(format!("RBAC Redis {scenario} recovery")),
        slug: Set(format!("rbac-redis-{scenario}-{tenant_id}")),
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

async fn insert_user(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    scenario: &str,
) -> TestResult<Uuid> {
    let user_id = Uuid::new_v4();
    users::Entity::insert(users::ActiveModel {
        id: Set(user_id),
        tenant_id: Set(tenant_id),
        email: Set(format!("rbac-redis-{scenario}-{user_id}@example.com")),
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
