use std::{error::Error, io, sync::Arc, time::Duration};

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, ModuleTermUpdateInput, TaxonomyError, TaxonomyModule,
    TaxonomyScopeType, TaxonomyService, TaxonomyTermKind, update_module_term_in_tx,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction, DbBackend,
    Statement, TransactionTrait,
};
use sea_orm_migration::SchemaManager;
use tokio::sync::Barrier;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const TAXONOMY_TEST_DATABASE_ENV: &str = "RUSTOK_TAXONOMY_TEST_DATABASE_URL";
const WORKER_A_APPLICATION_NAME: &str = "rustok_taxonomy_route_claim_a";
const WORKER_B_APPLICATION_NAME: &str = "rustok_taxonomy_route_claim_b";
const CONTESTED_ROUTE_KEY: &str = "shared-route";

struct PostgresTaxonomyRouteContentionDb {
    control: DatabaseConnection,
    db: DatabaseConnection,
    database_url: String,
    schema_name: String,
}

impl PostgresTaxonomyRouteContentionDb {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = postgres_database_url() else {
            eprintln!(
                "{TAXONOMY_TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping Taxonomy route-registry contention test"
            );
            return Ok(None);
        };

        let control = connect(&database_url, "rustok_taxonomy_route_claim_control").await?;
        let schema_name = format!("rustok_taxonomy_route_claim_{}", Uuid::new_v4().simple());
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = connect(&database_url, "rustok_taxonomy_route_claim_assertions").await?;
        set_search_path(&db, &schema_name).await?;

        let setup_result = async {
            let manager = SchemaManager::new(&db);
            for migration in TaxonomyModule.migrations() {
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

    async fn isolated_connection(&self, application_name: &str) -> TestResult<DatabaseConnection> {
        let db = connect(&self.database_url, application_name).await?;
        set_search_path(&db, &self.schema_name).await?;
        Ok(db)
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
async fn concurrent_localized_route_claim_commits_exactly_one_owner() -> TestResult<()> {
    let Some(test_db) = PostgresTaxonomyRouteContentionDb::setup().await? else {
        return Ok(());
    };

    let tenant_id = Uuid::new_v4();
    let service = TaxonomyService::new(test_db.db.clone());
    let first_term = create_module_term(&service, tenant_id, "Rust", "writer-a").await?;
    let second_term = create_module_term(&service, tenant_id, "Zig", "writer-b").await?;

    // Lock both localized rows before either worker starts. update_module_term_in_tx
    // performs route-key preflight before its translation UPDATE, so both workers
    // can prove the contested key is absent and then block on these row locks.
    let lock_db = test_db
        .isolated_connection("rustok_taxonomy_route_claim_lock_holder")
        .await?;
    let lock_txn: DatabaseTransaction = lock_db.begin().await?;
    let locked = lock_txn
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT id FROM taxonomy_term_translations \
             WHERE tenant_id = $1 AND term_id IN ($2, $3) FOR UPDATE",
            vec![tenant_id.into(), first_term.into(), second_term.into()],
        ))
        .await?;
    assert_eq!(locked.len(), 2, "both localized rows must be locked");

    let worker_a = test_db
        .isolated_connection(WORKER_A_APPLICATION_NAME)
        .await?;
    let worker_b = test_db
        .isolated_connection(WORKER_B_APPLICATION_NAME)
        .await?;
    let start = Arc::new(Barrier::new(2));

    let task_a = spawn_route_claim(worker_a, Arc::clone(&start), tenant_id, first_term);
    let task_b = spawn_route_claim(worker_b, Arc::clone(&start), tenant_id, second_term);

    wait_for_both_workers_to_block(&test_db.control).await?;
    lock_txn.commit().await?;

    let outcomes = [task_a.await?, task_b.await?];
    let success_count = outcomes.iter().filter(|outcome| outcome.is_ok()).count();
    let concurrent_conflict_count = outcomes
        .iter()
        .filter(|outcome| {
            matches!(
                outcome,
                Err(TaxonomyError::Conflict(message))
                    if message.contains("claimed concurrently")
            )
        })
        .count();
    assert_eq!(success_count, 1, "exactly one localized writer must commit");
    assert_eq!(
        concurrent_conflict_count, 1,
        "the losing writer must fail at the route-registry ownership claim"
    );

    let route_owner = load_route_owner(&test_db.db, tenant_id, CONTESTED_ROUTE_KEY)
        .await?
        .expect("the contested route key must have one durable owner");
    assert!(
        route_owner == first_term || route_owner == second_term,
        "the route owner must be one of the two contending terms"
    );

    let loser = if route_owner == first_term {
        second_term
    } else {
        first_term
    };
    assert_eq!(
        load_translation_slug(&test_db.db, tenant_id, route_owner).await?,
        CONTESTED_ROUTE_KEY,
        "winner translation and route reservation must commit together"
    );
    assert_ne!(
        load_translation_slug(&test_db.db, tenant_id, loser).await?,
        CONTESTED_ROUTE_KEY,
        "loser translation update must roll back with the failed reservation"
    );
    assert_eq!(
        count_route_owners(&test_db.db, tenant_id, CONTESTED_ROUTE_KEY).await?,
        1,
        "one localized route identity must have exactly one owner"
    );

    test_db.cleanup().await
}

fn spawn_route_claim(
    db: DatabaseConnection,
    start: Arc<Barrier>,
    tenant_id: Uuid,
    term_id: Uuid,
) -> tokio::task::JoinHandle<rustok_taxonomy::TaxonomyResult<()>> {
    tokio::spawn(async move {
        let txn = db.begin().await?;
        start.wait().await;
        let result = update_module_term_in_tx(
            &txn,
            tenant_id,
            term_id,
            &admin(),
            TaxonomyTermKind::Tag,
            "blog",
            ModuleTermUpdateInput {
                locale: "en".to_string(),
                name: None,
                slug: Some(CONTESTED_ROUTE_KEY.to_string()),
            },
        )
        .await;

        match result {
            Ok(_) => {
                txn.commit().await?;
                Ok(())
            }
            Err(error) => {
                let _ = txn.rollback().await;
                Err(error)
            }
        }
    })
}

async fn wait_for_both_workers_to_block(control: &DatabaseConnection) -> TestResult<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let row = control
                .query_one(Statement::from_string(
                    DbBackend::Postgres,
                    format!(
                        "SELECT COUNT(*)::bigint AS count \
                         FROM pg_stat_activity \
                         WHERE application_name IN ('{WORKER_A_APPLICATION_NAME}', '{WORKER_B_APPLICATION_NAME}') \
                           AND wait_event_type = 'Lock'"
                    ),
                ))
                .await?
                .expect("pg_stat_activity count query should return one row");
            let blocked: i64 = row.try_get("", "count")?;
            if blocked == 2 {
                return Ok::<(), sea_orm::DbErr>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| {
        io::Error::new(
            io::ErrorKind::TimedOut,
            "both Taxonomy route-claim workers did not block after route preflight",
        )
    })??;
    Ok(())
}

async fn create_module_term(
    service: &TaxonomyService,
    tenant_id: Uuid,
    name: &str,
    slug: &str,
) -> rustok_taxonomy::TaxonomyResult<Uuid> {
    service
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Tag,
                scope_type: TaxonomyScopeType::Module,
                scope_value: Some("blog".to_string()),
                locale: "en".to_string(),
                name: name.to_string(),
                slug: Some(slug.to_string()),
                canonical_key: Some(slug.to_string()),
                description: None,
                aliases: vec![],
            },
        )
        .await
}

