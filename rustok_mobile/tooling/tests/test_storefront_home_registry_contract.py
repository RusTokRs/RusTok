from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]


def read(path: str) -> str:
    return (REPO_ROOT / path).read_text(encoding="utf-8")


def test_storefront_home_uses_generated_registry_for_module_links() -> None:
    router = read("rustok_mobile/apps/rustok_frontend_mobile/lib/routes/storefront_router.dart")

    assert "const _StorefrontModuleLinksCard(registry: storefrontSurfaceRegistry)" in router
    assert "for (final entry in entries)" in router
    assert "context.go(" in router
    assert "'$storefrontModulesRootPath/${entry.routeSegment}'" in router
    assert "path: '$storefrontModulesRootPath/blog'" not in router


def test_storefront_home_distinguishes_package_and_manifest_backed_surfaces() -> None:
    router = read("rustok_mobile/apps/rustok_frontend_mobile/lib/routes/storefront_router.dart")

    assert "StorefrontMountedSurfaceKind.catalog => 'package'" in router
    assert "StorefrontMountedSurfaceKind.cart => 'package'" in router
    assert "StorefrontMountedSurfaceKind.generic => 'manifest'" in router
