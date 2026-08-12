use std::time::Duration;

use rustok_core::MigrationSource;
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, Value as SqlValue,
};
use sea_orm_migration::SchemaManager;
use serde_json::json;
use uuid::Uuid;

use super::{
    PostgresSchemaLeaseStore, SchemaApplicationLeaseRequest, SchemaLeaseAcquireOutcome,
    SchemaLeaseError,
};
use crate::{
    EntityName, FieldCardinality, FieldName, IndexField, IndexModule, IndexSchema, IndexValueType,
    LocaleMode, ModuleName, SchemaRef, SchemaVersion,
};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

struct Fixture {
    db: DatabaseConnection,
    store: PostgresSchemaLeaseStore,
    schema: IndexSchema,
}

impl Fixture {
    async fn new(status: &str) -> Self {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        db.execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .expect("foreign keys should be enabled");
        db.execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY)")
            .await
            .expect("tenant fixture should be created");
        db.execute_unprepared(&format!("INSERT INTO tenants (id) VALUES ('{TENANT}')"))
            .await
            .expect("tenant fixture should be inserted");
        let manager = SchemaManager::new(&db);
        for migration in IndexModule.migrations() {
            migration
                .up(&manager)
                .await
                .unwrap_or_else(|error| panic!("{} should apply: {error}", migration.name()));
        }

        let schema = schema();
        let fingerprint = schema.fingerprint().unwrap().to_string();
        let schema_json = serde_json::to_value(&schema).unwrap();
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO index_schemas (tenant_id, module_name, entity_name, schema_version, schema_fingerprint, schema_json, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            vec![
                TENANT.to_owned().into(),
                schema.reference.module.as_str().to_owned().into(),
                schema.reference.entity.as_str().to_owned().into(),
                i64::from(schema.reference.version.get()).into(),
                fingerprint.into(),
                SqlValue::Json(Some(Box::new(schema_json))),
                status.to_owned().into(),
            ],
        ))
        .await
        .expect("schema fixture should persist");
        let store = PostgresSchemaLeaseStore::new(db.clone());
        Self { db, store, schema }
    }

    fn request(&self, worker: &str) -> SchemaApplicationLeaseRequest {
        SchemaApplicationLeaseRequest::new(
            Uuid::parse_str(TENANT).unwrap(),
            &self.schema,
            worker,
            Duration::from_secs(60),
        )
        .unwrap()
    }
}

fn schema() -> IndexSchema {
    IndexSchema {
        reference: SchemaRef {
            module: ModuleName::new("catalog").unwrap(),
            entity: EntityName::new("product").unwrap(),
            version: SchemaVersion::INITIAL,
        },
        locale_mode: LocaleMode::Required,
        fields: vec![IndexField {
            name: FieldName::new("id").unwrap(),
            value_type: IndexValueType::Uuid,
            cardinality: FieldCardinality::One,
            nullable: false,
            selectable: true,
            filterable: true,
            sortable: false,
        }],
        links: Vec::new(),
    }
}

async fn scalar_i64(db: &DatabaseConnection, sql: &str) -> i64 {
    db.query_one(Statement::from_string(DbBackend::Sqlite, sql.to_owned()))
        .await
        .expect("scalar query should execute")
        .expect("scalar query should return one row")
        .try_get("", "value")
        .expect("scalar value should be integer")
}

#[tokio::test]
async fn acquire_excludes_other_workers_and_completion_is_terminal() {
    let fixture = Fixture::new("active").await;
    let first = match fixture
        .store
        .acquire(&fixture.request("worker-a"))
        .await
        .unwrap()
    {
        SchemaLeaseAcquireOutcome::Acquired(lease) => lease,
        outcome => panic!("first acquisition should win, got {outcome:?}"),
    };
    assert_eq!(first.attempt_count(), 1);
    assert_eq!(
        fixture
            .store
            .acquire(&fixture.request("worker-b"))
            .await
            .unwrap(),
        SchemaLeaseAcquireOutcome::Busy
    );
    fixture
        .store
        .heartbeat(&first, Duration::from_secs(120))
        .await
        .unwrap();
    fixture.store.succeed(&first).await.unwrap();
    assert_eq!(
        fixture
            .store
            .acquire(&fixture.request("worker-b"))
            .await
            .unwrap(),
        SchemaLeaseAcquireOutcome::AlreadyApplied {
            job_id: first.job_id(),
        }
    );
    assert_eq!(
        fixture
            .store
            .heartbeat(&first, Duration::from_secs(60))
            .await,
        Err(SchemaLeaseError::LeaseLost)
    );
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_jobs WHERE kind = 'schema_apply' AND state = 'succeeded' AND lease_owner IS NULL AND lease_expires_at IS NULL AND completed_at IS NOT NULL"
        )
        .await,
        1
    );
}

