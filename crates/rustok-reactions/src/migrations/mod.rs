mod m20260806_000001_create_reaction_owner_state;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(
        m20260806_000001_create_reaction_owner_state::Migration,
    )]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![MigrationDependencyDescriptor::new(
        "m20260806_000001_create_reaction_owner_state",
        vec!["m20260803_000001_create_owner_operation_receipts"],
    )]
}
