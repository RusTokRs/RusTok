use std::time::Duration;

use rustok_core::MigrationSource;
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, Value as SqlValue,
};
use sea_orm_migration::SchemaManager;
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

use super::{
    IndexReplayJobAcquireOutcome, IndexReplayJobError, IndexReplayJobLease,
    IndexReplayJobLeaseRequest, PostgresIndexReplayCheckpointStore, PostgresIndexReplayJobStore,
};
use crate::{
    EntityName, FieldCardinality, FieldName, IndexField, IndexModule, IndexReplayCheckpoint,
    IndexReplayCheckpointKey, IndexReplayCheckpointStore, IndexSchema, IndexValueType, LocaleKey,
    LocaleMode, ModuleName, SchemaRef, SchemaVersion,
};

const TENANT: &str = "22222222-2222-2222-2222-222222222222";
const SOURCE: &str = "product-primary";

struct Fixture {
    db: DatabaseConnection,
    jobs: PostgresIndexReplayJobStore,
    schema: IndexSchema,
}

impl Fixture {
    async fn new(locale_mode: LocaleMode) -> Self {
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

        let schema = schema(locale_mode);
        let fingerprint = schema.fingerprint().unwrap().to_string();
        let schema_json = serde_json::to_value(&schema).unwrap();
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO index_schemas (tenant_id, module_name, entity_name, schema_version, schema_fingerprint, schema_json, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'active')",
            vec![
                TENANT.to_owned().into(),
                schema.reference.module.as_str().to_owned().into(),
                schema.reference.entity.as_str().to_owned().into(),
                i64::from(schema.reference.version.get()).into(),
                fingerprint.into(),
                SqlValue::Json(Some(Box::new(schema_json))),
            ],
        ))
        .await
        .expect("schema fixture should persist");

        let jobs = PostgresIndexReplayJobStore::new(db.clone());
        Self { db, jobs, schema }
    }

    fn schema_request(&self, worker: &str) -> IndexReplayJobLeaseRequest {
        IndexReplayJobLeaseRequest::new(
            Uuid::parse_str(TENANT).unwrap(),
            self.schema.reference.clone(),
            SOURCE,
            worker,
            Duration::from_secs(60),
        )
        .unwrap()
    }

    fn locale_request(&self, locale: &str, worker: &str) -> IndexReplayJobLeaseRequest {
        IndexReplayJobLeaseRequest::for_locale(
            Uuid::parse_str(TENANT).unwrap(),
            self.schema.reference.clone(),
            LocaleKey::new(locale).unwrap(),
            SOURCE,
            worker,
            Duration::from_secs(60),
        )
        .unwrap()
    }

    async fn acquire(&self, request: &IndexReplayJobLeaseRequest) -> IndexReplayJobLease {
        match self.jobs.acquire(request).await.unwrap() {
            IndexReplayJobAcquireOutcome::Acquired(lease) => lease,
            outcome => panic!("replay job should be acquired, got {outcome:?}"),
        }
    }
}

fn schema(locale_mode: LocaleMode) -> IndexSchema {
    IndexSchema {
        reference: SchemaRef {
            module: ModuleName::new("catalog").unwrap(),
            entity: EntityName::new("product").unwrap(),
            version: SchemaVersion::INITIAL,
        },
        locale_mode,
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

#[tokio::test]
async fn locale_jobs_are_distinct_from_schema_and_other_locales() {
    let fixture = Fixture::new(LocaleMode::Required).await;
    let schema_lease = fixture
        .acquire(&fixture.schema_request("schema-worker"))
        .await;
    let en_lease = fixture
        .acquire(&fixture.locale_request("EN-us", "en-worker"))
        .await;
    let de_lease = fixture
        .acquire(&fixture.locale_request("de", "de-worker"))
        .await;

    assert!(schema_lease.locale().is_none());
    assert_eq!(en_lease.locale().unwrap().as_str(), "en-US");
    assert_eq!(de_lease.locale().unwrap().as_str(), "de");
    assert_ne!(schema_lease.job_id(), en_lease.job_id());
    assert_ne!(en_lease.job_id(), de_lease.job_id());

    assert_eq!(
        fixture
            .jobs
            .acquire(&fixture.locale_request("en-US", "other-en-worker"))
            .await
            .unwrap(),
        IndexReplayJobAcquireOutcome::Busy
    );

    let rows = fixture
        .db
        .query_all(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT scope_kind, locale_key, request FROM index_jobs WHERE tenant_id = '22222222-2222-2222-2222-222222222222' AND kind = 'rebuild' ORDER BY scope_kind, locale_key"
                .to_owned(),
        ))
        .await
        .unwrap();
    assert_eq!(rows.len(), 3);
    let mut observed = Vec::new();
    for row in rows {
        let scope_kind: String = row.try_get("", "scope_kind").unwrap();
        let locale_key: Option<String> = row.try_get("", "locale_key").unwrap();
        let request: JsonValue = row.try_get("", "request").unwrap();
        observed.push((scope_kind, locale_key, request));
    }
    assert!(observed.iter().any(|(scope, locale, request)| {
        scope == "schema"
            && locale.is_none()
            && request == &json!({"contract": "index_replay_job_v1", "source_name": SOURCE})
    }));
    assert!(observed.iter().any(|(scope, locale, request)| {
        scope == "locale"
            && locale.as_deref() == Some("en-US")
            && request
                == &json!({"contract": "index_replay_job_v2", "source_name": SOURCE, "locale": "en-US"})
    }));
    assert!(observed.iter().any(|(scope, locale, request)| {
        scope == "locale"
            && locale.as_deref() == Some("de")
            && request
                == &json!({"contract": "index_replay_job_v2", "source_name": SOURCE, "locale": "de"})
    }));

    // A complete schema-wide checkpoint must not satisfy a locale-scoped job.
    let schema_checkpoint = IndexReplayCheckpoint::new(
        IndexReplayCheckpointKey::for_locale(
            Uuid::parse_str(TENANT).unwrap(),
            fixture.schema.reference.clone(),
            LocaleKey::new("de").unwrap(),
            SOURCE,
        )
        .unwrap(),
        None,
        Some(1),
        Some(Uuid::from_u128(1).to_string()),
    )
    .unwrap();
    PostgresIndexReplayCheckpointStore::new(fixture.db.clone(), schema_lease.clone())
        .commit_replay_checkpoint(&schema_checkpoint)
        .await
        .unwrap();
    fixture.jobs.succeed(&schema_lease).await.unwrap();
    assert_eq!(
        fixture.jobs.succeed(&en_lease).await,
        Err(IndexReplayJobError::CheckpointMissing)
    );
}

#[tokio::test]
async fn locale_job_scope_rejects_nonlocalized_schema() {
    let fixture = Fixture::new(LocaleMode::None).await;
    assert_eq!(
        fixture
            .jobs
            .acquire(&fixture.locale_request("en", "locale-worker"))
            .await,
        Err(IndexReplayJobError::LocaleScopeUnsupported(
            fixture.schema.reference.clone()
        ))
    );

    let count: i64 = fixture
        .db
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS value FROM index_jobs".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "value")
        .unwrap();
    assert_eq!(count, 0);
}