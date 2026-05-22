#!/usr/bin/env python3
"""Verify generated mobile manifest is up to date."""

from __future__ import annotations

import argparse
import pathlib
import sys

if __package__:
    from .generate_mobile_manifest import render, scan_modules
else:
    current_dir = pathlib.Path(__file__).resolve().parent
    if str(current_dir) not in sys.path:
        sys.path.insert(0, str(current_dir))
    from generate_mobile_manifest import render, scan_modules


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--repo-root",
        default=".",
        help="Path to repository root containing crates/*/rustok-module.toml",
    )
    parser.add_argument(
        "--manifest",
        default=(
            "rustok_mobile/apps/rustok_admin_mobile/lib/registry/mobile_manifest.g.dart"
        ),
        help="Path to generated Dart manifest",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    manifest_path = pathlib.Path(args.manifest).resolve()

    expected = render(scan_modules(repo_root))
    if not manifest_path.exists():
        print(f"Manifest file is missing: {manifest_path}")
        return 1

    current = manifest_path.read_text(encoding="utf-8")
    if current == expected:
        print(f"OK: mobile manifest is up to date ({manifest_path})")
        return 0

    print("ERROR: mobile manifest is stale.")
    print("Run:")
    print(
        "  python3 rustok_mobile/tooling/scripts/generate_mobile_manifest.py "
        f"--repo-root {repo_root}"
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
