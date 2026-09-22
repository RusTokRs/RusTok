import contextlib
import io
import pathlib
import sys
import tempfile
import textwrap
import unittest

from rustok_mobile.tooling.scripts.check_mobile_codegen import main
from rustok_mobile.tooling.scripts.generate_mobile_manifest import (
    render,
    render_snapshot_json,
    scan_modules,
)


def write_module_manifest(root: pathlib.Path) -> None:
    manifest = root / "crates/mod-a/rustok-module.toml"
    manifest.parent.mkdir(parents=True, exist_ok=True)
    manifest.write_text(
        textwrap.dedent("""
            [module]
            slug = "blog"

            [provides.admin_ui]
            route_segment = "blog"
            nav_label = "Blog"
            """).strip(),
        encoding="utf-8",
    )


class CheckMobileCodegenTests(unittest.TestCase):
    def _run_check(
        self, root: pathlib.Path, manifest: pathlib.Path, snapshot: pathlib.Path
    ):
        argv_backup = sys.argv
        sys.argv = [
            "check_mobile_codegen",
            "--repo-root",
            str(root),
            "--manifest",
            str(manifest),
            "--snapshot",
            str(snapshot),
        ]
        stdout = io.StringIO()
        try:
            with contextlib.redirect_stdout(stdout):
                code = main()
        finally:
            sys.argv = argv_backup
        return code, stdout.getvalue()

    def test_returns_zero_when_generated_outputs_match(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            write_module_manifest(root)
            modules = scan_modules(root)
            manifest = root / "mobile_manifest.g.dart"
            manifest.write_text(render(modules), encoding="utf-8")
            snapshot = root / "mobile_manifest.snapshot.json"
            snapshot.write_text(render_snapshot_json(modules), encoding="utf-8")

            code, output = self._run_check(root, manifest, snapshot)

            self.assertEqual(code, 0)
            self.assertIn("OK: generated mobile manifest outputs match", output)

    def test_prints_diff_when_committed_manifest_is_stale(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            write_module_manifest(root)
            modules = scan_modules(root)
            manifest = root / "mobile_manifest.g.dart"
            manifest.write_text("// stale manifest\n", encoding="utf-8")
            snapshot = root / "mobile_manifest.snapshot.json"
            snapshot.write_text(render_snapshot_json(modules), encoding="utf-8")

            code, output = self._run_check(root, manifest, snapshot)

            self.assertEqual(code, 1)
            self.assertIn("generated output differs from committed file", output)
            self.assertIn("Diff (committed -> generated):", output)
            self.assertIn("-// stale manifest", output)
            self.assertIn("+// GENERATED CODE - DO NOT MODIFY BY HAND.", output)
            self.assertIn("generate_mobile_manifest.py", output)


if __name__ == "__main__":
    unittest.main()
