use std::fs;
use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_source(path: impl AsRef<Path>) -> String {
    let path = manifest_dir().join(path);
    fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "failed to read inventory admin boundary source {}: {error}",
            path.display()
        )
    })
}

#[test]
fn graphql_runtime_details_stay_inside_transport_adapter() {
    let forbidden_markers = [
        "leptos_graphql",
        "GraphqlRequest",
        "GraphqlHttpError",
        "execute_graphql",
        "/api/graphql",
        "RUSTOK_GRAPHQL_URL",
    ];

    for source_path in [
        "src/api.rs",
        "src/core.rs",
        "src/model.rs",
        "src/native.rs",
        "src/ui/leptos.rs",
        "src/ui/mod.rs",
    ] {
        let source = read_source(source_path);
        for marker in forbidden_markers {
            assert!(
                !source.contains(marker),
                "{} must not depend on GraphQL runtime detail `{}`; keep it in src/transport.rs",
                source_path,
                marker
            );
        }
    }

    let transport = read_source("src/transport.rs");
    for marker in forbidden_markers {
        assert!(
            transport.contains(marker),
            "src/transport.rs should own transitional GraphQL runtime detail `{}`",
            marker
        );
    }
}

#[test]
fn package_root_exports_ui_only_without_exposing_transport_adapter() {
    let lib = read_source("src/lib.rs");

    assert!(
        lib.contains("pub use ui::leptos::InventoryAdmin;"),
        "crate root should keep exporting the Leptos inventory admin entry point"
    );

    for forbidden_export in [
        "pub mod transport",
        "pub use transport",
        "pub mod core",
        "pub use core",
        "pub mod native",
        "pub use native",
    ] {
        assert!(
            !lib.contains(forbidden_export),
            "crate root must not publicly expose inventory admin implementation boundary `{}`",
            forbidden_export
        );
    }
}

#[test]
fn native_read_path_targets_inventory_backend_service() {
    let native = read_source("src/native.rs");

    for marker in [
        "#[server(prefix = \"/api/fn\", endpoint = \"inventory/bootstrap\")]",
        "#[server(prefix = \"/api/fn\", endpoint = \"inventory/products\")]",
        "#[server(prefix = \"/api/fn\", endpoint = \"inventory/product\")]",
        "AdminInventoryReadService::new",
        "assert_requested_tenant",
        "Permission::INVENTORY_LIST",
        "Permission::INVENTORY_READ",
    ] {
        assert!(
            native.contains(marker),
            "src/native.rs must keep native inventory read path marker `{}`",
            marker
        );
    }
}

#[test]
fn native_write_path_targets_inventory_service() {
    let native = read_source("src/native.rs");

    for marker in [
        r#"#[server(prefix = "/api/fn", endpoint = "inventory/variant/set-quantity")]"#,
        r#"#[server(prefix = "/api/fn", endpoint = "inventory/variant/adjust-quantity")]"#,
        r#"#[server(prefix = "/api/fn", endpoint = "inventory/variant/reserve-quantity")]"#,
        r#"#[server(prefix = "/api/fn", endpoint = "inventory/variant/check-availability")]"#,
        "INVENTORY_SET_QUANTITY_REQUIRES_SSR_ERROR",
        "INVENTORY_ADJUST_QUANTITY_REQUIRES_SSR_ERROR",
        "INVENTORY_RESERVE_QUANTITY_REQUIRES_SSR_ERROR",
        "INVENTORY_CHECK_AVAILABILITY_REQUIRES_SSR_ERROR",
        "transactional_event_bus_from_context",
        "assert_requested_tenant",
        "Permission::INVENTORY_UPDATE",
        "Permission::INVENTORY_MANAGE",
        "InventoryService::new",
        "set_variant_quantity",
        "adjust_variant_quantity",
        "reserve_variant_quantity",
        "check_variant_availability",
        "InventoryQuantityWriteResult",
        "InventoryReservationWriteResult",
        "InventoryAvailabilityCheckResult",
        "map_availability_result",
        "in_stock: result.in_stock",
    ] {
        assert!(
            native.contains(marker),
            "src/native.rs must keep native inventory write path marker `{}`",
            marker
        );
    }
}

