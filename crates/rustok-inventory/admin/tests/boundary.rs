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
fn graphql_runtime_details_are_removed_from_inventory_admin_package() {
    let forbidden_markers = [
        "rustok_graphql",
        "GraphqlRequest",
        "GraphqlHttpError",
        "execute_graphql",
        "/api/graphql",
        "RUSTOK_GRAPHQL_URL",
        "CommerceGraphqlInventoryReadAdapter",
        "removed_graphql_read_transport",
        "graphql_selected_path_",
    ];

    for source_path in [
        "src/core.rs",
        "src/model.rs",
        "src/transport/mod.rs",
        "src/transport/native_server_adapter.rs",
        "src/ui/leptos.rs",
        "src/ui/mod.rs",
        "src/lib.rs",
        "Cargo.toml",
    ] {
        let source = read_source(source_path);
        for marker in forbidden_markers {
            assert!(
                !source.contains(marker),
                "{} must not depend on removed GraphQL fallback detail `{}`",
                source_path,
                marker
            );
        }
    }

    assert!(
        !manifest_dir().join("src/transport.rs").exists(),
        "src/transport.rs must stay removed after native-only read parity replaced the GraphQL adapter"
    );
    assert!(
        !manifest_dir().join("src/api.rs").exists(),
        "src/api.rs must stay removed after FFA transport facade split"
    );
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
    let native = read_source("src/transport/native_server_adapter.rs");

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
            "src/transport/native_server_adapter.rs must keep native inventory read path marker `{}`",
            marker
        );
    }
}

#[test]
fn native_write_path_targets_inventory_service() {
    let native = read_source("src/transport/native_server_adapter.rs");

    for marker in [
        r#"#[server(prefix = "/api/fn", endpoint = "inventory/variant/set-quantity")]"#,
        r#"#[server(prefix = "/api/fn", endpoint = "inventory/variant/adjust-quantity")]"#,
        r#"#[server(prefix = "/api/fn", endpoint = "inventory/variant/reserve-quantity")]"#,
        r#"#[server(prefix = "/api/fn", endpoint = "inventory/variant/check-availability")]"#,
        r#"#[server(prefix = "/api/fn", endpoint = "inventory/variant/release-reservation")]"#,
        "inventory/variant/set-quantity requires the `ssr` feature",
        "inventory/variant/adjust-quantity requires the `ssr` feature",
        "inventory/variant/reserve-quantity requires the `ssr` feature",
        "inventory/variant/check-availability requires the `ssr` feature",
        "inventory/variant/release-reservation requires the `ssr` feature",
        "shared_get::<rustok_outbox::TransactionalEventBus>()",
        "inventory/admin native transport requires TransactionalEventBus in host runtime context",
        "assert_requested_tenant",
        "Permission::INVENTORY_UPDATE",
        "Permission::INVENTORY_MANAGE",
        "InventoryService::new",
        "set_variant_quantity",
        "adjust_variant_quantity",
        "reserve_variant_quantity",
        "check_variant_availability",
        "release_reservation_quantity",
        "InventoryQuantityWriteResult",
        "InventoryReservationWriteResult",
        "InventoryAvailabilityCheckResult",
        "InventoryReservationReleaseWriteResult",
        "map_availability_result",
        "map_release_result",
        "in_stock: result.in_stock",
    ] {
        assert!(
            native.contains(marker),
            "src/transport/native_server_adapter.rs must keep native inventory write path marker `{}`",
            marker
        );
    }
}

