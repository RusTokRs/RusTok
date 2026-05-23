import pathlib
import tempfile
import textwrap
import unittest

from rustok_mobile.tooling.scripts.generate_mobile_manifest import render, scan_modules


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


if __name__ == "__main__":
    unittest.main()
