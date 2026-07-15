use rustok_order::entities::{
    order, order_adjustment, order_change, order_line_item, order_line_item_translation,
    order_return, order_return_item, order_tax_line,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Schema};

pub async fn ensure_order_schema(db: &DatabaseConnection) {
    if db.get_database_backend() != DbBackend::Sqlite {
        return;
    }

    let builder = db.get_database_backend();
    let schema = Schema::new(builder);
    let tenants_table = sea_orm::sea_query::Table::create()
        .table(sea_orm::sea_query::Alias::new("tenants"))
        .if_not_exists()
        .col(
            sea_orm::sea_query::ColumnDef::new(sea_orm::sea_query::Alias::new("id"))
                .uuid()
                .not_null()
                .primary_key(),
        )
        .col(
            sea_orm::sea_query::ColumnDef::new(sea_orm::sea_query::Alias::new("default_locale"))
                .string_len(32)
                .not_null()
                .default("en"),
        )
        .to_owned();
    db.execute(builder.build(&tenants_table))
        .await
        .expect("tenants table should be created for locale resolution");

    for statement in [
        schema.create_table_from_entity(order::Entity),
        schema.create_table_from_entity(order_line_item::Entity),
        schema.create_table_from_entity(order_line_item_translation::Entity),
        schema.create_table_from_entity(order_adjustment::Entity),
        schema.create_table_from_entity(order_tax_line::Entity),
        schema.create_table_from_entity(order_change::Entity),
        schema.create_table_from_entity(order_return::Entity),
        schema.create_table_from_entity(order_return_item::Entity),
    ] {
        crate::support::create_entity_table(db, &builder, statement).await;
    }
}
