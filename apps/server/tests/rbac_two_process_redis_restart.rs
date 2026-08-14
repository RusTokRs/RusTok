use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;
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
use tokio::process::{Child, Command};
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const REDIS_SERVER_BIN_ENV: &str = "RUSTOK_CACHE_REDIS_SERVER_BIN";
const CHILD_ROLE_ENV: &str = "RUSTOK_RBAC_REDIS_CHILD_ROLE";
const CHILD_DATABASE_URL_ENV: &str = "RUSTOK_RBAC_REDIS_DATABASE_URL";
const CHILD_REDIS_URL_ENV: &str = "RUSTOK_RBAC_REDIS_URL";
const CHILD_TENANT_ID_ENV: &str = "RUSTOK_RBAC_REDIS_TENANT_ID";
const CHILD_USER_ID_ENV: &str = "RUSTOK_RBAC_REDIS_USER_ID";
const CHILD_FAST_USER_ID_ENV: &str = "RUSTOK_RBAC_REDIS_FAST_USER_ID";
const CHILD_RESTART_USER_ID_ENV: &str = "RUSTOK_RBAC_REDIS_RESTART_USER_ID";
const CHILD_READY_PATH_ENV: &str = "RUSTOK_RBAC_REDIS_READY_PATH";
const CHILD_FAST_RESULT_PATH_ENV: &str = "RUSTOK_RBAC_REDIS_FAST_RESULT_PATH";
const CHILD_OUTAGE_REQUEST_PATH_ENV: &str = "RUSTOK_RBAC_REDIS_OUTAGE_REQUEST_PATH";
const CHILD_OUTAGE_RESULT_PATH_ENV: &str = "RUSTOK_RBAC_REDIS_OUTAGE_RESULT_PATH";
const CHILD_RESTART_RESULT_PATH_ENV: &str = "RUSTOK_RBAC_REDIS_RESTART_RESULT_PATH";
const CHILD_RESTART_ACK_PATH_ENV: &str = "RUSTOK_RBAC_REDIS_RESTART_ACK_PATH";
const CHILD_MUTATION_RESULT_PATH_ENV: &str = "RUSTOK_RBAC_REDIS_MUTATION_RESULT_PATH";
const CHILD_TEST_NAME: &str = "rbac_redis_replica_child";
const FAST_PATH_BOUND: Duration = Duration::from_secs(3);
const RESTART_RECOVERY_BOUND: Duration = Duration::from_secs(8);
const REPLICA_SEQUENCE_BOUND: Duration = Duration::from_secs(25);
const CHILD_LIFETIME_BOUND: Duration = Duration::from_secs(20);

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Serialize, Deserialize)]
struct ObserverReady {
    initial_generation: u64,
    fast_user_allowed: bool,
    restart_user_allowed: bool,
    redis_configured: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct DecisionResult {
    allowed: bool,
    authoritative_allowed: bool,
    redis_configured: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct MutationResult {
    committed_generation: u64,
    redis_configured: bool,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access, redis-server and subprocess execution"]
async fn separate_process_redis_fast_path_survives_restart_and_recovers_missed_publication() {
    run_two_process_redis_scenario()
        .await
        .unwrap_or_else(|error| panic!("RBAC two-process Redis recovery evidence failed: {error}"));
}

async fn run_two_process_redis_scenario() -> TestResult<()> {
    let redis_binary = required_env(REDIS_SERVER_BIN_ENV)?;
    let redis_port = reserve_loopback_port()?;
    let redis_url = format!("redis://127.0.0.1:{redis_port}/");
    let mut redis_process = spawn_redis(redis_binary.as_str(), redis_port).await?;

    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);
    let database_name = unique_postgres_database_name("rustok_rbac_two_process_redis");
    let target_url = postgres_database_url(&admin_url, &database_name);
    let admin = connect_postgres(&admin_url).await?;
    drop_postgres_database_if_exists(&admin, &database_name).await?;
    create_postgres_database(&admin, &database_name).await?;

    let scenario_result = async {
        let db = connect_postgres(&target_url).await?;
        Migrator::up(&db, None).await?;
        let tenant_id = insert_tenant(&db).await?;
        let fast_user_id = insert_user(&db, tenant_id, "fast").await?;
        let restart_user_id = insert_user(&db, tenant_id, "restart").await?;
        let writer = RbacRoleAssignmentDbWriter::new(db.clone());
        writer
            .assign_role_permissions(tenant_id, fast_user_id, UserRole::Admin)
            .await?;
        writer
            .assign_role_permissions(tenant_id, restart_user_id, UserRole::Admin)
            .await?;

        let workspace = tempfile::tempdir()?;
        let ready_path = workspace.path().join("observer-ready.json");
        let fast_result_path = workspace.path().join("fast-result.json");
        let outage_request_path = workspace.path().join("outage-check.request");
        let outage_result_path = workspace.path().join("outage-result.json");
        let restart_result_path = workspace.path().join("restart-result.json");
        let restart_ack_path = workspace.path().join("restart-result.ack");
        let first_mutation_path = workspace.path().join("first-mutation.json");
        let second_mutation_path = workspace.path().join("second-mutation.json");
        let replica_sequence_started = Instant::now();

        let mut observer = spawn_observer(
            &target_url,
            &redis_url,
            tenant_id,
            fast_user_id,
            restart_user_id,
            &ready_path,
            &fast_result_path,
            &outage_request_path,
            &outage_result_path,
            &restart_result_path,
            &restart_ack_path,
        )?;
        wait_for_file(&ready_path, Duration::from_secs(8)).await?;
        let ready: ObserverReady = read_json(&ready_path)?;
        if !ready.redis_configured || !ready.fast_user_allowed || !ready.restart_user_allowed {
            return Err(test_error(format!("invalid observer readiness evidence: {ready:?}")));
        }
        if ready.initial_generation != 0 {
            return Err(test_error(format!(
                "unexpected initial durable generation {}",
                ready.initial_generation
            )));
        }
        wait_for_redis_subscribers(&redis_url, 1, "parent initial observer subscription").await?;

        let fast_started = Instant::now();
        let mut first_mutator = spawn_mutator(
            &target_url,
            &redis_url,
            tenant_id,
            fast_user_id,
            &first_mutation_path,
        )?;
        wait_for_child(&mut first_mutator, Duration::from_secs(10), "first mutator").await?;
        wait_for_file(&fast_result_path, FAST_PATH_BOUND).await?;
        if fast_started.elapsed() > FAST_PATH_BOUND {
            return Err(test_error("Redis fast-path decision exceeded the three-second bound"));
        }
        let first_mutation: MutationResult = read_json(&first_mutation_path)?;
        let fast_result: DecisionResult = read_json(&fast_result_path)?;
        if first_mutation.committed_generation != 1
            || !first_mutation.redis_configured
            || fast_result.allowed
            || fast_result.authoritative_allowed
            || !fast_result.redis_configured
        {
            return Err(test_error(format!(
                "invalid Redis fast-path evidence: mutation={first_mutation:?}, decision={fast_result:?}"
            )));
        }

        stop_redis(&mut redis_process).await?;

        let mut second_mutator = spawn_mutator(
            &target_url,
            &redis_url,
            tenant_id,
            restart_user_id,
            &second_mutation_path,
        )?;
        wait_for_child(&mut second_mutator, Duration::from_secs(10), "outage mutator").await?;
        let second_mutation: MutationResult = read_json(&second_mutation_path)?;
        if second_mutation.committed_generation != 2 || !second_mutation.redis_configured {
            return Err(test_error(format!(
                "invalid Redis-outage mutation evidence: {second_mutation:?}"
            )));
        }

        std::fs::write(&outage_request_path, b"check-stale")?;
        wait_for_file(&outage_result_path, Duration::from_secs(3)).await?;
        let outage_result: DecisionResult = read_json(&outage_result_path)?;
        if !outage_result.allowed
            || outage_result.authoritative_allowed
            || !outage_result.redis_configured
        {
            return Err(test_error(format!(
                "observer did not retain only the process-cache allow during outage: {outage_result:?}"
            )));
        }

        let restart_started = Instant::now();
        redis_process = spawn_redis(redis_binary.as_str(), redis_port).await?;
        wait_for_redis_subscribers(&redis_url, 1, "observer resubscription after Redis restart")
            .await?;
        wait_for_file(&restart_result_path, RESTART_RECOVERY_BOUND).await?;
        if restart_started.elapsed() > RESTART_RECOVERY_BOUND {
            return Err(test_error("Redis reconnect recovery exceeded the eight-second bound"));
        }
        let restart_result: DecisionResult = read_json(&restart_result_path)?;
        if restart_result.allowed
            || restart_result.authoritative_allowed
            || !restart_result.redis_configured
        {
            return Err(test_error(format!(
                "existing observer did not recover after Redis restart: {restart_result:?}"
            )));
        }
        if replica_sequence_started.elapsed() > REPLICA_SEQUENCE_BOUND {
            return Err(test_error(
                "replica sequence crossed the 25-second poll-exclusion bound",
            ));
        }

        std::fs::write(&restart_ack_path, b"release-observer")?;
        wait_for_child(&mut observer, CHILD_LIFETIME_BOUND, "observer").await?;
        db.close().await?;
        Ok(())
    }
    .await;

    let _ = stop_redis(&mut redis_process).await;
    drop_postgres_database_if_exists(&admin, &database_name).await?;
    admin.close().await?;
    scenario_result
}

#[test]
#[ignore = "subprocess entry point for the two-process RBAC Redis harness"]
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
        .block_on(run_child(role.as_str()))
        .unwrap_or_else(|error| panic!("RBAC Redis child {role} failed: {error}"));
}

async fn run_child(role: &str) -> TestResult<()> {
    let database_url = required_env(CHILD_DATABASE_URL_ENV)?;
    let redis_url = required_env(CHILD_REDIS_URL_ENV)?;
    let tenant_id = Uuid::parse_str(&required_env(CHILD_TENANT_ID_ENV)?)?;
    let db = connect_postgres(&database_url).await?;
    let context = ServerRuntimeContext::new(db.clone(), settings_with_redis(redis_url.as_str()));
    let cache = ensure_cache_service(&context);
    if !cache.redis_client_initialized() {
        return Err(test_error(
            "RBAC Redis child did not initialize the Redis client",
        ));
    }
    start_rbac_cache_invalidation_listener(&context, cache.clone()).await?;

    match role {
        "observer" => {
            wait_for_redis_subscribers(
                redis_url.as_str(),
                1,
                "observer child initial subscription",
            )
            .await?;
            let fast_user_id = Uuid::parse_str(&required_env(CHILD_FAST_USER_ID_ENV)?)?;
            let restart_user_id = Uuid::parse_str(&required_env(CHILD_RESTART_USER_ID_ENV)?)?;
            run_observer(
                db,
                tenant_id,
                fast_user_id,
                restart_user_id,
                cache.redis_configuration_present(),
                PathBuf::from(required_env(CHILD_READY_PATH_ENV)?),
                PathBuf::from(required_env(CHILD_FAST_RESULT_PATH_ENV)?),
                PathBuf::from(required_env(CHILD_OUTAGE_REQUEST_PATH_ENV)?),
                PathBuf::from(required_env(CHILD_OUTAGE_RESULT_PATH_ENV)?),
                PathBuf::from(required_env(CHILD_RESTART_RESULT_PATH_ENV)?),
                PathBuf::from(required_env(CHILD_RESTART_ACK_PATH_ENV)?),
            )
            .await
        }
        "mutator" => {
            let user_id = Uuid::parse_str(&required_env(CHILD_USER_ID_ENV)?)?;
            run_mutator(
                db,
                tenant_id,
                user_id,
                cache.redis_configuration_present(),
                PathBuf::from(required_env(CHILD_MUTATION_RESULT_PATH_ENV)?),
            )
            .await
        }
        other => Err(test_error(format!(
            "unsupported RBAC Redis child role {other}"
        ))),
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_observer(
    db: DatabaseConnection,
    tenant_id: Uuid,
    fast_user_id: Uuid,
    restart_user_id: Uuid,
    redis_configured: bool,
    ready_path: PathBuf,
    fast_result_path: PathBuf,
    outage_request_path: PathBuf,
    outage_result_path: PathBuf,
    restart_result_path: PathBuf,
    restart_ack_path: PathBuf,
) -> TestResult<()> {
    let initial_generation = rustok_rbac::read_permission_invalidation_generation(&db).await?;
    let fast_user_allowed =
        RbacService::has_permission(&db, &tenant_id, &fast_user_id, &Permission::SETTINGS_MANAGE)
            .await?;
    let restart_user_allowed = RbacService::has_permission(
        &db,
        &tenant_id,
        &restart_user_id,
        &Permission::SETTINGS_MANAGE,
    )
    .await?;
    write_json(
        &ready_path,
        &ObserverReady {
            initial_generation,
            fast_user_allowed,
            restart_user_allowed,
            redis_configured,
        },
    )?;

    let fast_allowed =
        wait_for_permission(&db, tenant_id, fast_user_id, false, FAST_PATH_BOUND).await?;
    let fast_authoritative =
        RbacService::get_user_permissions_authoritative(&db, &tenant_id, &fast_user_id).await?;
    write_json(
        &fast_result_path,
        &DecisionResult {
            allowed: fast_allowed,
            authoritative_allowed: fast_authoritative.contains(&Permission::SETTINGS_MANAGE),
            redis_configured,
        },
    )?;

    wait_for_file(&outage_request_path, Duration::from_secs(12)).await?;
    let outage_allowed = RbacService::has_permission(
        &db,
        &tenant_id,
        &restart_user_id,
        &Permission::SETTINGS_MANAGE,
    )
    .await?;
    let outage_authoritative =
        RbacService::get_user_permissions_authoritative(&db, &tenant_id, &restart_user_id).await?;
    write_json(
        &outage_result_path,
        &DecisionResult {
            allowed: outage_allowed,
            authoritative_allowed: outage_authoritative.contains(&Permission::SETTINGS_MANAGE),
            redis_configured,
        },
    )?;

    let restart_allowed = wait_for_permission(
        &db,
        tenant_id,
        restart_user_id,
        false,
        RESTART_RECOVERY_BOUND,
    )
    .await?;
    let restart_authoritative =
        RbacService::get_user_permissions_authoritative(&db, &tenant_id, &restart_user_id).await?;
    write_json(
        &restart_result_path,
        &DecisionResult {
            allowed: restart_allowed,
            authoritative_allowed: restart_authoritative.contains(&Permission::SETTINGS_MANAGE),
            redis_configured,
        },
    )?;
    wait_for_file(&restart_ack_path, Duration::from_secs(3)).await?;
    Ok(())
}

async fn run_mutator(
    db: DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    redis_configured: bool,
    result_path: PathBuf,
) -> TestResult<()> {
    RbacService::replace_user_role_committed(&db, &user_id, &tenant_id, UserRole::Customer).await?;
    let committed_generation = rustok_rbac::read_permission_invalidation_generation(&db).await?;
    write_json(
        &result_path,
        &MutationResult {
            committed_generation,
            redis_configured,
        },
    )?;
    Ok(())
}

async fn wait_for_permission(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    expected: bool,
    timeout: Duration,
) -> TestResult<bool> {
    tokio::time::timeout(timeout, async {
        loop {
            let allowed =
                RbacService::has_permission(db, &tenant_id, &user_id, &Permission::SETTINGS_MANAGE)
                    .await?;
            if allowed == expected {
                return Ok::<bool, Box<dyn std::error::Error + Send + Sync>>(allowed);
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .map_err(|_| {
        test_error(format!(
            "permission did not converge to {expected} before {timeout:?}"
        ))
    })?
}

#[allow(clippy::too_many_arguments)]
fn spawn_observer(
    database_url: &str,
    redis_url: &str,
    tenant_id: Uuid,
    fast_user_id: Uuid,
    restart_user_id: Uuid,
    ready_path: &Path,
    fast_result_path: &Path,
    outage_request_path: &Path,
    outage_result_path: &Path,
    restart_result_path: &Path,
    restart_ack_path: &Path,
) -> TestResult<Child> {
    let mut command = child_command("observer", database_url, redis_url, tenant_id)?;
    command
        .env(CHILD_FAST_USER_ID_ENV, fast_user_id.to_string())
        .env(CHILD_RESTART_USER_ID_ENV, restart_user_id.to_string())
        .env(CHILD_READY_PATH_ENV, ready_path)
        .env(CHILD_FAST_RESULT_PATH_ENV, fast_result_path)
        .env(CHILD_OUTAGE_REQUEST_PATH_ENV, outage_request_path)
        .env(CHILD_OUTAGE_RESULT_PATH_ENV, outage_result_path)
        .env(CHILD_RESTART_RESULT_PATH_ENV, restart_result_path)
        .env(CHILD_RESTART_ACK_PATH_ENV, restart_ack_path);
    Ok(command.spawn()?)
}

fn spawn_mutator(
    database_url: &str,
    redis_url: &str,
    tenant_id: Uuid,
    user_id: Uuid,
    result_path: &Path,
) -> TestResult<Child> {
    let mut command = child_command("mutator", database_url, redis_url, tenant_id)?;
    command
        .env(CHILD_USER_ID_ENV, user_id.to_string())
        .env(CHILD_MUTATION_RESULT_PATH_ENV, result_path);
    Ok(command.spawn()?)
}

fn child_command(
    role: &str,
    database_url: &str,
    redis_url: &str,
    tenant_id: Uuid,
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
        .env(CHILD_REDIS_URL_ENV, redis_url)
        .env(CHILD_TENANT_ID_ENV, tenant_id.to_string())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    Ok(command)
}

fn settings_with_redis(url: &str) -> RustokSettings {
    let mut settings = RustokSettings::default();
    settings.cache.redis_url = Some(url.to_string());
    settings
}

fn reserve_loopback_port() -> TestResult<u16> {
    Ok(TcpListener::bind(("127.0.0.1", 0))?.local_addr()?.port())
}

async fn spawn_redis(binary: &str, port: u16) -> TestResult<Child> {
    let child = Command::new(binary)
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
    wait_for_redis(format!("redis://127.0.0.1:{port}/").as_str()).await?;
    Ok(child)
}

async fn stop_redis(child: &mut Child) -> TestResult<()> {
    if child.try_wait()?.is_none() {
        child.kill().await?;
    }
    let _ = child.wait().await?;
    Ok(())
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
    .map_err(|_| test_error("spawned Redis did not become ready"))?;
    Ok(())
}

async fn wait_for_redis_subscribers(url: &str, expected: usize, stage: &str) -> TestResult<()> {
    tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            if let Ok(client) = redis::Client::open(url)
                && let Ok(mut connection) = client.get_multiplexed_async_connection().await
            {
                let counts = redis::cmd("PUBSUB")
                    .arg("NUMSUB")
                    .arg(RBAC_PERMISSION_INVALIDATION_CHANNEL)
                    .query_async::<Vec<(String, usize)>>(&mut connection)
                    .await;
                if counts
                    .ok()
                    .and_then(|counts| counts.into_iter().next())
                    .is_some_and(|(_, count)| count >= expected)
                {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .map_err(|_| {
        test_error(format!(
            "Redis did not expose {expected} RBAC subscribers during {stage}"
        ))
    })?;
    Ok(())
}

async fn wait_for_child(child: &mut Child, timeout: Duration, label: &str) -> TestResult<()> {
    tokio::time::timeout(timeout, async {
        let status = child.wait().await?;
        if status.success() {
            Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
        } else {
            Err(test_error(format!("{label} exited with {status}")))
        }
    })
    .await
    .map_err(|_| test_error(format!("{label} exceeded {timeout:?}")))?
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
        name: Set("RBAC two-process Redis recovery".to_string()),
        slug: Set(format!("rbac-two-process-redis-{tenant_id}")),
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
        email: Set(format!("rbac-redis-{suffix}-{user_id}@example.com")),
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
