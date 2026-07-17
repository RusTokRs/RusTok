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
| Dependency path | Resolved transitive `rsa` package; capture the complete reverse path with `cargo tree -i rsa --workspace --all-features` before renewal |
| Reachability | Direct RusToK RSA private-key decryption call sites have not been identified; treat as reachable until the reverse path and call-site inventory prove otherwise |
| Compensating controls | Do not introduce application RSA decryption through this crate; keep private-key operations outside attacker-observable request paths and prefer maintained constant-time implementations |
| Remediation | Remove the transitive path or migrate to a maintained constant-time implementation when available |
| Approved | 2026-07-17, temporary dependency stabilization exception |
| Expires | 2026-07-24 |
| Evidence required | `cargo tree -i rsa --workspace --all-features`, RSA call-site inventory and an upstream remediation issue |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2023-0071.html> |

### RUSTSEC-2023-0089 — `atomic-polyfill` is unmaintained

| Field | Value |
|---|---|
| Severity | INFO, unmaintained dependency |
| Risk | Archived dependency receives no maintenance or security fixes and creates avoidable supply-chain exposure |
| Patched version | No patched release; recommended replacement is `portable-atomic` |
| Repository policy location | `.cargo/audit.toml` |
| Accountable owner | Platform security / dependency maintainers |
| Dependency path | `atomic-polyfill 1.0.3` is present in `Cargo.lock`; capture the complete reverse path with `cargo tree -i atomic-polyfill --workspace --all-features` before renewal |
| Reachability | The package is expected to be target-specific transitive support, but target reachability has not yet been proven; treat the dependency as active supply-chain debt |
| Compensating controls | Do not add direct usage; keep supported deployment targets on platforms with native atomics and review build output for unexpected embedded-target activation |
| Remediation | Upgrade the owning dependency chain to a version using `portable-atomic` or remove the parent dependency |
| Approved | 2026-07-17, temporary dependency stabilization exception |
| Expires | 2026-07-24 |
| Evidence required | `cargo tree -i atomic-polyfill --workspace --all-features`, target-specific feature output and parent-upgrade issue |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2023-0089.html> |

### RUSTSEC-2026-0098 — `rustls-webpki` URI name constraints

| Field | Value |
|---|---|
| Severity | Vulnerability; exploitation requires certificate misissuance |
| Risk | URI name constraints can be ignored, allowing an otherwise valid but misissued certificate outside the intended constraint |
| Patched version | `rustls-webpki >= 0.103.12, < 0.104.0-alpha.1` or `>= 0.104.0-alpha.6` |
| Repository policy location | `.cargo/audit.toml` |
| Accountable owner | Platform security / dependency maintainers |
| Dependency path | Legacy `rustls-webpki` through the AWS Smithy / rustls 0.21 dependency lane; confirm with `cargo tree -i rustls-webpki --workspace --all-features` |
| Reachability | Potentially reachable in outbound TLS certificate validation; exploitation additionally requires a misissued constrained certificate |
| Compensating controls | Restrict outbound destinations, use trusted public/private CAs with monitored issuance and do not rely on URI name constraints as an authorization boundary |
| Remediation | Upgrade the AWS Smithy/rustls dependency lane to a patched `rustls-webpki` generation |
| Approved | 2026-07-17, temporary upstream-blocked exception |
| Expires | 2026-07-24 |
| Evidence required | Reverse dependency tree, outbound TLS inventory and upstream upgrade issue |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2026-0098.html> |

### RUSTSEC-2026-0099 — `rustls-webpki` wildcard name constraints

| Field | Value |
|---|---|
| Severity | Vulnerability; exploitation requires certificate misissuance |
| Risk | A wildcard certificate may be accepted despite a DNS name constraint that should exclude the asserted name |
| Patched version | `rustls-webpki >= 0.103.12, < 0.104.0-alpha.1` or `>= 0.104.0-alpha.6` |
| Repository policy location | `.cargo/audit.toml` |
| Accountable owner | Platform security / dependency maintainers |
| Dependency path | Legacy `rustls-webpki` through the AWS Smithy / rustls 0.21 dependency lane; confirm with `cargo tree -i rustls-webpki --workspace --all-features` |
| Reachability | Potentially reachable in outbound TLS certificate validation; exploitation additionally requires a misissued wildcard certificate and applicable name constraints |
| Compensating controls | Restrict outbound destinations, monitor certificate issuance and avoid treating constrained wildcard certificates as an application authorization control |
| Remediation | Upgrade the AWS Smithy/rustls dependency lane to a patched `rustls-webpki` generation |
| Approved | 2026-07-17, temporary upstream-blocked exception |
| Expires | 2026-07-24 |
| Evidence required | Reverse dependency tree, outbound TLS inventory and upstream upgrade issue |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2026-0099.html> |

### RUSTSEC-2026-0104 — `rustls-webpki` CRL parsing panic

| Field | Value |
|---|---|
| Severity | Denial-of-service vulnerability |
| Risk | Parsing a syntactically valid crafted CRL can panic before CRL signature verification |
| Patched version | `rustls-webpki >= 0.103.13, < 0.104.0-alpha.1` or `>= 0.104.0-alpha.7` |
| Repository policy location | `.cargo/audit.toml` |
| Accountable owner | Platform security / dependency maintainers |
| Dependency path | Legacy `rustls-webpki` through the AWS Smithy / rustls 0.21 dependency lane; confirm with `cargo tree -i rustls-webpki --workspace --all-features` |
| Reachability | No RusToK CRL ingestion configuration has been identified; treat as reachable until TLS configuration and all revocation-list entry points are inventoried |
| Compensating controls | Do not load attacker-controlled CRLs, keep CRL parsing disabled unless required and isolate any future revocation-list processing from request-critical executors |
| Remediation | Upgrade the AWS Smithy/rustls dependency lane to `rustls-webpki >= 0.103.13` or another patched branch |
| Approved | 2026-07-17, temporary upstream-blocked exception |
| Expires | 2026-07-24 |
| Evidence required | Reverse dependency tree, CRL configuration inventory and upstream upgrade issue |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2026-0104.html> |

## Closed Exceptions

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
cargo tree -i rustls-webpki --workspace --all-features
cargo deny check advisories --all-features
cargo audit
```

The preferred resolution is dependency remediation or removal, not extension of an exception.
Any future exception requires a new dated approval, current dependency-path evidence and a
short compensating-control review cycle.
