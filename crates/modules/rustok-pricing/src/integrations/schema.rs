use sea_orm::{ConnectionTrait, DatabaseConnection, DbErr, Schema};

/// Creates the Pricing tables from their entities when they do not exist yet.
///
/// A failed statement is returned to the caller: a fixture that continued past it would report
/// the missing table later, as an unrelated query error.
pub async fn ensure_pricing_tables(db: &DatabaseConnection) -> Result<(), DbErr> {
    let builder = db.get_database_backend();
    let schema = Schema::new(builder);
    let tables = [
        schema.create_table_from_entity(crate::entities::price::Entity),
        schema.create_table_from_entity(crate::entities::price_list::Entity),
        schema.create_table_from_entity(crate::entities::price_list_translation::Entity),
    ];
    for mut stmt in tables {
        let stmt = builder.build(stmt.if_not_exists());
        db.execute_raw(stmt).await?;
    }
    Ok(())
}
