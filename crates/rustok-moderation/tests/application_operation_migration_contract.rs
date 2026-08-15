use std::{collections::BTreeSet, env, error::Error};

use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, QueryResult,
    Statement,
};
use sea_orm_migration::{MigrationTrait, MigratorTrait};
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_MODERATION_TEST_DATABASE_URL";
const LEGACY_CREATED_AT: &str = "2026-08-07 12:34:56+00:00";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

struct TestMigrator;

#[async_trait::async_trait]
impl MigratorTrait for TestMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        rustok_moderation::migrations::migrations()
    }
}

#[derive(Clone, Copy)]
struct UpgradeFixture {
    tenant_id: Uuid,
    typed_case_id: Uuid,
    typed_decision_id: Uuid,
    typed_subject_id: Uuid,
    typed_subject_revision: i64,
    untyped_decision_id: Uuid,
}

struct PostgresHarness {
    control: DatabaseConnection,
    database_url: String,
}

impl PostgresHarness {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Moderation application-operation PostgreSQL migration contract"
            );
            return Ok(None);
        };
        Ok(Some(Self {
            control: connect_postgres(&database_url, "moderation_migration_contract_control")
                .await?,
            database_url,
        }))
    }

    async fn create_schema(&self, label: &str) -> TestResult<(String, DatabaseConnection)> {
        let schema = format!(
            "rustok_moderation_migration_{}_{}",
            label,
            Uuid::new_v4().simple()
        );
        self.control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema}""#))
            .await?;
        let db =
            connect_postgres(&self.database_url, &format!("moderation_migration_{label}")).await?;
        db.execute_unprepared(&format!(r#"SET search_path TO "{schema}""#))
            .await?;
        Ok((schema, db))
    }

    async fn drop_schema(&self, schema: &str) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(r#"DROP SCHEMA IF EXISTS "{schema}" CASCADE"#))
            .await?;
        Ok(())
    }
}

#[tokio::test]
async fn sqlite_clean_install_has_application_operation_schema() -> TestResult<()> {
    let db = sqlite_database().await?;
    TestMigrator::up(&db, None).await?;
    assert_clean_install(&db).await?;
    db.close().await?;
    Ok(())
}

#[tokio::test]
async fn sqlite_upgrade_backfills_only_typed_decisions() -> TestResult<()> {
    let db = sqlite_database().await?;
    TestMigrator::up(&db, Some(3)).await?;
    let fixture = seed_legacy_decisions(&db).await?;
    TestMigrator::up(&db, None).await?;
    assert_upgrade_contract(&db, fixture).await?;
    db.close().await?;
    Ok(())
}

#[tokio::test]
async fn postgres_clean_and_upgrade_application_operation_migration_contract() -> TestResult<()> {
    let Some(harness) = PostgresHarness::setup().await? else {
        return Ok(());
    };

    let (clean_schema, clean_db) = harness.create_schema("clean").await?;
    let clean_result = async {
        TestMigrator::up(&clean_db, None).await?;
        assert_clean_install(&clean_db).await
    }
    .await;
    clean_db.close().await?;
    harness.drop_schema(&clean_schema).await?;
    clean_result?;

    let (upgrade_schema, upgrade_db) = harness.create_schema("upgrade").await?;
    let upgrade_result = async {
        TestMigrator::up(&upgrade_db, Some(3)).await?;
        let fixture = seed_legacy_decisions(&upgrade_db).await?;
        TestMigrator::up(&upgrade_db, None).await?;
        assert_upgrade_contract(&upgrade_db, fixture).await
    }
    .await;
    upgrade_db.close().await?;
    harness.drop_schema(&upgrade_schema).await?;
    upgrade_result?;

    harness.control.close().await?;
    Ok(())
}

async fn assert_clean_install(db: &DatabaseConnection) -> TestResult<()> {
    assert_eq!(migration_count(db).await?, 4);
    assert_application_schema(db).await?;
    assert_eq!(application_operation_count(db).await?, 0);
    Ok(())
}

