#!/usr/bin/env python3
import json
from collections import deque
from pathlib import Path

METADATA_PATH = Path("/tmp/cargo-metadata-all-features.json")
TARGET_NAMES = {"atomic-polyfill", "rsa"}


def package_label(package: dict) -> str:
    source = package.get("source") or "workspace/path"
    return f"{package['name']} {package['version']} ({source})"


def describe_dep(dependency: dict) -> str:
    kinds = dependency.get("dep_kinds") or []
    if not kinds:
        return dependency.get("name", "dependency")
    rendered = []
    for item in kinds:
        kind = item.get("kind") or "normal"
        target = item.get("target") or "all targets"
        rendered.append(f"{kind} @ {target}")
    return f"{dependency.get('name', 'dependency')} [{'; '.join(rendered)}]"


def main() -> None:
    metadata = json.loads(METADATA_PATH.read_text())
    packages_by_id = {package["id"]: package for package in metadata["packages"]}
    nodes_by_id = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    workspace_roots = list(metadata["workspace_members"])

    parent: dict[str, tuple[str, dict] | None] = {root: None for root in workspace_roots}
    queue = deque(workspace_roots)

    while queue:
        package_id = queue.popleft()
        node = nodes_by_id.get(package_id)
        if node is None:
            raise SystemExit(f"workspace dependency graph is missing resolve node {package_id}")
        for dependency in node.get("deps", []):
            dependency_id = dependency["pkg"]
            if dependency_id in parent:
                continue
            parent[dependency_id] = (package_id, dependency)
            queue.append(dependency_id)

    target_ids = [
        package_id
        for package_id in parent
        if packages_by_id[package_id]["name"] in TARGET_NAMES
    ]
    if not target_ids:
        raise SystemExit(
            "metadata workspace-root traversal does not reach atomic-polyfill or rsa; "
            "lock pruning may proceed"
        )

    diagnostics = [
        "full cargo metadata reaches temporary-waiver packages from workspace roots:"
    ]
    for target_id in sorted(target_ids, key=lambda value: package_label(packages_by_id[value])):
        chain: list[tuple[str, dict | None]] = []
        cursor = target_id
        while True:
            edge = parent[cursor]
            chain.append((cursor, edge[1] if edge else None))
            if edge is None:
                break
            cursor = edge[0]
        chain.reverse()

        diagnostics.append(f"\nTARGET {package_label(packages_by_id[target_id])}")
        root_id, _ = chain[0]
        diagnostics.append(f"  ROOT {package_label(packages_by_id[root_id])}")
        for package_id, dependency in chain[1:]:
            diagnostics.append(
                f"  -> {describe_dep(dependency)} -> {package_label(packages_by_id[package_id])}"
            )

    raise SystemExit("\n".join(diagnostics))


if __name__ == "__main__":
    main()
