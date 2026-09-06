mod automation;
mod events;
mod schema;
mod validation;

use sea_orm_migration::prelude::*;

pub(super) async fn up(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    schema::apply(manager).await?;
    validation::apply(manager).await?;
    events::apply(manager).await?;
    automation::apply(manager).await?;
    Ok(())
}
