---
id: doc://docs/verification/PLATFORM_HARDENING_IMPLEMENTATION_PLAN.md
kind: implementation_plan
language: markdown
source_language: markdown
status: active
---
# Platform Hardening Implementation Plan

## Purpose

This document is the execution plan for moving RusToK from an ambitious development platform to a reproducible, production-ready platform with explicit security, tenancy, compatibility, release and scale contracts.

The plan was revalidated against `main` on 2026-07-17 at commit `9c3a5f1b443d7fc0fa1dae8ee9b09a29d2edfb67`.

## Revalidation Summary

### Confirmed high-risk findings

1. The UI Content Security Policy still permits `unsafe-inline`, `unsafe-eval` and plaintext `http:` browser connections.
2. Header-based tenant resolution can still fall back silently to the default tenant when explicitly configured.
3. Unknown tenant resolution modes still fall back to the default tenant instead of failing closed.
4. Catalog routes still bypass tenant resolution without a documented global-catalog isolation contract.
5. `current_unix_ms()` still hides system clock anomalies through `unwrap_or_default()`.
6. `modules.toml.example` and `docs/modules/overview.md` still drift from the canonical `modules.toml` topology and dependency graph.
7. Browser E2E suites are documented, but the main required CI gate does not execute them.
8. `deny.toml` contains active advisory exceptions without an owner, expiry date and reachability evidence in a dedicated exception register.

### Findings that changed since the previous audit

1. The previously ignored `rustls-webpki` advisories are no longer present in `deny.toml`.
2. CI has materially improved and now includes MSRV validation, coverage enforcement, SBOM generation and provenance attestation.
3. The current advisory exceptions are `RUSTSEC-2026-0194` and `RUSTSEC-2026-0195` for a transitive `quick-xml` path and require a fresh reachability review.
4. Marketplace modules were added to the canonical manifest, increasing the documentation drift in the example manifest and central module overview.

## Execution Rules

- `modules.toml` is the canonical platform composition source.
- Security and tenant isolation changes must fail closed in production.
- Every exception must have an owner, rationale, compensating control and expiry date.
- Every public performance claim must be backed by a reproducible benchmark specification and archived result.
- A feature is not complete until its supported Rust, Next.js and mobile surfaces have compatibility evidence.
- Direct pushes to `main` are temporary for the initial stabilization batch only. After Phase 0, protected-branch required checks become mandatory.

## Priority Model

| Priority | Meaning | Target response |
|---|---|---|
| P0 | Cross-tenant exposure, authentication bypass, exploitable browser policy, known critical dependency issue | Fix or explicitly disable affected capability immediately |
| P1 | Production reliability, release integrity, missing required test gate, contract drift | Complete before production-ready declaration |
| P2 | Enterprise operations, compliance evidence, resilience, performance regression prevention | Complete before enterprise support |
| P3 | Hyper-scale isolation, regional topology, workload extraction and advanced automation | Execute after stable production baselines |

## Phase 0 — Baseline and Trust Restoration

**Goal:** establish a truthful source of truth and prevent regressions while urgent fixes land.

### Work items

- `HARD-001` Synchronize `modules.toml.example` and central module documentation with `modules.toml`.
- `HARD-002` Add a manifest/documentation drift verifier to `cargo xtask validate-manifest` or a dedicated CI script.
- `HARD-003` Add this plan to the documentation map and create a lightweight status ledger.
- `HARD-004` Create `docs/security/advisory-exceptions.md` with owner, affected dependency path, reachability, compensating controls and expiry.
- `HARD-005` Define branch protection: required `CI Success`, signed commits or verified bot identity, no force push, linear history or documented merge policy.
- `HARD-006` Remove unsupported benchmark numbers from README files until reproducible evidence exists.

### Exit criteria

- Canonical module topology and generated documentation are identical.
- Every advisory ignore has time-bounded evidence.
- README claims link to benchmark artifacts or are explicitly labeled as targets.
- Main branch protection is enabled after the initial direct-push stabilization batch.

