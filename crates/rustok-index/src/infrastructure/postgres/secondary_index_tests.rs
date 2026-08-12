use std::time::Duration;

use rustok_core::MigrationSource;
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, Value as SqlValue,
};
use sea_orm_migration::SchemaManager;
use serde_json::json;
use uuid::Uuid;

use super::{
    PostgresSecondaryIndexManager, SecondaryIndexClaimOutcome, SecondaryIndexError,
    SecondaryIndexExecutionOutcome, SecondaryIndexKind, SecondaryIndexOperation,
    SecondaryIndexPlan, SecondaryIndexRequest,
};
use crate::{
    EntityName, FieldCardinality, FieldName, IndexField, IndexModule, IndexSchema, IndexValueType,
    LocaleMode, ModuleName, SchemaRef, SchemaVersion,
};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";

struct Fixture {
    db: DatabaseConnection,
    manager: PostgresSecondaryIndexManager,
    schema: IndexSchema,
    plan: SecondaryIndexPlan,
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
        let plan = SecondaryIndexPlan::from_schema(Uuid::parse_str(TENANT).unwrap(), &schema)
            .expect("secondary indexes should derive");
        let index_manager = PostgresSecondaryIndexManager::new(db.clone());
        Self {
            db,
            manager: index_manager,
            schema,
            plan,
        }
    }

    fn spec(&self, field: &str) -> super::SecondaryIndexSpec {
        self.plan
            .indexes()
            .iter()
            .find(|spec| spec.field_name().as_str() == field)
            .unwrap_or_else(|| panic!("missing secondary index for {field}"))
            .clone()
    }

    fn request(
        &self,
        field: &str,
        operation: SecondaryIndexOperation,
        worker: &str,
    ) -> SecondaryIndexRequest {
        SecondaryIndexRequest::new(self.spec(field), operation, worker, Duration::from_secs(60))
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
        fields: vec![
            field(
                "status",
                IndexValueType::String,
                FieldCardinality::One,
                true,
                false,
            ),
            field(
                "price_minor",
                IndexValueType::Integer,
                FieldCardinality::One,
                true,
                true,
            ),
            field(
                "tags",
                IndexValueType::String,
                FieldCardinality::Many,
                true,
                false,
            ),
            field(
                "internal_note",
                IndexValueType::String,
                FieldCardinality::One,
                false,
                false,
            ),
        ],
        links: Vec::new(),
    }
}

