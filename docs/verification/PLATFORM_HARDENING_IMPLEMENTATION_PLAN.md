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

The plan was initially revalidated against `main` on 2026-07-17 at commit `9c3a5f1b443d7fc0fa1dae8ee9b09a29d2edfb67`. The progress ledger was refreshed on 2026-07-17 after completing the typed tenant profile, cross-transport tenant isolation coverage, bounded CSP violation collection, server-hosted script nonce enforcement, production WSS-only browser connections and dependency feature cleanup.

## Current Revalidation Summary

### Confirmed open high-risk findings

1. The enforced UI Content Security Policy still permits inline styles; style nonce/hash migration, standalone admin SSR header integration and browser evidence remain required before `HARD-101` is complete.
2. Browser E2E runs in a dedicated workflow, but repository branch protection has not yet been verified to require that workflow.
3. Two dependency waivers remain until `Cargo.lock` is regenerated after disabling the unused SeaORM migration CLI/MySQL and Postcard heapless default features.
4. Production JWT bootstrap policy validates algorithm-specific key material, issuer, audience and HS256 secret quality; operational key rotation and emergency revocation remain separate production-readiness work.

### Findings closed or materially reduced

1. Plaintext `http:` was removed from the enforced UI CSP `connect-src`, and object/plugin content is blocked.
2. `unsafe-eval` was removed from the enforced UI CSP and the hardening gate prevents its reintroduction.
3. Server-hosted UI responses use one UUIDv4-derived nonce in both the CSP header and request extensions; `script-src 'unsafe-inline'` and inline event handlers are blocked.
4. Embedded admin scripts receive a nonce only while rendering the immutable bundled `index.html`; tenant or user-authored HTML is never blanket-authorized.
5. Storefront JSON-LD receives a nonce only through the exact trusted SEO-renderer opening tag; arbitrary script markup remains without a nonce.
6. Production UI `connect-src` is restricted to same-origin, HTTPS and WSS; plaintext `ws:` is retained only for non-production development profiles.
7. A strict CSP report-only policy contains no `unsafe-inline`, `unsafe-eval`, plaintext HTTP or plaintext WebSocket source.
8. CSP reports are collected through a bounded pre-auth/pre-tenant endpoint with legacy and Reporting API support, origin-only logging, bounded metric labels and a reviewed migration inventory.
9. Tenant resolution is a typed enum with an exhaustive canonical resolver; unknown modes fail configuration deserialization and cannot reach a default-tenant catch-all.
10. `DefaultTenant` fallback is forbidden in production, rejected outside header mode and emits dedicated telemetry plus a warning only when it is actually selected.
11. HTTP and GraphQL WebSocket use one cache-aware tenant read-port loader with typed errors; transport code no longer queries tenant persistence or reconstructs `TenantContext` independently.
12. Operator routes, self-resolving handshakes and the global read-only registry catalog are represented by one segment-safe route policy rather than duplicated bypass lists.
13. Tenant runtime behavior is selected by an explicit `multi_tenant`, `single_tenant` or `development` profile; the development profile is forbidden in production.
14. Tenant resolution uses the dedicated `rustok_tenant_resolutions_total` metric with bounded transport, typed source and outcome labels rather than cache-operation telemetry.
15. Negative tenant isolation coverage rejects missing, malformed, unknown, conflicting and disabled tenant assertions across REST, GraphQL HTTP, GraphQL WebSocket and storefront paths.
16. Subdomain tenant resolution requires at least one configured base domain at bootstrap.
17. Production startup requires an explicit HTTPS deployment declaration, and HSTS flag parsing is normalized.
18. The `/v1/catalog*` bypass was reviewed and documented as a global read-only registry boundary; `/v2/catalog/*` mutation routes remain tenant-bound.
19. `modules.toml.example` and `docs/modules/overview.md` were synchronized with `modules.toml`, and an automated drift gate now protects them.
20. The stale `quick-xml` advisory waivers were removed after confirming that the package is absent from the resolved `Cargo.lock` graph.
21. Three stale `rustls-webpki` waivers were removed because the resolved version is `0.103.13`, which meets all three patched thresholds.
22. Both `deny.toml` and `.cargo/audit.toml` ignores are governed by the same expiry-enforcing exception register.
23. Unused SeaORM migration CLI/MySQL and Postcard heapless defaults are disabled at workspace level and protected from member override by a repository gate.
24. A dedicated browser Playwright matrix runs smoke tests for `next-admin` and `next-frontend`.
25. Durable tenant cache generation publication aborts and logs an error rather than emitting timestamp zero on a pre-epoch clock anomaly.
26. Production JWT claims cannot use framework defaults; HS256 requires at least 64 bytes and rejects common placeholder or low-diversity secrets.

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
- Enforced CSP contains no `unsafe-eval`, no inline-script allowance and no plaintext production connection source.
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

