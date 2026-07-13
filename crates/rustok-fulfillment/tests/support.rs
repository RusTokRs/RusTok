use rustok_fulfillment::entities::{
    fulfillment, fulfillment_item, provider_operation, shipping_option, shipping_option_translation,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Schema};

pub async fn ensure_fulfillment_schema(db: &DatabaseConnection) {
    if db.get_database_backend() != DbBackend::Sqlite {
        return;
    }

    let builder = db.get_database_backend();
    let schema = Schema::new(builder);

    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(shipping_option::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(shipping_option_translation::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(fulfillment::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(fulfillment_item::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(provider_operation::Entity),
    )
    .await;
}

async fn create_entity_table(
    db: &DatabaseConnection,
    builder: &DbBackend,
    mut statement: sea_orm::sea_query::TableCreateStatement,
) {
    statement.if_not_exists();
    db.execute(builder.build(&statement))
        .await
        .expect("failed to create fulfillment test table");
}