#[test]
fn native_write_facades_stay_native_without_graphql_fallback() {
    let transport = read_source("src/transport/mod.rs");

    for (function_name, required_markers) in [
        (
            "set_variant_quantity",
            [
                "set_quantity_request",
                "native_server_adapter::set_variant_quantity",
            ],
        ),
        (
            "adjust_variant_quantity",
            [
                "adjust_quantity_request",
                "native_server_adapter::adjust_variant_quantity",
            ],
        ),
        (
            "reserve_variant_quantity",
            [
                "reserve_quantity_request",
                "native_server_adapter::reserve_variant_quantity",
            ],
        ),
        (
            "check_variant_availability",
            [
                "availability_check_request",
                "native_server_adapter::check_variant_availability",
            ],
        ),
        (
            "release_reservation_quantity",
            [
                "release_reservation_request",
                "native_server_adapter::release_reservation_quantity",
            ],
        ),
    ] {
        let start = transport
            .find(&format!("pub async fn {function_name}"))
            .unwrap_or_else(|| {
                panic!(
                    "src/transport/mod.rs should expose {} facade",
                    function_name
                )
            });
        let end = transport[start..]
            .find("\npub async fn ")
            .map(|offset| start + offset)
            .or_else(|| {
                transport[start..]
                    .find("#[cfg(test)]")
                    .map(|offset| start + offset)
            })
            .unwrap_or(transport.len());
        let write_facade = &transport[start..end];

        for required in required_markers {
            assert!(
                write_facade.contains(required),
                "{function_name} facade must keep native write marker `{}`",
                required
            );
        }
        assert!(
            write_facade.contains(".map_err(Into::into)"),
            "{} facade must map native server errors through InventoryTransportError",
            function_name
        );

        for forbidden in [
            "graphql_selected_path_",
            "removed_graphql_read_transport",
            "CommerceGraphqlInventoryReadAdapter",
            "token",
            "tenant_slug",
        ] {
            assert!(
                !write_facade.contains(forbidden),
                "{function_name} facade must not use removed read/fallback marker `{}`",
                forbidden
            );
        }
    }
}

#[test]
fn ui_stock_quantity_controls_use_inventory_transport_facade_only() {
    let ui = read_source("src/ui/leptos.rs");

    for required in [
        "parse_set_quantity",
        "parse_availability_quantity",
        "crate::transport::set_variant_quantity",
        "crate::transport::adjust_variant_quantity",
        "crate::transport::reserve_variant_quantity",
        "crate::transport::release_reservation_quantity",
        "crate::transport::check_variant_availability",
        "inventory.action.checkAvailability",
        "inventory.error.invalidAvailabilityQuantity",
        "inventory.error.invalidReservationQuantity",
        "inventory.action.releaseReservation",
        "inventory.notice.releasedReservation",
        "inventory.notice.available",
        "inventory.notice.unavailable",
        "apply_variant_quantity_update",
        "apply_variant_reservation_update",
        "apply_variant_reservation_release_update",
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
        "native_server_adapter::set_variant_quantity",
        "native_server_adapter::reserve_variant_quantity",
        "native_server_adapter::check_variant_availability",
        "native_server_adapter::release_reservation_quantity",
        "CommerceGraphqlInventoryReadAdapter",
        "removed_graphql_read_transport",
    ] {
        assert!(
            !ui.contains(forbidden),
            "inventory UI must not bypass the transport facade or use removed fallback marker `{}`",
            forbidden
        );
    }
}

#[test]
fn native_only_graphql_adapter_removal_is_documented() {
    let readme = read_source("README.md");

    for marker in [
        "Native-only transport status",
        "`CommerceGraphqlInventoryReadAdapter` has been removed",
        "No GraphQL fallback remains",
    ] {
        assert!(
            readme.contains(marker),
            "admin README must document native-only transport marker `{marker}`"
        );
    }

    for forbidden in [
        "Transitional adapter removal criteria",
        "limited to native-unavailable fallback",
        "remaining dedicated write transport",
    ] {
        assert!(
            !readme.contains(forbidden),
            "admin README must not keep removed GraphQL adapter marker `{forbidden}`"
        );
    }
}

