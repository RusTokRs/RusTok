use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

mod m20260319_000001_create_mcp_management_tables;
mod m20260320_000001_create_mcp_scaffold_drafts;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260319_000001_create_mcp_management_tables::Migration),
        Box::new(m20260320_000001_create_mcp_scaffold_drafts::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![
        MigrationDependencyDescriptor::new(
            "m20260319_000001_create_mcp_management_tables",
            vec![
                "m20250101_000001_create_tenants",
                "m20250101_000002_create_users",
            ],
        ),
        MigrationDependencyDescriptor::new(
            "m20260320_000001_create_mcp_scaffold_drafts",
            vec!["m20260319_000001_create_mcp_management_tables"],
        ),
    ]
}
