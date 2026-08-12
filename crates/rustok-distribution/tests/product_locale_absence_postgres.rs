#![cfg(feature = "mod-product")]

use std::{env, error::Error, time::Duration};

use rustok_core::{MigrationSource, ModuleRegistry};
use rustok_index::{
    EntityKey, EntityName, IndexDriftDependencyFailureKind, IndexDriftDigestRequest,
    IndexDriftEntityState, IndexDriftSnapshotReader, IndexModule, LocaleKey, ModuleName, SchemaRef,
    SchemaVersion, materialize_index_source_absence_registry, materialize_index_source_registry,
    materialize_postgres_index_drift_snapshot_reader, materialize_postgres_index_sources,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
    TransactionTrait,
};
use sea_orm_migration::{MigrationTrait, MigratorTrait, SchemaManager};
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_INDEX_TEST_DATABASE_URL";
const TENANT_ID: Uuid = Uuid::from_u128(1);
const PRODUCT_ID: Uuid = Uuid::from_u128(101);
const EN_TRANSLATION_ID: Uuid = Uuid::from_u128(111);
const DE_TRANSLATION_ID: Uuid = Uuid::from_u128(112);

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

struct ProductMigrator;

#[async_trait::async_trait]
impl MigratorTrait for ProductMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        rustok_product::migrations::migrations()
    }
}

struct TestDatabase {
    control: DatabaseConnection,
    source: DatabaseConnection,
    snapshot: DatabaseConnection,
    lock: DatabaseConnection,
    writer: DatabaseConnection,
    observer: DatabaseConnection,
    schema_name: String,
    snapshot_application_name: String,
}

impl TestDatabase {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Product locale absence harness"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let suffix = Uuid::new_v4().simple().to_string();
        let schema_name = format!("rustok_index_product_absence_{suffix}");
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let migration = scoped_connection(
            &database_url,
            &schema_name,
            &format!("idx_abs_migration_{suffix}"),
        )
        .await?;
        create_product_migration_prerequisites(&migration).await?;
        ProductMigrator::up(&migration, None).await?;
        let manager = SchemaManager::new(&migration);
        for migration in IndexModule.migrations() {
            migration.up(&manager).await?;
        }
        seed_product(&migration).await?;
        migration.close().await?;

        let snapshot_application_name = format!("idx_abs_snapshot_{suffix}");
        Ok(Some(Self {
            control,
            source: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_abs_source_{suffix}"),
            )
            .await?,
            snapshot: scoped_connection(&database_url, &schema_name, &snapshot_application_name)
                .await?,
            lock: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_abs_lock_{suffix}"),
            )
            .await?,
            writer: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_abs_writer_{suffix}"),
            )
            .await?,
            observer: scoped_connection(
                &database_url,
                &schema_name,
                &format!("idx_abs_observer_{suffix}"),
            )
            .await?,
            schema_name,
            snapshot_application_name,
        }))
    }

    async fn cleanup(self) -> TestResult<()> {
        self.source.close().await?;
        self.snapshot.close().await?;
        self.lock.close().await?;
        self.writer.close().await?;
        self.observer.close().await?;
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        self.control.close().await?;
        Ok(())
    }
}

#[tokio::test]
async fn production_product_locale_absence_is_fenced_across_real_migrations() -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };

    let outcome = run_product_locale_absence_scenarios(&database).await;
    let cleanup = database.cleanup().await;
    outcome?;
    cleanup
}

async fn run_product_locale_absence_scenarios(database: &TestDatabase) -> TestResult<()> {
    let reader = product_reader(database)?;

    let stable_key = product_key("fr")?;
    let stable = reader
        .capture_entity_snapshot(&IndexDriftDigestRequest::new(stable_key.clone())?)
        .await?;
    assert!(stable.boundary().as_str().starts_with("pg:"));
    assert_eq!(stable.boundary().as_str().len(), 67);
    assert_missing(stable.source(), &stable_key);
    assert_missing(stable.materialized(), &stable_key);

    let changing_key = product_key("de")?;
    let lock = database.lock.begin().await?;
    lock.execute_unprepared("LOCK TABLE index_entities IN ACCESS EXCLUSIVE MODE")
        .await?;

    let capture_reader = reader.clone();
    let capture_request = IndexDriftDigestRequest::new(changing_key)?;
    let capture = tokio::spawn(async move {
        capture_reader
            .capture_entity_snapshot(&capture_request)
            .await
    });

    if let Err(error) =
        wait_for_blocked_materialized_read(&database.observer, &database.snapshot_application_name)
            .await
    {
        let _ = lock.rollback().await;
        capture.abort();
        return Err(error);
    }

    if let Err(error) = insert_de_translation(&database.writer).await {
        let _ = lock.rollback().await;
        capture.abort();
        return Err(error);
    }
    lock.commit().await?;

    let changed = capture
        .await?
        .expect_err("translation insertion between observations must reject the snapshot pair");
    assert_eq!(changed.kind(), IndexDriftDependencyFailureKind::Retryable);
    assert_eq!(changed.code(), "index_drift_source_changed_during_capture");
    Ok(())
}

fn product_reader(
    database: &TestDatabase,
) -> TestResult<rustok_index::PostgresIndexDriftSnapshotReader> {
    let registry = ModuleRegistry::new()
        .register(IndexModule)
        .register(rustok_product::ProductModule);
    let mut extensions = rustok_distribution::build_runtime_extensions(&registry)?;

    materialize_postgres_index_sources(&mut extensions, database.source.clone())?;
    let sources = materialize_index_source_registry(&extensions)?
        .ok_or_else(|| std::io::Error::other("Product replay source registry is missing"))?;
    extensions.insert(sources);
    let absence = materialize_index_source_absence_registry(&extensions)?
        .ok_or_else(|| std::io::Error::other("Product absence registry is missing"))?;
    extensions.insert(absence);

    materialize_postgres_index_drift_snapshot_reader(&extensions, database.snapshot.clone())?
        .ok_or_else(|| std::io::Error::other("Product drift snapshot reader is missing").into())
}