async fn assert_upgrade_contract(
    db: &DatabaseConnection,
    fixture: UpgradeFixture,
) -> TestResult<()> {
    assert_eq!(migration_count(db).await?, 4);
    assert_application_schema(db).await?;
    assert_eq!(application_operation_count(db).await?, 1);

    let row = db
        .query_one(Statement::from_string(
            db.get_database_backend(),
            format!(
                r#"
SELECT
    a.status AS status,
    a.attempt_count AS attempt_count,
    CAST(CASE WHEN a.tenant_id = d.tenant_id THEN 1 ELSE 0 END AS BIGINT) AS tenant_matches,
    CAST(CASE WHEN a.case_id = d.case_id THEN 1 ELSE 0 END AS BIGINT) AS case_matches,
    CAST(CASE WHEN a.decision_hash = d.decision_hash THEN 1 ELSE 0 END AS BIGINT) AS hash_matches,
    CAST(CASE WHEN a.subject_module = c.subject_module THEN 1 ELSE 0 END AS BIGINT) AS module_matches,
    CAST(CASE WHEN a.subject_kind = c.subject_kind THEN 1 ELSE 0 END AS BIGINT) AS kind_matches,
    CAST(CASE WHEN a.subject_id = c.subject_id THEN 1 ELSE 0 END AS BIGINT) AS subject_matches,
    CAST(CASE WHEN a.subject_revision = d.subject_revision THEN 1 ELSE 0 END AS BIGINT) AS revision_matches,
    CAST(CASE WHEN a.next_attempt_at = d.created_at THEN 1 ELSE 0 END AS BIGINT) AS due_matches,
    CAST(CASE WHEN a.created_at = d.created_at THEN 1 ELSE 0 END AS BIGINT) AS created_matches,
    CAST(CASE WHEN a.updated_at = d.created_at THEN 1 ELSE 0 END AS BIGINT) AS updated_matches,
    CAST(CASE WHEN a.lease_token IS NULL AND a.lease_owner IS NULL AND a.lease_expires_at IS NULL THEN 1 ELSE 0 END AS BIGINT) AS lease_empty,
    CAST(CASE WHEN a.last_error_code IS NULL AND a.last_error_message IS NULL THEN 1 ELSE 0 END AS BIGINT) AS error_empty,
    CAST(CASE WHEN a.applied_revision IS NULL AND a.applied_at IS NULL THEN 1 ELSE 0 END AS BIGINT) AS applied_empty
FROM moderation_application_operations a
JOIN moderation_decisions d ON d.id = a.decision_id
JOIN moderation_cases c ON c.id = a.case_id
WHERE a.decision_id = '{}'
"#,
                fixture.typed_decision_id
            ),
        ))
        .await?
        .ok_or_else(|| test_error("typed legacy decision was not backfilled"))?;

    assert_eq!(row.try_get::<String>("", "status")?, "pending");
    assert_eq!(row.try_get::<i32>("", "attempt_count")?, 0);
    for flag in [
        "tenant_matches",
        "case_matches",
        "hash_matches",
        "module_matches",
        "kind_matches",
        "subject_matches",
        "revision_matches",
        "due_matches",
        "created_matches",
        "updated_matches",
        "lease_empty",
        "error_empty",
        "applied_empty",
    ] {
        assert_flag(&row, flag)?;
    }

    let identity = db
        .query_one(Statement::from_string(
            db.get_database_backend(),
            format!(
                "SELECT tenant_id, case_id, subject_id, subject_revision FROM moderation_application_operations WHERE decision_id = '{}'",
                fixture.typed_decision_id
            ),
        ))
        .await?
        .ok_or_else(|| test_error("backfilled typed operation identity is missing"))?;
    assert_eq!(
        identity.try_get::<Uuid>("", "tenant_id")?,
        fixture.tenant_id
    );
    assert_eq!(
        identity.try_get::<Uuid>("", "case_id")?,
        fixture.typed_case_id
    );
    assert_eq!(
        identity.try_get::<Uuid>("", "subject_id")?,
        fixture.typed_subject_id
    );
    assert_eq!(
        identity.try_get::<i64>("", "subject_revision")?,
        fixture.typed_subject_revision
    );

    assert_eq!(
        scalar_i64(
            db,
            &format!(
                "SELECT COUNT(*) AS value FROM moderation_application_operations WHERE decision_id = '{}'",
                fixture.untyped_decision_id
            ),
        )
        .await?,
        0
    );
    Ok(())
}

async fn assert_application_schema(db: &DatabaseConnection) -> TestResult<()> {
    db.query_all(Statement::from_string(
        db.get_database_backend(),
        "SELECT decision_id, tenant_id, case_id, decision_hash, subject_module, subject_kind, subject_id, subject_revision, status, attempt_count, next_attempt_at, lease_token, lease_owner, lease_expires_at, last_error_code, last_error_message, applied_revision, applied_at, created_at, updated_at FROM moderation_application_operations WHERE 1 = 0".to_string(),
    ))
    .await?;

    let indexes = application_index_names(db).await?;
    assert!(indexes.contains("idx_moderation_application_operations_due"));
    assert!(indexes.contains("idx_moderation_application_operations_case"));
    Ok(())
}

async fn application_index_names(db: &DatabaseConnection) -> TestResult<BTreeSet<String>> {
    let statement = match db.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT indexname AS name FROM pg_indexes WHERE schemaname = current_schema() AND tablename = 'moderation_application_operations'".to_string(),
        ),
        DatabaseBackend::Sqlite => Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA index_list('moderation_application_operations')".to_string(),
        ),
        backend => {
            return Err(test_error(format!(
                "unsupported migration contract backend: {backend:?}"
            )))
        }
    };
    let rows = db.query_all(statement).await?;
    let mut names = BTreeSet::new();
    for row in rows {
        names.insert(row.try_get::<String>("", "name")?);
    }
    Ok(names)
}

