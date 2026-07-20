#!/usr/bin/env python3
import json
import re
import tomllib
from pathlib import Path

LOCK_PATH = Path("Cargo.lock")
METADATA_PATH = Path("/tmp/cargo-metadata-all-features.json")
REMOVED_PATH = Path("/tmp/removed-lock-packages.txt")
WAIVERS = {"RUSTSEC-2023-0071", "RUSTSEC-2023-0089"}


def identity(package: dict) -> tuple[str, str, str | None]:
    return (package["name"], package["version"], package.get("source"))


def reachable_package_identities(metadata: dict) -> set[tuple[str, str, str | None]]:
    packages_by_id = {package["id"]: package for package in metadata["packages"]}
    nodes_by_id = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    reachable_ids: set[str] = set()
    pending = list(metadata["workspace_members"])

    while pending:
        package_id = pending.pop()
        if package_id in reachable_ids:
            continue
        node = nodes_by_id.get(package_id)
        if node is None:
            raise SystemExit(f"workspace dependency graph is missing resolve node {package_id}")
        reachable_ids.add(package_id)
        pending.extend(dependency["pkg"] for dependency in node.get("deps", []))

    return {identity(packages_by_id[package_id]) for package_id in reachable_ids}


def prune_lockfile(reachable: set[tuple[str, str, str | None]]) -> list[tuple[str, str, str | None]]:
    original = LOCK_PATH.read_text()
    before = {identity(package) for package in tomllib.loads(original).get("package", [])}
    missing_from_lock = sorted(reachable - before)
    if missing_from_lock:
        raise SystemExit(
            f"reachable package identities missing from Cargo.lock: {missing_from_lock[:20]}"
        )

    headers = list(re.finditer(r"(?m)^\[\[([^\]]+)\]\]\s*$", original))
    if not headers:
        raise SystemExit("Cargo.lock contains no array-table sections")

    output = [original[: headers[0].start()]]
    kept: set[tuple[str, str, str | None]] = set()
    removed: list[tuple[str, str, str | None]] = []

    for index, header in enumerate(headers):
        end = headers[index + 1].start() if index + 1 < len(headers) else len(original)
        block = original[header.start() : end]
        if header.group(1) != "package":
            output.append(block)
            continue

        package = tomllib.loads(block)["package"][0]
        package_identity = identity(package)
        if package_identity in reachable:
            kept.add(package_identity)
            output.append(block)
        else:
            removed.append(package_identity)

    if kept != reachable:
        missing = sorted(reachable - kept)
        unexpected = sorted(kept - reachable)
        raise SystemExit(
            f"pruned lock identity mismatch; missing={missing[:20]} unexpected={unexpected[:20]}"
        )

    removed_names = {name for name, _, _ in removed}
    required_names = {"atomic-polyfill", "rsa"}
    if not required_names.issubset(removed_names):
        raise SystemExit(
            f"required unreachable packages were not pruned: {sorted(required_names - removed_names)}"
        )

    LOCK_PATH.write_text("".join(output))
    after = {
        identity(package)
        for package in tomllib.loads(LOCK_PATH.read_text()).get("package", [])
    }
    if after != reachable:
        raise SystemExit("serialized Cargo.lock does not equal workspace-root reachability set")

    REMOVED_PATH.write_text(
        "\n".join(f"{name} {version}" for name, version, _ in sorted(removed)) + "\n"
    )
    return removed


def retire_audit_waivers() -> None:
    audit = Path(".cargo/audit.toml")
    source = audit.read_text()
    observed = set(re.findall(r'"(RUSTSEC-\d{4}-\d{4})"', source))
    if observed != WAIVERS:
        raise SystemExit(
            f"unexpected .cargo/audit.toml waiver set: {sorted(observed)}"
        )
    audit.write_text(
        """[advisories]
# No active repository advisory exceptions. Additions require a complete,
# time-bounded entry in docs/security/advisory-exceptions.md.
ignore = []
"""
    )


