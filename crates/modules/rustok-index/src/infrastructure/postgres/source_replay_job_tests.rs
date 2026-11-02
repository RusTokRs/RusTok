use std::time::Duration;

use rustok_core::MigrationSource;
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, Value as SqlValue,
};
use sea_orm_migration::SchemaManager;
use serde_json::json;
use uuid::Uuid;

use super::{
    IndexReplayJobAcquireOutcome, IndexReplayJobEnqueueOutcome, IndexReplayJobEnqueueRequest,
    IndexReplayJobError, IndexReplayJobLease, IndexReplayJobLeaseRequest,
    PostgresIndexReplayCheckpointStore, PostgresIndexReplayJobStore,
};
use crate::{
    EntityName, FieldCardinality, FieldName, IndexField, IndexModule, IndexReplayCheckpoint,
    IndexReplayCheckpointKey, IndexReplayCheckpointStore, IndexReplayFailureKind, IndexSchema,
    IndexSourceCursor, IndexValueType, LocaleMode, ModuleName, SchemaRef, SchemaVersion,
};

const TENANT: &str = "11111111-1111-1111-1111-111111111111";
const SOURCE: &str = "product-primary";

struct Fixture {
    db: DatabaseConnection,
    jobs: PostgresIndexReplayJobStore,
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
        db.execute_raw(Statement::from_sql_and_values(
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
        let jobs = PostgresIndexReplayJobStore::new(db.clone());
        Self { db, jobs, schema }
    }

    fn request(&self, worker: &str) -> IndexReplayJobLeaseRequest {
        IndexReplayJobLeaseRequest::new(
            Uuid::parse_str(TENANT).unwrap(),
            self.schema.reference.clone(),
            SOURCE,
            worker,
            Duration::from_secs(60),
        )
        .unwrap()
    }

    async fn acquire(&self, worker: &str) -> IndexReplayJobLease {
        match self.jobs.acquire(&self.request(worker)).await.unwrap() {
            IndexReplayJobAcquireOutcome::Acquired(lease) => lease,
            outcome => panic!("replay job should be acquired, got {outcome:?}"),
        }
    }

    fn checkpoint(
        &self,
        cursor: Option<IndexSourceCursor>,
        source_version: u64,
    ) -> IndexReplayCheckpoint {
        IndexReplayCheckpoint::new(
            IndexReplayCheckpointKey::new(
                Uuid::parse_str(TENANT).unwrap(),
                SOURCE,
                self.schema.reference.clone(),
            )
            .unwrap(),
            cursor,
            Some(source_version),
            Some(Uuid::from_u128(u128::from(source_version)).to_string()),
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

async fn expire(db: &DatabaseConnection, lease: &IndexReplayJobLease) {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "UPDATE index_jobs SET lease_expires_at = datetime('now', '-1 second') WHERE tenant_id = ?1 AND job_id = ?2",
        vec![TENANT.to_owned().into(), lease.job_id().to_string().into()],
    ))
    .await
    .unwrap();
}

#[tokio::test]
async fn replay_job_excludes_other_workers_and_requires_complete_checkpoint() {
    let fixture = Fixture::new("active").await;
    let lease = fixture.acquire("worker-a").await;
    assert_eq!(lease.attempt_count(), 1);
    assert_eq!(
        fixture
            .jobs
            .acquire(&fixture.request("worker-b"))
            .await
            .unwrap(),
        IndexReplayJobAcquireOutcome::Busy
    );
    assert_eq!(
        fixture.jobs.succeed(&lease).await,
        Err(IndexReplayJobError::CheckpointMissing)
    );

    let checkpoints = PostgresIndexReplayCheckpointStore::new(fixture.db.clone(), lease.clone());
    checkpoints
        .commit_replay_checkpoint(&fixture.checkpoint(
            Some(IndexSourceCursor::new(json!({"after": 1})).unwrap()),
            1,
        ))
        .await
        .unwrap();
    assert_eq!(
        fixture.jobs.succeed(&lease).await,
        Err(IndexReplayJobError::CheckpointIncomplete)
    );

    checkpoints
        .commit_replay_checkpoint(&fixture.checkpoint(None, 2))
        .await
        .unwrap();
    fixture
        .jobs
        .heartbeat(&lease, Duration::from_secs(120))
        .await
        .unwrap();
    fixture.jobs.succeed(&lease).await.unwrap();
    assert_eq!(
        fixture
            .jobs
            .acquire(&fixture.request("worker-b"))
            .await
            .unwrap(),
        IndexReplayJobAcquireOutcome::AlreadyComplete {
            job_id: lease.job_id(),
        }
    );
    assert_eq!(
        fixture
            .jobs
            .heartbeat(&lease, Duration::from_secs(60))
            .await,
        Err(IndexReplayJobError::LeaseLost)
    );
}

#[tokio::test]
async fn expired_replay_job_is_reclaimed_and_old_checkpoint_writer_is_fenced() {
    let fixture = Fixture::new("active").await;
    let first = fixture.acquire("worker-a").await;
    let stale_checkpoints =
        PostgresIndexReplayCheckpointStore::new(fixture.db.clone(), first.clone());
    expire(&fixture.db, &first).await;

    let second = fixture.acquire("worker-b").await;
    assert_eq!(second.job_id(), first.job_id());
    assert_eq!(second.attempt_count(), 2);

    let failure = stale_checkpoints
        .commit_replay_checkpoint(&fixture.checkpoint(None, 1))
        .await
        .expect_err("stale checkpoint writer must be fenced");
    assert_eq!(failure.kind(), IndexReplayFailureKind::Permanent);
    assert_eq!(failure.code(), "checkpoint_lease_lost");
    assert_eq!(
        fixture.jobs.succeed(&first).await,
        Err(IndexReplayJobError::LeaseLost)
    );

    let active_checkpoints =
        PostgresIndexReplayCheckpointStore::new(fixture.db.clone(), second.clone());
    active_checkpoints
        .commit_replay_checkpoint(&fixture.checkpoint(None, 2))
        .await
        .unwrap();
    fixture.jobs.succeed(&second).await.unwrap();
}

#[tokio::test]
async fn failed_terminal_replay_job_blocks_scope_without_raw_details() {
    let fixture = Fixture::new("active").await;
    let first = fixture.acquire("worker-a").await;
    fixture
        .jobs
        .fail(
            &first,
            "index.replay_source_failed",
            json!({"retryable": false, "private": "must-not-be-returned"}),
        )
        .await
        .unwrap();

    assert_eq!(
        fixture.jobs.acquire(&fixture.request("worker-b")).await,
        Err(IndexReplayJobError::DeadLettered {
            job_id: first.job_id(),
            attempt_count: 1,
            error_code: Some("index.replay_source_failed".to_owned()),
        })
    );

    let count: i64 = fixture
        .db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS job_count FROM index_jobs WHERE tenant_id = ?1 AND kind = 'rebuild'",
            vec![TENANT.to_owned().into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "job_count")
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn replay_job_schema_source_and_stored_request_fail_closed() {
    let retired = Fixture::new("retired").await;
    assert_eq!(
        retired.jobs.acquire(&retired.request("worker-a")).await,
        Err(IndexReplayJobError::SchemaRetired(
            retired.schema.reference.clone()
        ))
    );

    let active = Fixture::new("active").await;
    assert!(matches!(
        IndexReplayJobLeaseRequest::new(
            Uuid::nil(),
            active.schema.reference.clone(),
            SOURCE,
            "worker-a",
            Duration::from_secs(60),
        ),
        Err(IndexReplayJobError::NilTenantId)
    ));
    assert!(matches!(
        IndexReplayJobLeaseRequest::new(
            Uuid::parse_str(TENANT).unwrap(),
            active.schema.reference.clone(),
            "Invalid Source",
            "worker-a",
            Duration::from_secs(60),
        ),
        Err(IndexReplayJobError::InvalidSourceName { .. })
    ));
    assert!(matches!(
        IndexReplayJobLeaseRequest::new(
            Uuid::parse_str(TENANT).unwrap(),
            active.schema.reference.clone(),
            SOURCE,
            " worker-a ",
            Duration::from_secs(60),
        ),
        Err(IndexReplayJobError::InvalidWorkerId { .. })
    ));
    assert!(matches!(
        IndexReplayJobLeaseRequest::new(
            Uuid::parse_str(TENANT).unwrap(),
            active.schema.reference.clone(),
            SOURCE,
            "worker-a",
            Duration::ZERO,
        ),
        Err(IndexReplayJobError::InvalidLeaseDuration)
    ));

    let lease = active.acquire("worker-a").await;
    expire(&active.db, &lease).await;
    active
        .db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE index_jobs SET request = ?1 WHERE tenant_id = ?2 AND job_id = ?3",
            vec![
                SqlValue::Json(Some(Box::new(json!({
                    "contract": "index_replay_job_v1",
                    "source_name": "another-source"
                })))),
                TENANT.to_owned().into(),
                lease.job_id().to_string().into(),
            ],
        ))
        .await
        .unwrap();
    assert!(matches!(
        active.jobs.acquire(&active.request("worker-b")).await,
        Err(IndexReplayJobError::InvalidStoredJob(_))
    ));
}

fn enqueue_request(fixture: &Fixture, job_id: Uuid) -> IndexReplayJobEnqueueRequest {
    IndexReplayJobEnqueueRequest::new(
        Uuid::parse_str(TENANT).unwrap(),
        job_id,
        fixture.schema.reference.clone(),
        SOURCE,
    )
    .unwrap()
}

#[test]
fn original_replay_enqueue_and_exact_run_fence_reject_nil_identities() {
    let tenant = Uuid::parse_str(TENANT).unwrap();
    assert!(
        IndexReplayJobEnqueueRequest::new(tenant, Uuid::nil(), schema().reference, SOURCE).is_err()
    );
    assert!(
        IndexReplayJobEnqueueRequest::new(
            Uuid::nil(),
            Uuid::from_u128(900),
            schema().reference,
            SOURCE
        )
        .is_err()
    );
    assert!(
        IndexReplayJobLeaseRequest::new(
            tenant,
            schema().reference,
            SOURCE,
            "worker",
            Duration::from_secs(60)
        )
        .unwrap()
        .for_job(Uuid::nil())
        .is_err()
    );
}

#[tokio::test]
async fn enqueue_is_durable_and_duplicate_does_not_acquire_or_increment_a_lease() {
    let fixture = Fixture::new("active").await;
    let original = Uuid::from_u128(900);
    let request = enqueue_request(&fixture, original);
    assert_eq!(
        fixture.jobs.enqueue(&request).await.unwrap(),
        IndexReplayJobEnqueueOutcome::Enqueued { job_id: original }
    );
    assert_eq!(
        fixture.jobs.enqueue(&request).await.unwrap(),
        IndexReplayJobEnqueueOutcome::AlreadyRequested { job_id: original }
    );
    let row = fixture.db.query_one_raw(Statement::from_string(DbBackend::Sqlite, "SELECT COUNT(*) AS n, MIN(state) AS state, MIN(attempt_count) AS attempts, MIN(lease_owner) AS owner FROM index_jobs")).await.unwrap().unwrap();
    assert_eq!(row.try_get::<i64>("", "n").unwrap(), 1);
    assert_eq!(row.try_get::<String>("", "state").unwrap(), "pending");
    assert_eq!(row.try_get::<i64>("", "attempts").unwrap(), 0);
    assert_eq!(row.try_get::<Option<String>>("", "owner").unwrap(), None);
}

#[tokio::test]
async fn duplicate_activation_retains_original_checkpoint_and_expired_job_attempt_fence() {
    let fixture = Fixture::new("active").await;
    let original = Uuid::from_u128(900);
    let request = enqueue_request(&fixture, original);
    fixture.jobs.enqueue(&request).await.unwrap();
    let lease = match fixture
        .jobs
        .acquire(&fixture.request("first").for_job(original).unwrap())
        .await
        .unwrap()
    {
        IndexReplayJobAcquireOutcome::Acquired(lease) => lease,
        other => panic!("expected original lease, got {other:?}"),
    };
    let checkpoint = fixture.checkpoint(
        Some(IndexSourceCursor::new(json!({"after": 7})).unwrap()),
        7,
    );
    PostgresIndexReplayCheckpointStore::new(fixture.db.clone(), lease.clone())
        .commit_replay_checkpoint(&checkpoint)
        .await
        .unwrap();
    expire(&fixture.db, &lease).await;
    assert_eq!(
        fixture.jobs.enqueue(&request).await.unwrap(),
        IndexReplayJobEnqueueOutcome::AlreadyRequested { job_id: original }
    );
    let resumed = match fixture
        .jobs
        .acquire(&fixture.request("resumed").for_job(original).unwrap())
        .await
        .unwrap()
    {
        IndexReplayJobAcquireOutcome::Acquired(lease) => lease,
        other => panic!("expected original resume, got {other:?}"),
    };
    assert_eq!(resumed.job_id(), original);
    assert_eq!(resumed.attempt_count(), 2);
    assert_eq!(
        PostgresIndexReplayCheckpointStore::new(fixture.db.clone(), resumed)
            .load_replay_checkpoint(checkpoint.key())
            .await
            .unwrap(),
        Some(checkpoint)
    );
}

#[tokio::test]
async fn exact_job_fence_never_creates_missing_or_replaces_differently_enqueued_original() {
    let fixture = Fixture::new("active").await;
    let original = Uuid::from_u128(900);
    assert!(
        matches!(fixture.jobs.acquire(&fixture.request("worker").for_job(original).unwrap()).await, Err(IndexReplayJobError::JobNotFound(job_id)) if job_id == original)
    );
    fixture
        .jobs
        .enqueue(&enqueue_request(&fixture, original))
        .await
        .unwrap();
    assert!(matches!(
        fixture
            .jobs
            .acquire(
                &fixture
                    .request("worker")
                    .for_job(Uuid::from_u128(901))
                    .unwrap()
            )
            .await,
        Err(IndexReplayJobError::JobIdentityConflict { .. })
    ));
    assert!(matches!(
        fixture
            .jobs
            .enqueue(&enqueue_request(&fixture, Uuid::from_u128(901)))
            .await,
        Err(IndexReplayJobError::JobIdentityConflict { .. })
    ));
}

#[tokio::test]
async fn cancelled_or_cancellation_requested_original_cannot_be_replaced_by_activation() {
    let fixture = Fixture::new("active").await;
    let original = Uuid::from_u128(900);
    let request = enqueue_request(&fixture, original);
    fixture.jobs.enqueue(&request).await.unwrap();
    fixture
        .db
        .execute_unprepared("UPDATE index_jobs SET cancel_requested=TRUE")
        .await
        .unwrap();
    assert!(fixture.jobs.enqueue(&request).await.is_err());
    fixture
        .db
        .execute_unprepared(
            "UPDATE index_jobs SET state='cancelled', completed_at=CURRENT_TIMESTAMP",
        )
        .await
        .unwrap();
    assert!(fixture.jobs.enqueue(&request).await.is_err());
    assert!(
        fixture
            .jobs
            .enqueue(&enqueue_request(&fixture, Uuid::from_u128(901)))
            .await
            .is_err()
    );
    let row = fixture
        .db
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS n FROM index_jobs",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i64>("", "n").unwrap(), 1);
}

#[tokio::test]
async fn succeeded_original_enqueue_requires_retained_exact_complete_checkpoint() {
    let fixture = Fixture::new("active").await;
    let original = Uuid::from_u128(900);
    let request = enqueue_request(&fixture, original);
    fixture.jobs.enqueue(&request).await.unwrap();
    let lease = match fixture
        .jobs
        .acquire(&fixture.request("worker").for_job(original).unwrap())
        .await
        .unwrap()
    {
        IndexReplayJobAcquireOutcome::Acquired(lease) => lease,
        other => panic!("expected original lease, got {other:?}"),
    };
    PostgresIndexReplayCheckpointStore::new(fixture.db.clone(), lease.clone())
        .commit_replay_checkpoint(&fixture.checkpoint(None, 1))
        .await
        .unwrap();
    fixture.jobs.succeed(&lease).await.unwrap();
    assert_eq!(
        fixture.jobs.enqueue(&request).await.unwrap(),
        IndexReplayJobEnqueueOutcome::AlreadyComplete { job_id: original }
    );
    fixture
        .db
        .execute_unprepared("DELETE FROM index_checkpoints")
        .await
        .unwrap();
    assert_eq!(
        fixture.jobs.enqueue(&request).await,
        Err(IndexReplayJobError::CheckpointMissing)
    );
}

#[tokio::test]
async fn pending_enqueue_storage_failure_never_returns_success_or_leaves_partial_job() {
    let fixture = Fixture::new("active").await;
    fixture.db.execute_unprepared("CREATE TRIGGER refuse_pending BEFORE INSERT ON index_jobs BEGIN SELECT RAISE(ABORT, 'fixture_storage_refused'); END").await.unwrap();
    assert!(
        fixture
            .jobs
            .enqueue(&enqueue_request(&fixture, Uuid::from_u128(900)))
            .await
            .is_err()
    );
    let row = fixture
        .db
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS n FROM index_jobs",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i64>("", "n").unwrap(), 0);
}

#[tokio::test]
async fn orphan_checkpoint_refuses_initial_enqueue_without_creating_job_or_resetting_cursor() {
    for cursor in [serde_json::Value::Null, json!({"after": 7})] {
        let fixture = Fixture::new("active").await;
        fixture.db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO index_checkpoints (tenant_id, checkpoint_kind, source_name, module_name, entity_name, schema_version, locale_key, partition_key, cursor) VALUES (?1, 'rebuild', ?2, ?3, ?4, ?5, '', '', ?6)",
            vec![
                TENANT.to_owned().into(), SOURCE.to_owned().into(),
                fixture.schema.reference.module.as_str().to_owned().into(),
                fixture.schema.reference.entity.as_str().to_owned().into(),
                i64::from(fixture.schema.reference.version.get()).into(),
                SqlValue::Json(Some(Box::new(cursor.clone()))),
            ],
        )).await.unwrap();
        let request = enqueue_request(&fixture, Uuid::from_u128(900));
        assert_eq!(
            fixture.jobs.enqueue(&request).await,
            Err(IndexReplayJobError::CheckpointWithoutJob)
        );
        assert_eq!(
            fixture.jobs.enqueue(&request).await,
            Err(IndexReplayJobError::CheckpointWithoutJob)
        );
        let jobs = fixture
            .db
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS n FROM index_jobs",
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(jobs.try_get::<i64>("", "n").unwrap(), 0);
        let saved = fixture
            .db
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT cursor FROM index_checkpoints",
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            saved.try_get::<serde_json::Value>("", "cursor").unwrap(),
            cursor
        );
    }
}

