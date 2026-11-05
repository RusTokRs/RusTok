use rustok_migrations::execution::{ExecutionError, StepRunner};
use sea_orm_migration::{
    prelude::*,
    sea_orm::{Database, DatabaseConnection, DbBackend, Statement},
};
use std::{error::Error, sync::Arc, time::Duration};
use tokio::sync::Notify;

type Result<T = ()> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

struct Script {
    name: &'static str,
    sql: &'static str,
    started: Option<Arc<Notify>>,
    wait: bool,
    transaction: Option<bool>,
}

impl Script {
    fn migration(name: &'static str, sql: &'static str) -> Self {
        Self {
            name,
            sql,
            started: None,
            wait: false,
            transaction: None,
        }
    }
}

impl MigrationName for Script {
    fn name(&self) -> &str {
        self.name
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Script {
    fn use_transaction(&self) -> Option<bool> {
        self.transaction
    }

    async fn up(&self, manager: &SchemaManager) -> std::result::Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(self.sql)
            .await?;
        if let Some(started) = &self.started {
            started.notify_one();
        }
        if self.wait {
            std::future::pending::<()>().await;
        }
        Ok(())
    }
}

fn plan(fail: bool) -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(Script::migration(
            "create",
            "CREATE TABLE effects (id integer PRIMARY KEY, value integer NOT NULL); INSERT INTO effects VALUES (1, 1)",
        )),
        Box::new(Script::migration(
            "backfill",
            if fail {
                "UPDATE effects SET value = value + 1; CREATE TABLE partial_table (id integer); SELECT 1/0"
            } else {
                "UPDATE effects SET value = value + 1"
            },
        )),
        Box::new(Script::migration(
            "tail",
            "INSERT INTO effects VALUES (2, 20)",
        )),
    ]
}

async fn scalar(db: &DatabaseConnection, sql: &str) -> Result<i64> {
    Ok(db
        .query_one_raw(Statement::from_string(DbBackend::Postgres, sql))
        .await?
        .ok_or("missing scalar result")?
        .try_get("", "value")?)
}

async fn assert_state(db: &DatabaseConnection, value: i64, journal: i64, rows: i64) -> Result {
    assert_eq!(
        scalar(db, "SELECT value::bigint AS value FROM effects WHERE id=1").await?,
        value
    );
    assert_eq!(
        scalar(db, "SELECT count(*) AS value FROM seaql_migrations").await?,
        journal
    );
    assert_eq!(
        scalar(db, "SELECT count(*) AS value FROM effects").await?,
        rows
    );
    Ok(())
}

#[tokio::test]
async fn rejects_unsupported_plans_before_connecting() {
    assert!(matches!(
        StepRunner::connect("invalid URL", vec![]).await,
        Err(ExecutionError::InvalidPlan)
    ));
    let mut duplicate = plan(false);
    duplicate.push(Box::new(Script::migration("create", "SELECT 1")));
    assert!(matches!(
        StepRunner::connect("invalid URL", duplicate).await,
        Err(ExecutionError::InvalidPlan)
    ));
    let mut nontransactional = Script::migration("outside_transaction", "SELECT 1");
    nontransactional.transaction = Some(false);
    assert!(matches!(
        StepRunner::connect("invalid URL", vec![Box::new(nontransactional)]).await,
        Err(ExecutionError::UnsupportedRecovery(_))
    ));
}

