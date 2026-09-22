use sea_orm_migration::prelude::*;

mod category_topic;
mod reply_relations;
mod schema;

pub(super) async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    schema::schema(manager).await?;
    category_topic::category_topic(manager).await?;
    reply_relations::reply_relations(manager).await?;
    Ok(())
}
