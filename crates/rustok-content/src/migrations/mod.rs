mod shared;

mod m20250130_000005_create_nodes;
mod m20250130_000006_create_categories;
mod m20250130_000007_create_tags;
mod m20250130_000008_create_meta;
mod m20250130_000009_create_media;
mod m20260301_000002_alter_nodes_add_soft_delete;
mod m20260311_000001_create_content_orchestration_tables;
mod m20260316_000003_create_node_field_definitions;
mod m20260317_000001_alter_categories_add_updated_at;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20250130_000005_create_nodes::Migration),
        Box::new(m20250130_000006_create_categories::Migration),
        Box::new(m20250130_000007_create_tags::Migration),
        Box::new(m20250130_000008_create_meta::Migration),
        Box::new(m20250130_000009_create_media::Migration),
        Box::new(m20260301_000002_alter_nodes_add_soft_delete::Migration),
        Box::new(m20260311_000001_create_content_orchestration_tables::Migration),
        Box::new(m20260316_000003_create_node_field_definitions::Migration),
        Box::new(m20260317_000001_alter_categories_add_updated_at::Migration),
    ]
}
