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
const CHILD_DATABASE_URL_ENV: &str = "RUSTOK_RBAC_REDIS_CHILD_DATABASE_URL";
const CHILD_REDIS_URL_ENV: &str = "RUSTOK_RBAC_REDIS_CHILD_REDIS_URL";
const CHILD_TENANT_ID_ENV: &str = "RUSTOK_RBAC_REDIS_CHILD_TENANT_ID";
const CHILD_USER_ID_ENV: &str = "RUSTOK_RBAC_REDIS_CHILD_USER_ID";
const CHILD_RESULT_PATH_ENV: &str = "RUSTOK_RBAC_REDIS_CHILD_RESULT_PATH";
const CHILD_TEST_NAME: &str = "rbac_redis_mutator_child";
const FAST_PATH_BOUND: Duration = Duration::from_secs(3);
const RESTART_RECOVERY_BOUND: Duration = Duration::from_secs(5);

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Serialize, Deserialize)]
struct MutatorResult {
    committed_generation: u64,
    redis_configured: bool,
    redis_publish_success_total: u64,
    redis_publish_failure_total: u64,
}

#[derive(Debug)]
struct RecoveryResult {
    durable_generation: u64,
    authoritative_allowed: bool,
    elapsed: Duration,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial_test::serial]
#[ignore = "requires PostgreSQL admin access, redis-server and subprocess execution"]
async fn separate_process_replica_applies_available_redis_invalidation() {
    let binary = required_env(REDIS_SERVER_BIN_ENV).unwrap();
    let result = with_rbac_postgres_database("rustok_rbac_redis_available", |db, target_url| {
        let binary = binary.clone();
        async move {
            let port = reserve_loopback_port();
            let redis_url = format!("redis://127.0.0.1:{port}/");
            let mut redis_process = spawn_redis(&binary, port).await?;

            let result = async {
                let (tenant_id, user_id) = seed_admin_user(&db, "available").await?;
                let initial_generation = rustok_rbac::read_permission_invalidation_generation(&db).await?;
                let mut settings = RustokSettings::default();
                settings.cache.redis_url = Some(redis_url.clone());
                let context = ServerRuntimeContext::new(db.clone(), settings);
                let cache = ensure_cache_service(&context);
                start_rbac_cache_invalidation_listener(&context, cache.clone()).await?;
                assert_healthy_redis(&cache).await?;
                assert!(RbacService::has_permission(
                    &db,
                    &tenant_id,
                    &user_id,
                    &Permission::SETTINGS_MANAGE,
                )
                .await?);
                wait_for_redis_subscribers(
                    &redis_url,
                    RBAC_PERMISSION_INVALIDATION_CHANNEL,
                    1,
                    Duration::from_secs(5),
                )
                .await?;

                let workspace = tempfile::tempdir()?;
                let result_path = workspace.path().join("mutator-result.json");
                let mutator = spawn_mutator(
                    &target_url,
                    &redis_url,
                    tenant_id,
                    user_id,
                    &result_path,
                )?;
                wait_for_child(mutator, Duration::from_secs(12), "available Redis mutator").await?;
                let mutation: MutatorResult = read_json(&result_path)?;

                if mutation.committed_generation != initial_generation + 1
                    || !mutation.redis_configured
                    || mutation.redis_publish_success_total == 0
                    || mutation.redis_publish_failure_total != 0
                {
                    return Err(test_error(
                        "available Redis mutation did not commit and publish exactly once through the canonical fast path",
                    ));
                }

                let recovery = wait_for_terminal_deny(
                    &db,
                    tenant_id,
                    user_id,
                    initial_generation,
                    FAST_PATH_BOUND,
                )
                .await?;
                if recovery.durable_generation != mutation.committed_generation
                    || recovery.authoritative_allowed
                    || recovery.elapsed > FAST_PATH_BOUND
                {
                    return Err(test_error(
                        "available Redis observer did not converge to the authoritative deny at the committed generation",
                    ));
                }
                assert_healthy_redis(&cache).await?;
                Ok(())
            }
            .await;

            stop_redis(&mut redis_process).await;
            result
        }
    })
    .await;

    result.unwrap_or_else(|error| panic!("RBAC available Redis evidence failed: {error}"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial_test::serial]
#[ignore = "requires PostgreSQL admin access, redis-server and subprocess execution"]
async fn separate_process_replica_recovers_after_redis_restart_and_resubscribe() {
    let binary = required_env(REDIS_SERVER_BIN_ENV).unwrap();
    let result = with_rbac_postgres_database("rustok_rbac_redis_restart", |db, target_url| {
        let binary = binary.clone();
        async move {
            let port = reserve_loopback_port();
            let redis_url = format!("redis://127.0.0.1:{port}/");
            let mut redis_process = spawn_redis(&binary, port).await?;

            let result = async {
                let (tenant_id, user_id) = seed_admin_user(&db, "restart").await?;
                let initial_generation = rustok_rbac::read_permission_invalidation_generation(&db).await?;
                let mut settings = RustokSettings::default();
                settings.cache.redis_url = Some(redis_url.clone());
                let context = ServerRuntimeContext::new(db.clone(), settings);
                let cache = ensure_cache_service(&context);
                start_rbac_cache_invalidation_listener(&context, cache.clone()).await?;
                assert_healthy_redis(&cache).await?;
                assert!(RbacService::has_permission(
                    &db,
                    &tenant_id,
                    &user_id,
                    &Permission::SETTINGS_MANAGE,
                )
                .await?);
                wait_for_redis_subscribers(
                    &redis_url,
                    RBAC_PERMISSION_INVALIDATION_CHANNEL,
                    1,
                    Duration::from_secs(5),
                )
                .await?;

                stop_redis(&mut redis_process).await;

                let workspace = tempfile::tempdir()?;
                let result_path = workspace.path().join("mutator-result.json");
                let mutator = spawn_mutator(
                    &target_url,
                    &redis_url,
                    tenant_id,
                    user_id,
                    &result_path,
                )?;
                wait_for_child(mutator, Duration::from_secs(14), "stopped Redis mutator").await?;
                let mutation: MutatorResult = read_json(&result_path)?;

                if mutation.committed_generation != initial_generation + 1
                    || !mutation.redis_configured
                    || mutation.redis_publish_failure_total == 0
                    || mutation.redis_publish_success_total != 0
                {
                    return Err(test_error(
                        "stopped Redis mutation did not commit while recording deferred canonical publication",
                    ));
                }

                let durable_generation = rustok_rbac::read_permission_invalidation_generation(&db).await?;
                let stale_allowed = RbacService::has_permission(
                    &db,
                    &tenant_id,
                    &user_id,
                    &Permission::SETTINGS_MANAGE,
                )
                .await?;
                if durable_generation != mutation.committed_generation || !stale_allowed {
                    return Err(test_error(
                        "observer did not retain the intentionally missed stale allow before Redis restart",
                    ));
                }

                let restart_started = Instant::now();
                redis_process = spawn_redis(&binary, port).await?;
                wait_for_redis_subscribers(
                    &redis_url,
                    RBAC_PERMISSION_INVALIDATION_CHANNEL,
                    1,
                    RESTART_RECOVERY_BOUND,
                )
                .await?;
                let remaining = RESTART_RECOVERY_BOUND
                    .checked_sub(restart_started.elapsed())
                    .ok_or_else(|| test_error("Redis resubscribe exceeded the recovery bound"))?;
                let recovery = wait_for_terminal_deny(
                    &db,
                    tenant_id,
                    user_id,
                    initial_generation,
                    remaining,
                )
                .await?;

                if recovery.durable_generation != mutation.committed_generation
                    || recovery.authoritative_allowed
                    || recovery.elapsed > remaining
                    || restart_started.elapsed() > RESTART_RECOVERY_BOUND
                {
                    return Err(test_error(
                        "restarted Redis observer did not recover through subscriber-ready durable reconciliation",
                    ));
                }
                assert_healthy_redis(&cache).await?;
                Ok(())
            }
            .await;

            stop_redis(&mut redis_process).await;
            result
        }
    })
    .await;

    result.unwrap_or_else(|error| panic!("RBAC Redis restart evidence failed: {error}"));
}

#[test]
#[ignore = "subprocess entry point for RBAC Redis recovery harness"]
fn rbac_redis_mutator_child() {
    let Ok(role) = std::env::var(CHILD_ROLE_ENV) else {
        return;
    };
    assert_eq!(role, "mutator", "unsupported RBAC Redis child role");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("build RBAC Redis mutator runtime");
    runtime
        .block_on(run_mutator_child())
        .unwrap_or_else(|error| panic!("RBAC Redis mutator child failed: {error}"));
}

async fn run_mutator_child() -> TestResult<()> {
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
    db.close().await?;
    Ok(())
}

async fn wait_for_terminal_deny(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    initial_generation: u64,
    timeout: Duration,
) -> TestResult<RecoveryResult> {
    let started = Instant::now();
    let deadline = started + timeout;
    let mut durable_generation = initial_generation;
    while Instant::now() < deadline {
        durable_generation = rustok_rbac::read_permission_invalidation_generation(db).await?;
        let allowed = RbacService::has_permission(
            db,
            &tenant_id,
            &user_id,
            &Permission::SETTINGS_MANAGE,
        )
        .await?;
        if durable_generation > initial_generation && !allowed {
            let authoritative = RbacService::get_user_permissions_authoritative(
                db,
                &tenant_id,
                &user_id,
            )
            .await?;
            return Ok(RecoveryResult {
                durable_generation,
                authoritative_allowed: authoritative.contains(&Permission::SETTINGS_MANAGE),
                elapsed: started.elapsed(),
            });
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Err(test_error(format!(
        "observer did not reach deny after generation advanced from {initial_generation} to {durable_generation} within {timeout:?}",
    )))
}

fn spawn_mutator(
    database_url: &str,
    redis_url: &str,
    tenant_id: Uuid,
    user_id: Uuid,
    result_path: &Path,
) -> TestResult<ProcessChild> {
    let executable = std::env::current_exe()?;
    Ok(ProcessCommand::new(executable)
        .arg("--ignored")
        .arg("--exact")
        .arg(CHILD_TEST_NAME)
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env(CHILD_ROLE_ENV, "mutator")
        .env(CHILD_DATABASE_URL_ENV, database_url)
        .env(CHILD_REDIS_URL_ENV, redis_url)
        .env(CHILD_TENANT_ID_ENV, tenant_id.to_string())
        .env(CHILD_USER_ID_ENV, user_id.to_string())
        .env(CHILD_RESULT_PATH_ENV, result_path)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?)
}

async fn wait_for_child(
    mut child: ProcessChild,
    timeout: Duration,
    label: &str,
) -> TestResult<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return if status.success() {
                Ok(())
            } else {
                Err(test_error(format!("{label} exited with {status}")))
            };
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

async fn spawn_redis(binary: &str, port: u16) -> TestResult<RedisChild> {
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
        .spawn()?;
    wait_for_redis(&format!("redis://127.0.0.1:{port}/"), Duration::from_secs(5)).await?;
    Ok(child)
}

async fn stop_redis(child: &mut RedisChild) {
    if child.id().is_some() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}

async fn wait_for_redis(url: &str, timeout: Duration) -> TestResult<()> {
    tokio::time::timeout(timeout, async {
        loop {
            if let Ok(client) = redis::Client::open(url)
                && let Ok(mut connection) = client.get_multiplexed_async_connection().await
            {
                let pong = redis::cmd("PING")
                    .query_async::<String>(&mut connection)
                    .await;
                if matches!(pong.as_deref(), Ok("PONG")) {
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
                    .query_async::<std::collections::HashMap<String, u64>>(&mut connection)
                    .await;
                if response.is_ok_and(|counts| {
                    counts.get(channel).copied().unwrap_or_default() >= minimum
                }) {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .map_err(|_| test_error(format!(
        "Redis subscriber count for {channel} did not reach {minimum}",
    )))?;
    Ok(())
}

async fn assert_healthy_redis(cache: &rustok_cache::CacheService) -> TestResult<()> {
    let health = cache.health().await;
    if !health.redis_configured || !health.redis_healthy {
        return Err(test_error(format!(
            "Redis health is not ready: {}",
            health.redis_error.unwrap_or_else(|| "unknown error".to_string()),
        )));
    }
    Ok(())
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

async fn seed_admin_user(
    db: &DatabaseConnection,
    scenario: &str,
) -> TestResult<(Uuid, Uuid)> {
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

    RbacRoleAssignmentDbWriter::new(db.clone())
        .assign_role_permissions(tenant_id, user_id, UserRole::Admin)
        .await?;
    Ok((tenant_id, user_id))
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

fn required_env(name: &str) -> TestResult<String> {
    std::env::var(name)
        .map_err(|_| test_error(format!("required environment variable {name} is missing")))
}

fn test_error(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::other(message.into()))
}
