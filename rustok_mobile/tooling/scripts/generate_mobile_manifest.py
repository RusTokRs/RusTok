#!/usr/bin/env python3
"""Generate mobile manifest adapter from RusTok module manifests."""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import tomllib

_PERMISSION_RE = re.compile(r"^[a-z0-9_.:]+$")

_ICON_RULES: tuple[tuple[str, str], ...] = (
    ("auth", "shield"),
    ("rbac", "shield"),
    ("product", "inventory_2"),
    ("inventory", "inventory"),
    ("order", "receipt_long"),
    ("customer", "people"),
    ("tenant", "apartment"),
    ("blog", "article"),
    ("forum", "forum"),
    ("comment", "chat"),
    ("workflow", "account_tree"),
    ("seo", "travel_explore"),
    ("search", "search"),
    ("media", "perm_media"),
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--repo-root",
        default=".",
        help="Path to repository root containing crates/*/rustok-module.toml",
    )
    parser.add_argument(
        "--output",
        default=(
            "rustok_mobile/apps/rustok_admin_mobile/lib/registry/mobile_manifest.g.dart"
        ),
        help="Output dart file path",
    )
    parser.add_argument(
        "--snapshot-output",
        default=(
            "rustok_mobile/tooling/snapshots/mobile_manifest.snapshot.json"
        ),
        help="Output JSON snapshot path for registry contract checks",
    )
    return parser.parse_args()


def _dart_escape(value: str) -> str:
    return value.replace("\\", "\\\\").replace("'", "\\'")


def _normalize_key(raw: str) -> str:
    return re.sub(r"[^a-z0-9_]+", "_", raw.lower()).strip("_")


def _pick_icon(slug: str) -> str:
    normalized_slug = slug.lower()
    for needle, icon in _ICON_RULES:
        if needle in normalized_slug:
            return icon
    return "module"




def _parse_permissions(admin_ui: dict[str, object]) -> list[str]:
    raw = admin_ui.get("permissions")
    if not isinstance(raw, list):
        return []

    permissions: list[str] = []
    seen: set[str] = set()
    for item in raw:
        if not isinstance(item, str):
            continue
        value = item.strip().lower()
        if not value or not _PERMISSION_RE.fullmatch(value) or value in seen:
            continue
        seen.add(value)
        permissions.append(value)
    return sorted(permissions)


def _parse_locale_namespace(admin_ui: dict[str, object], module_slug: str) -> str:
    raw = str(admin_ui.get("locale_namespace", "")).strip()
    normalized = _normalize_key(raw or module_slug)
    if normalized:
        return normalized
    return _normalize_key(module_slug)

def _parse_child_pages(admin_ui: dict[str, object]) -> list[dict[str, str]]:
    pages_raw = admin_ui.get("child_pages")
    if not isinstance(pages_raw, list):
        return []

    pages: list[dict[str, str]] = []
    for page in pages_raw:
        if not isinstance(page, dict):
            continue
        subpath = _normalize_key(str(page.get("subpath", "")).strip())
        title = str(page.get("title", "")).strip()
        if not subpath or not title:
            continue
        nav_label = str(page.get("nav_label", "")).strip() or None
        payload = {"subpath": subpath, "title": title}
        if nav_label is not None:
            payload["nav_label"] = nav_label
        pages.append(payload)

    return sorted(pages, key=lambda item: item["subpath"])


