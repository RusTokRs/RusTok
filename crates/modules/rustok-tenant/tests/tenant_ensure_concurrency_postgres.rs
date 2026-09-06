use std::{error::Error, io, time::Duration};

use rustok_outbox::SysEvents;
use rustok_tenant::{
    CreateTenantInput, TenantService,
    entities::{tenant, tenant_locale},
};
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection,
    EntityTrait, PaginatorTrait, QueryFilter, Statement, TransactionTrait,
};
use tokio::time::{Instant, sleep};
use uuid::Uuid;

const TENANT_TEST_DATABASE_ENV: &str = "RUSTOK_TENANT_TEST_DATABASE_URL";
const INSERT_BARRIER_LOCK: i64 = 7_301_001;
type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

struct PostgresTenantTestDb {
    control: DatabaseConnection,
    db_a: DatabaseConnection,
    db_b: DatabaseConnection,
    blocker: DatabaseConnection,
    schema_name: String,
    application_name_a: String,
    application_name_b: String,
}

impl PostgresTenantTestDb {
    async fn setup(prefix: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{TENANT_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping tenant ensure concurrency test"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let nonce = Uuid::new_v4().simple().to_string();
        let schema_name = format!("rustok_tenant_{}_{}", sanitize_identifier(prefix), nonce);
        let application_name_a = format!("rustok_tenant_ensure_a_{}", &nonce[..12]);
        let application_name_b = format!("rustok_tenant_ensure_b_{}", &nonce[..12]);

        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let setup_result = async {
            let db_a = connect(&database_url).await?;
            let db_b = connect(&database_url).await?;
            let blocker = connect(&database_url).await?;

            configure_session(&db_a, &schema_name, &application_name_a).await?;
            configure_session(&db_b, &schema_name, &application_name_b).await?;
            configure_session(&blocker, &schema_name, "rustok_tenant_ensure_blocker").await?;
            create_postgres_test_tables(&db_a).await?;
            install_insert_barrier(&db_a).await?;

            Ok::<_, Box<dyn Error + Send + Sync>>((db_a, db_b, blocker))
        }
        .await;

        match setup_result {
            Ok((db_a, db_b, blocker)) => Ok(Some(Self {
                control,
                db_a,
                db_b,
                blocker,
                schema_name,
                application_name_a,
                application_name_b,
            })),
            Err(error) => {
                let _ = control
                    .execute_unprepared(&format!(
                        r#"DROP SCHEMA IF EXISTS "{schema_name}" CASCADE"#
                    ))
                    .await;
                Err(error)
            }
        }
    }

    async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_concurrent_ensure_tenant_replays_unique_winner() -> TestResult<()> {
    let Some(test_db) = PostgresTenantTestDb::setup("ensure_race").await? else {
        return Ok(());
    };

    let outcome = async {
        let blocker_transaction = test_db.blocker.begin().await?;
        blocker_transaction
            .query_one_raw(Statement::from_string(
                DatabaseBackend::Postgres,
                format!("SELECT pg_advisory_xact_lock({INSERT_BARRIER_LOCK})"),
            ))
            .await?
            .ok_or_else(|| test_error("failed to acquire tenant insert barrier"))?;

        let slug = format!("concurrent-ensure-{}", Uuid::new_v4().simple());
        let input = CreateTenantInput {
            name: "Concurrent Ensure Tenant".to_string(),
            slug: slug.clone(),
            domain: None,
        };
        let service_a = TenantService::new(test_db.db_a.clone());
        let service_b = TenantService::new(test_db.db_b.clone());
        let input_a = input.clone();
        let input_b = input;

        let task_a = tokio::spawn(async move { service_a.ensure_tenant(input_a).await });
        let task_b = tokio::spawn(async move { service_b.ensure_tenant(input_b).await });

        let wait_result = wait_for_lock_waiters(
            &test_db.control,
            &test_db.application_name_a,
            &test_db.application_name_b,
        )
        .await;
        blocker_transaction.commit().await?;

        if let Err(error) = wait_result {
            task_a.abort();
            task_b.abort();
            let _ = task_a.await;
            let _ = task_b.await;
            return Err(error);
        }

        let first = task_a
            .await
            .map_err(|error| test_error(format!("first ensure task failed: {error}")))??;
        let second = task_b
            .await
            .map_err(|error| test_error(format!("second ensure task failed: {error}")))??;

        assert_eq!(first.0.id, second.0.id);
        assert_eq!(first.0.slug, slug);
        assert_eq!(second.0.slug, slug);
        assert_ne!(first.1, second.1);
        assert_eq!(
            [first.1, second.1]
                .into_iter()
                .filter(|created| *created)
                .count(),
            1
        );

        let tenant_count = tenant::Entity::find()
            .filter(tenant::Column::Slug.eq(slug.clone()))
            .count(&test_db.db_a)
            .await?;
        assert_eq!(tenant_count, 1);

        let locale_count = tenant_locale::Entity::find()
            .filter(tenant_locale::Column::TenantId.eq(first.0.id))
            .count(&test_db.db_a)
            .await?;
        assert_eq!(locale_count, 1);

        let events = SysEvents::find().all(&test_db.db_a).await?;
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_type == "tenant.created")
                .count(),
            1
        );

        Ok(())
    }
    .await;