## Phase 1 — Security and Tenant Isolation

**Goal:** remove fail-open behavior and establish verifiable isolation boundaries.

### Work items

- `HARD-101` Replace UI CSP with nonce/hash-based script and style policies; remove `unsafe-eval`; remove plaintext `http:` from production `connect-src`.
- `HARD-102` Add CSP report-only rollout, violation telemetry and an allowlist inventory before enforcement.
- `HARD-103` Bind HSTS to a validated production HTTPS deployment profile rather than an unvalidated standalone flag.
- `HARD-104` Make unknown tenant resolution modes a bootstrap error and request-time internal error, never a default-tenant fallback.
- `HARD-105` Restrict default-tenant fallback to explicitly declared single-tenant/development profiles and emit metrics whenever it is used.
- `HARD-106` Remove catalog routes from the tenant bypass list unless a reviewed global-catalog data model exists.
- `HARD-107` Add negative integration tests for missing, malformed and attacker-controlled tenant identifiers across REST, GraphQL, WebSocket and storefront paths.
- `HARD-108` Add database-level tenant integrity checks for every tenant-owned relation and validate query filters with integration tests.
- `HARD-109` Make system clock anomalies observable and test cache expiration behavior under clock skew.
- `HARD-110` Validate production JWT policy at bootstrap: allowed algorithms, issuer, audience, key rotation and secret quality.

### Exit criteria

- No request can resolve to a tenant by implicit fallback in production.
- Catalog/global routes have an approved isolation ADR and tests.
- Enforced CSP contains no `unsafe-eval` and no plaintext production connection source.
- Tenant isolation tests run as required CI checks.

## Phase 2 — Compatibility, Testing and Release Engineering

**Goal:** turn architecture promises into required, repeatable evidence.

### Work items

- `HARD-201` Add browser E2E jobs for `next-admin` and `next-frontend` to required CI.
- `HARD-202` Add Leptos admin/storefront smoke tests with the same core user journeys.
- `HARD-203` Add mobile package build/analyze/test matrix and API contract smoke tests.
- `HARD-204` Generate and diff OpenAPI and GraphQL compatibility artifacts on every pull request.
- `HARD-205` Add database migration compatibility tests: fresh install, N-1 upgrade, rollback-safe checks and data backfill verification.
- `HARD-206` Establish SemVer tags, signed release artifacts, container publication, checksums, SBOM and provenance attachment.
- `HARD-207` Convert `CHANGELOG.md` to release-oriented entries and move sprint progress to implementation plans or project tracking.
- `HARD-208` Publish a release readiness checklist covering migrations, security exceptions, compatibility, docs and rollback.

### Exit criteria

- Supported hosts have required smoke/E2E evidence.
- Releases are versioned, reproducible and include compatibility and migration notes.
- API breaking changes are detected before merge.

## Phase 3 — Production Readiness and Enterprise Operations

**Goal:** make the platform operable under defined SLOs and failure modes.

### Work items

- `HARD-301` Define SLIs/SLOs for API latency, error rate, availability, outbox lag, search lag, queue depth and tenant-resolution failures.
- `HARD-302` Add bounded concurrency, backpressure, timeouts and cancellation to every worker and outbound integration lane.
- `HARD-303` Add structured audit logs for authentication, authorization, tenant changes, privileged operations and configuration changes.
- `HARD-304` Add backup, point-in-time recovery and restore verification runbooks with scheduled restore drills.
- `HARD-305` Add secret rotation, key rotation and emergency credential revocation runbooks.
- `HARD-306` Add chaos and dependency degradation tests for PostgreSQL, Redis, search, event transport and storage.
- `HARD-307` Add per-tenant quotas, rate limits, storage budgets and noisy-neighbor protection.
- `HARD-308` Produce a compliance evidence pack: threat model, data-flow diagrams, access matrix, dependency inventory and exception register.

