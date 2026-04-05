mod shared;

mod m20250130_000012_create_commerce_products;
mod m20250130_000013_create_commerce_options;
mod m20250130_000014_create_commerce_variants;
mod m20260301_000001_alter_product_variants_add_fields;
mod m20260316_000002_create_product_field_definitions;
mod m20260325_000003_align_runtime_compatibility_columns;
mod m20260329_000001_create_product_tags;
mod m20260405_000004_add_variant_shipping_profile_slug;
mod m20260405_000005_add_product_shipping_profile_slug;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20250130_000012_create_commerce_products::Migration),
        Box::new(m20250130_000013_create_commerce_options::Migration),
        Box::new(m20250130_000014_create_commerce_variants::Migration),
        Box::new(m20260301_000001_alter_product_variants_add_fields::Migration),
        Box::new(m20260316_000002_create_product_field_definitions::Migration),
        Box::new(m20260325_000003_align_runtime_compatibility_columns::Migration),
        Box::new(m20260329_000001_create_product_tags::Migration),
        Box::new(m20260405_000004_add_variant_shipping_profile_slug::Migration),
        Box::new(m20260405_000005_add_product_shipping_profile_slug::Migration),
    ]
}
