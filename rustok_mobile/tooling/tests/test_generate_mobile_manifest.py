import pathlib
import tempfile
import textwrap
import unittest

from rustok_mobile.tooling.scripts.generate_mobile_manifest import (
    render,
    render_snapshot_json,
    scan_modules,
)


class GenerateMobileManifestTests(unittest.TestCase):
    def test_scan_modules_filters_and_sorts(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "crates/mod-a").mkdir(parents=True)
            (root / "crates/mod-b").mkdir(parents=True)
            (root / "crates/mod-c").mkdir(parents=True)

            (root / "crates/mod-a/rustok-module.toml").write_text(
                textwrap.dedent(
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
                    """
                ).strip()
            )
            (root / "crates/mod-b/rustok-module.toml").write_text(
                textwrap.dedent(
                    """
                    [module]
                    slug = "blog"
                    name = "Blog"

                    [provides.admin_ui]
                    route_segment = "blog"
                    nav_label = "Blog"
                    """
                ).strip()
            )
            (root / "crates/mod-c/rustok-module.toml").write_text(
                textwrap.dedent(
                    """
                    [module]
                    slug = "service-only"
                    """
                ).strip()
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

    def test_render_includes_child_pages(self):
        content = render(
            [
                {
                    "module_key": "rustok_test",
                    "route_segment": "test",
                    "nav_label": "Test",
                    "icon": "module",
                    "child_pages": [
                        {"subpath": "items", "title": "Items", "nav_label": "All items"}
                    ],
                }
            ]
        )
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
        self.assertIn('"child_pages"', payload)

    def test_scan_modules_raises_on_duplicate_route_segment(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "crates/mod-a").mkdir(parents=True)
            (root / "crates/mod-b").mkdir(parents=True)

            (root / "crates/mod-a/rustok-module.toml").write_text(
                textwrap.dedent(
                    """
                    [module]
                    slug = "blog"

                    [provides.admin_ui]
                    route_segment = "content"
                    """
                ).strip()
            )
            (root / "crates/mod-b/rustok-module.toml").write_text(
                textwrap.dedent(
                    """
                    [module]
                    slug = "forum"

                    [provides.admin_ui]
                    route_segment = "content"
                    """
                ).strip()
            )


            with self.assertRaises(ValueError) as ctx:
                scan_modules(root)
            self.assertIn("content", str(ctx.exception))
            self.assertIn("already declared in", str(ctx.exception))

    def test_scan_modules_includes_permissions_and_locale_namespace(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "crates/mod-a").mkdir(parents=True)

            (root / "crates/mod-a/rustok-module.toml").write_text(
                textwrap.dedent(
                    """
                    [module]
                    slug = "blog"

                    [provides.admin_ui]
                    route_segment = "blog"
                    locale_namespace = "content_blog"
                    permissions = ["modules.read", "modules.read", " "]
                    """
                ).strip()
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
            (root / "crates/mod-a").mkdir(parents=True)

            (root / "crates/mod-a/rustok-module.toml").write_text(
                textwrap.dedent(
                    """
                    [module]
                    slug = "blog"

                    [provides.admin_ui]
                    route_segment = "blog"
                    locale_namespace = "Content Blog"
                    permissions = ["z.read", "a.read", "z.read"]
                    """
                ).strip()
            )

            modules = scan_modules(root)
            self.assertEqual(modules[0]["locale_namespace"], "content_blog")
            self.assertEqual(modules[0]["permissions"], ["a.read", "z.read"])

    def test_scan_modules_normalizes_permissions_to_lowercase(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "crates/mod-a").mkdir(parents=True)

            (root / "crates/mod-a/rustok-module.toml").write_text(
                textwrap.dedent(
                    """
                    [module]
                    slug = "blog"

                    [provides.admin_ui]
                    route_segment = "blog"
                    permissions = ["Blog.Read", "BLOG.READ", "blog.write"]
                    """
                ).strip()
            )

            modules = scan_modules(root)
            self.assertEqual(modules[0]["permissions"], ["blog.read", "blog.write"])


if __name__ == "__main__":
    unittest.main()
