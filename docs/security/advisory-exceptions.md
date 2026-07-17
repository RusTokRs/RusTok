---
id: doc://docs/security/advisory-exceptions.md
kind: security_exception_register
language: markdown
source_language: markdown
status: active
---
# Security Advisory Exception Register

## Policy

An advisory may be ignored by automated dependency policy only when every field below is complete:

- accountable owner;
- affected package and dependency path;
- reachability analysis tied to concrete RusToK entry points;
- compensating controls;
- remediation plan;
- approval date and expiry date;
- evidence link to a test, issue, commit or threat-model note.

Exceptions expire automatically. An expired or incomplete entry must fail the dependency gate.
The repository-level enforcement entry point is `scripts/verify/verify-advisory-exceptions.mjs`,
which is also executed by `.github/workflows/hardening-gates.yml`.

The automated register governs both `deny.toml` and `.cargo/audit.toml`. An advisory present in
either ignore list must have one active entry below, and an active entry without a matching policy
waiver must also fail the gate.

## Active Exceptions

### RUSTSEC-2023-0071 — `rsa` timing side channel

| Field | Value |
|---|---|
| Severity | MEDIUM, CVSS 5.9 |
| Risk | Network-observable RSA private-key operations may leak timing information and enable key recovery |
| Patched version | No patched release is currently available |
| Repository policy location | `.cargo/audit.toml` |
| Accountable owner | Platform security / dependency maintainers |
| Dependency path | Workspace SeaORM consumers → `sea-orm 1.1.20` / `sea-orm-migration 1.1.20` → `sqlx 0.8.6` → `sqlx-mysql 0.8.6` → `rsa 0.9.10`; MySQL is not a declared RusToK database backend and is pulled by migration/SQLx default features |
| Reachability | The supported runtime database features are PostgreSQL and SQLite. No RusToK RSA private-key operation or MySQL connection path has been identified; the package remains in the resolved graph through unused default features until the lockfile is regenerated |
| Compensating controls | Do not enable the MySQL backend or introduce application RSA private-key operations; keep production database configuration restricted to PostgreSQL and SQLite |
| Remediation | Disable unused SeaORM migration CLI/SQLx MySQL defaults, regenerate `Cargo.lock`, confirm `cargo tree -i rsa` is empty, then remove this waiver |
| Approved | 2026-07-17, temporary dependency stabilization exception |
| Expires | 2026-07-24 |
| Evidence required | Root feature-policy diff, regenerated lockfile, empty `cargo tree -i rsa --workspace --all-features`, and `cargo audit` |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2023-0071.html> |

### RUSTSEC-2023-0089 — `atomic-polyfill` is unmaintained

| Field | Value |
|---|---|
| Severity | INFO, unmaintained dependency |
| Risk | Archived dependency receives no maintenance or security fixes and creates avoidable supply-chain exposure |
| Patched version | No patched release; recommended replacement is `portable-atomic` |
| Repository policy location | `.cargo/audit.toml` |
| Accountable owner | Platform security / dependency maintainers |
| Dependency path | `rustok-cache` / `rustok-iggy` → `postcard 1.1.3` default `heapless-cas` feature → `heapless 0.7.17` → `atomic-polyfill 1.0.3` |
| Reachability | RusToK uses Postcard standard-library and I/O APIs (`to_stdvec`, `to_io`, `from_bytes`, size flavor), not heapless serialization; the embedded-support dependency remains solely because Postcard defaults are enabled |
| Compensating controls | Do not add heapless Postcard APIs or embedded targets to production profiles; keep serialization on bounded std/I/O paths |
| Remediation | Disable Postcard default features while retaining `use-std`, regenerate `Cargo.lock`, confirm `cargo tree -i atomic-polyfill` is empty, then remove this waiver |
| Approved | 2026-07-17, temporary dependency stabilization exception |
| Expires | 2026-07-24 |
| Evidence required | Root feature-policy diff, regenerated lockfile, empty `cargo tree -i atomic-polyfill --workspace --all-features`, and serialization tests |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2023-0089.html> |

## Closed Exceptions

### RUSTSEC-2026-0098 — `rustls-webpki` URI name constraints

