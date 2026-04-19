use rustok_commerce_foundation::entities::region_country_tax_policy;
use rustok_commerce_foundation::entities::region_translation;
use rustok_region::entities::region;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Schema};

pub async fn ensure_region_schema(db: &DatabaseConnection) {
    if db.get_database_backend() != DbBackend::Sqlite {
        return;
    }

    let builder = db.get_database_backend();
    let schema = Schema::new(builder);
    let mut statement = schema.create_table_from_entity(region::Entity);
    statement.if_not_exists();
    db.execute(builder.build(&statement))
        .await
        .expect("failed to create region test table");
    let mut translations_statement = schema.create_table_from_entity(region_translation::Entity);
    translations_statement.if_not_exists();
    db.execute(builder.build(&translations_statement))
        .await
        .expect("failed to create region translation test table");
    let mut country_policy_statement =
        schema.create_table_from_entity(region_country_tax_policy::Entity);
    country_policy_statement.if_not_exists();
    db.execute(builder.build(&country_policy_statement))
        .await
        .expect("failed to create region country tax policy test table");
}