def scan_modules(repo_root: pathlib.Path) -> list[dict[str, object]]:
    manifests = sorted(repo_root.glob("crates/*/rustok-module.toml"))
    modules: list[dict[str, object]] = []
    used_segments: dict[str, pathlib.Path] = {}

    for manifest in manifests:
        data = tomllib.loads(manifest.read_text(encoding="utf-8"))
        module = data.get("module", {})
        provides = data.get("provides", {})
        admin_ui = provides.get("admin_ui")
        if not isinstance(admin_ui, dict):
            continue

        slug = str(module.get("slug", "")).strip()
        if not slug:
            continue

        route_segment = str(admin_ui.get("route_segment", slug)).strip() or slug
        route_segment = _normalize_key(route_segment)
        if not route_segment:
            continue
        previous_manifest = used_segments.get(route_segment)
        if previous_manifest is not None:
            raise ValueError(
                "Duplicate admin_ui.route_segment "
                f"'{route_segment}' in {manifest}; already declared in {previous_manifest}"
            )

        nav_label = str(admin_ui.get("nav_label", module.get("name", slug.title()))).strip()
        nav_label = nav_label or slug.title()
        module_key = f"rustok_{_normalize_key(slug.replace('-', '_'))}"

        modules.append(
            {
                "module_key": module_key,
                "module_slug": slug,
                "route_segment": route_segment,
                "nav_label": nav_label,
                "icon": _pick_icon(slug),
                "child_pages": _parse_child_pages(admin_ui),
                "permissions": _parse_permissions(admin_ui),
                "locale_namespace": _parse_locale_namespace(admin_ui, slug),
            }
        )
        used_segments[route_segment] = manifest

    return sorted(modules, key=lambda item: item["route_segment"])


def render(modules: list[dict[str, object]]) -> str:
    lines = [
        "// GENERATED CODE - DO NOT MODIFY BY HAND.",
        "// Generated by rustok_mobile/tooling/scripts/generate_mobile_manifest.py",
        "",
        "import 'package:app_module_contracts/app_module_contracts.dart';",
        "",
        "const generatedMobileManifest = <MobileModuleEntry>[",
    ]
    for module in modules:
        lines.extend(
            [
                "  MobileModuleEntry(",
                f"    moduleKey: '{_dart_escape(module['module_key'])}',",
                f"    routeSegment: '{_dart_escape(module['route_segment'])}',",
                (
                    "    nav: MobileNavMeta("
                    f"title: '{_dart_escape(module['nav_label'])}', icon: '{_dart_escape(module['icon'])}'),"
                ),
                "    childPages: <MobileChildPage>[",
            ]
        )
        child_pages = module.get("child_pages", [])
        if isinstance(child_pages, list):
            for page in child_pages:
                if not isinstance(page, dict):
                    continue
                lines.append("      MobileChildPage(")
                lines.append(
                    f"        subpath: '{_dart_escape(str(page['subpath']))}',"
                )
                lines.append(f"        title: '{_dart_escape(str(page['title']))}',")
                nav_label = page.get("nav_label")
                if isinstance(nav_label, str):
                    lines.append(f"        navLabel: '{_dart_escape(nav_label)}',")
                lines.append("      ),")
        lines.extend(["    ],", "  ),"])
    lines.append("];")
    lines.append("")
    return "\n".join(lines)



def to_snapshot(modules: list[dict[str, object]]) -> list[dict[str, object]]:
    snapshot: list[dict[str, object]] = []
    for module in modules:
        route_segment = str(module["route_segment"])
        snapshot.append(
            {
                "module_slug": str(module.get("module_slug") or str(module["module_key"]).removeprefix("rustok_")),
                "surface_kind": "admin_mobile",
                "route_segment": route_segment,
                "permissions": list(module.get("permissions", [])),
                "locale_namespace": str(module.get("locale_namespace") or module.get("module_slug") or route_segment),
                "child_pages": [
                    {
                        "subpath": str(page["subpath"]),
                        "title": str(page["title"]),
                        "nav_label": str(page.get("nav_label") or page["title"]),
                    }
                    for page in module.get("child_pages", [])
                    if isinstance(page, dict)
                ],
            }
        )
    return snapshot


def render_snapshot_json(modules: list[dict[str, object]]) -> str:
    return json.dumps(to_snapshot(modules), ensure_ascii=False, indent=2) + "\n"

def main() -> None:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    output = pathlib.Path(args.output).resolve()
    snapshot_output = pathlib.Path(args.snapshot_output).resolve()
    modules = scan_modules(repo_root)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(render(modules), encoding="utf-8")
    snapshot_output.parent.mkdir(parents=True, exist_ok=True)
    snapshot_output.write_text(render_snapshot_json(modules), encoding="utf-8")
    print(f"Generated {len(modules)} modules into {output}")
    print(f"Generated snapshot into {snapshot_output}")


if __name__ == "__main__":
    main()
