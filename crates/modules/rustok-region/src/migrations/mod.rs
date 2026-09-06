mod m20250130_000014_create_regions;
mod m20260411_000001_add_region_translations;
mod m20260412_000110_add_region_tax_provider_id;
mod m20260412_000111_add_region_country_tax_policies;
mod m20260713_000112_enforce_region_policy_integrity;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20250130_000014_create_regions::Migration),
        Box::new(m20260411_000001_add_region_translations::Migration),
        Box::new(m20260412_000110_add_region_tax_provider_id::Migration),
        Box::new(m20260412_000111_add_region_country_tax_policies::Migration),
        Box::new(m20260713_000112_enforce_region_policy_integrity::Migration),
    ]
}
