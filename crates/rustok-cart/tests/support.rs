use rustok_cart::entities::{
    cart, cart_adjustment, cart_line_item, cart_line_item_translation, cart_shipping_selection,
    cart_tax_line,
};
use rustok_commerce_foundation::entities::{region, region_country_tax_policy};
use rustok_fulfillment::entities::shipping_option;
use rustok_tenant::entities::tenant;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DatabaseConnection, DbBackend, Schema,
};
use uuid::{uuid, Uuid};

pub const TEST_TENANT_ID: Uuid = uuid!("11111111-1111-1111-1111-111111111111");

pub async fn ensure_cart_schema(db: &DatabaseConnection) {
    if db.get_database_backend() != DbBackend::Sqlite {
        return;
    }

    let builder = db.get_database_backend();
    let schema = Schema::new(builder);

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
        schema.create_table_from_entity(shipping_option::Entity),
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
        schema.create_table_from_entity(region_country_tax_policy::Entity),
    )
    .await;
    create_entity_table(
        db,
        &builder,
        schema.create_table_from_entity(tenant::Entity),
    )
    .await;

    tenant::ActiveModel {
        id: Set(TEST_TENANT_ID),
        name: Set("Test Tenant".to_string()),
        slug: Set("test-tenant".to_string()),
        domain: Set(None),
        settings: Set(serde_json::json!({})),
        default_locale: Set("en".to_string()),
        is_active: Set(true),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(db)
    .await
    .expect("failed to seed tenant row for cart tests");
}

async fn create_entity_table(
    db: &DatabaseConnection,
    builder: &DbBackend,
    mut statement: sea_orm::sea_query::TableCreateStatement,
) {
    statement.if_not_exists();
    db.execute(builder.build(&statement))
        .await
        .expect("failed to create cart test table");
}
