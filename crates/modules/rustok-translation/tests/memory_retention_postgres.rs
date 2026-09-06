#![cfg(feature = "runtime")]

use std::{error::Error, io, time::Duration};

use chrono::{Duration as ChronoDuration, Utc};
use rustok_api::HostRuntimeContext;
use rustok_core::{ModuleRuntimeExtensions, RusToKModule};
use rustok_runtime::{ModuleWorkRegistrations, ModuleWorkScheduler};
use rustok_translation::{
    TranslationModule,
    entities::{memory_entry, memory_receipt},
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseBackend,
    DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, Set, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_TRANSLATION_TEST_POSTGRES_URL";
type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

struct TestDatabase {
    control: DatabaseConnection,
    database_url: String,
    schema_name: String,
}

impl TestDatabase {
    async fn setup() -> TestResult<Self> {
        let database_url = std::env::var(DATABASE_ENV)
            .map_err(|_| test_error(format!("{DATABASE_ENV} must be configured")))?;
        let control = connect(&database_url).await?;
        let suffix = Uuid::new_v4().simple().to_string();
        let schema_name = format!("rustok_translation_retention_{suffix}");
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let migration = scoped_connection(&database_url, &schema_name).await?;
        migration
            .execute_unprepared("CREATE TABLE tenants (id UUID PRIMARY KEY NOT NULL)")
            .await?;
        let manager = SchemaManager::new(&migration);
        for migration_step in rustok_translation::migrations::migrations() {
            migration_step.up(&manager).await?;
        }
        migration.close().await?;

        Ok(Self {
            control,
            database_url,
            schema_name,
        })
    }

    async fn connection(&self) -> TestResult<DatabaseConnection> {
        scoped_connection(&self.database_url, &self.schema_name).await
    }

    async fn cleanup(self) -> TestResult<()> {
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
#[ignore = "requires RUSTOK_TRANSLATION_TEST_POSTGRES_URL"]
async fn postgres_retention_multi_replica_converges_on_one_transition_and_receipt() -> TestResult<()>
{
    let database = TestDatabase::setup().await?;
    let outcome = run_multi_replica_contract(&database).await;
    let cleanup = database.cleanup().await;
    outcome?;
    cleanup
}

async fn run_multi_replica_contract(database: &TestDatabase) -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let seed = database.connection().await?;
    seed.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "INSERT INTO tenants (id) VALUES ($1)",
        [tenant_id.into()],
    ))
    .await?;

    let tombstone_id = insert_memory_entry(
        &seed,
        tenant_id,
        "retain_until",
        Some(Utc::now().fixed_offset() - ChronoDuration::minutes(1)),
        None,
        None,
    )
    .await?;
    seed.close().await?;

    let first = scheduler(database.connection().await?).await?;
    let second = scheduler(database.connection().await?).await?;

    let (first_run, second_run) = tokio::join!(first.run_once(), second.run_once());
    let executed = first_run? + second_run?;
    assert!(
        (1..=2).contains(&executed),
        "one or both PostgreSQL replicas should observe the same tombstone work"
    );

    let observer = database.connection().await?;
    let tombstoned = memory_entry::Entity::find_by_id(tombstone_id)
        .one(&observer)
        .await?
        .ok_or_else(|| test_error("tombstoned Translation Memory entry disappeared"))?;
    assert!(tombstoned.tombstoned_at.is_some());
    assert_eq!(tombstoned.revision, 2);
    assert_eq!(
        receipt_count(&observer, tombstone_id, "tombstone").await?,
        1
    );

    let purge_id = insert_memory_entry(
        &observer,
        tenant_id,
        "owner_lifecycle",
        None,
        Some(Utc::now().fixed_offset() - ChronoDuration::hours(30)),
        Some(Utc::now().fixed_offset() - ChronoDuration::hours(25)),
    )
    .await?;

    let (first_run, second_run) = tokio::join!(first.run_once(), second.run_once());
    let executed = first_run? + second_run?;
    assert!(
        (1..=2).contains(&executed),
        "one or both PostgreSQL replicas should observe the same purge work"
    );
    assert!(
        memory_entry::Entity::find_by_id(purge_id)
            .one(&observer)
            .await?
            .is_none(),
        "eligible Translation Memory entry should be purged"
    );
    assert_eq!(receipt_count(&observer, purge_id, "purge").await?, 1);

    let (first_run, second_run) = tokio::join!(first.run_once(), second.run_once());
    assert_eq!(first_run? + second_run?, 0);
    observer.close().await?;
    Ok(())
}