#[tokio::test]
#[ignore = "requires isolated loopback PostgreSQL and RUSTOK_MIGRATION_VERIFY_TEST_ADMIN_URL"]
async fn postgres_stepwise_failure_cancellation_and_restart() -> Result {
    match std::env::var("RUSTOK_STEPWISE_PROCESS_FIXTURE_URL") {
        Ok(url) => return process_fixture(&url).await,
        Err(std::env::VarError::NotPresent) => {}
        Err(error) => return Err(error.into()),
    }
    let admin_url = std::env::var("RUSTOK_MIGRATION_VERIFY_TEST_ADMIN_URL")?;
    let mut url = url::Url::parse(&admin_url)?;
    assert!(matches!(
        url.host_str(),
        Some("127.0.0.1" | "localhost" | "::1")
    ));
    let admin = Database::connect(&admin_url).await?;
    let id = uuid::Uuid::new_v4().simple().to_string();
    let name = format!("rustok_step_{id}");
    let role = format!("rustok_step_role_{id}");
    let password = uuid::Uuid::new_v4().simple().to_string();
    url.set_username(&role)
        .map_err(|_| "invalid fixture role")?;
    url.set_password(Some(&password))
        .map_err(|_| "invalid fixture credentials")?;
    url.set_query(None);
    url.set_fragment(None);
    url.set_path(&name);
    let fixture_url = url.as_str();
    let outcome = async {
        admin.execute_unprepared(&format!("CREATE ROLE {role} LOGIN PASSWORD '{password}' NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS")).await?;
        admin.execute_unprepared(&format!("CREATE DATABASE {name} OWNER {role} TEMPLATE template0")).await?;
        // JoinError captures assertion panics so outer fixture cleanup still runs.
        tokio::spawn(run_recovery_scenarios(fixture_url.to_owned(), admin.clone()))
            .await.map_err(|error| format!("recovery assertions failed: {error}"))?

    }.await;
    let cleanup = async {
        admin
            .execute_unprepared(&format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"))
            .await?;
        admin
            .execute_unprepared(&format!("DROP ROLE IF EXISTS {role}"))
            .await?;
        admin.close().await?;
        Ok::<_, Box<dyn Error + Send + Sync>>(())
    }
    .await;
    match (outcome, cleanup) {
        (Err(error), Err(cleanup)) => Err(format!("{error}; cleanup: {cleanup}").into()),
        (Err(error), _) | (_, Err(error)) => Err(error),
        _ => Ok(()),
    }
}

async fn run_recovery_scenarios(url: String, admin: DatabaseConnection) -> Result {
    let db = Database::connect(&url).await?;
    let result = async {
        transaction_failure_and_restart(&url, &db).await?;
        reset(&db).await?;
        cancellation(&url, &db).await?;
        reset(&db).await?;
        session_loss(&url, &db, &admin, true).await?;
        reset(&db).await?;
        session_loss(&url, &db, &admin, false).await?;
        reset(&db).await?;
        process_interruption(&url, &db).await?;
        reset(&db).await?;
        journal_corruption(&url, &db).await?;
        reset(&db).await?;
        duplicate_journal(&url, &db).await?;
        Ok::<_, Box<dyn Error + Send + Sync>>(())
    }
    .await;
    let close = db.close().await;
    match (result, close) {
        (Err(error), Err(cleanup)) => Err(format!("{error}; connection close: {cleanup}").into()),
        (Err(error), _) => Err(error),
        (_, Err(error)) => Err(error.into()),
        _ => Ok(()),
    }
}

async fn process_fixture(url: &str) -> Result {
    let parsed = url::Url::parse(url)?;
    if !parsed.path().starts_with("/rustok_step_") {
        return Err("process fixture requires its private test database".into());
    }
    let mut steps = plan(false);
    steps[1] = Box::new(Script {
        name: "backfill",
        sql: "UPDATE effects SET value = value + 1; SELECT pg_sleep(2)",
        started: None,
        wait: true,
        transaction: None,
    });
    let mut runner = StepRunner::connect(url, steps).await?;
    while runner.apply_next().await?.is_some() {}
    runner.close().await?;
    Err("process fixture unexpectedly completed".into())
}

async fn process_interruption(url: &str, db: &DatabaseConnection) -> Result {
    let mut child = tokio::process::Command::new(std::env::current_exe()?)
        .args([
            "--exact",
            "postgres_stepwise_failure_cancellation_and_restart",
            "--ignored",
        ])
        .env("RUSTOK_STEPWISE_PROCESS_FIXTURE_URL", url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let result = async {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if child.try_wait()?.is_some() { return Err("fixture process exited before interruption".into()); }
                if scalar(db, "SELECT count(*) AS value FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() AND state<>'idle' AND query LIKE '%pg_sleep(2)%'").await? == 1 { break; }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Ok::<_, Box<dyn Error + Send + Sync>>(())
        }).await??;
        child.kill().await?;
        // PostgreSQL may finish the active query before observing socket loss.
        // Never force-clear the journal or steal an advisory lock to continue.
        tokio::time::timeout(Duration::from_secs(10), async {
            while scalar(db, "SELECT count(*) AS value FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() AND state<>'idle' AND query LIKE '%pg_sleep(2)%'").await? != 0 {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Ok::<_, Box<dyn Error + Send + Sync>>(())
        }).await??;
        assert_state(db, 1, 1, 1).await?;
        let mut runner = StepRunner::connect(url, plan(false)).await?;
        assert_eq!(runner.apply_next().await?, Some("backfill".into()));
        assert_state(db, 2, 2, 1).await?;
        runner.close().await?;
        Ok::<_, Box<dyn Error + Send + Sync>>(())
    }.await;
    if child.try_wait()?.is_none() {
        child.kill().await?;
    }
    result
}

async fn reset(db: &DatabaseConnection) -> Result {
    db.execute_unprepared("DROP SCHEMA public CASCADE; CREATE SCHEMA public")
        .await?;
    Ok(())
}

async fn transaction_failure_and_restart(url: &str, db: &DatabaseConnection) -> Result {
    let mut runner = StepRunner::connect(url, plan(true)).await?;
    assert!(matches!(
        StepRunner::connect(url, plan(true)).await,
        Err(ExecutionError::ConcurrentWriter)
    ));
    assert_eq!(runner.apply_next().await?, Some("create".into()));
    assert_state(db, 1, 1, 1).await?;
    assert!(matches!(
        runner.apply_next().await,
        Err(ExecutionError::Migration { .. })
    ));
    assert!(matches!(
        runner.apply_next().await,
        Err(ExecutionError::Interrupted)
    ));
    assert_state(db, 1, 1, 1).await?;
    assert_eq!(
        scalar(
            db,
            "SELECT (to_regclass('public.partial_table') IS NOT NULL)::int::bigint AS value"
        )
        .await?,
        0
    );
    runner.close().await?;

    // Fresh sessions resume only after reading the committed prefix. Each
    // independent assertion runs before allowing the following migration.
    let mut runner = StepRunner::connect(url, plan(false)).await?;
    assert_eq!(
        runner.pending_migrations()?.collect::<Vec<_>>(),
        ["backfill", "tail"]
    );
    assert_eq!(runner.apply_next().await?, Some("backfill".into()));
    assert_state(db, 2, 2, 1).await?;
    runner.close().await?;
    let mut runner = StepRunner::connect(url, plan(false)).await?;
    assert_eq!(runner.apply_next().await?, Some("tail".into()));
    assert_state(db, 2, 3, 2).await?;
    assert_eq!(runner.apply_next().await?, None);
    runner.close().await?;
    let mut runner = StepRunner::connect(url, plan(false)).await?;
    assert_eq!(runner.apply_next().await?, None);
    assert_state(db, 2, 3, 2).await?;
    runner.close().await?;
    Ok(())
}

async fn cancellation(url: &str, db: &DatabaseConnection) -> Result {
    let started = Arc::new(Notify::new());
    let mut steps = plan(false);
    steps[1] = Box::new(Script {
        name: "backfill",
        sql: "UPDATE effects SET value = value + 1; CREATE TABLE partial_table (id integer)",
        started: Some(started.clone()),
        wait: true,
        transaction: None,
    });
    let mut runner = StepRunner::connect(url, steps).await?;
    runner.apply_next().await?;
    {
        let apply = runner.apply_next();
        tokio::pin!(apply);
        tokio::select! {
            result = &mut apply => return Err(format!("step finished before cancellation: {result:?}").into()),
            _ = started.notified() => {}
            _ = tokio::time::sleep(Duration::from_secs(10)) => return Err("step did not reach cancellation boundary".into()),
        }
    }
    assert!(matches!(
        runner.apply_next().await,
        Err(ExecutionError::Interrupted)
    ));
    assert!(matches!(
        runner.pending_migrations(),
        Err(ExecutionError::Interrupted)
    ));
    runner.close().await?;
    assert_state(db, 1, 1, 1).await?;
    let mut runner = StepRunner::connect(url, plan(false)).await?;
    assert_eq!(runner.apply_next().await?, Some("backfill".into()));
    assert_state(db, 2, 2, 1).await?;
    runner.close().await?;
    Ok(())
}

async fn session_loss(
    url: &str,
    db: &DatabaseConnection,
    admin: &DatabaseConnection,
    during_step: bool,
) -> Result {
    let mut steps = plan(false);
    if during_step {
        steps[1] = Box::new(Script {
            name: "backfill",
            sql: "UPDATE effects SET value = value + 1; SELECT pg_sleep(30)",
            started: None,
            wait: false,
            transaction: None,
        });
    }
    let mut runner = StepRunner::connect(url, steps).await?;
    let pid = scalar(
        runner.database(),
        "SELECT pg_backend_pid()::bigint AS value",
    )
    .await?;
    runner.apply_next().await?;
    if during_step {
        // Wait for the actual PostgreSQL statement, not a wall-clock guess.
        let kill = async {
            tokio::time::timeout(Duration::from_secs(10), async {
                loop {
                    let sleeping = scalar(admin, &format!("SELECT count(*) AS value FROM pg_stat_activity WHERE pid={pid} AND state='active' AND query LIKE '%pg_sleep(30)%'")).await?;
                    if sleeping == 1 { break; }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Ok::<_, Box<dyn Error + Send + Sync>>(())
            }).await??;
            admin
                .execute_unprepared(&format!("SELECT pg_terminate_backend({pid})"))
                .await?;
            Ok::<_, Box<dyn Error + Send + Sync>>(())
        };
        let (step, killed) = tokio::join!(runner.apply_next(), kill);
        killed?;
        assert!(step.is_err());
    } else {
        admin
            .execute_unprepared(&format!("SELECT pg_terminate_backend({pid})"))
            .await?;
        assert!(runner.apply_next().await.is_err());
    }
    assert!(matches!(
        runner.apply_next().await,
        Err(ExecutionError::Interrupted)
    ));
    runner.close().await?;
    assert_state(db, 1, 1, 1).await?;
    let mut runner = StepRunner::connect(url, plan(false)).await?;
    assert_eq!(runner.apply_next().await?, Some("backfill".into()));
    assert_state(db, 2, 2, 1).await?;
    runner.close().await?;
    Ok(())
}

async fn journal_corruption(url: &str, db: &DatabaseConnection) -> Result {
    let mut steps = plan(false);
    steps[1] = Box::new(Script::migration(
        "backfill",
        "DELETE FROM seaql_migrations WHERE version = 'create'",
    ));
    let mut runner = StepRunner::connect(url, steps).await?;
    runner.apply_next().await?;
    assert!(matches!(
        runner.apply_next().await,
        Err(ExecutionError::InvalidJournal)
    ));
    assert!(matches!(
        runner.apply_next().await,
        Err(ExecutionError::Interrupted)
    ));
    assert_state(db, 1, 1, 1).await?;
    runner.close().await?;
    assert!(matches!(
        StepRunner::connect(url, plan(false)).await,
        Err(ExecutionError::InvalidJournal)
    ));
    Ok(())
}

async fn duplicate_journal(url: &str, db: &DatabaseConnection) -> Result {
    db.execute_unprepared("CREATE TABLE seaql_migrations (version varchar(255) NOT NULL, applied_at bigint NOT NULL); INSERT INTO seaql_migrations VALUES ('create', 1), ('create', 2)").await?;
    assert!(matches!(
        StepRunner::connect(url, plan(false)).await,
        Err(ExecutionError::InvalidJournal)
    ));
    assert_eq!(
        scalar(
            db,
            "SELECT (to_regclass('public.effects') IS NOT NULL)::int::bigint AS value"
        )
        .await?,
        0
    );
    Ok(())
}
