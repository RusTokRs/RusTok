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
| Risk | Network-observable RSA private-key operations could leak timing information if the affected private-key implementation became runtime reachable |
| Patched version | No patched `rsa 0.9.x` release is available |
| Repository policy location | `.cargo/audit.toml` |
| Accountable owner | Platform security / dependency maintainers |
| Dependency path | Lockfile-only optional SQLx MySQL path: `sqlx-mysql 0.8.6` → `rsa 0.9.10`; workspace SeaORM/SQLx policy selects PostgreSQL and SQLite only |
| Reachability | `cargo tree --locked --workspace --all-features --target all -i rsa` has empty stdout; no workspace package or supported target selects this path |
| Compensating controls | Feature-hygiene verification forbids SeaORM/SQLx MySQL, `sqlx-all`, migration CLI and native-TLS drift; supported database backends remain PostgreSQL and SQLite |
| Remediation | Remove the waiver when upstream Cargo/SQLx metadata no longer retains the optional path, or a patched upstream release becomes available; never delete lockfile blocks manually |
| Approved | 2026-08-13, lockfile-only reachability exception |
| Expires | 2026-09-13 |
| Evidence required | Empty locked all-feature/all-target inverse tree, feature-hygiene verification, and `cargo audit` output |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2023-0071.html> |

### RUSTSEC-2026-0235 — `rkyv` insufficient archive validation

| Field | Value |
|---|---|
| Severity | Unspecified by advisory |
| Risk | Malformed archives with `Rc` or `Arc` could cause out-of-bounds reads if the affected archival runtime became reachable |
| Patched version | `rkyv >= 0.8.17`; the retained optional dependency is constrained to `rkyv 0.7` by `rust_decimal 1.42.1` |
| Repository policy location | `.cargo/audit.toml` |
| Accountable owner | Platform security / dependency maintainers |
| Dependency path | Lockfile-only optional path: `rust_decimal 1.42.1` → `rkyv 0.7.46`; the workspace uses `rust_decimal` without its `rkyv` feature |
| Reachability | `cargo tree --locked --workspace --all-features --target all -i rkyv` has empty stdout; no workspace package or supported target selects this path |
| Compensating controls | All feature combinations are checked through the workspace graph; the finance/domain code serializes through canonical Serde boundaries rather than Rkyv archives |
| Remediation | Remove the waiver when `rust_decimal` updates its optional archival dependency to a patched major line or Cargo stops retaining unused optional paths; never delete lockfile blocks manually |
| Approved | 2026-08-13, lockfile-only reachability exception |
| Expires | 2026-09-13 |
| Evidence required | Empty locked all-feature/all-target inverse tree, workspace feature verification, and `cargo audit` output |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2026-0235.html> |

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
cargo tree --locked -i rsa --workspace --all-features --target all
cargo tree --locked -i atomic-polyfill --workspace --all-features --target all
cargo deny check advisories --all-features
cargo audit
```

The preferred resolution is dependency remediation or removal, not extension of an exception.
Any future exception requires a new dated approval, current dependency-path evidence and a
short compensating-control review cycle.
