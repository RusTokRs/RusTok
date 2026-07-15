#!/usr/bin/env python3
from __future__ import annotations

import os
import pathlib
import subprocess
import sys
import tempfile
import textwrap
import unittest


SCRIPT = pathlib.Path(__file__).with_name("check-dependabot-directories.py")


class DependabotDirectoryCheckTests(unittest.TestCase):
    def run_script(self, root: pathlib.Path, config: pathlib.Path) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--root", str(root), "--config", str(config)],
            text=True,
            capture_output=True,
            check=False,
        )

    def test_returns_zero_when_all_directories_exist(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "apps" / "server").mkdir(parents=True)
            (root / "crates").mkdir()

            config = root / ".github" / "dependabot.yml"
            config.parent.mkdir(parents=True)
            config.write_text(
                textwrap.dedent(
                    """
                    version: 2
                    updates:
                      - package-ecosystem: "cargo"
                        directory: "/"
                      - package-ecosystem: "cargo"
                        directory: "/apps/server"
                      - package-ecosystem: "cargo"
                        directory: "/crates"
                    """
                ).strip()
                + "\n",
                encoding="utf-8",
            )

            result = self.run_script(root, config)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("All Dependabot update directories exist.", result.stdout)

    def test_fails_when_directory_is_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "apps" / "server").mkdir(parents=True)

            config = root / ".github" / "dependabot.yml"
            config.parent.mkdir(parents=True)
            config.write_text(
                textwrap.dedent(
                    """
                    version: 2
                    updates:
                      - package-ecosystem: "cargo"
                        directory: "/apps/server"
                      - package-ecosystem: "cargo"
                        directory: "/apps/mcp"
                    """
                ).strip()
                + "\n",
                encoding="utf-8",
            )

            result = self.run_script(root, config)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Dependabot directories do not exist:", result.stderr)
            self.assertIn("/apps/mcp", result.stderr)

    def test_fails_when_directory_entries_are_duplicated(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "apps" / "server").mkdir(parents=True)

            config = root / ".github" / "dependabot.yml"
            config.parent.mkdir(parents=True)
            config.write_text(
                textwrap.dedent(
                    """
                    version: 2
                    updates:
                      - package-ecosystem: "cargo"
                        directory: "/apps/server"
                      - package-ecosystem: "cargo"
                        directory: "/apps/server"
                    """
                ).strip()
                + "\n",
                encoding="utf-8",
            )

            result = self.run_script(root, config)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Dependabot directories contain duplicates:", result.stderr)
            self.assertIn("/apps/server", result.stderr)

    def test_treats_trailing_slash_variants_as_duplicates(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "apps" / "server").mkdir(parents=True)

            config = root / ".github" / "dependabot.yml"
            config.parent.mkdir(parents=True)
            config.write_text(
                textwrap.dedent(
                    """
                    version: 2
                    updates:
                      - package-ecosystem: "cargo"
                        directory: "/apps/server"
                      - package-ecosystem: "cargo"
                        directory: "/apps/server/"
                    """
                ).strip()
                + "\n",
                encoding="utf-8",
            )

            result = self.run_script(root, config)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Dependabot directories contain duplicates:", result.stderr)
            self.assertIn("/apps/server", result.stderr)

    def test_fails_when_directory_path_contains_parent_segment(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "apps" / "server").mkdir(parents=True)

            config = root / ".github" / "dependabot.yml"
            config.parent.mkdir(parents=True)
            config.write_text(
                textwrap.dedent(
                    """
                    version: 2
                    updates:
                      - package-ecosystem: "cargo"
                        directory: "../outside"
                    """
                ).strip()
                + "\n",
                encoding="utf-8",
            )

            result = self.run_script(root, config)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Dependabot directories contain invalid paths:", result.stderr)
            self.assertIn("../outside", result.stderr)

    def test_allows_quoted_directory_with_inline_comment(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "apps" / "server").mkdir(parents=True)

            config = root / ".github" / "dependabot.yml"
            config.parent.mkdir(parents=True)
            config.write_text(
                textwrap.dedent(
                    """
                    version: 2
                    updates:
                      - package-ecosystem: "cargo"
                        directory: "/apps/server" # canonical server path
                    """
                ).strip()
                + "\n",
                encoding="utf-8",
            )

            result = self.run_script(root, config)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("All Dependabot update directories exist.", result.stdout)

    def test_fails_on_unterminated_quoted_directory_value(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "apps" / "server").mkdir(parents=True)

            config = root / ".github" / "dependabot.yml"
            config.parent.mkdir(parents=True)
            config.write_text(
                textwrap.dedent(
                    """
                    version: 2
                    updates:
                      - package-ecosystem: "cargo"
                        directory: "/apps/server
                    """
                ).strip()
                + "\n",
                encoding="utf-8",
            )

            result = self.run_script(root, config)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Dependabot directories contain invalid paths:", result.stderr)

    def test_fails_when_no_directory_entries_exist(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            config = root / ".github" / "dependabot.yml"
            config.parent.mkdir(parents=True)
            config.write_text(
                textwrap.dedent(
                    """
                    version: 2
                    updates:
                      - package-ecosystem: "cargo"
                        schedule:
                          interval: "daily"
                    """
                ).strip()
                + "\n",
                encoding="utf-8",
            )

            result = self.run_script(root, config)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(
                "Dependabot config does not contain any directory entries.",
                result.stderr,
            )

    def test_fails_on_quoted_directory_with_trailing_non_comment_content(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "apps" / "server").mkdir(parents=True)
            config = root / ".github" / "dependabot.yml"
            config.parent.mkdir(parents=True)
            config.write_text(
                textwrap.dedent(
                    """
                    version: 2
                    updates:
                      - package-ecosystem: "cargo"
                        directory: "/apps/server" trailing-garbage
                    """
                ).strip()
                + "\n",
                encoding="utf-8",
            )

            result = self.run_script(root, config)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Dependabot directories contain invalid paths:", result.stderr)

    def test_allows_hash_character_inside_quoted_directory_value(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / "apps" / "server#prod").mkdir(parents=True)
            config = root / ".github" / "dependabot.yml"
            config.parent.mkdir(parents=True)
            config.write_text(
                textwrap.dedent(
                    """
                    version: 2
                    updates:
                      - package-ecosystem: "cargo"
                        directory: "/apps/server#prod"
                    """
                ).strip()
                + "\n",
                encoding="utf-8",
            )

            result = self.run_script(root, config)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("All Dependabot update directories exist.", result.stdout)

    @unittest.skipIf(os.name == "nt", "Windows does not allow double quotes in directory names")
    def test_allows_escaped_quote_inside_double_quoted_scalar(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            (root / 'apps' / 'server"prod').mkdir(parents=True)
            config = root / ".github" / "dependabot.yml"
            config.parent.mkdir(parents=True)
            config.write_text(
                textwrap.dedent(
                    r"""
                    version: 2
                    updates:
                      - package-ecosystem: "cargo"
                        directory: "/apps/server\"prod"
                    """
                ).strip()
                + "\n",
                encoding="utf-8",
            )

            result = self.run_script(root, config)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("All Dependabot update directories exist.", result.stdout)


if __name__ == "__main__":
    unittest.main()
