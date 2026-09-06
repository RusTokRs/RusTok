#![cfg(feature = "migrations")]

use std::error::Error;
use std::time::Duration;

use rustok_iggy_connector::migrations::{
    ConsumerPoisonIdentity, ConsumerPoisonPublishClaim, ConsumerPoisonReceiptError,
    ConsumerPoisonReceiptInspector, ConsumerPoisonReceiptState, ConsumerPoisonReceiptStore,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const TEST_DATABASE_ENV: &str = "RUSTOK_IGGY_CONNECTOR_TEST_DATABASE_URL";
const CONSUMER_GROUP: &str = "rustok-social-graph-index";

struct PostgresPoisonReceiptTestDb {
    control: DatabaseConnection,
    db: DatabaseConnection,
    database_url: String,
    schema_name: String,
}

impl PostgresPoisonReceiptTestDb {
    async fn setup(prefix: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping consumer poison receipt PostgreSQL evidence"
            );
            return Ok(None);
        };

        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_iggy_poison_{}_{}",
            sanitize_identifier(prefix),
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = connect_in_schema(&database_url, &schema_name).await?;
        let setup_result = async {
            let manager = SchemaManager::new(&db);
            for migration in rustok_iggy_connector::migrations::migrations() {
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
            database_url,
            schema_name,
        }))
    }

    async fn connect_worker(&self) -> TestResult<DatabaseConnection> {
        connect_in_schema(&self.database_url, &self.schema_name).await
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

#[tokio::test]
async fn concurrent_publishers_have_one_claim_owner() -> TestResult<()> {
    let Some(test_db) = PostgresPoisonReceiptTestDb::setup("claim_owner").await? else {
        return Ok(());
    };

    let identity = identity(Uuid::new_v4(), 1, 42, vec![1, 2, 3])?;
    let first_publisher = Uuid::new_v4();
    let second_publisher = Uuid::new_v4();
    let first_store = ConsumerPoisonReceiptStore::new(test_db.connect_worker().await?);
    let second_store = ConsumerPoisonReceiptStore::new(test_db.connect_worker().await?);

    let (first, second) = tokio::join!(
        first_store.reserve_and_claim(
            &identity,
            "iggy.contract.decode_invalid",
            1,
            first_publisher,
            Duration::from_secs(30),
        ),
        second_store.reserve_and_claim(
            &identity,
            "iggy.contract.schema_invalid",
            2,
            second_publisher,
            Duration::from_secs(30),
        )
    );
    let first = first?;
    let second = second?;
    assert!(
        matches!(
            (first, second),
            (
                ConsumerPoisonPublishClaim::Claimed,
                ConsumerPoisonPublishClaim::Busy
            ) | (
                ConsumerPoisonPublishClaim::Busy,
                ConsumerPoisonPublishClaim::Claimed
            )
        ),
        "exactly one concurrent publisher must own the publication claim: {first:?}, {second:?}"
    );

    let retained = first_store
        .find(&identity)
        .await?
        .expect("concurrent reservation must retain one receipt");
    assert_eq!(retained.state, ConsumerPoisonReceiptState::Publishing);
    assert!(
        matches!(
            (
                retained.stable_error_code.as_str(),
                retained.first_delivery_attempt_count,
            ),
            ("iggy.contract.decode_invalid", 1) | ("iggy.contract.schema_invalid", 2)
        ),
        "the winning reservation must retain one atomic first-observed diagnostic pair"
    );

    test_db.cleanup().await
}

#[tokio::test]
async fn expired_lease_is_reclaimed_and_fences_the_previous_publisher() -> TestResult<()> {
    let Some(test_db) = PostgresPoisonReceiptTestDb::setup("lease_reclaim").await? else {
        return Ok(());
    };

    let identity = identity(Uuid::new_v4(), 1, 43, Vec::new())?;
    let first_publisher = Uuid::new_v4();
    let second_publisher = Uuid::new_v4();
    let store = ConsumerPoisonReceiptStore::new(test_db.db.clone());

    assert_eq!(
        store
            .reserve_and_claim(
                &identity,
                "iggy.contract.decode_invalid",
                1,
                first_publisher,
                Duration::from_secs(30),
            )
            .await?,
        ConsumerPoisonPublishClaim::Claimed
    );
    expire_publishing_lease(&test_db.db, identity.delivery_id()).await?;
    assert_eq!(
        store
            .reserve_and_claim(
                &identity,
                "iggy.contract.schema_invalid",
                9,
                second_publisher,
                Duration::from_secs(30),
            )
            .await?,
        ConsumerPoisonPublishClaim::Claimed
    );
    assert!(matches!(
        store.mark_published(&identity, first_publisher).await,
        Err(ConsumerPoisonReceiptError::ClaimLost)
    ));

    store.mark_published(&identity, second_publisher).await?;
    let retained = store
        .find(&identity)
        .await?
        .expect("reclaimed receipt must remain durable");
    assert_eq!(retained.state, ConsumerPoisonReceiptState::Published);
    assert_eq!(retained.stable_error_code, "iggy.contract.decode_invalid");
    assert_eq!(retained.first_delivery_attempt_count, 1);

    test_db.cleanup().await
}

#[tokio::test]
async fn conflicts_roll_back_without_overwriting_original_identity() -> TestResult<()> {
    let Some(test_db) = PostgresPoisonReceiptTestDb::setup("identity_conflict").await? else {
        return Ok(());
    };

    let delivery_id = Uuid::new_v4();
    let original = identity(delivery_id, 1, 44, vec![7, 8, 9])?;
    let conflicting_source = identity(delivery_id, 2, 45, vec![7, 8, 9])?;
    let conflicting_bytes = identity(Uuid::new_v4(), 1, 44, vec![9, 8, 7])?;
    let publisher = Uuid::new_v4();
    let store = ConsumerPoisonReceiptStore::new(test_db.db.clone());

    assert_eq!(
        store
            .reserve_and_claim(
                &original,
                "iggy.contract.decode_invalid",
                1,
                publisher,
                Duration::from_secs(30),
            )
            .await?,
        ConsumerPoisonPublishClaim::Claimed
    );
    assert!(matches!(
        store
            .reserve_and_claim(
                &conflicting_source,
                "iggy.contract.schema_invalid",
                2,
                Uuid::new_v4(),
                Duration::from_secs(30),
            )
            .await,
        Err(ConsumerPoisonReceiptError::IdentityConflict)
    ));
    assert!(matches!(
        store.find(&conflicting_bytes).await,
        Err(ConsumerPoisonReceiptError::IdentityConflict)
    ));

    let retained = store
        .find(&original)
        .await?
        .expect("identity conflict must not remove or rewrite the original receipt");
    assert_eq!(retained.state, ConsumerPoisonReceiptState::Publishing);
    assert_eq!(retained.stable_error_code, "iggy.contract.decode_invalid");
    assert_eq!(retained.first_delivery_attempt_count, 1);
    assert_eq!(count_receipts(&test_db.db).await?, 1);

    test_db.cleanup().await
}

#[tokio::test]
async fn terminal_states_and_aggregate_inspection_remain_consistent() -> TestResult<()> {
    let Some(test_db) = PostgresPoisonReceiptTestDb::setup("terminal_summary").await? else {
        return Ok(());
    };

    let published_identity = identity(Uuid::new_v4(), 1, 46, vec![1])?;
    let acknowledged_identity = identity(Uuid::new_v4(), 1, 47, vec![2])?;
    let reserved_identity = identity(Uuid::new_v4(), 1, 48, vec![3])?;
    let store = ConsumerPoisonReceiptStore::new(test_db.db.clone());
    let inspector = ConsumerPoisonReceiptInspector::new(test_db.db.clone());

    let published_owner = Uuid::new_v4();
    assert_eq!(
        store
            .reserve_and_claim(
                &published_identity,
                "iggy.contract.decode_invalid",
                1,
                published_owner,
                Duration::from_secs(30),
            )
            .await?,
        ConsumerPoisonPublishClaim::Claimed
    );
    store
        .mark_published(&published_identity, published_owner)
        .await?;

    let acknowledged_owner = Uuid::new_v4();
    assert_eq!(
        store
            .reserve_and_claim(
                &acknowledged_identity,
                "iggy.contract.decode_invalid",
                1,
                acknowledged_owner,
                Duration::from_secs(30),
            )
            .await?,
        ConsumerPoisonPublishClaim::Claimed
    );
    store
        .mark_published(&acknowledged_identity, acknowledged_owner)
        .await?;
    store.mark_acknowledged(&acknowledged_identity).await?;

    let reserved_owner = Uuid::new_v4();
    assert_eq!(
        store
            .reserve_and_claim(
                &reserved_identity,
                "iggy.contract.decode_invalid",
                1,
                reserved_owner,
                Duration::from_secs(30),
            )
            .await?,
        ConsumerPoisonPublishClaim::Claimed
    );
    store
        .release_claim(&reserved_identity, reserved_owner)
        .await?;

    let summary = inspector.summarize(CONSUMER_GROUP).await?;
    assert_eq!(summary.total(), 3);
    assert_eq!(summary.reserved(), 1);
    assert_eq!(summary.publishing(), 0);
    assert_eq!(summary.expired_publishing(), 0);
    assert_eq!(summary.published(), 1);
    assert_eq!(summary.acknowledged(), 1);
    assert!(summary.has_recovery_work());
    assert!(!summary.has_expired_claims());

    assert_eq!(
        store
            .reserve_and_claim(
                &published_identity,
                "iggy.contract.schema_invalid",
                8,
                Uuid::new_v4(),
                Duration::from_secs(30),
            )
            .await?,
        ConsumerPoisonPublishClaim::AlreadyPublished
    );
    assert_eq!(
        store
            .reserve_and_claim(
                &acknowledged_identity,
                "iggy.contract.schema_invalid",
                8,
                Uuid::new_v4(),
                Duration::from_secs(30),
            )
            .await?,
        ConsumerPoisonPublishClaim::AlreadyAcknowledged
    );

    test_db.cleanup().await
}

fn identity(
    delivery_id: Uuid,
    source_partition: u32,
    source_offset: u64,
    payload: Vec<u8>,
) -> Result<ConsumerPoisonIdentity, ConsumerPoisonReceiptError> {
    ConsumerPoisonIdentity::new(
        delivery_id,
        CONSUMER_GROUP,
        "rustok",
        "domain",
        source_partition,
        source_offset,
        payload,
    )
}

async fn expire_publishing_lease(
    db: &DatabaseConnection,
    delivery_id: Uuid,
) -> Result<(), sea_orm::DbErr> {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE iggy_consumer_poison_receipts SET lease_expires_at = CURRENT_TIMESTAMP - INTERVAL '1 second' WHERE delivery_id = $1 AND state = 'publishing'",
        vec![delivery_id.into()],
    ))
    .await?;
    Ok(())
}

async fn count_receipts(db: &DatabaseConnection) -> Result<i64, sea_orm::DbErr> {
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS count FROM iggy_consumer_poison_receipts".to_string(),
        ))
        .await?
        .expect("count query must return one row");
    row.try_get("", "count")
}

fn postgres_database_url() -> Option<String> {
    std::env::var(TEST_DATABASE_ENV)
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

async fn connect_in_schema(
    database_url: &str,
    schema_name: &str,
) -> TestResult<DatabaseConnection> {
    let db = connect(database_url).await?;
    set_search_path(&db, schema_name).await?;
    Ok(db)
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
