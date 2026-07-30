#[test]
fn product_declares_owner_local_dto_and_entity_sources() {
    let root = include_str!("lib.rs");
    let entities = include_str!("entities/mod.rs");
    let cargo = include_str!("../Cargo.toml");

    assert!(root.contains("pub mod dto;"));
    assert!(root.contains("pub mod entities;"));
    assert!(!root.contains("pub use rustok_commerce_foundation::dto::*"));
    assert!(!entities.contains("pub use rustok_commerce_foundation::entities::{"));

    for source in [
        "dto/product.rs",
        "dto/variant.rs",
        "entities/product.rs",
        "entities/product_translation.rs",
        "entities/product_variant.rs",
        "entities/variant_translation.rs",
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join(source);
        assert!(path.is_file(), "Product owner source is missing: {source}");
    }

    assert!(!cargo.contains("rustok-commerce-foundation"));
    assert!(cargo.contains("rustok-pricing-persistence.workspace = true"));
    assert!(!cargo.contains("rustok-commerce-foundation.workspace = true"));
}

#[test]
fn product_owner_boundaries_do_not_depend_on_foundation() {
    let root = include_str!("lib.rs");
    let entities = include_str!("entities/mod.rs");
    let error = include_str!("error.rs");

    assert!(root.contains("pub mod error;"));
    assert!(error.contains("pub enum CommerceError"));
    assert!(!error.contains("commerce_foundation"));
    assert!(!entities.contains("price"));
    assert!(!entities.contains("inventory_item"));
    assert!(!entities.contains("stock_location"));
}

#[cfg(feature = "index")]
#[test]
fn product_publishes_index_schema_and_postgres_source_factory() {
    use rustok_core::{ModuleRuntimeExtensions, RusToKModule};

    let mut extensions = ModuleRuntimeExtensions::default();
    crate::ProductModule
        .register_runtime_extensions(&mut extensions)
        .expect("Product Index contracts should register");

    let schema = crate::product_index_schema().expect("Product Index schema");
    let schemas = extensions
        .get::<rustok_index::IndexSchemaSourceCatalog>()
        .expect("schema source catalog");
    let descriptor = schemas
        .get(&schema.reference)
        .expect("Product schema descriptor");
    assert_eq!(descriptor.owner_module, "product");
    assert_eq!(descriptor.schema, schema);

    let factories = extensions
        .get::<rustok_index::PostgresIndexSourceFactoryCatalog>()
        .expect("PostgreSQL source factory catalog");
    assert_eq!(factories.len(), 1);
    let factory = factories.iter().next().expect("Product source factory");
    assert_eq!(factory.owner_module(), "product");
    assert_eq!(factory.factory_name(), crate::PRODUCT_INDEX_SOURCE_FACTORY);
}
