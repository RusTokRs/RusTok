import pathlib
import tempfile
import textwrap
import unittest

from rustok_mobile.tooling.scripts.generate_mobile_manifest import render, render_snapshot_json, scan_modules
from rustok_mobile.tooling.scripts.verify_mobile_manifest import _validate_snapshot_schema, main


class VerifyMobileManifestTests(unittest.TestCase):
    def test_verify_returns_zero_for_fresh_manifest(self):
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
                    nav_label = "Blog"
                    """
                ).strip(),
                encoding="utf-8",
            )
            manifest = root / "mobile_manifest.g.dart"
            modules = scan_modules(root)
            manifest.write_text(render(modules), encoding="utf-8")
            snapshot = root / "mobile_manifest.snapshot.json"
            snapshot.write_text(render_snapshot_json(modules), encoding="utf-8")

            import sys
            argv_backup = sys.argv
            sys.argv = ["verify", "--repo-root", str(root), "--manifest", str(manifest), "--snapshot", str(snapshot)]
            try:
                self.assertEqual(main(), 0)
            finally:
                sys.argv = argv_backup


    def test_validate_snapshot_schema_rejects_duplicate_route_segments(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "content",
                    "permissions": [],
                    "locale_namespace": "content",
                    "child_pages": [],
                },
                {
                    "module_slug": "news",
                    "surface_kind": "admin_mobile",
                    "route_segment": "content",
                    "permissions": [],
                    "locale_namespace": "content",
                    "child_pages": [],
                },
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("duplicates route_segment", error)

    def test_validate_snapshot_schema_rejects_invalid_child_page(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "blog",
                    "permissions": [],
                    "locale_namespace": "blog",
                    "child_pages": [{"subpath": "", "title": "Posts", "nav_label": "Posts"}],
                }
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("invalid subpath", error)

    def test_validate_snapshot_schema_rejects_empty_locale_namespace(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "blog",
                    "permissions": [],
                    "locale_namespace": "",
                    "child_pages": [],
                }
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("invalid locale_namespace", error)


if __name__ == "__main__":
    unittest.main()
