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

    let change_journal_table = sea_orm::sea_query::Table::create()
        .table(sea_orm::sea_query::Alias::new(
            "shipping_option_translation_change_journal",
        ))
        .if_not_exists()
        .col(
            sea_orm::sea_query::ColumnDef::new(sea_orm::sea_query::Alias::new("change_seq"))
                .integer()
                .not_null()
                .auto_increment()
                .primary_key(),
        )
        .col(
            sea_orm::sea_query::ColumnDef::new(sea_orm::sea_query::Alias::new("operation_id"))
                .uuid()
                .not_null(),
        )
        .col(
            sea_orm::sea_query::ColumnDef::new(sea_orm::sea_query::Alias::new("tenant_id"))
                .uuid()
                .not_null(),
        )
        .col(
            sea_orm::sea_query::ColumnDef::new(sea_orm::sea_query::Alias::new(
                "shipping_option_id",
            ))
            .uuid()
            .not_null(),
        )
        .col(
            sea_orm::sea_query::ColumnDef::new(sea_orm::sea_query::Alias::new("resource_revision"))
                .string_len(96)
                .not_null(),
        )
        .col(
            sea_orm::sea_query::ColumnDef::new(sea_orm::sea_query::Alias::new("lifecycle"))
                .string_len(16)
                .not_null(),
        )
        .col(
            sea_orm::sea_query::ColumnDef::new(sea_orm::sea_query::Alias::new("created_at"))
                .date_time()
                .not_null()
                .default(sea_orm::sea_query::Expr::current_timestamp()),
        )
        .to_owned();
    db.execute_raw(builder.build(&change_journal_table))
        .await
        .expect("failed to create shipping_option_translation_change_journal");

    let uq_index = sea_orm::sea_query::Index::create()
        .name("uq_shipping_option_translation_change_operation_target")
        .table(sea_orm::sea_query::Alias::new(
            "shipping_option_translation_change_journal",
        ))
        .col(sea_orm::sea_query::Alias::new("operation_id"))
        .col(sea_orm::sea_query::Alias::new("shipping_option_id"))
        .unique()
        .if_not_exists()
        .to_owned();
    db.execute_raw(builder.build(&uq_index))
        .await
        .expect("failed to create uq_shipping_option_translation_change_operation_target");

    let idx_tenant_seq = sea_orm::sea_query::Index::create()
        .name("idx_shipping_option_translation_change_tenant_seq")
        .table(sea_orm::sea_query::Alias::new(
            "shipping_option_translation_change_journal",
        ))
        .col(sea_orm::sea_query::Alias::new("tenant_id"))
        .col(sea_orm::sea_query::Alias::new("change_seq"))
        .if_not_exists()
        .to_owned();
    db.execute_raw(builder.build(&idx_tenant_seq))
        .await
        .expect("failed to create idx_shipping_option_translation_change_tenant_seq");

    let idx_target_seq = sea_orm::sea_query::Index::create()
        .name("idx_shipping_option_translation_change_target_seq")
        .table(sea_orm::sea_query::Alias::new(
            "shipping_option_translation_change_journal",
        ))
        .col(sea_orm::sea_query::Alias::new("tenant_id"))
        .col(sea_orm::sea_query::Alias::new("shipping_option_id"))
        .col(sea_orm::sea_query::Alias::new("change_seq"))
        .if_not_exists()
        .to_owned();
    db.execute_raw(builder.build(&idx_target_seq))
        .await
        .expect("failed to create idx_shipping_option_translation_change_target_seq");
}

async fn create_entity_table(
    db: &DatabaseConnection,
    builder: &DbBackend,
    mut statement: sea_orm::sea_query::TableCreateStatement,
) {
    statement.if_not_exists();
    db.execute_raw(builder.build(&statement))
        .await
        .expect("failed to create fulfillment test table");
}
