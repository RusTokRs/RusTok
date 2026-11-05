use sea_orm::{ConnectionTrait, DatabaseConnection, Schema};

pub async fn ensure_pricing_tables(db: &DatabaseConnection) {
    let builder = db.get_database_backend();
    let schema = Schema::new(builder);
    let tables = [
        schema.create_table_from_entity(crate::entities::price::Entity),
        schema.create_table_from_entity(crate::entities::price_list::Entity),
        schema.create_table_from_entity(crate::entities::price_list_translation::Entity),
    ];
    for mut stmt in tables {
        let stmt = builder.build(stmt.if_not_exists());
        let _ = db.execute_raw(stmt).await;
    }
}