fn field(
    name: &str,
    value_type: IndexValueType,
    cardinality: FieldCardinality,
    filterable: bool,
    sortable: bool,
) -> IndexField {
    IndexField {
        name: FieldName::new(name).unwrap(),
        value_type,
        cardinality,
        nullable: false,
        selectable: true,
        filterable,
        sortable,
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

#[test]
fn plan_derives_stable_typed_and_containment_indexes() {
    let schema = schema();
    let tenant = Uuid::parse_str(TENANT).unwrap();
    let first = SecondaryIndexPlan::from_schema(tenant, &schema).unwrap();
    let second = SecondaryIndexPlan::from_schema(tenant, &schema).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.indexes().len(), 3);
    assert!(
        first
            .indexes()
            .windows(2)
            .all(|indexes| indexes[0].field_name() < indexes[1].field_name())
    );
    assert!(
        first
            .indexes()
            .iter()
            .all(|index| index.index_name().len() <= 63)
    );

    let price = first
        .indexes()
        .iter()
        .find(|index| index.field_name().as_str() == "price_minor")
        .unwrap();
    assert_eq!(price.kind(), SecondaryIndexKind::Scalar);
    let price_sql = price.create_statement(DbBackend::Postgres).unwrap();
    assert!(price_sql.contains("CREATE INDEX CONCURRENTLY"));
    assert!(price_sql.contains("(payload -> 'price_minor') ->> 'value'"));
    assert!(price_sql.contains("::bigint"));
    assert!(price_sql.contains("schema_fingerprint ="));
    assert!(price_sql.contains("is_deleted = FALSE"));

    let tags = first
        .indexes()
        .iter()
        .find(|index| index.field_name().as_str() == "tags")
        .unwrap();
    assert_eq!(tags.kind(), SecondaryIndexKind::JsonContainment);
    let tags_sql = tags.create_statement(DbBackend::Postgres).unwrap();
    assert!(tags_sql.contains("USING gin"));
    assert!(tags_sql.contains("((payload -> 'tags') -> 'value') jsonb_path_ops"));
    assert!(
        first
            .indexes()
            .iter()
            .all(|index| index.field_name().as_str() != "internal_note")
    );
}

#[tokio::test]
async fn ensure_reindex_and_retire_are_durable_and_idempotent() {
    let fixture = Fixture::new("active").await;
    let ensure = fixture.request("price_minor", SecondaryIndexOperation::Ensure, "worker-a");
    let lease = match fixture.manager.claim(&ensure).await.unwrap() {
        SecondaryIndexClaimOutcome::Acquired(lease) => lease,
        outcome => panic!("ensure should acquire, got {outcome:?}"),
    };
    assert_eq!(
        fixture
            .manager
            .claim(&fixture.request("price_minor", SecondaryIndexOperation::Ensure, "worker-b",))
            .await
            .unwrap(),
        SecondaryIndexClaimOutcome::Busy
    );
    assert_eq!(
        fixture.manager.execute(&lease).await.unwrap(),
        SecondaryIndexExecutionOutcome::Ready {
            index_name: lease.spec().index_name().to_owned(),
            created: true,
        }
    );
    fixture.manager.succeed(&lease).await.unwrap();
    assert_eq!(
        scalar_i64(
            &fixture.db,
            &format!(
                "SELECT COUNT(*) AS value FROM sqlite_master WHERE type = 'index' AND name = '{}'",
                lease.spec().index_name()
            )
        )
        .await,
        1
    );

    let repeated = fixture.request("price_minor", SecondaryIndexOperation::Ensure, "worker-b");
    let repeated_lease = match fixture.manager.claim(&repeated).await.unwrap() {
        SecondaryIndexClaimOutcome::Acquired(lease) => lease,
        outcome => panic!("repeated ensure should acquire, got {outcome:?}"),
    };
    assert_eq!(
        fixture.manager.execute(&repeated_lease).await.unwrap(),
        SecondaryIndexExecutionOutcome::Ready {
            index_name: repeated_lease.spec().index_name().to_owned(),
            created: false,
        }
    );
    fixture.manager.succeed(&repeated_lease).await.unwrap();

    let reindex = fixture.request("price_minor", SecondaryIndexOperation::Reindex, "worker-c");
    let reindex_lease = match fixture.manager.claim(&reindex).await.unwrap() {
        SecondaryIndexClaimOutcome::Acquired(lease) => lease,
        outcome => panic!("reindex should acquire, got {outcome:?}"),
    };
    assert_eq!(
        fixture.manager.execute(&reindex_lease).await.unwrap(),
        SecondaryIndexExecutionOutcome::Reindexed {
            index_name: reindex_lease.spec().index_name().to_owned(),
        }
    );
    fixture.manager.succeed(&reindex_lease).await.unwrap();

    let retire = fixture.request("price_minor", SecondaryIndexOperation::Retire, "worker-d");
    let retire_lease = match fixture.manager.claim(&retire).await.unwrap() {
        SecondaryIndexClaimOutcome::Acquired(lease) => lease,
        outcome => panic!("retire should acquire, got {outcome:?}"),
    };
    assert_eq!(
        fixture.manager.execute(&retire_lease).await.unwrap(),
        SecondaryIndexExecutionOutcome::Retired {
            index_name: retire_lease.spec().index_name().to_owned(),
            dropped: true,
        }
    );
    fixture.manager.succeed(&retire_lease).await.unwrap();
    assert_eq!(
        scalar_i64(
            &fixture.db,
            &format!(
                "SELECT COUNT(*) AS value FROM sqlite_master WHERE type = 'index' AND name = '{}'",
                retire_lease.spec().index_name()
            )
        )
        .await,
        0
    );
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_jobs WHERE kind = 'secondary_index' AND state = 'succeeded' AND completed_at IS NOT NULL"
        )
        .await,
        4
    );
}

