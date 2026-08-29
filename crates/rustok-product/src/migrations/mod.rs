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
mod m20260405_000006_add_is_localized_to_product_field_definitions;
mod m20260405_000007_expand_product_locale_storage_columns;
mod m20260409_000007_add_product_seller_id;
mod m20260701_000001_create_product_catalog_attributes;
mod m20260701_000002_add_product_catalog_tenant_consistency_constraints;
mod m20260711_000001_product_status_enum;
mod m20260711_000002_enforce_product_tenant_integrity;
mod m20260711_000003_enforce_catalog_value_invariants;
mod m20260711_000004_normalize_product_channel_visibility;
mod m20260716_000002_add_product_field_cache_generation_trigger;
mod m20260725_000001_remove_product_image_media_foreign_key;
mod m20260725_000002_enforce_catalog_category_tree_invariants;
mod m20260725_000003_remove_transitional_catalog_columns;
mod m20260730_000001_add_product_index_revision;
mod m20260730_000002_add_product_variant_index_revision;
mod m20260731_000003_bump_product_index_revision_for_variant_membership;
mod m20260731_000004_add_product_index_tombstones;
mod m20260806_000005_add_product_index_locale_refresh_ledger;
mod m20260806_000006_add_product_variant_index_refresh_ledger;
mod m20260806_000007_add_product_index_refresh_relay_cursors;
mod m20260807_000008_add_product_sales_channel_index_relation_snapshots;
// Immutable migration history is retained; the next migration removes the
// versioned database objects so the live schema exposes only canonical names.
mod m20260807_000009_add_product_index_graph_v3_projection_snapshots;
mod m20260807_000010_canonicalize_product_index_graph_projection;
mod m20260807_000011_add_product_sales_channel_relation_freshness;
mod m20260807_000012_add_product_sales_channel_relation_convergence;
mod m20260812_000013_normalize_catalog_category_translation_locales;
mod m20260813_000014_canonicalize_product_metadata_tags;
mod m20260828_000015_add_product_taxonomy_category_binding;

use rustok_core::MigrationDependencyDescriptor;
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
        Box::new(m20260405_000006_add_is_localized_to_product_field_definitions::Migration),
        Box::new(m20260405_000007_expand_product_locale_storage_columns::Migration),
        Box::new(m20260409_000007_add_product_seller_id::Migration),
        Box::new(m20260701_000001_create_product_catalog_attributes::Migration),
        Box::new(m20260701_000002_add_product_catalog_tenant_consistency_constraints::Migration),
        Box::new(m20260711_000001_product_status_enum::Migration),
        Box::new(m20260711_000002_enforce_product_tenant_integrity::Migration),
        Box::new(m20260711_000003_enforce_catalog_value_invariants::Migration),
        Box::new(m20260711_000004_normalize_product_channel_visibility::Migration),
        Box::new(m20260716_000002_add_product_field_cache_generation_trigger::Migration),
        Box::new(m20260725_000001_remove_product_image_media_foreign_key::Migration),
        Box::new(m20260725_000002_enforce_catalog_category_tree_invariants::Migration),
        Box::new(m20260725_000003_remove_transitional_catalog_columns::Migration),
        Box::new(m20260730_000001_add_product_index_revision::Migration),
        Box::new(m20260730_000002_add_product_variant_index_revision::Migration),
        Box::new(m20260731_000003_bump_product_index_revision_for_variant_membership::Migration),
        Box::new(m20260731_000004_add_product_index_tombstones::Migration),
        Box::new(m20260806_000005_add_product_index_locale_refresh_ledger::Migration),
        Box::new(m20260806_000006_add_product_variant_index_refresh_ledger::Migration),
        Box::new(m20260806_000007_add_product_index_refresh_relay_cursors::Migration),
        Box::new(m20260807_000008_add_product_sales_channel_index_relation_snapshots::Migration),
        Box::new(m20260807_000009_add_product_index_graph_v3_projection_snapshots::Migration),
        Box::new(m20260807_000010_canonicalize_product_index_graph_projection::Migration),
        Box::new(m20260807_000011_add_product_sales_channel_relation_freshness::Migration),
        Box::new(m20260807_000012_add_product_sales_channel_relation_convergence::Migration),
        Box::new(m20260812_000013_normalize_catalog_category_translation_locales::Migration),
        Box::new(m20260813_000014_canonicalize_product_metadata_tags::Migration),
        Box::new(m20260828_000015_add_product_taxonomy_category_binding::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![
        MigrationDependencyDescriptor::new(
            "m20260329_000001_create_product_tags",
            vec!["m20260329_000001_create_taxonomy_tables"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260711_000002_enforce_product_tenant_integrity",
            vec![
                "m20260329_000001_create_product_tags",
                "m20260711_000001_add_tenant_identity_key",
            ],
        ),
        MigrationDependencyDescriptor::new(
            "m20260716_000002_add_product_field_cache_generation_trigger",
            vec!["m20260716_000000_create_field_definition_cache_generation"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260813_000014_canonicalize_product_metadata_tags",
            vec!["m20260812_000008_add_route_key_registry"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260828_000015_add_product_taxonomy_category_binding",
            vec![
                "m20260701_000002_add_product_catalog_tenant_consistency_constraints",
                "m20260711_000001_add_tenant_identity_key",
            ],
        ),
    ]
}
