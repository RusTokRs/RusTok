mod m20260329_000001_create_taxonomy_tables;
mod m20260711_000001_add_tenant_identity_key;
mod m20260721_000006_expand_taxonomy_locale_storage_columns;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260329_000001_create_taxonomy_tables::Migration),
        Box::new(m20260711_000001_add_tenant_identity_key::Migration),
        Box::new(m20260721_000006_expand_taxonomy_locale_storage_columns::Migration),
    ]
}