1. Complete `HARD-101` with nonce/hash style CSP, standalone admin SSR integration and browser evidence; remove the remaining style `unsafe-inline` allowance.
2. Regenerate `Cargo.lock`, verify `rsa` and `atomic-polyfill` are absent from the selected graph and remove the final two audit waivers.
3. Make `HARD-201` a required branch-protection check.
4. `HARD-204` API compatibility diff gates.
5. `HARD-205` Migration upgrade and rollback verification.
6. `HARD-206` Signed SemVer release workflow and artifacts.
7. `HARD-005` Protected main branch and merge policy.
8. `HARD-006` Benchmark claim evidence cleanup.
9. `HARD-202` Leptos admin/storefront browser smoke coverage.
10. `HARD-305` JWT/key rotation and emergency revocation runbooks.
11. `HARD-301` SLI/SLO definitions and dashboards.
12. `HARD-302` Worker backpressure and cancellation policy.
13. `HARD-307` Per-tenant resource quotas.
14. `HARD-304` Restore drills and disaster recovery evidence.
15. `HARD-306` Dependency degradation and chaos tests.
16. `HARD-406` Reproducible performance regression suite.
17. `HARD-108` Database-level tenant integrity checks for every tenant-owned relation.
18. `HARD-303` Structured audit logs for privileged and tenant-changing operations.
19. `HARD-308` Compliance evidence pack with threat model and data-flow diagrams.
20. `HARD-208` Release readiness checklist with rollback evidence.

## Validation Commands

Run the narrowest checks first, then the full gate:

```bash
cargo fmt --all -- --check
cargo test -p rustok-web
cargo test -p rustok-storefront --features ssr
cargo test -p rustok-server services::app_router
cargo test -p rustok-server host::tests
cargo test -p rustok-server middleware::csp_reports
cargo test -p rustok-server middleware::security_headers
cargo test -p rustok-server middleware::tenant
cargo test -p rustok-server --test tenant_resolver_invariants_test
node scripts/verify/verify-csp-reporting-contract.mjs
node scripts/verify/verify-dependency-feature-hygiene.mjs
node scripts/verify/verify-tenant-resolution-architecture.mjs
node scripts/verify/verify-module-manifest-docs-drift.mjs
node scripts/verify/verify-advisory-exceptions.mjs
cargo generate-lockfile
cargo tree -i rsa --workspace --all-features
cargo tree -i atomic-polyfill --workspace --all-features
cargo audit
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
| `HARD-004` Advisory exception governance | Manifest remediation landed; lock refresh pending | Unified register `6b7b6cb`, gate `f9ac9ae`, stale TLS cleanup `c663746`, exact paths `22dcb01`, feature cleanup `c38a8ea`, feature gate `a307cb8`/`0c201ea` |
| `HARD-101` CSP enforcement hardening | In progress; server-hosted script and production connection phases complete | Shared nonce `8492391`; storefront trusted JSON-LD `712168a`; embedded admin scripts `531146a`/`65ae8c8`; nonce enforcement `9b1b1af`; production WSS-only `8800f79`; gate `7cf9c99`/`dd567b3`; style and standalone admin SSR work remains |
| `HARD-102` CSP report-only and telemetry | Completed | Bounded collector `6c71c30`, minimized telemetry `0990b59`, report headers `ac93c41`, inventory `273ece5`/`1dba75a`, gate `c7436f9`/`dd567b3`, middleware test `50ef318` |
| `HARD-103` Production HSTS contract | Completed | `822430e`, `3a9f936` |
| `HARD-104` Tenant resolution fail-closed | Completed | Typed configuration and canonical resolver `adca4014`; route/header hardening `f3b475e0`; unified HTTP/WS loader `21ad3a99` |
| `HARD-105` Default-tenant fallback restriction | Completed | Explicit runtime profiles, production development-profile ban and fallback/profile validation in tenant hardening batch |
| `HARD-106` Global catalog isolation review | Completed | Boundary test `f1ae6e1`; accepted decision `4d9cbb0`; wrapper parity `8965919` |
| `HARD-107` Negative tenant isolation coverage | Completed | REST, GraphQL HTTP, GraphQL WebSocket and storefront fail-closed tests in tenant hardening batch |
| `HARD-109` Clock anomaly handling | Completed | Durable generation `07ed2ab`; request/cache timestamps return errors; pre-epoch unit coverage |
| Canonical tenant context loading | Completed | Shared HTTP/GraphQL WebSocket read-port pipeline plus dedicated typed-source outcome telemetry |
| `HARD-110` Production JWT bootstrap policy | Implemented; rotation remains operational work | Bootstrap policy `ec5111b`; production example `c6cb4a3` |
| `HARD-201` Browser E2E CI | Implemented, not yet required | Workflow `8982982`; branch-protection requirement unverified |
| Quick-xml advisory debt | Closed | Waivers removed and register entries closed in `0b4d003`, `b988167`, `a6682fc` |
| Rustls-webpki advisory debt | Closed | Patched lock version `0.103.13`; waivers removed in `c663746`; register closed in `22dcb01` |
