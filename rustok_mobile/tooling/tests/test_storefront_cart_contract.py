from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]


def read(path: str) -> str:
    return (REPO_ROOT / path).read_text(encoding="utf-8")


def test_storefront_cart_writes_use_host_cart_id_store() -> None:
    context = read("rustok_mobile/apps/rustok_frontend_mobile/lib/app_shell/storefront_context.dart")
    repo = read("rustok_mobile/apps/rustok_frontend_mobile/lib/data/storefront_catalog_repository.dart")

    assert "abstract interface class StorefrontCartIdStore" in context
    assert "abstract interface class StorefrontCartIdPersistence" in context
    assert "class DurableStorefrontCartIdStore implements StorefrontCartIdStore" in context
    assert "class FileStorefrontCartIdPersistence implements StorefrontCartIdPersistence" in context
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


def test_storefront_checkout_intent_stays_host_owned() -> None:
    router = read("rustok_mobile/apps/rustok_frontend_mobile/lib/routes/storefront_router.dart")
    package_screens = read("rustok_mobile/packages/rustok_catalog_mobile/lib/src/catalog_screens.dart")

    context = read("rustok_mobile/apps/rustok_frontend_mobile/lib/app_shell/storefront_context.dart")

    assert "class StorefrontCheckoutIntentPage extends ConsumerWidget" in router
    assert "buildStorefrontCheckoutIntentViewModel" in router
    assert "storefrontCartIdStoreProvider" in router
    assert "class StorefrontCheckoutIntentViewModel" in context
    assert "tenant: ${runtime.tenantSlug} · locale: ${runtime.locale}" in context
    assert "Checkout remains host-owned" in context
    assert "context.go(checkoutPath)" in router
    assert "/api/flutter" not in router
    assert "/api/mobile" not in router
    assert "StorefrontCheckoutIntentPage" not in package_screens


def test_storefront_checkout_policy_is_host_view_model_not_widget_branching() -> None:
    context = read("rustok_mobile/apps/rustok_frontend_mobile/lib/app_shell/storefront_context.dart")
    router = read("rustok_mobile/apps/rustok_frontend_mobile/lib/routes/storefront_router.dart")

    assert "StorefrontCheckoutIntentViewModel buildStorefrontCheckoutIntentViewModel" in context
    assert "canContinueCheckout: normalizedCartId != null" in context
    assert "cart: not started yet — return to cart before checkout." in context
    assert "cartId.trim().isNotEmpty" not in router
    assert "cart: $cartId" not in router
