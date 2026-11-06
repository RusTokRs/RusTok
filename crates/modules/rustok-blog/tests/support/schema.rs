// Shared actual migration composition for Blog service and PostgreSQL tests.
// Foundation owns identity creation, Auth owns Channel's OAuth prerequisite,
// and each module supplies its complete migration chain. No private table DDL
// or SQLite migration-name exclusions belong in a reference fixture.

use rustok_core::MigrationSource;
use rustok_outbox::SysEventsMigration;
use sea_orm::{DatabaseConnection, DbErr};
use sea_orm_migration::{MigrationTrait, SchemaManager};

pub async fn apply_blog_schema(
    db: &DatabaseConnection,
    blog_migrations: Vec<Box<dyn MigrationTrait>>,
) -> Result<(), DbErr> {
    let manager = SchemaManager::new(db);
    for migration in rustok_migrations::identity::migrations() {
        migration.up(&manager).await?;
    }
    SysEventsMigration.up(&manager).await?;
    let oauth = rustok_auth::migrations::migrations()
        .into_iter()
        .find(|migration| migration.name() == "m20260308_000001_create_oauth_apps")
        .ok_or_else(|| DbErr::Custom("Channel OAuth schema prerequisite is absent".into()))?;
    oauth.up(&manager).await?;
    for migration in rustok_content::ContentModule
        .migrations()
        .into_iter()
        .chain(rustok_taxonomy::TaxonomyModule.migrations())
        .chain(rustok_channel::migrations::migrations())
        .chain(blog_migrations)
    {
        migration.up(&manager).await?;
    }
    Ok(())
}
