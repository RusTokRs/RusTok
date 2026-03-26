use rustok_cart::entities::{cart, cart_line_item};
use rustok_commerce::entities::{
    inventory_item, inventory_level, price, product, product_image, product_image_translation,
    product_option, product_option_translation, product_option_value,
    product_option_value_translation, product_translation, product_variant, region,
    reservation_item, stock_location, variant_translation,
};
use rustok_fulfillment::entities::{fulfillment, shipping_option};
use rustok_order::entities::{order, order_line_item};
use rustok_payment::entities::{payment, payment_collection};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, DbBackend, Schema, Statement};

pub async fn ensure_commerce_schema(db: &DatabaseConnection) {
    if db.get_database_backend() != DbBackend::Sqlite {
        return;
    }

    let builder = db.get_database_backend();
    let schema = Schema::new(builder);

    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(product::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(product_translation::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(product_option::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(product_option_translation::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(product_option_value::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(product_option_value_translation::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(product_variant::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(stock_location::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(inventory_item::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(inventory_level::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(reservation_item::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(variant_translation::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(region::Entity),
    )
    .await;
    create_entity_table(db, &builder, schema.create_table_from_entity(price::Entity)).await;
    create_entity_table(db, &builder, schema.create_table_from_entity(cart::Entity)).await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(cart_line_item::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(payment_collection::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(payment::Entity),
    )
    .await;
    create_entity_table(db, &builder, schema.create_table_from_entity(order::Entity)).await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(order_line_item::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(shipping_option::Entity),
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
        schema.create_table_from_entity(product_image::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(product_image_translation::Entity),
    )
    .await;
    ensure_tenant_tables(db).await;
}

async fn create_entity_table(
    db: &DatabaseConnection,
    builder: &DbBackend,
    mut statement: sea_orm::sea_query::TableCreateStatement,
) {
    statement.if_not_exists();
    db.execute(builder.build(&statement))
        .await
        .expect("failed to create commerce test table");
}

async fn ensure_tenant_tables(db: &DatabaseConnection) {
    for sql in [
        "CREATE TABLE IF NOT EXISTS tenants (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            slug TEXT NOT NULL,
            domain TEXT NULL,
            settings TEXT NOT NULL DEFAULT '{}',
            default_locale TEXT NOT NULL,
            is_active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        "CREATE TABLE IF NOT EXISTS tenant_locales (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            name TEXT NOT NULL,
            native_name TEXT NOT NULL,
            is_default INTEGER NOT NULL DEFAULT 0,
            is_enabled INTEGER NOT NULL DEFAULT 1,
            fallback_locale TEXT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
    ] {
        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            sql.to_string(),
        ))
        .await
        .expect("failed to create tenant context test table");
    }
}