async fn scheduler(database: DatabaseConnection) -> TestResult<ModuleWorkScheduler> {
    let mut extensions = ModuleRuntimeExtensions::default();
    TranslationModule.register_runtime_extensions(&mut extensions)?;
    let registrations = extensions
        .get::<ModuleWorkRegistrations>()
        .cloned()
        .ok_or_else(|| test_error("Translation module work registrations are missing"))?;
    let host = HostRuntimeContext::new(database);
    let scheduler = ModuleWorkScheduler::new();
    registrations.register_all(&host, &scheduler).await?;
    Ok(scheduler)
}

async fn insert_memory_entry(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    retention_policy: &str,
    retain_until: Option<chrono::DateTime<chrono::FixedOffset>>,
    owner_deleted_at: Option<chrono::DateTime<chrono::FixedOffset>>,
    tombstoned_at: Option<chrono::DateTime<chrono::FixedOffset>>,
) -> TestResult<Uuid> {
    let id = Uuid::new_v4();
    let now = Utc::now().fixed_offset();
    memory_entry::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        source_locale: Set("en".to_string()),
        target_locale: Set("de".to_string()),
        owner_slug: Set("pages".to_string()),
        resource_kind: Set("page".to_string()),
        resource_id: Set(Uuid::new_v4().to_string()),
        subresource_id: Set(None),
        field_key: Set("title".to_string()),
        source_text: Set("Source".to_string()),
        target_text: Set("Target".to_string()),
        source_key: Set(Uuid::new_v4().simple().to_string()),
        source_hash: Set(Uuid::new_v4().simple().to_string()),
        target_hash: Set(Uuid::new_v4().simple().to_string()),
        context_fingerprint: Set(Uuid::new_v4().simple().to_string()),
        segmentation_version: Set("owner-field-v1".to_string()),
        origin: Set("manual".to_string()),
        quality_state: Set("human_approved_applied".to_string()),
        reviewer_actor_kind: Set("system".to_string()),
        reviewer_actor_id: Set("postgres-retention-evidence".to_string()),
        proposal_id: Set(Uuid::new_v4()),
        apply_receipt_id: Set(Uuid::new_v4()),
        retention_policy: Set(retention_policy.to_string()),
        retain_until: Set(retain_until),
        owner_lifecycle_revision: Set(owner_deleted_at.map(|_| "deleted-7".to_string())),
        owner_deleted_at: Set(owner_deleted_at),
        tombstoned_at: Set(tombstoned_at),
        revision: Set(1),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(database)
    .await?;
    Ok(id)
}

async fn receipt_count(
    database: &DatabaseConnection,
    entry_id: Uuid,
    operation: &str,
) -> TestResult<u64> {
    Ok(memory_receipt::Entity::find()
        .filter(memory_receipt::Column::EntryId.eq(entry_id))
        .filter(memory_receipt::Column::Operation.eq(operation))
        .count(database)
        .await?)
}

async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_string());
    options
        .max_connections(4)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(5))
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

async fn scoped_connection(
    database_url: &str,
    schema_name: &str,
) -> TestResult<DatabaseConnection> {
    let separator = if database_url.contains('?') { '&' } else { '?' };
    let scoped_url =
        format!("{database_url}{separator}options=-csearch_path%3D{schema_name}%2Cpublic");
    connect(&scoped_url).await
}

fn test_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
