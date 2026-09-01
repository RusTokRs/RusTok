use std::{env, error::Error as StdError, sync::Arc, time::Duration};

use rustok_core::{SecurityContext, UserRole};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, SetTaxonomyCategoryPlacementInput, TaxonomyScopeType, TaxonomyService,
    TaxonomyTermKind,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use tokio::sync::Barrier;
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_TAXONOMY_TEST_DATABASE_URL";

type TestResult<T> = Result<T, Box<dyn StdError + Send + Sync>>;

#[tokio::test]
async fn concurrent_opposite_parent_moves_commit_once() -> TestResult<()> {
    let Some(database_url) = postgres_database_url() else {
        eprintln!(
            "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Taxonomy Category hierarchy contention evidence"
        );
        return Ok(());
    };

    let db = connect(&database_url).await?;
    ensure_category_hierarchy_schema(&db).await?;
    let tenant_id = Uuid::new_v4();
    let a = create_category(&db, tenant_id, "Concurrent A").await?;
    let b = create_category(&db, tenant_id, "Concurrent B").await?;

    let service_a = TaxonomyService::new(connect(&database_url).await?);
    let service_b = TaxonomyService::new(connect(&database_url).await?);
    let barrier = Arc::new(Barrier::new(2));

    let left_barrier = Arc::clone(&barrier);
    let left = async move {
        left_barrier.wait().await;
        service_a
            .set_category_placement(
                tenant_id,
                admin(),
                a,
                SetTaxonomyCategoryPlacementInput {
                    parent_id: Some(b),
                    position: 0,
                },
            )
            .await
    };

    let right_barrier = Arc::clone(&barrier);
    let right = async move {
        right_barrier.wait().await;
        service_b
            .set_category_placement(
                tenant_id,
                admin(),
                b,
                SetTaxonomyCategoryPlacementInput {
                    parent_id: Some(a),
                    position: 0,
                },
            )
            .await
    };

    let (left, right) = tokio::join!(left, right);
    let success_count = usize::from(left.is_ok()) + usize::from(right.is_ok());
    assert_eq!(
        success_count, 1,
        "tenant-serialized opposite hierarchy moves must admit exactly one writer; left={left:?} right={right:?}"
    );

    let loser = left
        .as_ref()
        .err()
        .or_else(|| right.as_ref().err())
        .expect("one hierarchy writer must lose");
    assert!(
        loser.to_string().contains("cycle"),
        "losing writer must observe the committed hierarchy and fail as a cycle: {loser}"
    );

    let reader = TaxonomyService::new(db);
    let a_placement = reader.get_category_placement(tenant_id, admin(), a).await?;
    let b_placement = reader.get_category_placement(tenant_id, admin(), b).await?;
    assert!(
        (a_placement.parent_id == Some(b) && b_placement.parent_id.is_none())
            || (b_placement.parent_id == Some(a) && a_placement.parent_id.is_none()),
        "final hierarchy must contain one directed edge and one root: a={a_placement:?} b={b_placement:?}"
    );

    Ok(())
}

fn postgres_database_url() -> Option<String> {
    env::var(DATABASE_ENV)
        .ok()
        .filter(|value| value.starts_with("postgres://") || value.starts_with("postgresql://"))
}

async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_string());
    options
        .max_connections(1)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(10))
        .acquire_timeout(Duration::from_secs(10));
    Ok(Database::connect(options).await?)
}

async fn ensure_category_hierarchy_schema(db: &DatabaseConnection) -> TestResult<()> {
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT to_regclass('taxonomy_category_hierarchy') IS NOT NULL AS present".to_string(),
        ))
        .await?
        .ok_or("PostgreSQL schema probe returned no row")?;
    let present: bool = row.try_get("", "present")?;
    if !present {
        return Err(
            "canonical server migrations did not create taxonomy_category_hierarchy".into(),
        );
    }
    Ok(())
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

async fn create_category(db: &DatabaseConnection, tenant_id: Uuid, name: &str) -> TestResult<Uuid> {
    let service = TaxonomyService::new(db.clone());
    Ok(service
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind: TaxonomyTermKind::Category,
                scope_type: TaxonomyScopeType::Global,
                scope_value: None,
                locale: "en".to_string(),
                name: name.to_string(),
                slug: None,
                canonical_key: Some(format!("category-{}", Uuid::new_v4())),
                description: None,
                aliases: Vec::new(),
            },
        )
        .await?)
}
