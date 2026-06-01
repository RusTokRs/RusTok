from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]


def read(path: str) -> str:
    return (REPO_ROOT / path).read_text(encoding="utf-8")


def test_storefront_mobile_cart_operations_match_commerce_graphql_surface() -> None:
    repo = read(
        "rustok_mobile/apps/rustok_frontend_mobile/lib/data/storefront_catalog_repository.dart"
    )
    mutation = read("crates/rustok-commerce/src/graphql/mutation.rs")
    types = read("crates/rustok-commerce/src/graphql/types.rs")

    expected_operations = {
        "createStorefrontCart": "async fn create_storefront_cart",
        "addStorefrontCartLineItem": "async fn add_storefront_cart_line_item",
        "updateStorefrontCartLineItem": "async fn update_storefront_cart_line_item",
        "removeStorefrontCartLineItem": "async fn remove_storefront_cart_line_item",
    }
    for dart_operation, rust_resolver in expected_operations.items():
        assert dart_operation in repo
        assert rust_resolver in mutation

    expected_inputs = [
        "CreateStorefrontCartInput",
        "AddStorefrontCartLineItemInput",
        "UpdateStorefrontCartLineItemInput",
    ]
    for input_type in expected_inputs:
        assert f"{input_type}!" in repo
        assert f"pub struct {input_type}" in types

    assert "storefrontCart(id: $id)" in repo
    assert "cart_id: Uuid" in mutation
    assert "line_id: Uuid" in mutation


def test_storefront_mobile_cart_transport_does_not_define_flutter_only_api() -> None:
    repo = read(
        "rustok_mobile/apps/rustok_frontend_mobile/lib/data/storefront_catalog_repository.dart"
    )
    context = read(
        "rustok_mobile/apps/rustok_frontend_mobile/lib/app_shell/storefront_context.dart"
    )

    assert "/api/flutter" not in repo
    assert "/api/mobile" not in repo
    assert "GraphQlStorefrontCatalogRepository" in repo
    assert "GraphQlClientFactory().create" in context
