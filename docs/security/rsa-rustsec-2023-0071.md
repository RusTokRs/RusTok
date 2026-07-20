---
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