#[test]
fn native_write_facades_stay_native_without_graphql_fallback() {
    let api = read_source("src/api.rs");

    for (function_name, required_markers) in [
        (
            "set_variant_quantity",
            [
                "set_quantity_request",
                "crate::native::set_variant_quantity",
            ],
        ),
        (
            "adjust_variant_quantity",
            [
                "adjust_quantity_request",
                "crate::native::adjust_variant_quantity",
            ],
        ),
        (
            "reserve_variant_quantity",
            [
                "reserve_quantity_request",
                "crate::native::reserve_variant_quantity",
            ],
        ),
        (
            "check_variant_availability",
            [
                "availability_check_request",
                "crate::native::check_variant_availability",
            ],
        ),
    ] {
        let start = api
            .find(&format!("pub async fn {function_name}"))
            .unwrap_or_else(|| panic!("src/api.rs should expose {} facade", function_name));
        let end = api[start..]
            .find("\npub async fn ")
            .map(|offset| start + offset)
            .or_else(|| {
                api[start..]
                    .find("#[cfg(test)]")
                    .map(|offset| start + offset)
            })
            .unwrap_or(api.len());
        let write_facade = &api[start..end];

        for required in required_markers {
            assert!(
                write_facade.contains(required),
                "{function_name} facade must keep native write marker `{}`",
                required
            );
        }
        assert!(
            write_facade.contains(".map_err(Into::into)"),
            "{} facade must map native server errors through ApiError",
            function_name
        );

        for forbidden in [
            "fallback_",
            "transitional_read_transport",
            "CommerceGraphqlInventoryReadAdapter",
            "token",
            "tenant_slug",
        ] {
            assert!(
                !write_facade.contains(forbidden),
                "{function_name} facade must not use transitional read/fallback marker `{}`",
                forbidden
            );
        }
    }
}

#[test]
fn ui_stock_quantity_controls_use_inventory_api_facade_only() {
    let ui = read_source("src/ui/leptos.rs");

    for required in [
        "parse_set_quantity",
        "crate::api::set_variant_quantity",
        "crate::api::adjust_variant_quantity",
        "crate::api::reserve_variant_quantity",
        "crate::api::check_variant_availability",
        "inventory.action.checkAvailability",
        "inventory.notice.available",
        "inventory.notice.unavailable",
        "apply_variant_quantity_update",
        "apply_variant_reservation_update",
        "set_quantity_input.set(result.quantity.to_string())",
        "set_quantity_input.set(result.available_quantity.to_string())",
    ] {
        assert!(
            ui.contains(required),
            "inventory UI stock quantity controls must keep marker `{}`",
            required
        );
    }

    for forbidden in [
        "crate::native::set_variant_quantity",
        "crate::native::reserve_variant_quantity",
        "crate::native::check_variant_availability",
        "CommerceGraphqlInventoryReadAdapter",
        "transitional_read_transport",
    ] {
        assert!(
            !ui.contains(forbidden),
            "inventory UI must not bypass the API facade or use transitional marker `{}`",
            forbidden
        );
    }
}

#[test]
fn transitional_graphql_adapter_is_read_only_with_documented_removal_criteria() {
    let transport = read_source("src/transport.rs");
    let readme = read_source("README.md");

    for marker in [
        "const BOOTSTRAP_QUERY",
        "const PRODUCTS_QUERY",
        "const PRODUCT_QUERY",
    ] {
        assert!(
            transport.contains(marker),
            "transitional GraphQL adapter must keep read-only query marker `{marker}`"
        );
    }

    for forbidden in [
        "mutation ",
        "Mutation",
        "setQuantity",
        "adjustQuantity",
        "setVariantQuantity",
        "adjustVariantQuantity",
        "reserveVariantQuantity",
        "checkVariantAvailability",
    ] {
        assert!(
            !transport.contains(forbidden),
            "transitional GraphQL adapter must remain read-only and not contain `{forbidden}`"
        );
    }

    for marker in [
        "Transitional adapter removal criteria",
        "limited to native-unavailable fallback",
        "no GraphQL fallback",
        "remaining dedicated write transport",
    ] {
        assert!(
            readme.contains(marker),
            "admin README must document transitional adapter removal marker `{marker}`"
        );
    }
}

#[test]
fn native_read_mapper_and_transitional_adapter_keep_read_model_parity() {
    let model = read_source("src/model.rs");
    let native = read_source("src/native.rs");
    let transport = read_source("src/transport.rs");
    let backend = read_source("../src/services/admin_read.rs");

    for (model_marker, backend_marker, native_marker, transport_marker) in [
        (
            "pub per_page: u64",
            "pub per_page: u64",
            "per_page: value.per_page",
            "perPage",
        ),
        (
            "pub has_next: bool",
            "pub has_next: bool",
            "has_next: value.has_next",
            "hasNext",
        ),
        (
            "pub shipping_profile_slug: Option<String>",
            "pub shipping_profile_slug: Option<String>",
            "shipping_profile_slug: value.shipping_profile_slug",
            "shippingProfileSlug",
        ),
        (
            "pub translations: Vec<InventoryProductTranslation>",
            "pub translations: Vec<AdminInventoryProductTranslation>",
            "translations: value",
            "translations",
        ),
        (
            "pub variants: Vec<InventoryVariant>",
            "pub variants: Vec<AdminInventoryVariant>",
            "variants: value.variants.into_iter().map(map_variant).collect()",
            "variants",
        ),
        (
            "pub prices: Vec<InventoryPrice>",
            "pub prices: Vec<AdminInventoryPrice>",
            "prices: value",
            "prices",
        ),
        (
            "pub inventory_quantity: i32",
            "pub inventory_quantity: i32",
            "inventory_quantity: value.inventory_quantity",
            "inventoryQuantity",
        ),
        (
            "pub inventory_policy: String",
            "pub inventory_policy: String",
            "inventory_policy: value.inventory_policy",
            "inventoryPolicy",
        ),
        (
            "pub in_stock: bool",
            "pub in_stock: bool",
            "in_stock: value.in_stock",
            "inStock",
        ),
        (
            "pub currency_code: String",
            "pub currency_code: String",
            "currency_code: price.currency_code",
            "currencyCode",
        ),
        (
            "pub compare_at_amount: Option<String>",
            "pub compare_at_amount: Option<String>",
            "compare_at_amount: price.compare_at_amount",
            "compareAtAmount",
        ),
        (
            "pub on_sale: bool",
            "pub on_sale: bool",
            "on_sale: price.on_sale",
            "onSale",
        ),
    ] {
        assert!(
            model.contains(model_marker),
            "admin read model must keep field marker `{model_marker}`"
        );
        assert!(
            backend.contains(backend_marker),
            "backend AdminInventoryReadService DTO must keep field marker `{backend_marker}`"
        );
        assert!(
            native.contains(native_marker),
            "native read mapper must keep DTO-to-admin-model marker `{native_marker}`"
        );
        assert!(
            transport.contains(transport_marker),
            "transitional GraphQL adapter must keep read-model marker `{transport_marker}`"
        );
    }
}

