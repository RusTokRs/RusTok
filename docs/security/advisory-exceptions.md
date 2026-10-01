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

### RUSTSEC-2026-0253 — `lru` lack of panic safety in `LruCache::pop()`

| Field | Value |
|---|---|
| Severity | Unsound |
| Risk | Potential use-after-free or double-free in `LruCache::pop()` if a key's `Drop` panics during eviction under `catch_unwind` |
| Patched version | `lru >= 0.18.2`; the transitive dependency is constrained to `lru 0.16.4` by upstream `async-graphql 7.2.1` and `tantivy 0.22` |
| Repository policy location | `deny.toml`, `.cargo/audit.toml` |
| Accountable owner | Platform security / dependency maintainers |
| Dependency path | Transitive dependency via `async-graphql 7.2.1` and `tantivy 0.22` |
| Reachability | Neither `async-graphql` nor `tantivy` use panicking `Drop` types as cache keys; panics in RusToK domain keys are not caught with unwinding suppression |
| Compensating controls | RusToK uses standard structured error handling without panicking drop types; cache keys in GraphQL and search engines are primitive strings and integers |
| Remediation | Upgrade `async-graphql` and `tantivy` when upstreams adopt `lru >= 0.18.2` or release non-breaking patch releases |
| Approved | 2026-10-01, transitive dependency exception |
| Expires | 2026-11-01 |
| Evidence required | `cargo audit` and `cargo deny check` output, inverse dependency tree via `cargo tree -i lru` |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2026-0253.html> |

## Closed Exceptions

### RUSTSEC-2026-0235 — `rkyv` insufficient archive validation

| Field | Value |
|---|---|
| Original risk | Malformed archives with `Rc` or `Arc` could cause out-of-bounds reads if the affected archival runtime became reachable |
| Patched version | `rkyv >= 0.8.17` |
| Resolved version | `rkyv 0.8.18` in the current `Cargo.lock` |
| Opened | 2026-08-13 |
| Closed | 2026-10-01 |
| Closure reason | The resolved package `0.8.18` is above the patched threshold |
| Policy cleanup | Removed from `.cargo/audit.toml` |
| Verification | Run `node scripts/verify/verify-advisory-exceptions.mjs` and `cargo audit` |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2026-0235.html> |

### RUSTSEC-2023-0071 — `rsa` timing side channel

| Field | Value |
|---|---|
| Original risk | Network-observable RSA private-key operations could leak timing information if the affected private-key implementation became runtime reachable |
| Patched version | No patched `rsa 0.9.x` release is available |
| Resolved version | Removed from `Cargo.lock` following `sqlx-mysql 0.9.0` dependency modernization |
| Opened | 2026-08-13 |
| Closed | 2026-09-11 |
| Closure reason | `sqlx-mysql 0.9.0` dropped `rsa`; the package is no longer present in `Cargo.lock` |
| Policy cleanup | Removed from `.cargo/audit.toml` |
| Verification | Run `node scripts/verify/verify-advisory-exceptions.mjs` and `cargo audit` |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2023-0071.html> |

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
