use rustok_fulfillment::entities::{
    fulfillment, fulfillment_item, provider_operation, shipping_option, shipping_option_translation,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Schema};
use sea_orm_migration::{MigrationTrait, SchemaManager};

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

pub async fn ensure_provider_journal_guards(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    for migration in rustok_fulfillment::migrations::migrations()
        .into_iter()
        .skip(6)
    {
        migration
            .up(&manager)
            .await
            .expect("provider journal migration should run");
    }
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