#[tokio::test]
async fn expired_operation_is_reclaimed_with_attempt_fencing() {
    let fixture = Fixture::new("active").await;
    let first = match fixture
        .manager
        .claim(&fixture.request("status", SecondaryIndexOperation::Ensure, "worker-a"))
        .await
        .unwrap()
    {
        SecondaryIndexClaimOutcome::Acquired(lease) => lease,
        outcome => panic!("first claim should acquire, got {outcome:?}"),
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
        .manager
        .claim(&fixture.request("status", SecondaryIndexOperation::Ensure, "worker-b"))
        .await
        .unwrap()
    {
        SecondaryIndexClaimOutcome::Acquired(lease) => lease,
        outcome => panic!("expired claim should be reclaimed, got {outcome:?}"),
    };
    assert_eq!(second.job_id(), first.job_id());
    assert_eq!(second.attempt_count(), 2);
    assert_eq!(
        fixture.manager.succeed(&first).await,
        Err(SecondaryIndexError::LeaseLost)
    );
    fixture.manager.execute(&second).await.unwrap();
    fixture
        .manager
        .fail(
            &second,
            "secondary_index.operator_cancelled",
            json!({"retryable": true}),
        )
        .await
        .unwrap();
    assert_eq!(
        scalar_i64(
            &fixture.db,
            "SELECT COUNT(*) AS value FROM index_jobs WHERE kind = 'secondary_index' AND state = 'failed' AND last_error_code = 'secondary_index.operator_cancelled'"
        )
        .await,
        1
    );
}

#[tokio::test]
async fn schema_and_request_validation_fail_closed() {
    let retired = Fixture::new("retired").await;
    let ensure = retired.request("status", SecondaryIndexOperation::Ensure, "worker-a");
    assert_eq!(
        retired.manager.claim(&ensure).await,
        Err(SecondaryIndexError::SchemaRetired(
            retired.schema.reference.clone()
        ))
    );

    let retire = retired.request("status", SecondaryIndexOperation::Retire, "worker-b");
    let retire_lease = match retired.manager.claim(&retire).await.unwrap() {
        SecondaryIndexClaimOutcome::Acquired(lease) => lease,
        outcome => panic!("retirement should remain available, got {outcome:?}"),
    };
    assert_eq!(
        retired.manager.execute(&retire_lease).await.unwrap(),
        SecondaryIndexExecutionOutcome::Retired {
            index_name: retire_lease.spec().index_name().to_owned(),
            dropped: false,
        }
    );
    retired.manager.succeed(&retire_lease).await.unwrap();

    assert_eq!(
        SecondaryIndexPlan::from_schema(Uuid::nil(), &schema()),
        Err(SecondaryIndexError::NilTenantId)
    );
    let plan =
        SecondaryIndexPlan::from_schema(Uuid::parse_str(TENANT).unwrap(), &schema()).unwrap();
    assert!(matches!(
        SecondaryIndexRequest::new(
            plan.indexes()[0].clone(),
            SecondaryIndexOperation::Ensure,
            " worker ",
            Duration::from_secs(60),
        ),
        Err(SecondaryIndexError::InvalidWorkerId { .. })
    ));
    assert!(matches!(
        SecondaryIndexRequest::new(
            plan.indexes()[0].clone(),
            SecondaryIndexOperation::Ensure,
            "worker",
            Duration::ZERO,
        ),
        Err(SecondaryIndexError::InvalidLeaseDuration)
    ));
}
