import pathlib
import tempfile
import textwrap
import unittest

from rustok_mobile.tooling.scripts.generate_mobile_manifest import (
    render,
    render_snapshot_json,
    scan_modules,
)


def write_module_manifest(root: pathlib.Path, crate: str, manifest: str) -> None:
    manifest_path = root / f"crates/{crate}/rustok-module.toml"
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(textwrap.dedent(manifest).strip(), encoding="utf-8")


class GenerateMobileManifestTests(unittest.TestCase):
    def test_scan_modules_filters_and_sorts(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            write_module_manifest(
                root,
                "mod-a",
                """
                [module]
                slug = "auth"
                name = "Auth"

                [provides.admin_ui]
                route_segment = "Auth"
                nav_label = "Auth"

                [[provides.admin_ui.child_pages]]
                subpath = "Users"
                title = "Users"
                """,
            )
            write_module_manifest(
                root,
                "mod-b",
                """
                [module]
                slug = "blog"
                name = "Blog"

                [provides.admin_ui]
                route_segment = "blog"
                nav_label = "Blog"
                """,
            )
            write_module_manifest(
                root,
                "mod-c",
                """
                [module]
                slug = "service-only"
                """,
            )

            modules = scan_modules(root)
            self.assertEqual([m["route_segment"] for m in modules], ["auth", "blog"])
            self.assertEqual(modules[0]["icon"], "shield")
            self.assertEqual(modules[0]["child_pages"][0]["subpath"], "users")

    def test_render_escapes_quotes(self):
        content = render(
            [
                {
                    "module_key": "rustok_test",
                    "route_segment": "test",
                    "nav_label": "Owner's",
                    "icon": "module",
                }
            ]
        )
        self.assertIn("Owner\\'s", content)

    def test_render_includes_child_pages_and_contract_metadata(self):
        content = render(
            [
                {
                    "module_key": "rustok_test",
                    "route_segment": "test",
                    "nav_label": "Test",
                    "icon": "module",
                    "locale_namespace": "test_module",
                    "permissions": ["test.read"],
                    "child_pages": [
                        {"subpath": "items", "title": "Items", "nav_label": "All items"}
                    ],
                }
            ]
        )
        self.assertIn("localeNamespace: 'test_module'", content)
        self.assertIn("permissions: <String>[", content)
        self.assertIn("'test.read'", content)
        self.assertIn("childPages: <MobileChildPage>[", content)
        self.assertIn("subpath: 'items'", content)
        self.assertIn("navLabel: 'All items'", content)

    def test_render_snapshot_contains_required_ffa_keys(self):
        payload = render_snapshot_json(
            [
                {
                    "module_key": "rustok_blog",
                    "route_segment": "blog",
                    "nav_label": "Blog",
                    "icon": "article",
                    "child_pages": [{"subpath": "posts", "title": "Posts"}],
                }
            ]
        )
        self.assertIn('"module_slug": "blog"', payload)
        self.assertIn('"surface_kind": "admin_mobile"', payload)
        self.assertIn('"route_segment": "blog"', payload)
        self.assertIn('"nav_icon": "article"', payload)
        self.assertIn('"child_pages"', payload)

    def test_scan_modules_reads_storefront_surface(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            write_module_manifest(
                root,
                "mod-product",
                """
                [module]
                slug = "product"
                name = "Product"

                [provides.admin_ui]
                route_segment = "admin-products"
                nav_label = "Admin Products"

                [provides.storefront_ui]
                route_segment = "products"
                page_title = "Products"
                """,
            )

            modules = scan_modules(root, surface="storefront")

            self.assertEqual(len(modules), 1)
            self.assertEqual(modules[0]["surface_kind"], "storefront")
            self.assertEqual(modules[0]["route_segment"], "products")
            self.assertEqual(modules[0]["nav_label"], "Products")

    def test_render_storefront_surface_kind(self):
        content = render(
            [
                {
                    "module_key": "rustok_product",
                    "module_slug": "product",
                    "surface_kind": "storefront",
                    "route_segment": "products",
                    "nav_label": "Products",
                    "icon": "inventory_2",
                    "child_pages": [],
                }
            ]
        )
        self.assertIn("surfaceKind: MobileSurfaceKind.storefront", content)
        payload = render_snapshot_json(
            [
                {
                    "module_key": "rustok_product",
                    "module_slug": "product",
                    "surface_kind": "storefront",
                    "route_segment": "products",
                    "nav_label": "Products",
                    "icon": "inventory_2",
                    "child_pages": [],
                }
            ]
        )
        self.assertIn('"surface_kind": "storefront_mobile"', payload)

    def test_scan_modules_reads_legacy_pages_alias_for_child_pages(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            write_module_manifest(
                root,
                "mod-a",
                """
                [module]
                slug = "legacy"

                [provides.admin_ui]
                route_segment = "legacy"

                [[provides.admin_ui.pages]]
                subpath = "Overview"
                title = "Legacy Overview"
                nav_label = "Overview"
                """,
            )

            modules = scan_modules(root)
            self.assertEqual(
                modules[0]["child_pages"],
                [
                    {
                        "subpath": "overview",
                        "title": "Legacy Overview",
                        "nav_label": "Overview",
                    }
                ],
            )

    def test_scan_modules_prefers_child_pages_over_legacy_pages_alias(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            write_module_manifest(
                root,
                "mod-a",
                """
                [module]
                slug = "canonical"

                [provides.admin_ui]
                route_segment = "canonical"

                [[provides.admin_ui.child_pages]]
                subpath = "current"
                title = "Current Page"

                [[provides.admin_ui.pages]]
                subpath = "legacy"
                title = "Legacy Page"
                """,
            )

            modules = scan_modules(root)
            self.assertEqual(
                modules[0]["child_pages"],
                [{"subpath": "current", "title": "Current Page"}],
            )

    def test_scan_modules_raises_on_duplicate_route_segment(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            write_module_manifest(
                root,
                "mod-a",
                """
                [module]
                slug = "blog"

                [provides.admin_ui]
                route_segment = "content"
                """,
            )
            write_module_manifest(
                root,
                "mod-b",
                """
                [module]
                slug = "forum"

                [provides.admin_ui]
                route_segment = "content"
                """,
            )

            with self.assertRaises(ValueError) as ctx:
                scan_modules(root)
            self.assertIn("content", str(ctx.exception))
            self.assertIn("already declared in", str(ctx.exception))

    def test_scan_modules_includes_permissions_and_locale_namespace(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            write_module_manifest(
                root,
                "mod-a",
                """
                [module]
                slug = "blog"

                [provides.admin_ui]
                route_segment = "blog"
                locale_namespace = "content_blog"
                permissions = ["modules.read", "modules.read", " "]
                """,
            )

            modules = scan_modules(root)
            self.assertEqual(modules[0]["locale_namespace"], "content_blog")
            self.assertEqual(modules[0]["permissions"], ["modules.read"])

    def test_render_snapshot_uses_module_locale_namespace_fallback(self):
        payload = render_snapshot_json(
            [
                {
                    "module_key": "rustok_blog",
                    "module_slug": "blog",
                    "route_segment": "content",
                    "nav_label": "Blog",
                    "icon": "article",
                    "permissions": ["blog.read"],
                    "child_pages": [],
                }
            ]
        )
        self.assertIn('"locale_namespace": "blog"', payload)
        self.assertIn('"permissions": [', payload)
        self.assertIn('"blog.read"', payload)

    def test_scan_modules_normalizes_locale_namespace_and_sorts_permissions(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            write_module_manifest(
                root,
                "mod-a",
                """
                [module]
                slug = "blog"

                [provides.admin_ui]
                route_segment = "blog"
                locale_namespace = "Content Blog"
                permissions = ["z.read", "a.read", "z.read"]
                """,
            )

            modules = scan_modules(root)
            self.assertEqual(modules[0]["locale_namespace"], "content_blog")
            self.assertEqual(modules[0]["permissions"], ["a.read", "z.read"])

    def test_scan_modules_normalizes_permissions_to_lowercase(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            write_module_manifest(
                root,
                "mod-a",
                """
                [module]
                slug = "blog"

                [provides.admin_ui]
                route_segment = "blog"
                permissions = ["Blog.Read", "BLOG.READ", "blog.write"]
                """,
            )

            modules = scan_modules(root)
            self.assertEqual(modules[0]["permissions"], ["blog.read", "blog.write"])

    def test_scan_modules_drops_permissions_with_invalid_format(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            write_module_manifest(
                root,
                "mod-a",
                """
                [module]
                slug = "blog"

                [provides.admin_ui]
                route_segment = "blog"
                permissions = ["blog.read", "blog/read", "bad space", "ok:perm"]
                """,
            )

            modules = scan_modules(root)
            self.assertEqual(modules[0]["permissions"], ["blog.read", "ok:perm"])

    def test_scan_modules_falls_back_locale_namespace_when_normalized_empty(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            write_module_manifest(
                root,
                "mod-a",
                """
                [module]
                slug = "blog"

                [provides.admin_ui]
                route_segment = "blog"
                locale_namespace = "!!!"
                """,
            )

            modules = scan_modules(root)
            self.assertEqual(modules[0]["locale_namespace"], "blog")


if __name__ == "__main__":
    unittest.main()
