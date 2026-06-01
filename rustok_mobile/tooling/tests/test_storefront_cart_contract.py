from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]


def read(path: str) -> str:
    return (REPO_ROOT / path).read_text(encoding="utf-8")


def test_storefront_cart_writes_use_host_cart_id_store() -> None:
    context = read("rustok_mobile/apps/rustok_frontend_mobile/lib/app_shell/storefront_context.dart")
    repo = read("rustok_mobile/apps/rustok_frontend_mobile/lib/data/storefront_catalog_repository.dart")

    assert "abstract interface class StorefrontCartIdStore" in context
    assert "final StorefrontCartIdStore _cartIdStore" in repo
    assert "_cartIdStore.write(id)" in repo
    assert "String? _activeCartId" not in repo


def test_storefront_catalog_package_does_not_fallback_product_id_as_variant_id() -> None:
    product = read("rustok_mobile/packages/rustok_catalog_mobile/lib/src/product_summary.dart")
    screens = read("rustok_mobile/packages/rustok_catalog_mobile/lib/src/catalog_screens.dart")

    assert "String get cartVariantId => variantId ?? id" not in product
    assert "bool get canAddToCart" in product
    assert "StorefrontAddCartLineDraft(variantId: variantId)" in screens
    assert "StorefrontAddCartLineDraft(variantId: product.id)" not in screens
