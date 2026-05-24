#!/usr/bin/env python3
"""Verify generated mobile manifest is up to date."""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

if __package__:
    from .generate_mobile_manifest import render, render_snapshot_json, scan_modules, to_snapshot
else:
    current_dir = pathlib.Path(__file__).resolve().parent
    if str(current_dir) not in sys.path:
        sys.path.insert(0, str(current_dir))
    from generate_mobile_manifest import render, render_snapshot_json, scan_modules, to_snapshot



_SNAKE_CASE_RE = re.compile(r"^[a-z0-9_]+$")


def _is_snake_case(value: str) -> bool:
    return bool(value) and bool(_SNAKE_CASE_RE.fullmatch(value))

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
    parser.add_argument(
        "--snapshot",
        default=(
            "rustok_mobile/tooling/snapshots/mobile_manifest.snapshot.json"
        ),
        help="Path to generated registry snapshot JSON",
    )
    return parser.parse_args()



def _validate_snapshot_schema(entries: object) -> str | None:
    if not isinstance(entries, list):
        return "snapshot root must be an array"

    required_keys = {
        "module_slug",
        "surface_kind",
        "route_segment",
        "permissions",
        "locale_namespace",
        "child_pages",
    }
    seen_route_segments: set[str] = set()

    for index, item in enumerate(entries):
        if not isinstance(item, dict):
            return f"snapshot entry #{index} is not an object"

        missing = required_keys.difference(item.keys())
        if missing:
            return f"snapshot entry #{index} missing keys: {sorted(missing)}"

        module_slug = item["module_slug"]
        route_segment = item["route_segment"]
        surface_kind = item["surface_kind"]
        locale_namespace = item["locale_namespace"]
        permissions = item["permissions"]
        child_pages = item["child_pages"]

        if not isinstance(module_slug, str) or not module_slug.strip():
            return f"snapshot entry #{index} has invalid module_slug"
        if not _is_snake_case(module_slug):
            return f"snapshot entry #{index} module_slug must be snake_case"
        if not isinstance(route_segment, str) or not route_segment.strip():
            return f"snapshot entry #{index} has invalid route_segment"
        if not _is_snake_case(route_segment):
            return f"snapshot entry #{index} route_segment must be snake_case"
        if route_segment in seen_route_segments:
            return f"snapshot entry #{index} duplicates route_segment '{route_segment}'"
        seen_route_segments.add(route_segment)

        if surface_kind != "admin_mobile":
            return f"snapshot entry #{index} has unsupported surface_kind '{surface_kind}'"
        if not isinstance(locale_namespace, str) or not locale_namespace.strip():
            return f"snapshot entry #{index} has invalid locale_namespace"
        if not _is_snake_case(locale_namespace):
            return f"snapshot entry #{index} locale_namespace must be snake_case"
        if not isinstance(permissions, list):
            return f"snapshot entry #{index} permissions must be an array"
        seen_permissions: set[str] = set()
        for permission_index, permission in enumerate(permissions):
            if not isinstance(permission, str) or not permission.strip():
                return (
                    f"snapshot entry #{index} permission #{permission_index} is invalid"
                )
            if permission in seen_permissions:
                return (
                    f"snapshot entry #{index} duplicates permission '{permission}'"
                )
            seen_permissions.add(permission)
        if not isinstance(child_pages, list):
            return f"snapshot entry #{index} child_pages must be an array"

        seen_subpaths: set[str] = set()
        for child_index, child in enumerate(child_pages):
            if not isinstance(child, dict):
                return f"snapshot entry #{index} child #{child_index} is not an object"
            for key in ("subpath", "title", "nav_label"):
                value = child.get(key)
                if not isinstance(value, str) or not value.strip():
                    return (
                        f"snapshot entry #{index} child #{child_index} has invalid {key}"
                    )
            subpath = child["subpath"]
            if not _is_snake_case(subpath):
                return (
                    f"snapshot entry #{index} child #{child_index} subpath must be snake_case"
                )
            if subpath in seen_subpaths:
                return (
                    f"snapshot entry #{index} child #{child_index} duplicates subpath '{subpath}'"
                )
            seen_subpaths.add(subpath)

    return None

def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    manifest_path = pathlib.Path(args.manifest).resolve()
    snapshot_path = pathlib.Path(args.snapshot).resolve()

    modules = scan_modules(repo_root)
    expected = render(modules)
    expected_snapshot_entries = to_snapshot(modules)
    schema_error = _validate_snapshot_schema(expected_snapshot_entries)
    if schema_error is not None:
        print(f"ERROR: generated snapshot schema is invalid: {schema_error}")
        return 1
    expected_snapshot = render_snapshot_json(modules)
    if not manifest_path.exists():
        print(f"Manifest file is missing: {manifest_path}")
        return 1

    current = manifest_path.read_text(encoding="utf-8")
    if current != expected:
        print("ERROR: mobile manifest is stale.")
        print("Run:")
        print(
            "  python3 rustok_mobile/tooling/scripts/generate_mobile_manifest.py "
            f"--repo-root {repo_root}"
        )
        return 1

    if not snapshot_path.exists():
        print(f"Snapshot file is missing: {snapshot_path}")
        return 1

    snapshot_current = snapshot_path.read_text(encoding="utf-8")
    if snapshot_current != expected_snapshot:
        print("ERROR: mobile manifest snapshot is stale.")
        print("Run:")
        print(
            "  python3 rustok_mobile/tooling/scripts/generate_mobile_manifest.py "
            f"--repo-root {repo_root}"
        )
        return 1

    try:
        parsed = json.loads(snapshot_current)
    except json.JSONDecodeError as exc:
        print(f"ERROR: snapshot is not valid JSON: {exc}")
        return 1

    schema_error = _validate_snapshot_schema(parsed)
    if schema_error is not None:
        print(f"ERROR: snapshot schema invalid: {schema_error}")
        return 1

    print(f"OK: mobile manifest and snapshot are up to date ({manifest_path})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
