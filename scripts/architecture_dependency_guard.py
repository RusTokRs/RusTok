#!/usr/bin/env python3
"""Architecture dependency guard for RusTok.

Scope is configurable via scripts/architecture_rules.toml and focuses on backend
boundaries (server/core/domain), while allowing frontend-specific workspace libs.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    import tomli as tomllib  # type: ignore

ROOT = Path(__file__).resolve().parent.parent
RULES_PATH = ROOT / "scripts" / "architecture_rules.toml"


def load_rules() -> dict:
    with RULES_PATH.open("rb") as f:
        return tomllib.load(f)


def run_metadata() -> dict:
    raw = subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT
    )
    return json.loads(raw)


def kind_is_runtime(dep: dict) -> bool:
    return dep.get("kind") in (None, "build")


def package_by_name(metadata: dict) -> dict[str, dict]:
    return {pkg["name"]: pkg for pkg in metadata["packages"]}


def target_app_src_dirs(metadata: dict, target_packages: list[str]) -> list[Path]:
    by_name = package_by_name(metadata)
    dirs: list[Path] = []
    for package_name in target_packages:
        pkg = by_name.get(package_name)
        if not pkg:
            continue
        manifest_dir = Path(pkg["manifest_path"]).parent
        dirs.append(manifest_dir / "src")
    return dirs


def check_app_workspace_rules(metadata: dict, rules: dict, errors: list[str]) -> None:
    packages = metadata["packages"]
    workspace_members = set(metadata["workspace_members"])

    internal_names = {
        pkg["name"]
        for pkg in packages
        if pkg["id"] in workspace_members
        and ("/apps/" in pkg["manifest_path"] or "/crates/" in pkg["manifest_path"])
    }

    target_packages = set(rules["apps"]["target_packages"])
    allowed_dep_prefixes = tuple(rules["apps"]["allowed_internal_dep_prefixes"])
    allowed_dep_exceptions = set(rules["apps"]["allowed_internal_dep_exceptions"])

    for pkg in packages:
        if pkg["id"] not in workspace_members or pkg["name"] not in target_packages:
            continue

        for dep in pkg.get("dependencies", []):
            if not kind_is_runtime(dep):
                continue
            dep_name = dep["name"]
            if dep_name not in internal_names:
                continue
            if dep_name in allowed_dep_exceptions or dep_name.startswith(allowed_dep_prefixes):
                continue
            errors.append(
                f"{pkg['name']} depends on workspace crate '{dep_name}', "
                f"but target backend apps may depend only on rustok-* (except explicit allow-list)."
            )


def normalize_edge(edge: str) -> tuple[str, str]:
    lhs, rhs = [p.strip() for p in edge.split("->", 1)]
    return lhs, rhs


def check_domain_edges(metadata: dict, rules: dict, errors: list[str]) -> None:
    packages = metadata["packages"]
    workspace_members = set(metadata["workspace_members"])
    domain_crates = set(rules["domain"]["crates"])
    allowed_edges = {normalize_edge(edge) for edge in rules["domain"]["allowed_edges"]}

    for pkg in packages:
        if pkg["id"] not in workspace_members or pkg["name"] not in domain_crates:
            continue
        for dep in pkg.get("dependencies", []):
            if not kind_is_runtime(dep):
                continue
            dep_name = dep["name"]
            if dep_name not in domain_crates:
                continue
            if (pkg["name"], dep_name) not in allowed_edges:
                errors.append(f"Forbidden domain dependency edge: {pkg['name']} -> {dep_name}.")


def parse_use_tree(s: str) -> list[str]:
    s = s.strip()
    if not s:
        return []
    if "{" not in s:
        return [part.strip() for part in s.split(",") if part.strip()]

    brace_idx = s.find("{")
    prefix = s[:brace_idx]
    depth = 0
    end_idx = -1
    for i in range(brace_idx, len(s)):
        if s[i] == "{":
            depth += 1
        elif s[i] == "}":
            depth -= 1
            if depth == 0:
                end_idx = i
                break
    if end_idx == -1:
        return []
    inner = s[brace_idx + 1 : end_idx]
    items = []
    cur = []
    d = 0
    for char in inner:
        if char == "{":
            d += 1
            cur.append(char)
        elif char == "}":
            d -= 1
            cur.append(char)
        elif char == "," and d == 0:
            item = "".join(cur).strip()
            if item:
                items.append(item)
            cur = []
        else:
            cur.append(char)
    last = "".join(cur).strip()
    if last:
        items.append(last)

    results = []
    for item in items:
        sub_items = parse_use_tree(item)
        for sub in sub_items:
            if sub == "self":
                results.append(prefix.rstrip(":"))
            else:
                results.append(prefix + sub)
    return results


def extract_use_statements(text: str) -> list[tuple[str, int]]:
    statements: list[tuple[str, int]] = []
    lines = text.splitlines()
    in_use = False
    cur_stmt: list[str] = []
    start_line = 0

    for idx, raw_line in enumerate(lines, start=1):
        line = raw_line
        comment_idx = line.find("//")
        if comment_idx != -1:
            line = line[:comment_idx]
        stripped = line.strip()
        if not stripped:
            continue

        if not in_use:
            m = re.search(r"\buse\s+(rustok_[A-Za-z0-9_]+.*)", stripped)
            if m:
                in_use = True
                start_line = idx
                cur_stmt = [m.group(1)]
                if ";" in stripped:
                    stmt_str = " ".join(cur_stmt)
                    stmt_str = stmt_str[: stmt_str.index(";")]
                    statements.append((stmt_str, start_line))
                    in_use = False
                    cur_stmt = []
        else:
            cur_stmt.append(stripped)
            if ";" in stripped:
                stmt_str = " ".join(cur_stmt)
                stmt_str = stmt_str[: stmt_str.index(";")]
                statements.append((stmt_str, start_line))
                in_use = False
                cur_stmt = []

    return statements


def check_internal_module_imports(metadata: dict, rules: dict, errors: list[str]) -> None:
    deny_segments = set(rules["imports"]["deny_internal_segments"])
    allowed_paths = tuple(rules["imports"]["explicitly_allowed_paths"])

    target_dirs = target_app_src_dirs(metadata, rules["imports"]["target_packages"])
    for src_dir in target_dirs:
        if not src_dir.exists():
            continue
        for rs_file in src_dir.glob("**/*.rs"):
            text = rs_file.read_text(encoding="utf-8", errors="ignore")
            for stmt_str, line_no in extract_use_statements(text):
                paths = parse_use_tree(stmt_str)
                for path in paths:
                    path = path.replace(" ", "")
                    if path.startswith(allowed_paths):
                        continue
                    segments = path.split("::")
                    if len(segments) >= 2 and segments[1] in deny_segments:
                        rel = rs_file.relative_to(ROOT)
                        errors.append(
                            f"{rel}:{line_no} imports internal module '{path}' outside explicit allow-list."
                        )


def report_fba_inventory(metadata: dict) -> None:
    with (ROOT / "modules.toml").open("rb") as manifest:
        owners = {
            spec["crate"]: slug
            for slug, spec in tomllib.load(manifest)["modules"].items()
            if spec.get("source") == "path"
        }
    edges = set()
    for package in metadata["packages"]:
        consumer = owners.get(package["name"])
        if consumer is None:
            continue
        for dep in package["dependencies"]:
            provider = owners.get(dep["name"])
            if provider is not None and provider != consumer and kind_is_runtime(dep):
                edges.add(
                    (consumer, provider, dep["kind"] or "normal", dep.get("target") or "", dep["optional"])
                )

    pairs = {(consumer, provider) for consumer, provider, *_ in edges}
    consumers = {consumer for consumer, _ in pairs}
    print(
        f"[fba-inventory] {len(owners)} local owners, {len(pairs)} direct runtime-package edges "
        f"from {len(consumers)} consumers. Review candidates, not confirmed violations."
    )
    for consumer, provider, kind, target, optional in sorted(edges):
        qualifiers = [kind, "optional" if optional else "required"]
        if target:
            qualifiers.append(target)
        print(f"  {consumer} -> {provider} ({', '.join(qualifiers)})")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--fba-inventory", action="store_true", help="List direct runtime-package edges between native owners"
    )
    args = parser.parse_args()
    metadata = run_metadata()
    if args.fba_inventory:
        report_fba_inventory(metadata)
        return 0

    rules = load_rules()
    errors: list[str] = []

    check_app_workspace_rules(metadata, rules, errors)
    check_domain_edges(metadata, rules, errors)
    check_internal_module_imports(metadata, rules, errors)

    if errors:
        print("[architecture-guard] violations found:")
        for idx, err in enumerate(errors, start=1):
            print(f"  {idx}. {err}")
        return 1

    print("[architecture-guard] OK: dependency and import boundaries are respected.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