#[tokio::test]
async fn succeeded_job_acquisition_requires_complete_checkpoint_for_scoped_and_exact_requests() {
    for missing in [true, false] {
        let fixture = Fixture::new("active").await;
        let original = Uuid::from_u128(900);
        fixture
            .jobs
            .enqueue(&enqueue_request(&fixture, original))
            .await
            .unwrap();
        let lease = fixture.acquire("first").await;
        assert_eq!(lease.job_id(), original);
        PostgresIndexReplayCheckpointStore::new(fixture.db.clone(), lease.clone())
            .commit_replay_checkpoint(&fixture.checkpoint(None, 1))
            .await
            .unwrap();
        fixture.jobs.succeed(&lease).await.unwrap();
        let expected = if missing {
            fixture
                .db
                .execute_unprepared("DELETE FROM index_checkpoints")
                .await
                .unwrap();
            IndexReplayJobError::CheckpointMissing
        } else {
            fixture
                .db
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    "UPDATE index_checkpoints SET cursor = ?1",
                    vec![SqlValue::Json(Some(Box::new(json!({"after": 7}))))],
                ))
                .await
                .unwrap();
            IndexReplayJobError::CheckpointIncomplete
        };
        for request in [
            fixture.request("second"),
            fixture.request("second").for_job(original).unwrap(),
        ] {
            assert_eq!(fixture.jobs.acquire(&request).await, Err(expected.clone()));
        }
        let saved = fixture.db.query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS n, MIN(state) AS state, MIN(attempt_count) AS attempts FROM index_jobs",
        )).await.unwrap().unwrap();
        assert_eq!(saved.try_get::<i64>("", "n").unwrap(), 1);
        assert_eq!(saved.try_get::<String>("", "state").unwrap(), "succeeded");
        assert_eq!(saved.try_get::<i64>("", "attempts").unwrap(), 1);
    }
}
