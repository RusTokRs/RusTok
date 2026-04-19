use rustok_cart::entities::{
    cart, cart_adjustment, cart_line_item, cart_line_item_translation, cart_shipping_selection,
    cart_tax_line,
};
use rustok_channel::entities::{channel, channel_module_binding};
use rustok_commerce::entities::{
    inventory_item, inventory_level, price, price_list, price_list_translation, product,
    product_image, product_image_translation, product_option, product_option_translation,
    product_option_value, product_option_value_translation, product_translation, product_variant,
    region, region_country_tax_policy, region_translation, reservation_item, shipping_profile,
    shipping_profile_translation, stock_location, stock_location_translation, variant_translation,
};
use rustok_customer::entities::customer;
use rustok_fulfillment::entities::{
    fulfillment, fulfillment_item, shipping_option, shipping_option_translation,
};
use rustok_order::entities::{
    order, order_adjustment, order_line_item, order_line_item_translation, order_tax_line,
};
use rustok_payment::entities::{payment, payment_collection, refund};
use rustok_product::entities::product_tag;
use rustok_taxonomy::entities::{taxonomy_term, taxonomy_term_alias, taxonomy_term_translation};
use rustok_tenant::entities::tenant_module;
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
        schema.create_table_from_entity(taxonomy_term::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(taxonomy_term_translation::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(taxonomy_term_alias::Entity),
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
        schema.create_table_from_entity(stock_location_translation::Entity),
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
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(region_translation::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(region_country_tax_policy::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(shipping_profile::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(shipping_profile_translation::Entity),
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
        schema.create_table_from_entity(cart_line_item_translation::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(cart_adjustment::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(cart_tax_line::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(cart_shipping_selection::Entity),
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
        schema.create_table_from_entity(customer::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(payment::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(refund::Entity),
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
        schema.create_table_from_entity(order_line_item_translation::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(order_adjustment::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(order_tax_line::Entity),
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
        schema.create_table_from_entity(product_image::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(product_image_translation::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(product_tag::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(channel::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(channel_module_binding::Entity),
    )
    .await;
    ensure_tenant_tables(db).await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(price_list::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(price_list_translation::Entity),
    )
    .await;
    ensure_field_definition_tables(db).await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(tenant_module::Entity),
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

async fn ensure_field_definition_tables(db: &DatabaseConnection) {
    for prefix in ["product", "order"] {
        for sql in [
            format!(
                "CREATE TABLE IF NOT EXISTS {prefix}_field_definitions (
                    id TEXT PRIMARY KEY NOT NULL,
                    tenant_id TEXT NOT NULL,
                    field_key TEXT NOT NULL,
                    field_type TEXT NOT NULL,
                    label TEXT NOT NULL,
                    description TEXT NULL,
                    is_localized INTEGER NOT NULL DEFAULT 0,
                    is_required INTEGER NOT NULL DEFAULT 0,
                    default_value TEXT NULL,
                    validation TEXT NULL,
                    position INTEGER NOT NULL DEFAULT 0,
                    is_active INTEGER NOT NULL DEFAULT 1,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                )"
            ),
            format!(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_{prefix}_fd_tenant_key
                 ON {prefix}_field_definitions (tenant_id, field_key)"
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS idx_{prefix}_fd_tenant_active
                 ON {prefix}_field_definitions (tenant_id, is_active)"
            ),
        ] {
            db.execute(Statement::from_string(DatabaseBackend::Sqlite, sql))
                .await
                .expect("failed to create field definitions test table");
        }
    }

    for sql in [
        "CREATE TABLE IF NOT EXISTS flex_attached_localized_values (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            field_key TEXT NOT NULL,
            locale TEXT NOT NULL,
            value TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )"
        .to_string(),
        "CREATE INDEX IF NOT EXISTS idx_flex_attached_values_owner
         ON flex_attached_localized_values (tenant_id, entity_type, entity_id)"
            .to_string(),
    ] {
        db.execute(Statement::from_string(DatabaseBackend::Sqlite, sql))
            .await
            .expect("failed to create attached localized values test table");
    }
}
