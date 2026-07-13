use sea_orm_migration::prelude::*;

mod content;
mod relations;
mod schema;

pub(super) async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    schema::schema(manager).await?;
    content::content(manager).await?;
    relations::relations(manager).await?;
    Ok(())
}
