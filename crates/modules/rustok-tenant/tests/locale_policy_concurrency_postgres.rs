use std::{error::Error, io, time::Duration};

use rustok_outbox::SysEvents;
use rustok_tenant::{
    CreateTenantInput, PortActor, PortContext, PortErrorKind, ReplaceTenantLocalePolicyRequest,
    TenantLocale, TenantLocalePolicyEntry, TenantLocalePolicyPort, TenantService,
    entities::{tenant_locale, tenant_locale_policy_receipt},
};
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection,
    EntityTrait, PaginatorTrait, QueryFilter, Statement, TransactionTrait,
};
use tokio::time::{Instant, sleep};
use uuid::Uuid;

const TENANT_TEST_DATABASE_ENV: &str = "RUSTOK_TENANT_TEST_DATABASE_URL";
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
                "{TENANT_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping tenant locale-policy concurrency test"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let nonce = Uuid::new_v4().simple().to_string();
        let schema_name = format!("rustok_tenant_{}_{}", sanitize_identifier(prefix), nonce);
        let application_name_a = format!("rustok_tenant_locale_a_{}", &nonce[..12]);
        let application_name_b = format!("rustok_tenant_locale_b_{}", &nonce[..12]);

        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let setup_result = async {
            let db_a = connect(&database_url).await?;
            let db_b = connect(&database_url).await?;
            let blocker = connect(&database_url).await?;

            configure_session(&db_a, &schema_name, &application_name_a).await?;
            configure_session(&db_b, &schema_name, &application_name_b).await?;
            configure_session(&blocker, &schema_name, "rustok_tenant_locale_blocker").await?;
            create_postgres_test_tables(&db_a).await?;

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
async fn postgres_concurrent_locale_policy_requests_replay_one_durable_receipt() -> TestResult<()> {
    let Some(test_db) = PostgresTenantTestDb::setup("locale_policy_race").await? else {
        return Ok(());
    };

    let outcome = async {
        let tenant = TenantService::new(test_db.db_a.clone())
            .create_tenant(CreateTenantInput {
                name: "Concurrent Locale Policy".to_string(),
                slug: format!("concurrent-locale-policy-{}", Uuid::new_v4().simple()),
                domain: None,
            })
            .await?;
        let idempotency_key = format!("locale-policy-race-{}", tenant.id);
        let request = ReplaceTenantLocalePolicyRequest {
            expected_revision: 1,
            locales: vec![
                locale_entry("en", true, true, None),
                locale_entry("pt_br", false, true, Some("en")),
            ],
        };

        let blocker_transaction = test_db.blocker.begin().await?;
        blocker_transaction
            .query_one_raw(Statement::from_string(
                DatabaseBackend::Postgres,
                format!(
                    "SELECT id FROM tenants WHERE id = '{}' FOR UPDATE",
                    tenant.id
                ),
            ))
            .await?
            .ok_or_else(|| test_error("tenant row was not available for the race barrier"))?;

        let service_a = TenantService::new(test_db.db_a.clone());
        let service_b = TenantService::new(test_db.db_b.clone());
        let request_a = request.clone();
        let request_b = request.clone();
        let context_a = locale_port_context(tenant.id, &idempotency_key, "locale-writer-a");
        let context_b = locale_port_context(tenant.id, &idempotency_key, "locale-writer-b");

        let task_a =
            tokio::spawn(
                async move { service_a.replace_locale_policy(context_a, request_a).await },
            );
        let task_b =
            tokio::spawn(
                async move { service_b.replace_locale_policy(context_b, request_b).await },
            );

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
            .map_err(|error| test_error(format!("first locale writer task failed: {error}")))?
            .map_err(|error| {
                test_error(format!(
                    "first locale writer returned {} ({:?}): {}",
                    error.code, error.kind, error.message
                ))
            })?;
        let second = task_b
            .await
            .map_err(|error| test_error(format!("second locale writer task failed: {error}")))?
            .map_err(|error| {
                test_error(format!(
                    "second locale writer returned {} ({:?}): {}",
                    error.code, error.kind, error.message
                ))
            })?;

        assert_eq!(first, second);
        assert_eq!(first.revision, 2);
        assert_eq!(first.default_locale.as_str(), "en");
        assert_eq!(
            first
                .locales
                .iter()
                .map(|entry| entry.locale.as_str())
                .collect::<Vec<_>>(),
            vec!["en", "pt-BR"]
        );

        let locales = tenant_locale::Entity::find()
            .filter(tenant_locale::Column::TenantId.eq(tenant.id))
            .all(&test_db.db_a)
            .await?;
        assert_eq!(locales.len(), 2);
        assert!(locales.iter().all(|locale| locale.policy_revision == 2));

        let receipt_count = tenant_locale_policy_receipt::Entity::find()
            .filter(tenant_locale_policy_receipt::Column::TenantId.eq(tenant.id))
            .filter(
                tenant_locale_policy_receipt::Column::IdempotencyKey.eq(idempotency_key.clone()),
            )
            .count(&test_db.db_a)
            .await?;
        assert_eq!(receipt_count, 1);

        let events = SysEvents::find().all(&test_db.db_a).await?;
        assert_eq!(events.len(), 3);
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_type == "tenant.created")
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_type == "tenant.updated")
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_type == "locale.enabled")
                .count(),
            1
        );

        let conflict = TenantService::new(test_db.db_a.clone())
            .replace_locale_policy(
                locale_port_context(tenant.id, &idempotency_key, "locale-writer-conflict"),
                ReplaceTenantLocalePolicyRequest {
                    expected_revision: 2,
                    locales: vec![
                        locale_entry("en", true, true, None),
                        locale_entry("de", false, true, Some("en")),
                    ],
                },
            )
            .await
            .expect_err("reusing the key for a different request must conflict");
        assert_eq!(conflict.kind, PortErrorKind::Conflict);
        assert_eq!(conflict.code, "tenant.locale_policy_idempotency_conflict");

        let events_after_conflict = SysEvents::find().all(&test_db.db_a).await?;
        assert_eq!(events_after_conflict.len(), events.len());
        let receipts_after_conflict = tenant_locale_policy_receipt::Entity::find()
            .filter(tenant_locale_policy_receipt::Column::TenantId.eq(tenant.id))
            .count(&test_db.db_a)
            .await?;
        assert_eq!(receipts_after_conflict, 1);

        let enabled_event = events
            .iter()
            .find(|event| event.event_type == "locale.enabled")
            .ok_or_else(|| test_error("locale.enabled event was not persisted"))?;
        assert_eq!(
            enabled_event.payload["event"]["data"]["locale"],
            serde_json::json!("pt-BR")
        );
        assert_eq!(
            enabled_event.payload["tenant_id"],
            serde_json::json!(tenant.id)
        );

        Ok(())
    }
    .await;

    test_db.cleanup().await?;
    outcome
}

fn locale_port_context(tenant_id: Uuid, key: &str, actor: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor),
        "en",
        format!("tenant-locale-policy-{actor}-{tenant_id}"),
    )
    .with_idempotency_key(key)
    .with_deadline(Duration::from_secs(15))
}

fn locale_entry(
    locale: &str,
    is_default: bool,
    is_enabled: bool,
    fallback_locale: Option<&str>,
) -> TenantLocalePolicyEntry {
    TenantLocalePolicyEntry {
        locale: TenantLocale::new(locale).expect("test locale must be valid"),
        name: locale.to_string(),
        native_name: locale.to_string(),
        is_default,
        is_enabled,
        fallback_locale: fallback_locale
            .map(TenantLocale::new)
            .transpose()
            .expect("test fallback locale must be valid"),
    }
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
                "expected two locale-policy writers to wait on the tenant row lock, observed {waiter_count}"
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
        CREATE TABLE tenant_locale_policy_receipts (
            tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
            idempotency_key VARCHAR(191) NOT NULL,
            request_hash VARCHAR(64) NOT NULL,
            response JSONB NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (tenant_id, idempotency_key)
        )
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