### Exit criteria

- Production SLO dashboards and alert policies are live.
- Restore drills and key rotation are tested, not only documented.
- Tenant resource isolation is measurable and enforceable.

## Phase 4 — Hyper-scale Architecture

**Goal:** scale independently without prematurely replacing the modular monolith.

### Work items

- `HARD-401` Profile workload lanes and extract only independently scaling paths: search/index reads, long-running jobs, outbound integrations and AI/operator execution.
- `HARD-402` Introduce durable queue partitioning, idempotency keys, replay policy and poison-message handling.
- `HARD-403` Add regional deployment topology, data residency policy and tenant placement controls.
- `HARD-404` Add cell-based or shard-based tenant placement to limit blast radius.
- `HARD-405` Add capacity models and automated load-shedding based on SLO error budgets.
- `HARD-406` Add continuous performance regression tests for representative read, write, GraphQL and background workloads.

### Exit criteria

- Scaling decisions are driven by measured bottlenecks.
- Failure domains and tenant placement are explicit.
- Performance claims are reproducible across documented hardware and topology profiles.

## Top 20 Ordered Backlog

1. `HARD-104` Fail closed on unknown tenant resolution modes.
2. `HARD-106` Review and remove unsafe catalog tenant bypass.
3. `HARD-105` Restrict and instrument default-tenant fallback.
4. `HARD-101` Enforce nonce/hash-based CSP and remove plaintext connections.
5. `HARD-107` Required negative tenant-isolation integration tests.
6. `HARD-004` Time-bounded security advisory exception register.
7. `HARD-001` Synchronize module manifest examples and documentation.
8. `HARD-002` Automated manifest/docs drift verification.
9. `HARD-201` Required browser E2E CI.
10. `HARD-204` API compatibility diff gates.
11. `HARD-205` Migration upgrade and rollback verification.
12. `HARD-206` Signed SemVer release workflow and artifacts.
13. `HARD-110` Production JWT bootstrap validation.
14. `HARD-301` SLI/SLO definitions and dashboards.
15. `HARD-302` Worker backpressure and cancellation policy.
16. `HARD-307` Per-tenant resource quotas.
17. `HARD-304` Restore drills and disaster recovery evidence.
18. `HARD-306` Dependency degradation and chaos tests.
19. `HARD-406` Reproducible performance regression suite.
20. `HARD-404` Cell/shard-based tenant blast-radius controls.

## Initial Stabilization Batch

The first implementation batch is intentionally low-risk and reviewable:

- add this execution plan;
- synchronize `modules.toml.example` and `docs/modules/overview.md` with the canonical manifest;
- harden the immediately safe CSP directives and add regression assertions;
- create follow-up tasks for nonce-based CSP and tenant fail-closed behavior where broader runtime changes are required.

## Validation Commands

Run the narrowest checks first, then the full gate:

```bash
cargo fmt --all -- --check
cargo test -p rustok-server middleware::security_headers
cargo xtask validate-manifest
cargo xtask module validate
cargo clippy --workspace --all-targets --no-deps -- -D warnings
cargo nextest run --workspace --all-targets --all-features
```

For UI compatibility phases:

```bash
npm --prefix apps/next-admin ci
npm --prefix apps/next-admin run test:e2e
npm --prefix apps/next-frontend ci
npm --prefix apps/next-frontend run test:e2e
```

## Progress Ledger

| Work item | Status | Evidence |
|---|---|---|
| `HARD-003` Add implementation plan | In progress | This document |
| `HARD-001` Synchronize manifest documentation | In progress | Initial stabilization batch |
| `HARD-101` CSP hardening | In progress | Safe directive hardening first; nonce rollout remains |
| `HARD-104` Tenant resolution fail-closed | Planned next | Requires runtime and configuration tests |
| `HARD-201` Required browser E2E | Planned | CI change after baseline stabilization |
