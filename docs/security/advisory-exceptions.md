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
| Reachability | Locked metadata retains a target-conditioned optional path for AVR, RISC-V, Thumb v6-M and Xtensa, but locked inverse trees are empty for `--target all` and each representative target triple; no workspace build currently selects `atomic-polyfill`, while `cargo audit` still reports its lockfile presence |
| Compensating controls | The permanent reachability workflow requires empty inverse trees for the all-target graph and representative embedded targets; production profiles target supported server operating systems; keep `athanor-surreal` optional and disabled by default |
| Remediation | Remove the waiver only when an Athanor/SurrealDB dependency update, bounded fork or lockfile/tooling change removes the lock-only package, or when the parent chain adopts `portable-atomic`; never delete lockfile package blocks manually |
| Approved | 2026-07-17, temporary dependency stabilization exception |
| Expires | 2026-07-24 |
| Evidence required | Empty locked inverse trees for `--target all` and representative embedded triples, locked metadata for the target-conditioned optional path, Athanor upstream remediation, and capability-specific integration tests |
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
cargo tree --locked -i rsa --workspace --all-features --target all
cargo tree --locked -i atomic-polyfill --workspace --all-features --target all
cargo deny check advisories --all-features
cargo audit
```

The preferred resolution is dependency remediation or removal, not extension of an exception.
Any future exception requires a new dated approval, current dependency-path evidence and a
short compensating-control review cycle.