#[test]
fn native_write_path_returns_quantity_contract_not_bare_integer() {
    let model = read_source("src/model.rs");
    let native = read_source("src/native.rs");
    let api = read_source("src/api.rs");
    let core = read_source("src/core.rs");
    let ui = read_source("src/ui/leptos.rs");
    let backend = read_source("../src/services/inventory.rs");
    let lib = read_source("../src/lib.rs");

    for marker in [
        "pub struct InventoryQuantityWriteResult",
        "pub struct InventoryReservationWriteResult",
        "pub struct InventoryAvailabilityCheckResult",
        "pub quantity: i32",
        "pub reserved_quantity: i32",
        "pub available_quantity: i32",
        "pub in_stock: bool",
        "pub available: bool",
        r#"#[serde(rename = "inStock")]"#,
    ] {
        assert!(
            model.contains(marker),
            "admin write result model must keep marker `{marker}`"
        );
        assert!(
            backend.contains(marker),
            "backend inventory write result contract must keep marker `{marker}`"
        );
    }

    for exported in [
        "InventoryQuantityWriteResult",
        "InventoryReservationWriteResult",
        "InventoryAvailabilityCheckResult",
    ] {
        assert!(
            lib.contains(exported),
            "rustok-inventory crate root must export the inventory write result contract `{exported}`"
        );
    }

    for source in [&native, &api] {
        assert!(
            source.contains("Result<InventoryQuantityWriteResult"),
            "native/API quantity write path must return InventoryQuantityWriteResult instead of a bare i32"
        );
        assert!(
            source.contains("Result<InventoryReservationWriteResult"),
            "native/API reservation write path must return InventoryReservationWriteResult instead of a bare unit"
        );
        assert!(
            source.contains("Result<InventoryAvailabilityCheckResult"),
            "native/API availability check path must return InventoryAvailabilityCheckResult instead of a bare bool"
        );
    }

    for marker in [
        "set_variant_quantity",
        "adjust_variant_quantity",
        "reserve_variant_quantity",
        "check_variant_availability",
        "map_reservation_result",
        "map_availability_result",
        "in_stock: result.in_stock",
    ] {
        assert!(
            native.contains(marker),
            "native write path must keep inventory-owned write result marker `{marker}`"
        );
    }

    assert!(
        core.contains("result: InventoryQuantityWriteResult"),
        "core optimistic update must consume the inventory-owned write result contract"
    );
    assert!(
        ui.contains("set_quantity_input.set(result.quantity.to_string())"),
        "UI must refresh the quantity input from the write result contract"
    );
    assert!(
        ui.contains("apply_variant_quantity_update(detail, variant_id.as_str(), result.clone())"),
        "UI optimistic detail refresh must apply the full quantity write result contract"
    );
    assert!(
        core.contains("result: InventoryReservationWriteResult"),
        "core optimistic update must consume the inventory-owned reservation result contract"
    );
    assert!(
        ui.contains(
            "apply_variant_reservation_update(detail, variant_id.as_str(), result.clone())"
        ),
        "UI optimistic detail refresh must apply the full reservation write result contract"
    );
    assert!(
        ui.contains("set_quantity_input.set(result.available_quantity.to_string())"),
        "UI must refresh the quantity input from the reservation available quantity contract"
    );
    assert!(
        ui.contains("crate::api::check_variant_availability"),
        "UI availability validation must go through the inventory-owned API facade"
    );
    assert!(
        ui.contains("if result.available"),
        "UI availability validation must consume the typed availability result contract"
    );
}