async fn load_route_owner(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    route_key: &str,
) -> Result<Option<Uuid>, sea_orm::DbErr> {
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT term_id FROM taxonomy_term_route_keys \
         WHERE tenant_id = $1 AND kind = 'tag' AND scope_type = 'module' \
           AND scope_value = 'blog' AND locale = 'en' AND route_key = $2",
        vec![tenant_id.into(), route_key.into()],
    ))
    .await?
    .map(|row| row.try_get("", "term_id"))
    .transpose()
}

async fn count_route_owners(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    route_key: &str,
) -> Result<i64, sea_orm::DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS count FROM taxonomy_term_route_keys \
             WHERE tenant_id = $1 AND kind = 'tag' AND scope_type = 'module' \
               AND scope_value = 'blog' AND locale = 'en' AND route_key = $2",
            vec![tenant_id.into(), route_key.into()],
        ))
        .await?
        .expect("route owner count query should return one row");
    row.try_get("", "count")
}

async fn load_translation_slug(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    term_id: Uuid,
) -> Result<String, sea_orm::DbErr> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT slug FROM taxonomy_term_translations \
             WHERE tenant_id = $1 AND term_id = $2 AND locale = 'en'",
            vec![tenant_id.into(), term_id.into()],
        ))
        .await?
        .expect("localized Taxonomy translation should exist");
    row.try_get("", "slug")
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

fn postgres_database_url() -> Option<String> {
    std::env::var(TAXONOMY_TEST_DATABASE_ENV)
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect(database_url: &str, application_name: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    db.query_one(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT set_config('application_name', $1, false)",
        vec![application_name.into()],
    ))
    .await?;
    Ok(db)
}

async fn set_search_path(db: &DatabaseConnection, schema_name: &str) -> TestResult<()> {
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}""#))
        .await?;
    Ok(())
}
