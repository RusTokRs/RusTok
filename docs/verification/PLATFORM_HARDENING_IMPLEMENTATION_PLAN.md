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

The plan was initially revalidated against `main` on 2026-07-17 at commit `9c3a5f1b443d7fc0fa1dae8ee9b09a29d2edfb67`. The progress ledger was refreshed on 2026-07-17 after the first implementation batches through commit `4d9cbb0300ddcda40231109a0ca87a9323973a66`.

## Current Revalidation Summary

### Confirmed open high-risk findings

1. The UI Content Security Policy still permits `unsafe-inline` and `unsafe-eval`; nonce/hash migration remains required.
2. `resolve_identifier` still has a request-time catch-all arm that selects the default tenant. Normal executable startup now rejects unknown modes, but the middleware must also fail closed as defense in depth.
3. Default-tenant fallback remains available in non-production header mode and does not yet emit dedicated usage metrics.
4. The legacy tenant resolver `current_unix_ms()` still hides pre-epoch system clock anomalies through `unwrap_or_default()`; the durable tenant cache generation path has already been corrected separately.
5. Browser E2E now runs in a dedicated workflow, but repository branch protection has not yet been verified to require that workflow.
6. Legacy advisory exceptions remain in `.cargo/audit.toml` and are not yet governed by the repository exception register.
7. Production JWT algorithm, issuer, audience, rotation and secret-quality requirements still need a bootstrap policy gate.

### Findings closed or materially reduced

1. Plaintext `http:` was removed from the UI CSP `connect-src`, and object/plugin content is blocked.
2. Unknown tenant resolution modes now fail executable bootstrap instead of silently starting with an unsafe configuration.
3. `DefaultTenant` fallback is forbidden in production and rejected outside header mode.
4. Subdomain tenant resolution now requires at least one configured base domain at bootstrap.
5. Production startup now requires an explicit HTTPS deployment declaration, and HSTS flag parsing is normalized.
6. The `/v1/catalog*` bypass was reviewed and documented as a global read-only registry boundary; `/v2/catalog/*` mutation routes remain tenant-bound.
7. `modules.toml.example` and `docs/modules/overview.md` were synchronized with `modules.toml`, and an automated drift gate now protects them.
8. The stale `quick-xml` advisory waivers were removed after confirming that the package is absent from the resolved `Cargo.lock` graph.
9. A dedicated browser Playwright matrix now runs smoke tests for `next-admin` and `next-frontend`.
10. Durable tenant cache generation publication now aborts and logs an error rather than emitting timestamp zero on a pre-epoch clock anomaly.

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
- Catalog/global routes have an approved isolation decision and tests.
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

1. Complete `HARD-104` by removing the request-time default-tenant catch-all from the legacy resolver.
2. Complete `HARD-105` with fallback usage metrics and an explicit development/single-tenant profile contract.
3. `HARD-101` Enforce nonce/hash-based CSP and remove `unsafe-eval`.
4. `HARD-107` Required negative tenant-isolation integration tests.
5. Complete `HARD-109` in the legacy tenant resolver and cache expiration paths.
6. Complete `HARD-004` by migrating legacy `.cargo/audit.toml` waivers into the exception register or removing them.
7. `HARD-110` Production JWT bootstrap validation.
8. Make `HARD-201` a required branch-protection check.
9. `HARD-204` API compatibility diff gates.
10. `HARD-205` Migration upgrade and rollback verification.
11. `HARD-206` Signed SemVer release workflow and artifacts.
12. `HARD-005` Protected main branch and merge policy.
13. `HARD-006` Benchmark claim evidence cleanup.
14. `HARD-202` Leptos admin/storefront browser smoke coverage.
15. `HARD-301` SLI/SLO definitions and dashboards.
16. `HARD-302` Worker backpressure and cancellation policy.
17. `HARD-307` Per-tenant resource quotas.
18. `HARD-304` Restore drills and disaster recovery evidence.
19. `HARD-306` Dependency degradation and chaos tests.
20. `HARD-406` Reproducible performance regression suite.

## Validation Commands

Run the narrowest checks first, then the full gate:

```bash
cargo fmt --all -- --check
cargo test -p rustok-server host::tests
cargo test -p rustok-server middleware::security_headers
cargo test -p rustok-server middleware::tenant
node scripts/verify/verify-module-manifest-docs-drift.mjs
node scripts/verify/verify-advisory-exceptions.mjs
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
| `HARD-001` Synchronize manifest documentation | Completed | `f31dc37`, `9303c59` |
| `HARD-002` Automated manifest/docs drift verification | Completed | `f7c1fbe`, `8d6f1fb`, `b579617` |
| `HARD-003` Implementation plan and ledger | Completed | `5eb0687`, this update |
| `HARD-004` Advisory exception governance | In progress | Register and expiry gate added; quick-xml entries closed; legacy cargo-audit waivers remain |
| `HARD-101` CSP hardening | In progress | Plaintext HTTP and object content blocked; nonce/hash migration remains |
| `HARD-103` Production HSTS contract | Completed | `822430e`, `3a9f936` |
| `HARD-104` Tenant resolution fail-closed | In progress | Bootstrap validation completed in `47c8003`; request-time catch-all remains |
| `HARD-105` Default-tenant fallback restriction | In progress | Production and non-header use rejected in `47c8003`; metrics/profile contract remain |
| `HARD-106` Global catalog isolation review | Completed | Boundary test `f1ae6e1`; accepted decision `4d9cbb0` |
| `HARD-109` Clock anomaly handling | In progress | Durable generation path fixed in `07ed2ab`; legacy resolver remains |
| `HARD-201` Browser E2E CI | Implemented, not yet required | Workflow `8982982`; branch-protection requirement unverified |
| Quick-xml advisory debt | Closed | Waivers removed and register entries closed in `0b4d003`, `b988167`, `a6682fc` |
