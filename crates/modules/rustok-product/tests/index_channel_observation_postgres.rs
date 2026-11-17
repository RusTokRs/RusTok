use std::time::Duration;

use rustok_product::{
    ProductSalesChannelIndexRelationError as RelationError,
    ProductSalesChannelIndexRelationObservation as Observation,
    ProductSalesChannelIndexRelationStore, ProductSalesChannelIndexRelationWriteOutcome,
    index_channel_visibility::ProductChannelVisibility,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use uuid::Uuid;

#[path = "support/postgres.rs"]
mod postgres;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL database"]
async fn commits_only_current_channel_observations_and_rolls_back_the_graph_on_failure()
-> TestResult {
    postgres::with_product_postgres_database("product_channel_observation", verify).await
}

async fn verify(db: DatabaseConnection) -> TestResult {
    let tenant = Uuid::new_v4();
    let product = Uuid::new_v4();
    let first_channel = Uuid::new_v4();
    let second_channel = Uuid::new_v4();
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO tenants (id) VALUES ($1)",
        [tenant.into()],
    ))
    .await?;
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO products (tenant_id, id) VALUES ($1, $2)",
        [tenant.into(), product.into()],
    ))
    .await?;
    let owner = ProductSalesChannelIndexRelationStore::new(db.clone());
    let initial = Observation {
        product_source_version: current_revision(&db, tenant, product).await?,
        visibility: ProductChannelVisibility::Unrestricted,
        channel_identity_generation: 1,
        channel_ids: vec![first_channel],
    };
    let first = owner.commit_observation(tenant, product, &initial).await?;
    assert!(matches!(
        first,
        ProductSalesChannelIndexRelationWriteOutcome::Initial(_)
    ));
    let duplicate = owner.commit_observation(tenant, product, &initial).await?;
    assert!(matches!(
        duplicate,
        ProductSalesChannelIndexRelationWriteOutcome::Unchanged(_)
    ));
    let initial_state = state(&db, tenant, product).await?;
    assert_eq!(
        initial_state,
        State {
            relations: 1,
            witnesses: 1,
            projections: 1,
            generation: 1
        }
    );

    let conflicting = Observation {
        channel_ids: vec![second_channel],
        ..initial.clone()
    };
    assert_eq!(
        owner
            .commit_observation(tenant, product, &conflicting)
            .await,
        Err(RelationError::ObservationConflict)
    );
    let advanced = Observation {
        channel_identity_generation: 2,
        ..initial.clone()
    };
    owner.commit_observation(tenant, product, &advanced).await?;
    assert_eq!(
        owner.commit_observation(tenant, product, &initial).await,
        Err(RelationError::ObservationStale)
    );
    assert_eq!(
        state(&db, tenant, product).await?,
        State {
            witnesses: 2,
            generation: 2,
            ..initial_state
        }
    );

    let writer = db.begin().await?;
    writer
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE products SET vendor = 'new vendor' WHERE tenant_id = $1 AND id = $2",
            [tenant.into(), product.into()],
        ))
        .await?;
    let obsolete = Observation {
        channel_identity_generation: 3,
        channel_ids: vec![second_channel],
        ..initial
    };
    let commit = owner.commit_observation(tenant, product, &obsolete);
    tokio::pin!(commit);
    assert!(
        tokio::time::timeout(Duration::from_millis(150), &mut commit)
            .await
            .is_err()
    );
    writer.commit().await?;
    assert_eq!(commit.await, Err(RelationError::ObservationStale));
    let after_edit = state(&db, tenant, product).await?;
    assert_eq!(
        after_edit,
        State {
            relations: 1,
            witnesses: 2,
            projections: 2,
            generation: 2
        }
    );

    let current = Observation {
        product_source_version: current_revision(&db, tenant, product).await?,
        visibility: ProductChannelVisibility::Unrestricted,
        channel_identity_generation: 3,
        channel_ids: vec![second_channel],
    };
    let wrong_visibility = Observation {
        visibility: ProductChannelVisibility::Restricted(vec!["web".into()]),
        ..current.clone()
    };
    assert_eq!(
        owner
            .commit_observation(tenant, product, &wrong_visibility)
            .await,
        Err(RelationError::ObservationStale)
    );
    db.execute_unprepared(
        "ALTER TABLE product_sales_channel_index_relation_freshness_snapshots
         ADD CONSTRAINT reject_test_witness CHECK (channel_identity_generation <> 3)",
    )
    .await?;
    assert_eq!(
        owner.commit_observation(tenant, product, &current).await,
        Err(RelationError::Unavailable)
    );
    assert_eq!(state(&db, tenant, product).await?, after_edit);
    db.execute_unprepared(
        "ALTER TABLE product_sales_channel_index_relation_freshness_snapshots DROP CONSTRAINT reject_test_witness",
    ).await?;
    let committed = owner.commit_observation(tenant, product, &current).await?;
    assert_eq!(committed.record().channel_ids(), &[second_channel]);
    assert_eq!(
        state(&db, tenant, product).await?,
        State {
            relations: 2,
            witnesses: 3,
            projections: 3,
            generation: 3
        }
    );
    Ok(())
}

async fn current_revision(db: &DatabaseConnection, tenant: Uuid, product: Uuid) -> TestResult<u64> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT index_revision FROM products WHERE tenant_id = $1 AND id = $2",
            [tenant.into(), product.into()],
        ))
        .await?
        .ok_or("fixture Product missing")?;
    Ok(u64::try_from(row.try_get::<i64>("", "index_revision")?)?)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct State {
    relations: i64,
    witnesses: i64,
    projections: i64,
    generation: i64,
}

async fn state(db: &DatabaseConnection, tenant: Uuid, product: Uuid) -> TestResult<State> {
    let row = db.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT
            (SELECT count(*) FROM product_sales_channel_index_relation_snapshots
              WHERE tenant_id = $1 AND product_id = $2) AS relations,
            (SELECT count(*) FROM product_sales_channel_index_relation_freshness_snapshots
              WHERE tenant_id = $1 AND product_id = $2) AS witnesses,
            (SELECT count(*) FROM product_index_graph_projection_snapshots
              WHERE tenant_id = $1 AND product_id = $2) AS projections,
            (SELECT channel_identity_generation FROM product_sales_channel_index_relation_freshness_snapshots
              WHERE tenant_id = $1 AND product_id = $2 ORDER BY sequence_no DESC LIMIT 1) AS generation",
        [tenant.into(), product.into()],
    )).await?.ok_or("fixture state missing")?;
    Ok(State {
        relations: row.try_get("", "relations")?,
        witnesses: row.try_get("", "witnesses")?,
        projections: row.try_get("", "projections")?,
        generation: row.try_get("", "generation")?,
    })
}
