use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use chrono::Utc;
use rustok_api::Permission;
use rustok_core::{UserRole, UserStatus};
use rustok_migrations::Migrator;
use rustok_rbac::RbacRoleAssignmentDbWriter;
use rustok_server::common::settings::RustokSettings;
use rustok_server::models::{tenants, users};
use rustok_server::services::cache_runtime::ensure_cache_service;
use rustok_server::services::rbac_cache_invalidation::start_rbac_cache_invalidation_listener;
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
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const CHILD_ROLE_ENV: &str = "RUSTOK_RBAC_REPLICA_CHILD_ROLE";
const CHILD_DATABASE_URL_ENV: &str = "RUSTOK_RBAC_REPLICA_DATABASE_URL";
const CHILD_TENANT_ID_ENV: &str = "RUSTOK_RBAC_REPLICA_TENANT_ID";
const CHILD_USER_ID_ENV: &str = "RUSTOK_RBAC_REPLICA_USER_ID";
const CHILD_READY_PATH_ENV: &str = "RUSTOK_RBAC_REPLICA_READY_PATH";
const CHILD_RESULT_PATH_ENV: &str = "RUSTOK_RBAC_REPLICA_RESULT_PATH";
const CHILD_TEST_NAME: &str = "rbac_multi_replica_child";
const OBSERVER_TIMEOUT: Duration = Duration::from_secs(9);
const DOCUMENTED_RECOVERY_BOUND: Duration = Duration::from_secs(7);

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Serialize, Deserialize)]
struct ObserverReady {
    initial_generation: u64,
    initial_allowed: bool,
    redis_configured: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ObserverResult {
    initial_generation: u64,
    durable_generation: u64,
    allowed_after_commit_before_recovery: bool,
    final_allowed: bool,
    authoritative_allowed: bool,
    recovery_elapsed_ms: u64,
    permission_checks: u64,
    redis_configured: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct MutatorResult {
    committed_generation: u64,
    redis_configured: bool,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access and subprocess execution"]
async fn separate_process_replica_recovers_missed_local_publication_from_durable_generation() {
    with_rbac_postgres_database("rustok_rbac_two_process", |db, target_url| async move {
        let tenant_id = insert_tenant(&db).await?;
        let user_id = insert_user(&db, tenant_id).await?;
        RbacRoleAssignmentDbWriter::new(db.clone())
            .assign_role_permissions(tenant_id, user_id, UserRole::Admin)
            .await?;

        let workspace = tempfile::tempdir()?;
        let observer_ready_path = workspace.path().join("observer-ready.json");
        let observer_result_path = workspace.path().join("observer-result.json");
        let mutator_result_path = workspace.path().join("mutator-result.json");

        let observer = spawn_child(
            "observer",
            &target_url,
            tenant_id,
            user_id,
            Some(&observer_ready_path),
            &observer_result_path,
        )?;
        wait_for_file(&observer_ready_path, Duration::from_secs(8)).await?;
        let ready: ObserverReady = read_json(&observer_ready_path)?;
        if !ready.initial_allowed {
            return Err(test_error("observer did not warm an allowed admin permission snapshot"));
        }
        if ready.redis_configured {
            return Err(test_error("observer unexpectedly initialized Redis"));
        }

        let mutator = spawn_child(
            "mutator",
            &target_url,
            tenant_id,
            user_id,
            None,
            &mutator_result_path,
        )?;
        wait_for_child(mutator, Duration::from_secs(12), "mutator").await?;
        wait_for_child(observer, Duration::from_secs(14), "observer").await?;

        let mutation: MutatorResult = read_json(&mutator_result_path)?;
        let observation: ObserverResult = read_json(&observer_result_path)?;

        if mutation.redis_configured || observation.redis_configured {
            return Err(test_error("two-process missed-publication scenario must run without Redis"));
        }
        if mutation.committed_generation != ready.initial_generation + 1 {
            return Err(test_error(format!(
                "mutation committed generation {}, expected {}",
                mutation.committed_generation,
                ready.initial_generation + 1
            )));
        }
        if observation.initial_generation != ready.initial_generation
            || observation.durable_generation != mutation.committed_generation
        {
            return Err(test_error("observer generation evidence does not match the committed mutation"));
        }
        if !observation.allowed_after_commit_before_recovery {
            return Err(test_error(
                "observer did not retain the intentionally missed stale allow before watchdog recovery",
            ));
        }
        if observation.final_allowed || observation.authoritative_allowed {
            return Err(test_error("observer did not converge to the authoritative deny decision"));
        }
        if observation.recovery_elapsed_ms > DOCUMENTED_RECOVERY_BOUND.as_millis() as u64 {
            return Err(test_error(format!(
                "observer recovered in {} ms, beyond the {} ms bound",
                observation.recovery_elapsed_ms,
                DOCUMENTED_RECOVERY_BOUND.as_millis()
            )));
        }
        if observation.permission_checks < 2 {
            return Err(test_error("observer did not record both stale and recovered decisions"));
        }

        Ok(())
    })
    .await
    .unwrap_or_else(|error| panic!("RBAC two-process durable recovery evidence failed: {error}"));
}

#[test]
#[ignore = "subprocess entry point for the two-process RBAC recovery harness"]
fn rbac_multi_replica_child() {
    let Ok(role) = std::env::var(CHILD_ROLE_ENV) else {
        return;
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("build RBAC replica child runtime");
    runtime
        .block_on(run_child(role.as_str()))
        .unwrap_or_else(|error| panic!("RBAC replica child {role} failed: {error}"));
}

async fn run_child(role: &str) -> TestResult<()> {
    let database_url = required_env(CHILD_DATABASE_URL_ENV)?;
    let tenant_id = Uuid::parse_str(&required_env(CHILD_TENANT_ID_ENV)?)?;
    let user_id = Uuid::parse_str(&required_env(CHILD_USER_ID_ENV)?)?;
    let result_path = PathBuf::from(required_env(CHILD_RESULT_PATH_ENV)?);
    let db = connect_postgres(&database_url).await?;
    let context = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    let cache = ensure_cache_service(&context);
    start_rbac_cache_invalidation_listener(&context, cache.clone()).await?;
    start_rbac_invalidation_generation_watchdog(&context).await?;

    match role {
        "observer" => {
            let ready_path = PathBuf::from(required_env(CHILD_READY_PATH_ENV)?);
            run_observer(
                db,
                tenant_id,
                user_id,
                cache.redis_configuration_present(),
                ready_path,
                result_path,
            )
            .await
        }
        "mutator" => {
            run_mutator(
                db,
                tenant_id,
                user_id,
                cache.redis_configuration_present(),
                result_path,
            )
            .await
        }
        other => Err(test_error(format!("unsupported replica child role {other}"))),
    }
}

async fn run_observer(
    db: DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    redis_configured: bool,
    ready_path: PathBuf,
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
    write_json(
        &ready_path,
        &ObserverReady {
            initial_generation,
            initial_allowed,
            redis_configured,
        },
    )?;

    let deadline = Instant::now() + OBSERVER_TIMEOUT;
    let mut durable_generation = initial_generation;
    let mut generation_advanced_at = None;
    let mut allowed_after_commit_before_recovery = None;
    let mut permission_checks = 1_u64;

    while Instant::now() < deadline {
        durable_generation = rustok_rbac::read_permission_invalidation_generation(&db).await?;
        if durable_generation > initial_generation && generation_advanced_at.is_none() {
            generation_advanced_at = Some(Instant::now());
            let allowed = RbacService::has_permission(
                &db,
                &tenant_id,
                &user_id,
                &Permission::SETTINGS_MANAGE,
            )
            .await?;
            permission_checks += 1;
            allowed_after_commit_before_recovery = Some(allowed);
        }

        if let Some(generation_advanced_at) = generation_advanced_at {
            let final_allowed = RbacService::has_permission(
                &db,
                &tenant_id,
                &user_id,
                &Permission::SETTINGS_MANAGE,
            )
            .await?;
            permission_checks += 1;
            if !final_allowed {
                let authoritative = RbacService::get_user_permissions_authoritative(
                    &db,
                    &tenant_id,
                    &user_id,
                )
                .await?;
                write_json(
                    &result_path,
                    &ObserverResult {
                        initial_generation,
                        durable_generation,
                        allowed_after_commit_before_recovery: allowed_after_commit_before_recovery
                            .unwrap_or(false),
                        final_allowed,
                        authoritative_allowed: authoritative.contains(&Permission::SETTINGS_MANAGE),
                        recovery_elapsed_ms: generation_advanced_at.elapsed().as_millis() as u64,
                        permission_checks,
                        redis_configured,
                    },
                )?;
                return Ok(());
            }
        }

        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    Err(test_error(format!(
        "observer did not recover through durable generation {durable_generation} before timeout"
    )))
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
        &MutatorResult {
            committed_generation,
            redis_configured,
        },
    )?;
    Ok(())
}

fn spawn_child(
    role: &str,
    database_url: &str,
    tenant_id: Uuid,
    user_id: Uuid,
    ready_path: Option<&Path>,
    result_path: &Path,
) -> TestResult<Child> {
    let executable = std::env::current_exe()?;
    let mut command = Command::new(executable);
    command
        .arg("--ignored")
        .arg("--exact")
        .arg(CHILD_TEST_NAME)
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env(CHILD_ROLE_ENV, role)
        .env(CHILD_DATABASE_URL_ENV, database_url)
        .env(CHILD_TENANT_ID_ENV, tenant_id.to_string())
        .env(CHILD_USER_ID_ENV, user_id.to_string())
        .env(CHILD_RESULT_PATH_ENV, result_path)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if let Some(ready_path) = ready_path {
        command.env(CHILD_READY_PATH_ENV, ready_path);
    }
    Ok(command.spawn()?)
}

async fn wait_for_child(mut child: Child, timeout: Duration, label: &str) -> TestResult<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            if status.success() {
                return Ok(());
            }
            return Err(test_error(format!("{label} child exited with {status}")));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(test_error(format!("{label} child exceeded {timeout:?}")));
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
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

async fn insert_tenant(db: &DatabaseConnection) -> TestResult<Uuid> {
    let tenant_id = Uuid::new_v4();
    tenants::Entity::insert(tenants::ActiveModel {
        id: Set(tenant_id),
        name: Set("RBAC two-process recovery".to_string()),
        slug: Set(format!("rbac-two-process-{tenant_id}")),
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

async fn insert_user(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<Uuid> {
    let user_id = Uuid::new_v4();
    users::Entity::insert(users::ActiveModel {
        id: Set(user_id),
        tenant_id: Set(tenant_id),
        email: Set(format!("rbac-two-process-{user_id}@example.com")),
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
