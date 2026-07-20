#!/usr/bin/env python3
import re
from pathlib import Path


def replace_once(path: Path, old: str, new: str) -> None:
    source = path.read_text()
    if source.count(old) != 1:
        raise SystemExit(f"{path}: expected exactly one bounded replacement marker")
    path.write_text(source.replace(old, new, 1))


cargo = Path("Cargo.toml")
replace_once(
    cargo,
    '''sea-orm = { version = "1.1", features = [
    "sqlx-postgres",
    "sqlx-sqlite",
    "runtime-tokio-rustls",
    "macros",
    "with-uuid",
    "with-chrono",
    "with-json",
] }
''',
    'sea-orm = { version = "1.1", default-features = false, features = ["sqlx-postgres", "sqlx-sqlite", "runtime-tokio-rustls", "macros", "with-uuid", "with-chrono", "with-json"] }\n',
)

verifier = Path("scripts/verify/verify-dependency-feature-hygiene.mjs")
replace_once(
    verifier,
    '''const cargo = read("Cargo.toml");
const migration = dependencySpec(cargo, "sea-orm-migration");
const postcard = dependencySpec(cargo, "postcard");

if (!migration) {
''',
    '''const cargo = read("Cargo.toml");
const orm = dependencySpec(cargo, "sea-orm");
const migration = dependencySpec(cargo, "sea-orm-migration");
const postcard = dependencySpec(cargo, "postcard");

if (!orm) {
  failures.push("Cargo.toml: sea-orm workspace dependency not found");
} else {
  requireSpec(orm, "default-features = false", "sea-orm");
  requireSpec(orm, '"sqlx-postgres"', "sea-orm");
  requireSpec(orm, '"sqlx-sqlite"', "sea-orm");
  requireSpec(orm, '"runtime-tokio-rustls"', "sea-orm");
  forbidSpec(orm, '"sqlx-mysql"', "sea-orm");
}

if (!migration) {
''',
)
replace_once(
    verifier,
    '  for (const dependency of ["sea-orm-migration", "postcard"]) {\n',
    '  for (const dependency of ["sea-orm", "sea-orm-migration", "postcard"]) {\n',
)
replace_once(
    verifier,
    'console.log("✔ unused SeaORM CLI/MySQL and Postcard heapless defaults remain disabled");\n',
    'console.log("✔ SeaORM MySQL/CLI and Postcard heapless defaults remain disabled across workspace manifests");\n',
)

audit = Path(".cargo/audit.toml")
replace_once(
    audit,
    '  "RUSTSEC-2023-0071", # rsa timing side-channel; no patched release currently available; expires 2026-07-24\n',
    "",
)

register = Path("docs/security/advisory-exceptions.md")
source = register.read_text()
active_start = source.index("## Active Exceptions")
closed_start = source.index("## Closed Exceptions")
active_text = source[active_start:closed_start]
active_ids = set(re.findall(r"^###\s+(RUSTSEC-\d{4}-\d{4})\b", active_text, re.MULTILINE))
if active_ids != {"RUSTSEC-2023-0071", "RUSTSEC-2023-0089"}:
    raise SystemExit(f"unexpected active advisory set: {sorted(active_ids)}")
closed_tail = source[closed_start + len("## Closed Exceptions") :].lstrip("\n")
replacement = '''## Active Exceptions

### RUSTSEC-2023-0089 — `atomic-polyfill` is unmaintained

| Field | Value |
|---|---|
| Severity | INFO, unmaintained dependency |
| Risk | Archived dependency receives no maintenance or security fixes and creates avoidable supply-chain exposure on embedded target graphs |
| Patched version | No patched release; recommended replacement is `portable-atomic` |
| Repository policy location | `.cargo/audit.toml` |
| Accountable owner | Platform security / Athanor integration maintainers |
| Dependency path | `rustok-ai-athanor` feature `athanor-surreal` → `athanor-runtime-defaults/store-surreal` → `athanor-store-surrealdb` → `surrealdb 2.6.5` embedded engines → `geo 0.28.0` → `rstar 0.9.3` → `heapless 0.7.17` → `atomic-polyfill 1.0.3` on AVR, RISC-V, Thumb v6-M and Xtensa embedded targets |
| Reachability | The package is not reachable on supported RusToK server targets, but it is present in the all-target feature graph because the optional embedded SurrealDB capability is compiled by `--all-features` |
| Compensating controls | Production deployment profiles target supported server operating systems only; do not ship RusToK or the Athanor Surreal adapter to the affected embedded target families; keep `athanor-surreal` optional and disabled by default |
| Remediation | Upgrade the Athanor/SurrealDB dependency path to a `geo`/`rstar`/`heapless` graph that no longer uses `atomic-polyfill`, or formally remove the embedded SurrealDB capability if it is no longer required; do not prune the reachable lock entry |
| Approved | 2026-07-17, temporary dependency stabilization exception |
| Expires | 2026-07-24 |
| Evidence required | `cargo tree -i atomic-polyfill --workspace --all-features --target all`, Athanor upstream dependency remediation, locked metadata, and serialization/integration tests |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2023-0089.html> |

## Closed Exceptions

### RUSTSEC-2023-0071 — `rsa` timing side channel

| Field | Value |
|---|---|
| Original severity | MEDIUM, CVSS 5.9 |
| Original risk | Network-observable RSA private-key operations could leak timing information |
| Patched version | No patched `rsa 0.9.x` release; remediation removes the unsupported MySQL dependency path |
| Opened | 2026-07-17 |
| Closed | 2026-07-20 |
| Closure reason | The base `sea-orm` workspace dependency now disables defaults while explicitly enabling only PostgreSQL, SQLite and Tokio/Rustls; lock refresh removed `sqlx-mysql` and `rsa` without package upgrades |
| Policy cleanup | Removed `RUSTSEC-2023-0071` from `.cargo/audit.toml`; `deny.toml` had no waiver |
| Verification | Dependency feature hygiene, locked all-feature metadata, absence of the `rsa` package in `Cargo.lock`, and a no-default server compile |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2023-0071.html> |

'''
updated = source[:active_start] + replacement + closed_tail
updated = updated.replace(
    "cargo tree -i rsa --workspace --all-features\n",
    "cargo tree -i rsa --workspace --all-features --target all\n",
)
updated = updated.replace(
    "cargo tree -i atomic-polyfill --workspace --all-features\n",
    "cargo tree -i atomic-polyfill --workspace --all-features --target all\n",
)
register.write_text(updated)

Path("docs/security/rsa-rustsec-2023-0071.md").write_text(
    '''---
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

The root `sea-orm` workspace dependency now sets `default-features = false` and explicitly
selects only PostgreSQL, SQLite, Tokio/Rustls and the data-type integrations used by RusToK.
This prevents the base ORM from enabling SQLx MySQL and its `rsa 0.9.10` dependency.

The lockfile refresh is constrained to the existing SeaORM version and rejects every added or
upgraded package identity. The `rsa` package and the unsupported MySQL path must disappear, while
the independent Athanor `atomic-polyfill` exception remains active with its actual all-target path.

## Verification

```bash
node scripts/verify/verify-dependency-feature-hygiene.mjs
node scripts/verify/verify-advisory-exceptions.mjs
cargo metadata --locked --all-features --format-version 1
cargo check --locked -p rustok-server --no-default-features
```

`Cargo.lock` must not contain a package named `rsa`, and the workspace feature verifier must reject
any future `sea-orm` or `sea-orm-migration` MySQL/default-feature drift.
'''
)
