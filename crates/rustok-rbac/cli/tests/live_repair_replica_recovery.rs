use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use chrono::Utc;
use rustok_api::Permission;
use rustok_cli_core::{CommandOutcome, CommandRequest};
use rustok_core::{UserRole, UserStatus};
use rustok_migrations::Migrator;
use rustok_rbac::RbacRoleAssignmentDbWriter;
use rustok_runtime::RuntimeComposition;
use rustok_server::common::settings::RustokSettings;
use rustok_server::models::{permissions, role_permissions, roles, tenants, users};
use rustok_server::services::rbac_invalidation_generation::start_rbac_invalidation_generation_watchdog;
use rustok_server::services::rbac_service::RbacService;
use rustok_server::services::server_runtime_context::ServerRuntimeContext;
use rustok_test_utils::{
    assert_postgres_url, connect_postgres, create_postgres_database,
    drop_postgres_database_if_exists, postgres_database_url, unique_postgres_database_name,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use sea_orm_migration::MigratorTrait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const ADMIN_URL_ENV: &str = "RUSTOK_MIGRATION_SMOKE_ADMIN_URL";
const CHILD_ROLE_ENV: &str = "RUSTOK_RBAC_CLI_REPAIR_CHILD_ROLE";
const CHILD_DATABASE_URL_ENV: &str = "RUSTOK_RBAC_CLI_REPAIR_DATABASE_URL";
const CHILD_TENANT_ID_ENV: &str = "RUSTOK_RBAC_CLI_REPAIR_TENANT_ID";
const CHILD_USER_ID_ENV: &str = "RUSTOK_RBAC_CLI_REPAIR_USER_ID";
const CHILD_READY_PATH_ENV: &str = "RUSTOK_RBAC_CLI_REPAIR_READY_PATH";
const CHILD_RESULT_PATH_ENV: &str = "RUSTOK_RBAC_CLI_REPAIR_RESULT_PATH";
const CHILD_TEST_NAME: &str = "rbac_cli_repair_live_replica_child";
const OBSERVER_RECOVERY_BOUND: Duration = Duration::from_secs(8);

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Serialize, Deserialize)]
struct ObserverReady {
    initial_generation: u64,
    cached_allowed: bool,
    authoritative_allowed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ObserverResult {
    durable_generation: u64,
    cached_allowed: bool,
    authoritative_allowed: bool,
    recovery_elapsed_ms: u64,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL admin access and subprocess execution"]
async fn cli_system_role_repair_recovers_two_live_replicas_without_restart() {
    let admin_url = std::env::var(ADMIN_URL_ENV)
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    assert_postgres_url(&admin_url);
    let database_name = unique_postgres_database_name("rustok_rbac_cli_live_repair");
    let target_url = postgres_database_url(&admin_url, &database_name);
    let admin = connect_postgres(&admin_url).await.unwrap();
    drop_postgres_database_if_exists(&admin, &database_name)
        .await
        .unwrap();
    create_postgres_database(&admin, &database_name)
        .await
        .unwrap();

    let result = run_parent_scenario(&target_url).await;

    drop_postgres_database_if_exists(&admin, &database_name)
        .await
        .unwrap();
    admin.close().await.unwrap();
    result.unwrap_or_else(|error| panic!("RBAC live CLI repair evidence failed: {error}"));
}

async fn run_parent_scenario(database_url: &str) -> TestResult<()> {
    let db = connect_postgres(database_url).await?;
    Migrator::up(&db, None).await?;
    let tenant_id = insert_tenant(&db).await?;
    let user_id = insert_user(&db, tenant_id).await?;
    RbacRoleAssignmentDbWriter::new(db.clone())
        .assign_role_permissions(tenant_id, user_id, UserRole::Manager)
        .await?;
    let stale_link_id = add_stale_manager_permission(&db, tenant_id).await?;

    let authoritative = RbacService::get_user_permissions_authoritative(&db, &tenant_id, &user_id)
        .await?;
    if !authoritative.contains(&Permission::SETTINGS_MANAGE) {
        return Err(test_error(
            "stale Manager permission did not create the expected pre-repair allow",
        ));
    }
    let initial_generation = rustok_rbac::read_permission_invalidation_generation(&db).await?;
    if initial_generation != 0 {
        return Err(test_error(format!(
            "unexpected initial durable generation {initial_generation}",
        )));
    }

    let workspace = tempfile::tempdir()?;
    let first_ready = workspace.path().join("observer-1-ready.json");
    let second_ready = workspace.path().join("observer-2-ready.json");
    let first_result = workspace.path().join("observer-1-result.json");
    let second_result = workspace.path().join("observer-2-result.json");
    let cli_result = workspace.path().join("cli-result.json");

    let mut first_observer = spawn_child(
        "observer",
        database_url,
        tenant_id,
        user_id,
        Some(&first_ready),
        &first_result,
    )?;
    let mut second_observer = spawn_child(
        "observer",
        database_url,
        tenant_id,
        user_id,
        Some(&second_ready),
        &second_result,
    )?;
    wait_for_file(&first_ready, Duration::from_secs(8)).await?;
    wait_for_file(&second_ready, Duration::from_secs(8)).await?;
    assert_observer_ready(read_json(&first_ready)?)?;
    assert_observer_ready(read_json(&second_ready)?)?;

    let mut cli = spawn_child(
        "cli",
        database_url,
        tenant_id,
        user_id,
        None,
        &cli_result,
    )?;
    wait_for_child(&mut cli, Duration::from_secs(15), "RBAC CLI repair").await?;
    let outcome: CommandOutcome = read_json(&cli_result)?;
    assert_cli_outcome(&outcome)?;

    wait_for_file(&first_result, Duration::from_secs(12)).await?;
    wait_for_file(&second_result, Duration::from_secs(12)).await?;
    wait_for_child(&mut first_observer, Duration::from_secs(5), "first observer").await?;
    wait_for_child(&mut second_observer, Duration::from_secs(5), "second observer").await?;

    let first: ObserverResult = read_json(&first_result)?;
    let second: ObserverResult = read_json(&second_result)?;
    assert_observer_recovered(&first, 1)?;
    assert_observer_recovered(&second, 1)?;

    let link_still_exists = role_permissions::Entity::find_by_id(stale_link_id)
        .one(&db)
        .await?
        .is_some();
    if link_still_exists {
        return Err(test_error("CLI repair left the stale Manager permission link in place"));
    }
    let generation = rustok_rbac::read_permission_invalidation_generation(&db).await?;
    if generation != 1 {
        return Err(test_error(format!(
            "CLI repair committed durable generation {generation}, expected 1",
        )));
    }

    db.close().await?;
    Ok(())
}

#[test]
#[ignore = "subprocess entry point for live RBAC CLI repair evidence"]
fn rbac_cli_repair_live_replica_child() {
    let Ok(role) = std::env::var(CHILD_ROLE_ENV) else {
        return;
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("build RBAC CLI repair child runtime");
    runtime
        .block_on(run_child(&role))
        .unwrap_or_else(|error| panic!("RBAC CLI repair child {role} failed: {error}"));
}

async fn run_child(role: &str) -> TestResult<()> {
    let database_url = required_env(CHILD_DATABASE_URL_ENV)?;
    let tenant_id = Uuid::parse_str(&required_env(CHILD_TENANT_ID_ENV)?)?;
    let user_id = Uuid::parse_str(&required_env(CHILD_USER_ID_ENV)?)?;
    let result_path = PathBuf::from(required_env(CHILD_RESULT_PATH_ENV)?);
    match role {
        "observer" => {
            let ready_path = PathBuf::from(required_env(CHILD_READY_PATH_ENV)?);
            run_observer(&database_url, tenant_id, user_id, ready_path, result_path).await
        }
        "cli" => run_cli(&database_url, tenant_id, result_path).await,
        other => Err(test_error(format!("unsupported child role {other}"))),
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
    start_rbac_invalidation_generation_watchdog(&context).await?;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let initial_generation = rustok_rbac::read_permission_invalidation_generation(&db).await?;
    let cached_allowed = RbacService::has_permission(
        &db,
        &tenant_id,
        &user_id,
        &Permission::SETTINGS_MANAGE,
    )
    .await?;
    let authoritative = RbacService::get_user_permissions_authoritative(&db, &tenant_id, &user_id)
        .await?;
    write_json(
        &ready_path,
        &ObserverReady {
            initial_generation,
            cached_allowed,
            authoritative_allowed: authoritative.contains(&Permission::SETTINGS_MANAGE),
        },
    )?;

    let started = Instant::now();
    let deadline = started + OBSERVER_RECOVERY_BOUND;
    while Instant::now() < deadline {
        let durable_generation = rustok_rbac::read_permission_invalidation_generation(&db).await?;
        let cached_allowed = RbacService::has_permission(
            &db,
            &tenant_id,
            &user_id,
            &Permission::SETTINGS_MANAGE,
        )
        .await?;
        if durable_generation > initial_generation && !cached_allowed {
            let authoritative = RbacService::get_user_permissions_authoritative(
                &db,
                &tenant_id,
                &user_id,
            )
            .await?;
            write_json(
                &result_path,
                &ObserverResult {
                    durable_generation,
                    cached_allowed,
                    authoritative_allowed: authoritative.contains(&Permission::SETTINGS_MANAGE),
                    recovery_elapsed_ms: started.elapsed().as_millis() as u64,
                },
            )?;
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    Err(test_error("live observer did not recover from the CLI repair generation"))
}

async fn run_cli(database_url: &str, tenant_id: Uuid, result_path: PathBuf) -> TestResult<()> {
    let db = connect_postgres(database_url).await?;
    let runtime = RuntimeComposition::from_database(db, serde_json::json!({}));
    let provider = rustok_rbac_cli::command_provider(&runtime);
    let outcome = provider
        .execute(CommandRequest {
            namespace: "rbac".to_string(),
            name: "repair-system-roles".to_string(),
            args: serde_json::json!({
                "options": {
                    "tenant_id": tenant_id.to_string(),
                    "apply": true
                }
            }),
            dry_run: false,
        })
        .await?;
    write_json(&result_path, &outcome)?;
    Ok(())
}

fn assert_observer_ready(ready: ObserverReady) -> TestResult<()> {
    if ready.initial_generation != 0 || !ready.cached_allowed || !ready.authoritative_allowed {
        return Err(test_error(format!("invalid observer readiness evidence: {ready:?}")));
    }
    Ok(())
}

fn assert_cli_outcome(outcome: &CommandOutcome) -> TestResult<()> {
    if outcome.exit_code != 0
        || outcome.data["mode"] != "apply"
        || outcome.data["runtime_restart_required_if_applied"] != false
        || outcome.data["durable_generation"] != 1
        || outcome.data["changes_total"].as_u64().unwrap_or_default() == 0
    {
        return Err(test_error(format!("invalid CLI repair outcome: {outcome:?}")));
    }
    Ok(())
}

fn assert_observer_recovered(result: &ObserverResult, generation: u64) -> TestResult<()> {
    if result.durable_generation != generation
        || result.cached_allowed
        || result.authoritative_allowed
        || result.recovery_elapsed_ms > OBSERVER_RECOVERY_BOUND.as_millis() as u64
    {
        return Err(test_error(format!("invalid replica recovery evidence: {result:?}")));
    }
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
        .env(CHILD_USER_ID_ENV, user_id.to_string())
        .env(CHILD_RESULT_PATH_ENV, result_path)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if let Some(path) = ready_path {
        command.env(CHILD_READY_PATH_ENV, path);
    }
    Ok(command.spawn()?)
}

async fn wait_for_child(child: &mut Child, timeout: Duration, label: &str) -> TestResult<()> {
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

async fn wait_for_file(path: &Path, timeout: Duration) -> TestResult<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.is_file() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Err(test_error(format!("timed out waiting for {}", path.display())))
}

async fn insert_tenant(db: &DatabaseConnection) -> TestResult<Uuid> {
    let tenant_id = Uuid::new_v4();
    tenants::Entity::insert(tenants::ActiveModel {
        id: Set(tenant_id),
        name: Set("RBAC CLI live repair".to_string()),
        slug: Set(format!("rbac-cli-live-repair-{tenant_id}")),
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
        email: Set(format!("rbac-cli-live-repair-{user_id}@example.com")),
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

async fn add_stale_manager_permission(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<Uuid> {
    let manager = roles::Entity::find()
        .filter(roles::Column::TenantId.eq(tenant_id))
        .filter(roles::Column::Slug.eq("manager"))
        .one(db)
        .await?
        .ok_or_else(|| test_error("Manager system role is missing"))?;
    let existing = permissions::Entity::find()
        .filter(permissions::Column::TenantId.eq(tenant_id))
        .filter(permissions::Column::Resource.eq(Permission::SETTINGS_MANAGE.resource.to_string()))
        .filter(permissions::Column::Action.eq(Permission::SETTINGS_MANAGE.action.to_string()))
        .one(db)
        .await?;
    let permission_id = if let Some(permission) = existing {
        permission.id
    } else {
        let permission_id = Uuid::new_v4();
        permissions::Entity::insert(permissions::ActiveModel {
            id: Set(permission_id),
            tenant_id: Set(tenant_id),
            resource: Set(Permission::SETTINGS_MANAGE.resource.to_string()),
            action: Set(Permission::SETTINGS_MANAGE.action.to_string()),
            description: Set(Some("stale CLI repair fixture".to_string())),
            created_at: Set(Utc::now().into()),
        })
        .exec(db)
        .await?;
        permission_id
    };
    let link_id = Uuid::new_v4();
    role_permissions::Entity::insert(role_permissions::ActiveModel {
        id: Set(link_id),
        role_id: Set(manager.id),
        permission_id: Set(permission_id),
    })
    .exec(db)
    .await?;
    Ok(link_id)
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
