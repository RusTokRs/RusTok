use std::error::Error;

use chrono::Utc;
use rustok_core::MigrationSource;
use rustok_events::{
    ContractEventEnvelope, DomainEvent, EventEnvelope, ForumSearchProjectionEvent,
};
use rustok_search::{
    ForumSearchContractIngress, ForumSearchContractIngressError,
    ForumSearchContractIngressOutcome, SearchModule,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
    Value as SqlValue,
};
use sea_orm_migration::SchemaManager;
use serde_json::Value as JsonValue;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const SEARCH_TEST_DATABASE_ENV: &str = "RUSTOK_SEARCH_TEST_DATABASE_URL";
const ROOT_EVENT_TYPE: &str = "index.reindex_requested";
const FORUM_SOURCE_MODULE: &str = "forum";

struct PostgresSearchTestDb {
    control: DatabaseConnection,
    db: DatabaseConnection,
    schema_name: String,
}

impl PostgresSearchTestDb {
    async fn setup(prefix: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{SEARCH_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Forum versioned invalidation ingress evidence"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_search_{}_{}",
            sanitize_identifier(prefix),
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = connect(&database_url).await?;
        set_search_path(&db, &schema_name).await?;

        let setup_result = async {
            let manager = SchemaManager::new(&db);
            for migration in SearchModule.migrations() {
                migration.up(&manager).await?;
            }
            Ok::<(), sea_orm::DbErr>(())
        }
        .await;

        if let Err(error) = setup_result {
            let _ = control
                .execute_unprepared(&format!(r#"DROP SCHEMA IF EXISTS "{schema_name}" CASCADE"#))
                .await;
            return Err(error.into());
        }

        Ok(Some(Self {
            control,
            db,
            schema_name,
        }))
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct InboxSnapshot {
    tenant_id: Uuid,
    source_module: String,
    scope_key: String,
    event_type: String,
    ingest_sequence: i64,
    envelope_json: JsonValue,
}

#[tokio::test]
async fn legacy_first_then_typed_restart_reuses_one_exact_root_row() -> TestResult<()> {
    let Some(test_db) = PostgresSearchTestDb::setup("forum_contract_legacy_first").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let root_event_id = Uuid::new_v4();
    let category_id = Uuid::new_v4();
    let owner_revision = 9_000_001_i64;
    let target_type = "forum_category";
    let target_id = Some(category_id);
    let scope_key = format!("forum_category:{category_id}");
    let root = legacy_root_envelope(tenant_id, root_event_id, target_type, target_id)?;
    insert_legacy_root(&test_db.db, &root, &scope_key).await?;
    let before = load_snapshot(&test_db.db, root_event_id).await?;

    let typed = typed_invalidation(
        tenant_id,
        root_event_id,
        owner_revision,
        target_type,
        target_id,
    )?;
    let first = ForumSearchContractIngress::new(test_db.db.clone())
        .ingest(&typed)
        .await?;
    assert_eq!(
        first,
        ForumSearchContractIngressOutcome::DurablyAccepted {
            root_event_id,
            owner_revision,
        }
    );

    let after_first = load_snapshot(&test_db.db, root_event_id).await?;
    assert_eq!(after_first, before);
    assert_eq!(count_root_rows(&test_db.db, root_event_id).await?, 1);

    let restarted = ForumSearchContractIngress::new(test_db.db.clone());
    let redelivery = restarted.ingest(&typed).await?;
    assert_eq!(redelivery, first);
    assert_eq!(load_snapshot(&test_db.db, root_event_id).await?, before);
    assert_eq!(count_root_rows(&test_db.db, root_event_id).await?, 1);

    test_db.cleanup().await
}

#[tokio::test]
async fn typed_first_then_legacy_delivery_keeps_search_owned_sequence() -> TestResult<()> {
    let Some(test_db) = PostgresSearchTestDb::setup("forum_contract_typed_first").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let root_event_id = Uuid::new_v4();
    let owner_revision = 8_000_003_i64;
    let target_type = "forum_topic";
    let target_id = Some(Uuid::new_v4());
    let typed = typed_invalidation(
        tenant_id,
        root_event_id,
        owner_revision,
        target_type,
        target_id,
    )?;

    let accepted = ForumSearchContractIngress::new(test_db.db.clone())
        .ingest(&typed)
        .await?;
    assert!(matches!(
        accepted,
        ForumSearchContractIngressOutcome::DurablyAccepted {
            root_event_id: accepted_id,
            owner_revision: accepted_revision,
        } if accepted_id == root_event_id && accepted_revision == owner_revision
    ));

    let typed_first = load_snapshot(&test_db.db, root_event_id).await?;
    assert_eq!(typed_first.source_module, FORUM_SOURCE_MODULE);
    assert_eq!(typed_first.scope_key, "forum");
    assert_eq!(typed_first.event_type, ROOT_EVENT_TYPE);
    assert!(typed_first.ingest_sequence > 0);
    assert_ne!(typed_first.ingest_sequence, owner_revision);

    let root = legacy_root_envelope(tenant_id, root_event_id, target_type, target_id)?;
    insert_legacy_root(&test_db.db, &root, "forum").await?;
    assert_eq!(load_snapshot(&test_db.db, root_event_id).await?, typed_first);
    assert_eq!(count_root_rows(&test_db.db, root_event_id).await?, 1);

    let restarted = ForumSearchContractIngress::new(test_db.db.clone());
    restarted.ingest(&typed).await?;
    assert_eq!(load_snapshot(&test_db.db, root_event_id).await?, typed_first);

    test_db.cleanup().await
}

#[tokio::test]
async fn conflicting_legacy_identity_is_non_retryable_semantic_poison() -> TestResult<()> {
    let Some(test_db) = PostgresSearchTestDb::setup("forum_contract_conflict").await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let root_event_id = Uuid::new_v4();
    let requested_category_id = Uuid::new_v4();
    let conflicting_category_id = Uuid::new_v4();
    let conflicting_root = legacy_root_envelope(
        tenant_id,
        root_event_id,
        "forum_category",
        Some(conflicting_category_id),
    )?;
    insert_legacy_root(
        &test_db.db,
        &conflicting_root,
        &format!("forum_category:{conflicting_category_id}"),
    )
    .await?;
    let before = load_snapshot(&test_db.db, root_event_id).await?;

    let typed = typed_invalidation(
        tenant_id,
        root_event_id,
        42,
        "forum_category",
        Some(requested_category_id),
    )?;
    let error = ForumSearchContractIngress::new(test_db.db.clone())
        .ingest(&typed)
        .await
        .expect_err("conflicting root identity must fail closed");
    assert_eq!(error, ForumSearchContractIngressError::InboxIdentityConflict);
    assert_eq!(
        error.stable_code(),
        "forum.search_projection.contract_inbox_identity_conflict"
    );
    assert!(!error.is_retryable());
    assert_eq!(load_snapshot(&test_db.db, root_event_id).await?, before);
    assert_eq!(count_root_rows(&test_db.db, root_event_id).await?, 1);

    test_db.cleanup().await
}

fn typed_invalidation(
    tenant_id: Uuid,
    root_event_id: Uuid,
    owner_revision: i64,
    target_type: &str,
    target_id: Option<Uuid>,
) -> TestResult<ContractEventEnvelope> {
    Ok(ContractEventEnvelope::new_caused_by(
        tenant_id,
        None,
        root_event_id,
        ForumSearchProjectionEvent::InvalidationIssued {
            owner_revision,
            target_type: target_type.to_string(),
            target_id,
        },
    )?)
}

fn legacy_root_envelope(
    tenant_id: Uuid,
    root_event_id: Uuid,
    target_type: &str,
    target_id: Option<Uuid>,
) -> TestResult<EventEnvelope> {
    let envelope = EventEnvelope {
        id: root_event_id,
        event_type: ROOT_EVENT_TYPE.to_string(),
        schema_version: 1,
        correlation_id: root_event_id,
        causation_id: None,
        tenant_id,
        trace_id: None,
        timestamp: Utc::now(),
        actor_id: None,
        event: DomainEvent::ReindexRequested {
            target_type: target_type.to_string(),
            target_id,
        },
        retry_count: 0,
    };
    envelope.validate_registered_schema()?;
    Ok(envelope)
}

async fn insert_legacy_root(
    db: &DatabaseConnection,
    envelope: &EventEnvelope,
    scope_key: &str,
) -> Result<(), sea_orm::DbErr> {
    let envelope_json = serde_json::to_value(envelope)
        .map_err(|error| sea_orm::DbErr::Custom(error.to_string()))?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        INSERT INTO search_projection_inbox (
            event_id, tenant_id, source_module, scope_key, event_type,
            revision_at, envelope_json
        ) VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (event_id) DO NOTHING
        "#,
        vec![
            envelope.id.into(),
            envelope.tenant_id.into(),
            FORUM_SOURCE_MODULE.to_string().into(),
            scope_key.to_string().into(),
            ROOT_EVENT_TYPE.to_string().into(),
            envelope.timestamp.to_owned().into(),
            SqlValue::Json(Some(Box::new(envelope_json))),
        ],
    ))
    .await?;
    Ok(())
}

async fn load_snapshot(
    db: &DatabaseConnection,
    event_id: Uuid,
) -> Result<InboxSnapshot, sea_orm::DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT tenant_id, source_module, scope_key, event_type,
                   ingest_sequence, envelope_json
            FROM search_projection_inbox
            WHERE event_id = $1
            "#,
            vec![event_id.into()],
        ))
        .await?
        .ok_or_else(|| sea_orm::DbErr::Custom("expected inbox row is missing".to_string()))?;
    Ok(InboxSnapshot {
        tenant_id: row.try_get("", "tenant_id")?,
        source_module: row.try_get("", "source_module")?,
        scope_key: row.try_get("", "scope_key")?,
        event_type: row.try_get("", "event_type")?,
        ingest_sequence: row.try_get("", "ingest_sequence")?,
        envelope_json: row.try_get("", "envelope_json")?,
    })
}

async fn count_root_rows(
    db: &DatabaseConnection,
    event_id: Uuid,
) -> Result<i64, sea_orm::DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS count FROM search_projection_inbox WHERE event_id = $1",
            vec![event_id.into()],
        ))
        .await?
        .ok_or_else(|| sea_orm::DbErr::Custom("count query returned no row".to_string()))?;
    row.try_get("", "count")
}

fn postgres_database_url() -> Option<String> {
    std::env::var(SEARCH_TEST_DATABASE_ENV)
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

async fn set_search_path(db: &DatabaseConnection, schema_name: &str) -> TestResult<()> {
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}", public"#))
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
