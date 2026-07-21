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

    assert!(cargo.contains(
        "commerce-foundation = { package = \"rustok-commerce-foundation\", path = \"../rustok-commerce-foundation\" }"
    ));
    assert!(!cargo.contains("rustok-commerce-foundation.workspace = true"));
}

#[test]
fn remaining_foundation_bridge_is_narrow_and_explicit() {
    let root = include_str!("lib.rs");
    let entities = include_str!("entities/mod.rs");

    assert!(root.contains("pub use commerce_foundation::error::*;"));
    assert!(entities.contains("pub use commerce_foundation::entities::price;"));
    assert!(!entities.contains("inventory_item"));
    assert!(!entities.contains("stock_location"));
}