#[test]
fn native_read_mapper_keeps_backend_read_model_parity() {
    let model = read_source("src/model.rs");
    let native = read_source("src/transport/native_server_adapter.rs");
    let backend = read_source("../src/services/admin_read.rs");

    for (model_marker, backend_marker, native_marker) in [
        (
            "pub per_page: u64",
            "pub per_page: u64",
            "per_page: value.per_page",
        ),
        (
            "pub has_next: bool",
            "pub has_next: bool",
            "has_next: value.has_next",
        ),
        (
            "pub shipping_profile_slug: Option<String>",
            "pub shipping_profile_slug: Option<String>",
            "shipping_profile_slug: value.shipping_profile_slug",
        ),
        (
            "pub translations: Vec<InventoryProductTranslation>",
            "pub translations: Vec<AdminInventoryProductTranslation>",
            "translations: value",
        ),
        (
            "pub variants: Vec<InventoryVariant>",
            "pub variants: Vec<AdminInventoryVariant>",
            "variants: value.variants.into_iter().map(map_variant).collect()",
        ),
        (
            "pub prices: Vec<InventoryPrice>",
            "pub prices: Vec<AdminInventoryPrice>",
            "prices: value",
        ),
        (
            "pub inventory_quantity: i32",
            "pub inventory_quantity: i32",
            "inventory_quantity: value.inventory_quantity",
        ),
        (
            "pub inventory_policy: String",
            "pub inventory_policy: String",
            "inventory_policy: value.inventory_policy",
        ),
        (
            "pub in_stock: bool",
            "pub in_stock: bool",
            "in_stock: value.in_stock",
        ),
        (
            "pub currency_code: String",
            "pub currency_code: String",
            "currency_code: price.currency_code",
        ),
        (
            "pub compare_at_amount: Option<String>",
            "pub compare_at_amount: Option<String>",
            "compare_at_amount: price.compare_at_amount",
        ),
        (
            "pub on_sale: bool",
            "pub on_sale: bool",
            "on_sale: price.on_sale",
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
    }
}

#[test]
fn native_write_path_returns_quantity_contract_not_bare_integer() {
    let model = read_source("src/model.rs");
    let native = read_source("src/transport/native_server_adapter.rs");
    let transport = read_source("src/transport/mod.rs");
    let core = read_source("src/core.rs");
    let ui = read_source("src/ui/leptos.rs");
    let backend = read_source("../src/services/inventory.rs");
    let lib = read_source("../src/lib.rs");

    for marker in [
        "pub struct InventoryQuantityWriteResult",
        "pub struct InventoryReservationWriteResult",
        "pub struct InventoryAvailabilityCheckResult",
        "pub struct InventoryReservationReleaseWriteResult",
        "pub quantity: i32",
        "pub reserved_quantity: i32",
        "pub available_quantity: i32",
        "pub in_stock: bool",
        "pub available: bool",
        "pub released_quantity: i32",
        r#"#[serde(rename = "releasedQuantity")]"#,
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
        "InventoryReservationReleaseWriteResult",
    ] {
        assert!(
            lib.contains(exported),
            "rustok-inventory crate root must export the inventory write result contract `{exported}`"
        );
    }

    for source in [&native, &transport] {
        assert!(
            source.contains("Result<InventoryQuantityWriteResult"),
            "native/transport quantity write path must return InventoryQuantityWriteResult instead of a bare i32"
        );
        assert!(
            source.contains("Result<InventoryReservationWriteResult"),
            "native/transport reservation write path must return InventoryReservationWriteResult instead of a bare unit"
        );
        assert!(
            source.contains("Result<InventoryAvailabilityCheckResult"),
            "native/transport availability check path must return InventoryAvailabilityCheckResult instead of a bare bool"
        );
        assert!(
            source.contains("Result<InventoryReservationReleaseWriteResult"),
            "native/transport reservation release path must return InventoryReservationReleaseWriteResult instead of a bare unit"
        );
    }

    for marker in [
        "set_variant_quantity",
        "adjust_variant_quantity",
        "reserve_variant_quantity",
        "check_variant_availability",
        "release_reservation_quantity",
        "map_reservation_result",
        "map_availability_result",
        "map_release_result",
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
        ui.contains("crate::transport::release_reservation_quantity"),
        "UI reservation release must go through the inventory-owned transport facade"
    );
    assert!(
        ui.contains(
            "apply_variant_reservation_release_update(detail, variant_id.as_str(), result.clone())"
        ),
        "UI optimistic detail refresh must apply the full reservation release result contract"
    );
    assert!(
        ui.contains("result.released_quantity"),
        "UI reservation release must consume the typed released quantity result contract"
    );
    assert!(
        ui.contains("crate::transport::check_variant_availability"),
        "UI availability validation must go through the inventory-owned transport facade"
    );
    assert!(
        ui.contains("if result.available"),
        "UI availability validation must consume the typed availability result contract"
    );
}
