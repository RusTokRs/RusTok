import contextlib
import io
import pathlib
import sys
import tempfile
import textwrap
import unittest

from rustok_mobile.tooling.scripts.generate_mobile_manifest import (
    render,
    render_snapshot_json,
    scan_modules,
)
from rustok_mobile.tooling.scripts.verify_mobile_manifest import (
    _validate_snapshot_schema,
    main,
)


class VerifyMobileManifestTests(unittest.TestCase):
    def _run_verify(
        self,
        root: pathlib.Path,
        manifest: pathlib.Path,
        snapshot: pathlib.Path,
        *,
        surface: str = "admin",
    ):
        argv_backup = sys.argv
        sys.argv = [
            "verify",
            "--repo-root",
            str(root),
            "--manifest",
            str(manifest),
            "--snapshot",
            str(snapshot),
            "--surface",
            surface,
        ]
        stdout = io.StringIO()
        try:
            with contextlib.redirect_stdout(stdout):
                code = main()
        finally:
            sys.argv = argv_backup
        return code, stdout.getvalue()

    def test_verify_returns_zero_for_fresh_manifest(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "crates/mod-a").mkdir(parents=True)
            (root / "crates/mod-a/rustok-module.toml").write_text(
                textwrap.dedent("""
                    [module]
                    slug = "blog"
                    [provides.admin_ui]
                    route_segment = "blog"
                    nav_label = "Blog"
                    """).strip(),
                encoding="utf-8",
            )
            manifest = root / "mobile_manifest.g.dart"
            modules = scan_modules(root)
            manifest.write_text(render(modules), encoding="utf-8")
            snapshot = root / "mobile_manifest.snapshot.json"
            snapshot.write_text(render_snapshot_json(modules), encoding="utf-8")

            code, output = self._run_verify(root, manifest, snapshot)

            self.assertEqual(code, 0)
            self.assertIn("OK: mobile manifest and snapshot are up to date", output)


    def test_verify_returns_zero_for_fresh_storefront_manifest(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "crates/mod-a").mkdir(parents=True)
            (root / "crates/mod-a/rustok-module.toml").write_text(
                textwrap.dedent("""
                    [module]
                    slug = "product"
                    name = "Product"
                    [provides.storefront_ui]
                    route_segment = "products"
                    page_title = "Products"
                    """).strip(),
                encoding="utf-8",
            )
            manifest = root / "storefront_mobile_manifest.g.dart"
            modules = scan_modules(root, surface="storefront")
            manifest.write_text(render(modules), encoding="utf-8")
            snapshot = root / "storefront_mobile_manifest.snapshot.json"
            snapshot.write_text(render_snapshot_json(modules), encoding="utf-8")

            code, output = self._run_verify(
                root, manifest, snapshot, surface="storefront"
            )

            self.assertEqual(code, 0)
            self.assertIn("OK: mobile manifest and snapshot are up to date", output)

    def test_verify_prints_manifest_diff_for_stale_manifest(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "crates/mod-a").mkdir(parents=True)
            (root / "crates/mod-a/rustok-module.toml").write_text(
                textwrap.dedent("""
                    [module]
                    slug = "blog"
                    [provides.admin_ui]
                    route_segment = "blog"
                    nav_label = "Blog"
                    """).strip(),
                encoding="utf-8",
            )
            modules = scan_modules(root)
            manifest = root / "mobile_manifest.g.dart"
            manifest.write_text("// stale manifest\n", encoding="utf-8")
            snapshot = root / "mobile_manifest.snapshot.json"
            snapshot.write_text(render_snapshot_json(modules), encoding="utf-8")

            code, output = self._run_verify(root, manifest, snapshot)

            self.assertEqual(code, 1)
            self.assertIn("ERROR: mobile manifest is stale", output)
            self.assertIn("Diff (current -> expected):", output)
            self.assertIn("---", output)
            self.assertIn("+++", output)
            self.assertIn("-// stale manifest", output)
            self.assertIn("+// GENERATED CODE - DO NOT MODIFY BY HAND.", output)
            self.assertIn("generate_mobile_manifest.py", output)

    def test_verify_prints_snapshot_diff_for_stale_snapshot(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "crates/mod-a").mkdir(parents=True)
            (root / "crates/mod-a/rustok-module.toml").write_text(
                textwrap.dedent("""
                    [module]
                    slug = "blog"
                    [provides.admin_ui]
                    route_segment = "blog"
                    nav_label = "Blog"
                    """).strip(),
                encoding="utf-8",
            )
            modules = scan_modules(root)
            manifest = root / "mobile_manifest.g.dart"
            manifest.write_text(render(modules), encoding="utf-8")
            snapshot = root / "mobile_manifest.snapshot.json"
            snapshot.write_text("[]\n", encoding="utf-8")

            code, output = self._run_verify(root, manifest, snapshot)

            self.assertEqual(code, 1)
            self.assertIn("ERROR: mobile manifest snapshot is stale", output)
            self.assertIn("Diff (current -> expected):", output)
            self.assertIn("-[]", output)
            self.assertIn('+    "module_slug": "blog"', output)
            self.assertIn('+    "nav_icon": "article"', output)
            self.assertIn("generate_mobile_manifest.py", output)

    def test_validate_snapshot_schema_rejects_duplicate_route_segments(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "content",
                    "nav_icon": "article",
                    "permissions": [],
                    "locale_namespace": "content",
                    "child_pages": [],
                },
                {
                    "module_slug": "news",
                    "surface_kind": "admin_mobile",
                    "route_segment": "content",
                    "nav_icon": "article",
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
                    "nav_icon": "article",
                    "permissions": [],
                    "locale_namespace": "blog",
                    "child_pages": [
                        {"subpath": "", "title": "Posts", "nav_label": "Posts"}
                    ],
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
                    "nav_icon": "article",
                    "permissions": [],
                    "locale_namespace": "",
                    "child_pages": [],
                }
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("invalid locale_namespace", error)

    def test_validate_snapshot_schema_rejects_duplicate_permissions(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "blog",
                    "nav_icon": "article",
                    "permissions": ["blog.read", "blog.read"],
                    "locale_namespace": "blog",
                    "child_pages": [],
                }
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("duplicates permission", error)

    def test_validate_snapshot_schema_rejects_non_string_permission(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "blog",
                    "nav_icon": "article",
                    "permissions": ["blog.read", 10],
                    "locale_namespace": "blog",
                    "child_pages": [],
                }
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("permission #1 is invalid", error)

    def test_validate_snapshot_schema_rejects_non_snake_case_route_segment(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "Blog-Route",
                    "nav_icon": "article",
                    "permissions": [],
                    "locale_namespace": "blog",
                    "child_pages": [],
                }
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("route_segment must be snake_case", error)

    def test_validate_snapshot_schema_rejects_non_snake_case_locale_namespace(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "blog",
                    "nav_icon": "article",
                    "permissions": [],
                    "locale_namespace": "Blog Locale",
                    "child_pages": [],
                }
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("locale_namespace must be snake_case", error)

    def test_validate_snapshot_schema_rejects_route_segment_with_space(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "blog route",
                    "nav_icon": "article",
                    "permissions": [],
                    "locale_namespace": "blog",
                    "child_pages": [],
                }
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("route_segment must be snake_case", error)

    def test_validate_snapshot_schema_rejects_non_snake_case_child_subpath(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "blog",
                    "nav_icon": "article",
                    "permissions": [],
                    "locale_namespace": "blog",
                    "child_pages": [
                        {
                            "subpath": "Blog Posts",
                            "title": "Posts",
                            "nav_label": "Posts",
                        }
                    ],
                }
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("subpath must be snake_case", error)

    def test_validate_snapshot_schema_rejects_non_snake_case_module_slug(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "Blog Module",
                    "surface_kind": "admin_mobile",
                    "route_segment": "blog",
                    "nav_icon": "article",
                    "permissions": [],
                    "locale_namespace": "blog",
                    "child_pages": [],
                }
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("module_slug must be snake_case", error)

    def test_validate_snapshot_schema_rejects_unsorted_permissions(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "blog",
                    "nav_icon": "article",
                    "permissions": ["z.read", "a.read"],
                    "locale_namespace": "blog",
                    "child_pages": [],
                }
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("permissions must be sorted ascending", error)

    def test_validate_snapshot_schema_rejects_unsorted_child_pages(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "blog",
                    "nav_icon": "article",
                    "permissions": [],
                    "locale_namespace": "blog",
                    "child_pages": [
                        {"subpath": "posts", "title": "Posts", "nav_label": "Posts"},
                        {"subpath": "all", "title": "All", "nav_label": "All"},
                    ],
                }
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("child_pages must be sorted by subpath", error)

    def test_validate_snapshot_schema_rejects_duplicate_module_slug(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "blog",
                    "nav_icon": "article",
                    "permissions": [],
                    "locale_namespace": "blog",
                    "child_pages": [],
                },
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "blog_2",
                    "nav_icon": "article",
                    "permissions": [],
                    "locale_namespace": "blog_2",
                    "child_pages": [],
                },
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("duplicates module_slug", error)

    def test_validate_snapshot_schema_rejects_permission_with_invalid_chars(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "blog",
                    "nav_icon": "article",
                    "permissions": ["blog/read"],
                    "locale_namespace": "blog",
                    "child_pages": [],
                }
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("must use [a-z0-9_.:]", error)

    def test_validate_snapshot_schema_rejects_unsorted_entries_by_route_segment(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "search",
                    "surface_kind": "admin_mobile",
                    "route_segment": "search",
                    "nav_icon": "article",
                    "permissions": [],
                    "locale_namespace": "search",
                    "child_pages": [],
                },
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "blog",
                    "nav_icon": "article",
                    "permissions": [],
                    "locale_namespace": "blog",
                    "child_pages": [],
                },
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("sorted by route_segment", error)

    def test_validate_snapshot_schema_rejects_untrimmed_child_title(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "blog",
                    "nav_icon": "article",
                    "permissions": [],
                    "locale_namespace": "blog",
                    "child_pages": [
                        {"subpath": "posts", "title": " Posts ", "nav_label": "Posts"}
                    ],
                }
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("title must be trimmed", error)

    def test_validate_snapshot_schema_rejects_untrimmed_module_slug(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": " blog ",
                    "surface_kind": "admin_mobile",
                    "route_segment": "blog",
                    "nav_icon": "article",
                    "permissions": [],
                    "locale_namespace": "blog",
                    "child_pages": [],
                }
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("module_slug must be trimmed", error)

    def test_validate_snapshot_schema_rejects_untrimmed_permission(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "blog",
                    "nav_icon": "article",
                    "permissions": [" blog.read "],
                    "locale_namespace": "blog",
                    "child_pages": [],
                }
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("permission #0 must be trimmed", error)

    def test_validate_snapshot_schema_rejects_untrimmed_surface_kind(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "blog",
                    "surface_kind": " admin_mobile ",
                    "route_segment": "blog",
                    "nav_icon": "article",
                    "permissions": [],
                    "locale_namespace": "blog",
                    "child_pages": [],
                }
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("invalid surface_kind", error)

    def test_validate_snapshot_schema_rejects_invalid_nav_icon(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "blog",
                    "nav_icon": "bad icon",
                    "permissions": [],
                    "locale_namespace": "blog",
                    "child_pages": [],
                }
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("nav_icon must be snake_case", error)

    def test_validate_snapshot_schema_rejects_unknown_top_level_keys(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "blog",
                    "nav_icon": "article",
                    "permissions": [],
                    "locale_namespace": "blog",
                    "child_pages": [],
                    "extra": True,
                }
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("unknown keys", error)

    def test_validate_snapshot_schema_rejects_unknown_child_keys(self):
        error = _validate_snapshot_schema(
            [
                {
                    "module_slug": "blog",
                    "surface_kind": "admin_mobile",
                    "route_segment": "blog",
                    "nav_icon": "article",
                    "permissions": [],
                    "locale_namespace": "blog",
                    "child_pages": [
                        {
                            "subpath": "posts",
                            "title": "Posts",
                            "nav_label": "All posts",
                            "extra": "x",
                        }
                    ],
                }
            ]
        )
        self.assertIsNotNone(error)
        self.assertIn("has unknown keys", error)


if __name__ == "__main__":
    unittest.main()
