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

## Active Exceptions

### RUSTSEC-2026-0194 — `quick-xml` quadratic attribute processing

| Field | Value |
|---|---|
| Severity | HIGH, CVSS 7.5 |
| Risk | CPU-exhaustion denial of service while parsing attacker-controlled XML attributes |
| Patched version | `quick-xml >= 0.41.0` |
| Repository policy location | `deny.toml` |
| Accountable owner | Platform security / dependency maintainers |
| Dependency path | **Required before approval:** record the complete `cargo tree -i quick-xml` path |
| Reachability | **Unproven.** Treat as reachable until all XML parser entry points are identified and tested |
| Compensating controls | Input-size limits alone are insufficient for the quadratic parser path; isolate parsing from request-critical executors and reject untrusted XML until remediated |
| Remediation | Upgrade or override the dependency graph to `quick-xml >= 0.41.0` |
| Approved | 2026-07-17, temporary stabilization exception |
| Expires | 2026-07-24 |
| Evidence required | Dependency-path output, parser call-site inventory and a regression test or proof that vulnerable APIs are unreachable |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2026-0194.html> |

### RUSTSEC-2026-0195 — `quick-xml` unbounded namespace allocation

| Field | Value |
|---|---|
| Severity | HIGH, CVSS 7.5 |
| Risk | Memory-exhaustion denial of service through `NsReader` or direct namespace resolver use |
| Patched version | `quick-xml >= 0.41.0` |
| Repository policy location | `deny.toml` |
| Accountable owner | Platform security / dependency maintainers |
| Dependency path | **Required before approval:** record the complete `cargo tree -i quick-xml` path |
| Reachability | **Unproven.** Treat as reachable until `NsReader` and namespace resolver call sites are excluded or bounded |
| Compensating controls | Reject untrusted XML and isolate XML parsing; caller-side input limits do not fully bound the vulnerable namespace allocation |
| Remediation | Upgrade or override the dependency graph to `quick-xml >= 0.41.0` |
| Approved | 2026-07-17, temporary stabilization exception |
| Expires | 2026-07-24 |
| Evidence required | Dependency-path output, parser call-site inventory and a regression test or proof that `NsReader` is unreachable |
| Upstream advisory | <https://rustsec.org/advisories/RUSTSEC-2026-0195.html> |

## Required Verification

```bash
cargo tree -i quick-xml --workspace --all-features
cargo deny check advisories --all-features
cargo audit
```

The preferred resolution is dependency remediation, not extension of the exception. Any extension requires a new dated approval, updated reachability evidence and a shorter compensating-control review cycle.