    test_db.cleanup().await?;
    outcome
}

async fn install_insert_barrier(db: &DatabaseConnection) -> TestResult<()> {
    db.execute_unprepared(&format!(
        r#"
CREATE FUNCTION tenant_ensure_insert_barrier() RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM pg_advisory_xact_lock({INSERT_BARRIER_LOCK});
    RETURN NEW;
END;
$$;
CREATE TRIGGER tenant_ensure_insert_barrier
BEFORE INSERT ON tenants
FOR EACH ROW
EXECUTE FUNCTION tenant_ensure_insert_barrier();
"#
    ))
    .await?;
    Ok(())
}

async fn wait_for_lock_waiters(
    control: &DatabaseConnection,
    application_name_a: &str,
    application_name_b: &str,
) -> TestResult<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let row = control
            .query_one_raw(Statement::from_string(
                DatabaseBackend::Postgres,
                format!(
                    "SELECT COUNT(*)::BIGINT AS waiter_count \
                     FROM pg_stat_activity \
                     WHERE application_name IN ('{application_name_a}', '{application_name_b}') \
                       AND state = 'active' \
                       AND wait_event_type = 'Lock'"
                ),
            ))
            .await?
            .ok_or_else(|| test_error("pg_stat_activity did not return a waiter count"))?;
        let waiter_count: i64 = row.try_get("", "waiter_count")?;
        if waiter_count >= 2 {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(test_error(format!(
                "expected two ensure writers to wait at the insert barrier, observed {waiter_count}"
            )));
        }
        sleep(Duration::from_millis(20)).await;
    }
}

async fn create_postgres_test_tables(db: &DatabaseConnection) -> TestResult<()> {
    for sql in [
        r#"
        CREATE TABLE tenants (
            id UUID PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            slug VARCHAR(255) NOT NULL UNIQUE,
            domain VARCHAR(255) UNIQUE,
            settings JSONB NOT NULL DEFAULT '{}'::jsonb,
            default_locale VARCHAR(35) NOT NULL,
            is_active BOOLEAN NOT NULL DEFAULT TRUE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
        r#"
        CREATE TABLE tenant_locales (
            id UUID PRIMARY KEY,
            tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
            locale VARCHAR(35) NOT NULL,
            name VARCHAR(50) NOT NULL,
            native_name VARCHAR(50) NOT NULL,
            is_default BOOLEAN NOT NULL DEFAULT FALSE,
            is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
            fallback_locale VARCHAR(35),
            policy_revision BIGINT NOT NULL DEFAULT 0,
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE (tenant_id, locale)
        )
        "#,
        r#"
        CREATE UNIQUE INDEX uq_tenant_locales_one_default
            ON tenant_locales (tenant_id)
            WHERE is_default
        "#,
        r#"
        CREATE TABLE sys_events (
            id UUID PRIMARY KEY,
            event_type VARCHAR(255) NOT NULL,
            schema_version SMALLINT NOT NULL,
            payload JSONB NOT NULL,
            status VARCHAR(32) NOT NULL,
            retry_count INTEGER NOT NULL DEFAULT 0,
            next_attempt_at TIMESTAMPTZ,
            last_error TEXT,
            claimed_by VARCHAR(255),
            claimed_at TIMESTAMPTZ,
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            dispatched_at TIMESTAMPTZ
        )
        "#,
    ] {
        db.execute_unprepared(sql).await?;
    }
    Ok(())
}

fn postgres_database_url() -> Option<String> {
    std::env::var(TENANT_TEST_DATABASE_ENV)
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

async fn configure_session(
    db: &DatabaseConnection,
    schema_name: &str,
    application_name: &str,
) -> TestResult<()> {
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}", public"#))
        .await?;
    db.execute_unprepared(&format!("SET application_name = '{application_name}'"))
        .await?;
    Ok(())
}

fn sanitize_identifier(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    let normalized = normalized.trim_matches('_');
    if normalized.is_empty() {
        "test".to_string()
    } else {
        normalized.to_string()
    }
}

fn test_error(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    Box::new(io::Error::other(message.into()))
}