async fn seed_legacy_decisions(db: &DatabaseConnection) -> TestResult<UpgradeFixture> {
    assert_eq!(migration_count(db).await?, 3);
    let fixture = UpgradeFixture {
        tenant_id: Uuid::new_v4(),
        typed_case_id: Uuid::new_v4(),
        typed_decision_id: Uuid::new_v4(),
        typed_subject_id: Uuid::new_v4(),
        typed_subject_revision: 17,
        untyped_decision_id: Uuid::new_v4(),
    };
    let actor_id = Uuid::new_v4();
    let untyped_case_id = Uuid::new_v4();

    insert_case(
        db,
        fixture.tenant_id,
        fixture.typed_case_id,
        fixture.typed_subject_id,
        fixture.typed_subject_revision,
    )
    .await?;
    insert_decision(
        db,
        fixture.tenant_id,
        fixture.typed_decision_id,
        fixture.typed_case_id,
        fixture.typed_subject_revision,
        actor_id,
        'a',
    )
    .await?;
    db.execute_unprepared(
        &format!(
            r#"
INSERT INTO moderation_decision_effects (
    decision_id, tenant_id, schema_version, effect_kind, effect_payload, created_at
) VALUES (
    '{}', '{}', 1, 'warning',
    '{{"schema_version":1,"action":{{"type":"no_domain_mutation"}}}}',
    '{}'
)
"#,
            fixture.typed_decision_id, fixture.tenant_id, LEGACY_CREATED_AT
        )
        .replace("\\\"", "\""),
    )
    .await?;

    insert_case(db, fixture.tenant_id, untyped_case_id, Uuid::new_v4(), 23).await?;
    insert_decision(
        db,
        fixture.tenant_id,
        fixture.untyped_decision_id,
        untyped_case_id,
        23,
        actor_id,
        'b',
    )
    .await?;

    assert_eq!(
        scalar_i64(db, "SELECT COUNT(*) AS value FROM moderation_decisions").await?,
        2
    );
    assert_eq!(
        scalar_i64(
            db,
            "SELECT COUNT(*) AS value FROM moderation_decision_effects"
        )
        .await?,
        1
    );
    Ok(fixture)
}

async fn insert_case(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    case_id: Uuid,
    subject_id: Uuid,
    subject_revision: i64,
) -> TestResult<()> {
    db.execute_unprepared(&format!(
        r#"
INSERT INTO moderation_cases (
    id, tenant_id, scope_kind, subject_module, subject_kind, subject_id, subject_revision,
    queue_key, policy_version, status, revision, metadata, opened_at, decided_at, created_at, updated_at
) VALUES (
    '{case_id}', '{tenant_id}', 'platform', 'forum', 'forum_post', '{subject_id}', {subject_revision},
    'content', 1, 'decided', 3, '{{}}', '{LEGACY_CREATED_AT}', '{LEGACY_CREATED_AT}',
    '{LEGACY_CREATED_AT}', '{LEGACY_CREATED_AT}'
)
"#
    ))
    .await?;
    Ok(())
}

async fn insert_decision(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    decision_id: Uuid,
    case_id: Uuid,
    subject_revision: i64,
    actor_id: Uuid,
    hash_char: char,
) -> TestResult<()> {
    let decision_hash: String = std::iter::repeat_n(hash_char, 64).collect();
    db.execute_unprepared(&format!(
        r#"
INSERT INTO moderation_decisions (
    id, tenant_id, case_id, decision_kind, reason_code, policy_snapshot, subject_revision,
    decision_hash, decided_by, decided_at, created_at
) VALUES (
    '{decision_id}', '{tenant_id}', '{case_id}', 'warning', 'other', '{{}}', {subject_revision},
    '{decision_hash}', '{actor_id}', '{LEGACY_CREATED_AT}', '{LEGACY_CREATED_AT}'
)
"#
    ))
    .await?;
    Ok(())
}

fn assert_flag(row: &QueryResult, name: &str) -> TestResult<()> {
    assert_eq!(row.try_get::<i64>("", name)?, 1, "backfill flag `{name}`");
    Ok(())
}

async fn application_operation_count(db: &DatabaseConnection) -> TestResult<i64> {
    scalar_i64(
        db,
        "SELECT COUNT(*) AS value FROM moderation_application_operations",
    )
    .await
}

async fn migration_count(db: &DatabaseConnection) -> TestResult<i64> {
    scalar_i64(db, "SELECT COUNT(*) AS value FROM seaql_migrations").await
}

async fn scalar_i64(db: &DatabaseConnection, sql: &str) -> TestResult<i64> {
    let row = db
        .query_one(Statement::from_string(
            db.get_database_backend(),
            sql.to_string(),
        ))
        .await?
        .ok_or_else(|| test_error("scalar migration query returned no row"))?;
    Ok(row.try_get("", "value")?)
}

async fn sqlite_database() -> TestResult<DatabaseConnection> {
    let db = Database::connect("sqlite::memory:").await?;
    db.execute_unprepared("PRAGMA foreign_keys = ON").await?;
    Ok(db)
}

fn postgres_url() -> Option<String> {
    env::var(DATABASE_ENV)
        .or_else(|_| env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect_postgres(
    database_url: &str,
    application_name: &str,
) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    db.execute_unprepared(&format!("SET application_name TO '{application_name}'"))
        .await?;
    Ok(db)
}

fn test_error(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    std::io::Error::other(message.into()).into()
}
