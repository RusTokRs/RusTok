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
    '''sea-orm = { version = "1.1", default-features = false, features = [
    "sqlx-postgres",
    "sqlx-sqlite",
    "runtime-tokio-rustls",
    "macros",
    "with-uuid",
    "with-chrono",
    "with-json",
    "with-rust_decimal",
    "with-bigdecimal",
    "with-time",
] }
''',
)

Path("scripts/verify/verify-dependency-feature-hygiene.mjs").write_text(
    r'''#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function dependencySpecs(source, name) {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const starts = [...source.matchAll(new RegExp(`^${escaped}\\s*=\\s*\\{`, "gm"))];
  return starts.map((match) => {
    const end = source.indexOf("}", match.index);
    if (end < 0) {
      failures.push(`${name}: dependency specification has no closing brace`);
      return source.slice(match.index);
    }
    return source.slice(match.index, end + 1);
  });
}

function dependencySpec(source, name) {
  return dependencySpecs(source, name)[0] ?? null;
}

function requireSpec(spec, marker, dependency) {
  if (!spec?.includes(marker)) {
    failures.push(`${dependency}: dependency specification must include ${marker}`);
  }
}

function forbidSpec(spec, marker, dependency) {
  if (spec?.includes(marker)) {
    failures.push(`${dependency}: dependency specification must not include ${marker}`);
  }
}

function walkFiles(relativeRoot, predicate) {
  const absoluteRoot = path.join(repoRoot, relativeRoot);
  if (!fs.existsSync(absoluteRoot)) {
    return [];
  }

  const files = [];
  const stack = [absoluteRoot];
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      if (entry.name === "target" || entry.name === "node_modules" || entry.name === ".git") {
        continue;
      }
      const absolute = path.join(current, entry.name);
      if (entry.isDirectory()) {
        stack.push(absolute);
      } else if (entry.isFile() && predicate(entry.name)) {
        files.push(absolute);
      }
    }
  }
  return files;
}

const parserFixture = `sea-orm = { version = "1.1", default-features = false, features = [
  "sqlx-postgres",
  "sqlx-sqlite",
] }`;
if (!dependencySpec(parserFixture, "sea-orm")?.includes('"sqlx-sqlite"')) {
  failures.push("dependency parser must retain multiline inline-table specifications");
}

const cargo = read("Cargo.toml");
const orm = dependencySpec(cargo, "sea-orm");
const migration = dependencySpec(cargo, "sea-orm-migration");
const postcard = dependencySpec(cargo, "postcard");

if (!orm) {
  failures.push("Cargo.toml: sea-orm workspace dependency not found");
} else {
  requireSpec(orm, "default-features = false", "sea-orm");
  for (const feature of [
    '"sqlx-postgres"',
    '"sqlx-sqlite"',
    '"runtime-tokio-rustls"',
    '"macros"',
    '"with-uuid"',
    '"with-chrono"',
    '"with-json"',
    '"with-rust_decimal"',
    '"with-bigdecimal"',
    '"with-time"',
  ]) {
    requireSpec(orm, feature, "sea-orm");
  }
  forbidSpec(orm, '"sqlx-mysql"', "sea-orm");
  forbidSpec(orm, '"sqlx-all"', "sea-orm");
  forbidSpec(orm, '"runtime-tokio-native-tls"', "sea-orm");
}

if (!migration) {
  failures.push("Cargo.toml: sea-orm-migration workspace dependency not found");
} else {
  requireSpec(migration, "default-features = false", "sea-orm-migration");
  requireSpec(migration, '"sqlx-postgres"', "sea-orm-migration");
  requireSpec(migration, '"sqlx-sqlite"', "sea-orm-migration");
  requireSpec(migration, '"runtime-tokio-rustls"', "sea-orm-migration");
  forbidSpec(migration, '"sqlx-mysql"', "sea-orm-migration");
  forbidSpec(migration, '"sqlx-all"', "sea-orm-migration");
  forbidSpec(migration, '"cli"', "sea-orm-migration");
}

if (!postcard) {
  failures.push("Cargo.toml: postcard workspace dependency not found");
} else {
  requireSpec(postcard, "default-features = false", "postcard");
  requireSpec(postcard, '"use-std"', "postcard");
  forbidSpec(postcard, '"heapless"', "postcard");
  forbidSpec(postcard, '"heapless-cas"', "postcard");
}

const memberManifests = [
  ...walkFiles("apps", (name) => name === "Cargo.toml"),
  ...walkFiles("crates", (name) => name === "Cargo.toml"),
  ...walkFiles("xtask", (name) => name === "Cargo.toml"),
  ...walkFiles("tests", (name) => name === "Cargo.toml"),
  ...walkFiles("ops", (name) => name === "Cargo.toml"),
  ...walkFiles("UI", (name) => name === "Cargo.toml"),
];

for (const absolutePath of memberManifests) {
  const source = fs.readFileSync(absolutePath, "utf8");
  const relativePath = path.relative(repoRoot, absolutePath);
  for (const dependency of ["sea-orm", "sea-orm-migration", "postcard"]) {
    for (const spec of dependencySpecs(source, dependency)) {
      if (!spec.includes("workspace = true")) {
        failures.push(`${relativePath}: ${dependency} must inherit the workspace dependency policy`);
      }
      for (const forbidden of [
        "default-features = true",
        '"cli"',
        '"sqlx-mysql"',
        '"sqlx-all"',
        '"runtime-tokio-native-tls"',
        '"heapless"',
        '"heapless-cas"',
      ]) {
        if (spec.includes(forbidden)) {
          failures.push(`${relativePath}: ${dependency} member override must not include ${forbidden}`);
        }
      }
    }
  }
}

const forbiddenRustPatterns = [
  ["sea_orm_migration::cli", "SeaORM migration CLI API"],
  ["sea_orm_cli", "sea-orm-cli API"],
  ["postcard::to_vec", "Postcard heapless vector API"],
  ["postcard::to_vec_cobs", "Postcard heapless COBS API"],
];

for (const absolutePath of [
  ...walkFiles("apps", (name) => name.endsWith(".rs")),
  ...walkFiles("crates", (name) => name.endsWith(".rs")),
  ...walkFiles("xtask", (name) => name.endsWith(".rs")),
  ...walkFiles("tests", (name) => name.endsWith(".rs")),
]) {
  const source = fs.readFileSync(absolutePath, "utf8");
  const relativePath = path.relative(repoRoot, absolutePath);
  for (const [pattern, description] of forbiddenRustPatterns) {
    if (source.includes(pattern)) {
      failures.push(`${relativePath}: ${description} is forbidden by workspace feature policy`);
    }
  }
}

if (failures.length > 0) {
  console.error("Dependency feature hygiene verification failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ SeaORM feature parity is explicit while MySQL/native-TLS and Postcard heapless defaults remain disabled");
'''
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

### RUSTSEC-2023-0071 — `rsa` timing side channel

| Field | Value |
|---|---|
| Severity | MEDIUM, CVSS 5.9 |
| Risk | Network-observable RSA private-key operations may leak timing information and enable key recovery if an affected private-key implementation becomes runtime reachable |
| Patched version | No patched `rsa 0.9.x` release is currently available |
| Repository policy location | `.cargo/audit.toml` |
| Accountable owner | Platform security / dependency maintainers |
| Dependency path | `Cargo.lock` retains the optional SQLx MySQL package path `sqlx-mysql 0.8.6` → `rsa 0.9.10`; the root `sea-orm` and `sea-orm-migration` specifications disable defaults and select only PostgreSQL, SQLite and Tokio/Rustls while explicitly preserving the prior data-type integrations |
| Reachability | `cargo tree --locked --workspace --all-features --target all -i rsa` has empty stdout, so no workspace package or supported target selects the RSA path; `cargo audit` still reports the package because it is present in the lockfile |
| Compensating controls | The permanent feature-hygiene verifier forbids SeaORM/SQLx MySQL, `sqlx-all`, migration CLI and native-TLS drift; supported database backends remain PostgreSQL and SQLite; application JWT verification uses `jsonwebtoken/aws_lc_rs` |
| Remediation | Remove the waiver only when an upstream SeaORM/SQLx update, a bounded fork or lockfile/tooling behavior removes the lock-only optional MySQL/RSA package, or when a patched RSA release resolves the advisory; never delete lockfile package blocks manually |
| Approved | 2026-07-17, temporary dependency stabilization exception |
| Expires | 2026-07-24 |
| Evidence required | Explicit-parity SeaORM feature policy, empty locked all-feature/all-target inverse tree, permanent dependency reachability workflow, and `cargo audit` output |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2023-0071.html> |

### RUSTSEC-2023-0089 — `atomic-polyfill` is unmaintained

| Field | Value |
|---|---|
| Severity | INFO, unmaintained dependency |
| Risk | Archived dependency receives no maintenance or security fixes and creates avoidable supply-chain exposure on embedded target graphs |
| Patched version | No patched release; recommended replacement is `portable-atomic` |
| Repository policy location | `.cargo/audit.toml` |
| Accountable owner | Platform security / Athanor integration maintainers |
| Dependency path | `rustok-ai-athanor` feature `athanor-surreal` → `athanor-runtime-defaults/store-surreal` → `athanor-store-surrealdb` → `surrealdb 2.6.5` embedded engines → `geo 0.28.0` → `geo-types 0.7.19` → `rstar 0.9.3` → `heapless 0.7.17` → `atomic-polyfill 1.0.3` |
| Reachability | The final edge is selected only for AVR, `riscv32i-unknown-none-elf`, `riscv32imc-unknown-none-elf`, `thumbv6m-none-eabi` and `xtensa-esp32s2-none-elf`; it is absent from supported RusToK server targets but intentionally appears in the all-target graph because embedded SurrealDB storage is an optional Athanor capability |
| Compensating controls | Production deployment profiles target supported server operating systems only; do not ship RusToK or the Athanor Surreal adapter to the affected embedded target families; keep `athanor-surreal` optional and disabled by default |
| Remediation | Upgrade the Athanor/SurrealDB dependency path to a `geo`/`rstar`/`heapless` graph that uses `portable-atomic`, or formally remove the embedded SurrealDB capability if it is no longer required; do not prune the reachable lock entry |
| Approved | 2026-07-17, temporary dependency stabilization exception |
| Expires | 2026-07-24 |
| Evidence required | Locked `cargo tree -i atomic-polyfill --workspace --all-features --target all`, Athanor upstream remediation, and capability-specific integration tests |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2023-0089.html> |

## Closed Exceptions

'''
updated = source[:active_start] + replacement + closed_tail
updated = updated.replace(
    "cargo tree -i rsa --workspace --all-features\n",
    "cargo tree --locked -i rsa --workspace --all-features --target all\n",
)
updated = updated.replace(
    "cargo tree -i atomic-polyfill --workspace --all-features\n",
    "cargo tree --locked -i atomic-polyfill --workspace --all-features --target all\n",
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

# RUSTSEC-2023-0071 containment note

## Status

Contained; the temporary audit exception remains active through July 24, 2026.

RusToK uses `jsonwebtoken/aws_lc_rs` for application JWT verification. The root `sea-orm`
and `sea-orm-migration` workspace dependencies disable default features and explicitly select only
PostgreSQL, SQLite, Tokio/Rustls and the same data-type integrations that were previously supplied
by SeaORM defaults. The permanent feature-hygiene verifier rejects MySQL, `sqlx-all`, native-TLS
and migration CLI drift without reducing the supported ORM type surface.

The locked all-workspace, all-feature, all-target inverse tree for `rsa` is empty. The package
remains as a lockfile-only optional dependency of the SQLx MySQL path, so `cargo audit` still reports
RUSTSEC-2023-0071 even though no RusToK build selects or compiles it. The waiver must remain until
upstream dependency or lockfile behavior removes that package, or a patched RSA release resolves
the advisory.

## Verification

```bash
node scripts/verify/verify-dependency-feature-hygiene.mjs
node scripts/verify/verify-advisory-exceptions.mjs
cargo tree --locked --workspace --all-features --target all -i rsa
cargo check --locked -p rustok-server --no-default-features
cargo audit
```

The inverse-tree command must produce no dependency tree. Do not manually delete package blocks
from `Cargo.lock`; removal must come from a reproducible dependency or tooling change.
'''
)