async fn create_product_migration_prerequisites(db: &DatabaseConnection) -> TestResult<()> {
    db.execute_unprepared(
        r#"
CREATE TABLE tenants (
    id UUID PRIMARY KEY
);
CREATE TABLE taxonomy_terms (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    UNIQUE (tenant_id, id)
);
-- This Product-only harness does not run Channel migrations, but the canonical Product source reads
-- the Channel-owned tenant identity watermark. An empty table represents a tenant with no Channels.
CREATE TABLE channel_index_identity_generations (
    tenant_id UUID PRIMARY KEY,
    generation BIGINT NOT NULL CHECK (generation > 0)
);
"#,
    )
    .await?;
    let manager = SchemaManager::new(db);
    flex::cache_generation::create_field_definition_cache_generation_table(&manager).await?;
    Ok(())
}

async fn seed_product(db: &DatabaseConnection) -> TestResult<()> {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO tenants (id) VALUES ('{TENANT_ID}');
INSERT INTO products (id, tenant_id) VALUES ('{PRODUCT_ID}', '{TENANT_ID}');
INSERT INTO product_translations (id, product_id, tenant_id, locale, title, handle)
VALUES (
    '{EN_TRANSLATION_ID}',
    '{PRODUCT_ID}',
    '{TENANT_ID}',
    'en',
    'Product locale absence fixture',
    'product-locale-absence-fixture'
);

-- Seed one resolved relation snapshot after the initial translation so its relation trigger creates
-- the exact current graph projection.
INSERT INTO product_sales_channel_index_relation_snapshots (
    tenant_id,
    product_id,
    relation_epoch,
    channel_ids
) VALUES (
    '{TENANT_ID}',
    '{PRODUCT_ID}',
    1,
    '[]'::jsonb
);

-- The tenant has no Channels, so generation 0 is the exact current identity watermark and default
-- Product metadata is unrestricted visibility (`all`).
INSERT INTO product_sales_channel_index_relation_freshness_snapshots (
    tenant_id,
    product_id,
    relation_epoch,
    product_source_version,
    visibility_key,
    channel_identity_generation
)
SELECT
    product.tenant_id,
    product.id,
    relation.relation_epoch,
    product.index_revision,
    'all',
    0
FROM products product
JOIN LATERAL (
    SELECT relation_epoch
    FROM product_sales_channel_index_relation_snapshots relation
    WHERE relation.tenant_id = product.tenant_id
      AND relation.product_id = product.id
    ORDER BY relation.relation_epoch DESC
    LIMIT 1
) relation ON TRUE
WHERE product.tenant_id = '{TENANT_ID}'
  AND product.id = '{PRODUCT_ID}';
"#
    ))
    .await?;
    Ok(())
}

async fn insert_de_translation(db: &DatabaseConnection) -> TestResult<()> {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO product_translations (id, product_id, tenant_id, locale, title, handle)
VALUES (
    '{DE_TRANSLATION_ID}',
    '{PRODUCT_ID}',
    '{TENANT_ID}',
    'de',
    'Concurrent German product',
    'concurrent-german-product'
)
"#
    ))
    .await?;
    Ok(())
}

async fn wait_for_blocked_materialized_read(
    db: &DatabaseConnection,
    snapshot_application_name: &str,
) -> TestResult<()> {
    for _ in 0..500 {
        let row = db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
SELECT EXISTS (
    SELECT 1
    FROM pg_stat_activity
    WHERE datname = current_database()
      AND application_name = $1
      AND wait_event_type = 'Lock'
      AND query LIKE '%FROM index_entities WHERE tenant_id%'
) AS blocked
"#,
                vec![snapshot_application_name.to_owned().into()],
            ))
            .await?
            .ok_or_else(|| std::io::Error::other("pg_stat_activity did not return a row"))?;
        if row.try_get::<bool>("", "blocked")? {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "snapshot reader did not reach the blocked materialized read",
    )
    .into())
}

fn product_key(locale: &str) -> TestResult<EntityKey> {
    Ok(EntityKey {
        tenant_id: TENANT_ID,
        schema: SchemaRef {
            module: ModuleName::new("rustok-product")?,
            entity: EntityName::new("product")?,
            // Index core requires a numeric schema key; only this current Product contract is
            // registered by rustok-distribution.
            version: SchemaVersion::new(4),
        },
        entity_id: PRODUCT_ID,
        locale: Some(LocaleKey::new(locale)?),
    })
}

fn assert_missing(state: &IndexDriftEntityState, expected_key: &EntityKey) {
    let IndexDriftEntityState::Missing { key } = state else {
        panic!("expected missing state, got {state:?}");
    };
    assert_eq!(key, expected_key);
}

fn database_url() -> Option<String> {
    env::var(DATABASE_ENV)
        .or_else(|_| env::var("DATABASE_URL"))
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

async fn scoped_connection(
    database_url: &str,
    schema_name: &str,
    application_name: &str,
) -> TestResult<DatabaseConnection> {
    let db = connect(database_url).await?;
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}""#))
        .await?;
    db.execute_unprepared(&format!("SET application_name TO '{application_name}'"))
        .await?;
    Ok(db)
}