| Field | Value |
|---|---|
| Original risk | URI name constraints could be ignored during certificate validation |
| Patched version | `rustls-webpki >= 0.103.12, < 0.104.0-alpha.1` or `>= 0.104.0-alpha.6` |
| Resolved version | `rustls-webpki 0.103.13` in the current `Cargo.lock` |
| Opened | 2026-07-17 |
| Closed | 2026-07-17 |
| Closure reason | The resolved package is above the patched threshold |
| Policy cleanup | Removed from `.cargo/audit.toml` in `c663746c` |
| Verification | Run `node scripts/verify/verify-advisory-exceptions.mjs` and `cargo audit` |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2026-0098.html> |

### RUSTSEC-2026-0099 — `rustls-webpki` wildcard name constraints

| Field | Value |
|---|---|
| Original risk | A wildcard certificate could be accepted despite an applicable DNS name constraint |
| Patched version | `rustls-webpki >= 0.103.12, < 0.104.0-alpha.1` or `>= 0.104.0-alpha.6` |
| Resolved version | `rustls-webpki 0.103.13` in the current `Cargo.lock` |
| Opened | 2026-07-17 |
| Closed | 2026-07-17 |
| Closure reason | The resolved package is above the patched threshold |
| Policy cleanup | Removed from `.cargo/audit.toml` in `c663746c` |
| Verification | Run `node scripts/verify/verify-advisory-exceptions.mjs` and `cargo audit` |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2026-0099.html> |

### RUSTSEC-2026-0104 — `rustls-webpki` CRL parsing panic

| Field | Value |
|---|---|
| Original risk | A syntactically valid crafted CRL could trigger a panic before signature verification |
| Patched version | `rustls-webpki >= 0.103.13, < 0.104.0-alpha.1` or `>= 0.104.0-alpha.7` |
| Resolved version | `rustls-webpki 0.103.13` in the current `Cargo.lock` |
| Opened | 2026-07-17 |
| Closed | 2026-07-17 |
| Closure reason | The resolved package meets the patched threshold |
| Policy cleanup | Removed from `.cargo/audit.toml` in `c663746c` |
| Verification | Run `node scripts/verify/verify-advisory-exceptions.mjs` and `cargo audit` |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2026-0104.html> |

### RUSTSEC-2026-0194 — `quick-xml` quadratic attribute processing

| Field | Value |
|---|---|
| Original severity | HIGH, CVSS 7.5 |
| Original risk | CPU-exhaustion denial of service while parsing attacker-controlled XML attributes |
| Patched version | `quick-xml >= 0.41.0` |
| Opened | 2026-07-17 |
| Closed | 2026-07-17 |
| Closure reason | The current `Cargo.lock` package list contains no `quick-xml` package, so the vulnerable dependency is no longer present in the resolved workspace graph |
| Policy cleanup | Removed from `deny.toml` and `.cargo/audit.toml` |
| Verification | Search the lockfile package list and run `cargo deny check advisories --all-features` plus `cargo audit` |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2026-0194.html> |

### RUSTSEC-2026-0195 — `quick-xml` unbounded namespace allocation

| Field | Value |
|---|---|
| Original severity | HIGH, CVSS 7.5 |
| Original risk | Memory-exhaustion denial of service through `NsReader` or direct namespace resolver use |
| Patched version | `quick-xml >= 0.41.0` |
| Opened | 2026-07-17 |
| Closed | 2026-07-17 |
| Closure reason | The current `Cargo.lock` package list contains no `quick-xml` package, so the vulnerable dependency is no longer present in the resolved workspace graph |
| Policy cleanup | Removed from `deny.toml` and `.cargo/audit.toml` |
| Verification | Search the lockfile package list and run `cargo deny check advisories --all-features` plus `cargo audit` |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2026-0195.html> |

## Required Verification

```bash
node scripts/verify/verify-advisory-exceptions.mjs
cargo tree -i rsa --workspace --all-features
cargo tree -i atomic-polyfill --workspace --all-features
cargo deny check advisories --all-features
cargo audit
```

The preferred resolution is dependency remediation or removal, not extension of an exception.
Any future exception requires a new dated approval, current dependency-path evidence and a
short compensating-control review cycle.
