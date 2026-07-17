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

The automated register currently governs `deny.toml`. Legacy `.cargo/audit.toml` waivers
must be migrated into this register in a separate hardening batch before the same gate is
extended to cargo-audit policy.

## Active Exceptions

None.

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
cargo deny check advisories --all-features
cargo audit
```

The preferred resolution is dependency remediation or removal, not extension of an exception.
Any future exception requires a new dated approval, current dependency-path evidence and a
short compensating-control review cycle.
