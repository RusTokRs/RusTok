mod shared;

mod m20250130_000010_create_index_content;
mod m20250130_000010a_create_search_index;
mod m20250130_000011_create_index_products;
mod m20260411_000001_create_index_flex_entries;
mod m20260701_000001_create_index_product_attribute_facets;
mod m20260713_000002_scope_content_identity_by_tenant;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20250130_000010_create_index_content::Migration),
        Box::new(m20250130_000010a_create_search_index::Migration),
        Box::new(m20250130_000011_create_index_products::Migration),
        Box::new(m20260411_000001_create_index_flex_entries::Migration),
        Box::new(m20260701_000001_create_index_product_attribute_facets::Migration),
        Box::new(m20260713_000002_scope_content_identity_by_tenant::Migration),
    ]
}