#[tokio::test]
async fn expired_lease_is_reclaimed_with_attempt_fencing() {
    let fixture = Fixture::new("active").await;
    let first = match fixture
        .store
        .acquire(&fixture.request("worker-a"))
        .await
        .unwrap()
    {
        SchemaLeaseAcquireOutcome::Acquired(lease) => lease,
        outcome => panic!("first acquisition should win, got {outcome:?}"),
    };
    fixture
        .db
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE index_jobs SET lease_expires_at = datetime('now', '-1 second') WHERE tenant_id = ?1 AND job_id = ?2",
            vec![TENANT.to_owned().into(), first.job_id().to_string().into()],
        ))
        .await
        .unwrap();

    let second = match fixture
        .store
        .acquire(&fixture.request("worker-b"))
        .await
        .unwrap()
    {
        SchemaLeaseAcquireOutcome::Acquired(lease) => lease,
        outcome => panic!("expired lease should be reclaimed, got {outcome:?}"),
    };
    assert_eq!(second.job_id(), first.job_id());
    assert_eq!(second.attempt_count(), 2);
    assert_eq!(
        fixture.store.succeed(&first).await,
        Err(SchemaLeaseError::LeaseLost)
    );
    fixture
        .store
        .fail(&second, "schema.ddl_failed", json!({"retryable": true}))
        .await
        .unwrap();

    let third = match fixture
        .store
        .acquire(&fixture.request("worker-c"))
        .await
        .unwrap()
    {
        SchemaLeaseAcquireOutcome::Acquired(lease) => lease,
        outcome => panic!("failed terminal job should permit a new attempt, got {outcome:?}"),
    };
    assert_ne!(third.job_id(), second.job_id());
    assert_eq!(third.attempt_count(), 1);
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_jobs WHERE kind = 'schema_apply' AND state = 'failed' AND last_error_code = 'schema.ddl_failed' AND completed_at IS NOT NULL"
        )
        .await,
        1
    );
}

#[tokio::test]
async fn schema_registration_and_request_identity_fail_closed() {
    let retired = Fixture::new("retired").await;
    assert_eq!(
        retired.store.acquire(&retired.request("worker-a")).await,
        Err(SchemaLeaseError::SchemaRetired(
            retired.schema.reference.clone()
        ))
    );

    let active = Fixture::new("active").await;
    active
        .db
        .execute(Statement::from_string(
            DbBackend::Sqlite,
            "DELETE FROM index_schemas".to_owned(),
        ))
        .await
        .unwrap();
    assert_eq!(
        active.store.acquire(&active.request("worker-a")).await,
        Err(SchemaLeaseError::SchemaNotRegistered(
            active.schema.reference.clone()
        ))
    );

    assert!(matches!(
        SchemaApplicationLeaseRequest::new(
            Uuid::nil(),
            &active.schema,
            "worker-a",
            Duration::from_secs(60),
        ),
        Err(SchemaLeaseError::NilTenantId)
    ));
    assert!(matches!(
        SchemaApplicationLeaseRequest::new(
            Uuid::parse_str(TENANT).unwrap(),
            &active.schema,
            " worker-a ",
            Duration::from_secs(60),
        ),
        Err(SchemaLeaseError::InvalidWorkerId { .. })
    ));
    assert!(matches!(
        SchemaApplicationLeaseRequest::new(
            Uuid::parse_str(TENANT).unwrap(),
            &active.schema,
            "worker-a",
            Duration::ZERO,
        ),
        Err(SchemaLeaseError::InvalidLeaseDuration)
    ));
}