def retire_register_entries() -> None:
    register = Path("docs/security/advisory-exceptions.md")
    source = register.read_text()
    active_start = source.index("## Active Exceptions")
    closed_start = source.index("## Closed Exceptions")
    active_text = source[active_start:closed_start]
    active_ids = set(re.findall(r"^###\s+(RUSTSEC-\d{4}-\d{4})\b", active_text, re.MULTILINE))
    if active_ids != WAIVERS:
        raise SystemExit(f"unexpected active advisory register set: {sorted(active_ids)}")

    closed_tail = source[closed_start + len("## Closed Exceptions") :].lstrip("\n")
    closed_entries = """## Active Exceptions

None.

## Closed Exceptions

### RUSTSEC-2023-0071 — `rsa` timing side channel

| Field | Value |
|---|---|
| Original severity | MEDIUM, CVSS 5.9 |
| Original risk | Network-observable RSA private-key operations could leak timing information |
| Patched version | No patched `rsa 0.9.x` release; remediation removes the unused dependency path |
| Opened | 2026-07-17 |
| Closed | 2026-07-20 |
| Closure reason | SeaORM migration defaults are disabled, only PostgreSQL/SQLite features remain, and the pruned lockfile contains no `rsa` package on any target |
| Policy cleanup | Removed from `.cargo/audit.toml`; `deny.toml` already had no waiver |
| Verification | Both current-platform and `--target all` inverse trees were empty before pruning; full locked metadata succeeds after pruning |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2023-0071.html> |

### RUSTSEC-2023-0089 — `atomic-polyfill` is unmaintained

| Field | Value |
|---|---|
| Original severity | INFO, unmaintained dependency |
| Original risk | Archived embedded-support dependency created avoidable supply-chain exposure |
| Patched version | No patched release; Postcard standard-library mode avoids the heapless dependency path |
| Opened | 2026-07-17 |
| Closed | 2026-07-20 |
| Closure reason | Postcard defaults are disabled with only `use-std` enabled, and the pruned lockfile contains no `atomic-polyfill` package on any target |
| Policy cleanup | Removed from `.cargo/audit.toml`; `deny.toml` already had no waiver |
| Verification | Both current-platform and `--target all` inverse trees were empty before pruning; full locked metadata and serialization feature hygiene succeed after pruning |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2023-0089.html> |

"""
    register.write_text(source[:active_start] + closed_entries + closed_tail)


def update_rsa_note() -> None:
    Path("docs/security/rsa-rustsec-2023-0071.md").write_text(
        """---
id: doc://docs/security/rsa-rustsec-2023-0071.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# RUSTSEC-2023-0071 remediation note

## Status

Fully retired on July 20, 2026.

RusToK uses `jsonwebtoken/aws_lc_rs` rather than the RustCrypto backend. In addition,
`sea-orm-migration` disables default features and enables only PostgreSQL, SQLite and the
Tokio/Rustls runtime, so the unused SQLx MySQL path does not reach `rsa 0.9.10`.

Both the current-platform and all-target inverse dependency trees were empty. The
lockfile was pruned by traversing the locked resolve graph from every workspace member,
without adding or upgrading package identities. This removed the unreachable `rsa`
record, after which the temporary audit exception was deleted.

## Verification

```bash
node scripts/verify/verify-dependency-feature-hygiene.mjs
node scripts/verify/verify-advisory-exceptions.mjs
cargo tree --locked -i rsa --workspace --all-features --target all
cargo metadata --locked --all-features --format-version 1
```

The inverse tree must be empty and `Cargo.lock` must not contain a package named `rsa`.
"""
    )


def main() -> None:
    metadata = json.loads(METADATA_PATH.read_text())
    reachable = reachable_package_identities(metadata)
    removed = prune_lockfile(reachable)
    retire_audit_waivers()
    retire_register_entries()
    update_rsa_note()
    print(f"pruned {len(removed)} unreachable package identities")


if __name__ == "__main__":
    main()
