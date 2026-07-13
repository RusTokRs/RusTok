use sea_orm_migration::prelude::*;

#[path = "sqlite_setup.rs"]
mod setup;
#[path = "sqlite_revisions.rs"]
mod revisions;
#[path = "sqlite_deletes.rs"]
mod deletes;
#[path = "sqlite_counters.rs"]
mod counters;

pub(super) async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    setup::apply_setup(manager).await?;
    revisions::apply_revisions(manager).await?;
    deletes::apply_deletes(manager).await?;
    counters::apply_counters(manager).await?;
    Ok(())
}
