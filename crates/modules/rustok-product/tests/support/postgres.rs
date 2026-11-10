use rustok_core::MigrationSource;
use rustok_test_utils::{
    assert_postgres_url, connect_postgres, create_postgres_database,
    drop_postgres_database_if_exists, postgres_database_url, unique_postgres_database_name,
};
use sea_orm::{ConnectionTrait, DatabaseConnection};
use sea_orm_migration::{MigrationTrait, MigratorTrait, SchemaManager};

pub(crate) struct ProductMigrator;

impl MigratorTrait for ProductMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        rustok_product::migrations::migrations()
    }
}

pub(crate) async fn with_product_postgres_database<T, F, Fut>(
    prefix: &str,
    test: F,
) -> Result<T, Box<dyn std::error::Error>>
where
    F: FnOnce(DatabaseConnection) -> Fut,
    Fut: std::future::Future<Output = Result<T, Box<dyn std::error::Error>>>,
{
    let admin_url = std::env::var("RUSTOK_MIGRATION_SMOKE_ADMIN_URL")?;
    assert_postgres_url(&admin_url);
    let name = unique_postgres_database_name(prefix);
    let url = postgres_database_url(&admin_url, &name);
    let admin = connect_postgres(&admin_url).await?;
    create_postgres_database(&admin, &name).await?;

    let result = async {
        let db = connect_postgres(&url).await?;
        db.execute_unprepared("CREATE TABLE tenants (id UUID PRIMARY KEY)")
            .await?;
        let manager = SchemaManager::new(&db);
        for migration in rustok_outbox::OutboxModule
            .migrations()
            .into_iter()
            .chain(rustok_taxonomy::TaxonomyModule.migrations())
        {
            migration.up(&manager).await?;
        }
        flex::cache_generation::create_field_definition_cache_generation_table(&manager).await?;
        ProductMigrator::up(&db, None).await?;
        test(db).await
    }
    .await;
    let cleanup = drop_postgres_database_if_exists(&admin, &name).await;
    match (result, cleanup) {
        (Err(error), Err(cleanup)) => Err(format!("{error}; database cleanup: {cleanup}").into()),
        (Err(error), _) => Err(error),
        (_, Err(error)) => Err(error.into()),
        (Ok(value), Ok(())) => Ok(value),
    }
}
