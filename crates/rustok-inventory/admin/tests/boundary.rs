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
        "#[server(prefix = \"/api/fn\", endpoint = \"inventory/variant/set-quantity\")]",
        "AdminInventoryReadService::new",
        "assert_requested_tenant",
        "Permission::INVENTORY_LIST",
        "Permission::INVENTORY_READ",
        "Permission::INVENTORY_UPDATE",
        "InventoryService::new",
        "set_inventory",
    ] {
        assert!(
            native.contains(marker),
            "src/native.rs must keep native inventory read path marker `{}`",
            marker
        );
    }
}

#[test]
fn set_quantity_write_path_stays_native_without_graphql_fallback() {
    let api = read_source("src/api.rs");
    let start = api
        .find("pub async fn set_variant_quantity")
        .expect("src/api.rs should expose set_variant_quantity facade");
    let end = api[start..]
        .find("#[cfg(test)]")
        .map(|offset| start + offset)
        .unwrap_or(api.len());
    let write_facade = &api[start..end];

    for required in [
        "set_quantity_request",
        "crate::native::set_variant_quantity",
        ".map_err(Into::into)",
    ] {
        assert!(
            write_facade.contains(required),
            "set_variant_quantity facade must keep native write marker `{}`",
            required
        );
    }

    for forbidden in [
        "fallback_",
        "transitional_read_transport",
        "CommerceGraphqlInventoryReadAdapter",
        "token",
        "tenant_slug",
    ] {
        assert!(
            !write_facade.contains(forbidden),
            "set_variant_quantity facade must not use transitional read/fallback marker `{}`",
            forbidden
        );
    }
}

#[test]
fn ui_set_quantity_control_uses_inventory_api_facade_only() {
    let ui = read_source("src/ui/leptos.rs");

    for required in [
        "parse_set_quantity",
        "crate::api::set_variant_quantity",
        "apply_variant_quantity_update",
        "set_quantity_input.set(new_quantity.to_string())",
    ] {
        assert!(
            ui.contains(required),
            "inventory UI set-quantity control must keep marker `{}`",
            required
        );
    }

    for forbidden in [
        "crate::native::set_variant_quantity",
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
