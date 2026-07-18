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

The plan was initially revalidated against `main` on 2026-07-17 at commit `9c3a5f1b443d7fc0fa1dae8ee9b09a29d2edfb67`. The progress ledger was refreshed on 2026-07-18 after completing the typed tenant profile, cross-transport tenant isolation coverage, bounded CSP violation collection, server-hosted and standalone script/style-element nonce enforcement, production WSS-only browser connections, zeroing the Rust-hosted inline-style baseline, protecting the classic admin bootstrap, registering the remaining Next/React style debt and completing dependency feature cleanup.

## Current Revalidation Summary

### Confirmed open high-risk findings

1. The enforced UI Content Security Policy still permits inline style attributes through the explicit `style-src-attr 'unsafe-inline'` migration boundary. The Rust-hosted register is empty with a `0/0` ratchet, but the Next boundary contains exactly 60 JSX style props across 10 files and one runtime `<style>` element through 2026-08-15. Migration and browser evidence remain required before `HARD-101` is complete.
2. The Next chart adapter generates one runtime style element without a demonstrated per-response nonce path; it must be removed, converted to a finite palette or integrated with the trusted host nonce before strict style enforcement.
3. Browser E2E runs in a dedicated workflow, but repository branch protection has not yet been verified to require that workflow.
4. Two dependency waivers remain until `Cargo.lock` is regenerated after disabling the unused SeaORM migration CLI/MySQL and Postcard heapless default features.
5. Production JWT bootstrap policy validates algorithm-specific key material, issuer, audience and HS256 secret quality; operational key rotation and emergency revocation remain separate production-readiness work.

### Findings closed or materially reduced

1. Plaintext `http:` was removed from the enforced UI CSP `connect-src`, and object/plugin content is blocked.
2. `unsafe-eval` was removed from the enforced UI CSP and the hardening gate prevents its reintroduction.
3. Server-hosted UI responses use one UUIDv4-derived nonce in both the CSP header and request extensions; `script-src 'unsafe-inline'` and inline event handlers are blocked.
4. Style elements share the same request nonce; the broader `style-src 'unsafe-inline'` source was replaced by nonce-bearing `style-src` plus an isolated temporary `style-src-attr` allowance.
5. Embedded admin script and style elements receive a nonce only while rendering the immutable bundled `index.html`; tenant or user-authored HTML is never blanket-authorized.
6. Storefront JSON-LD receives a nonce only through the exact trusted SEO-renderer opening tag; arbitrary script markup remains without a nonce.
7. The standalone admin SSR host installs its own nonce CSP and security headers, validates production HTTPS/HSTS, and applies the request nonce to its only inline auth bootstrap script, including fallback renders.
8. Production UI `connect-src` is restricted to same-origin, HTTPS and WSS on both server-hosted and standalone admin surfaces; plaintext `ws:` is retained only for non-production development profiles.
9. A strict CSP report-only policy contains no `unsafe-inline`, `unsafe-eval`, plaintext HTTP or plaintext WebSocket source and explicitly reports style attributes through `style-src-attr 'none'`.
10. CSP reports are collected through a bounded pre-auth/pre-tenant endpoint with legacy and Reporting API support, origin-only logging, bounded metric labels and a reviewed migration inventory. The standalone admin does not advertise a collector it does not own.
11. The Rust-hosted inline-style exception register is empty. Its gate rejects every new Rust UI `style=` site and has a non-increasing `0/0` ratchet.
12. The remaining Next/React style surface is covered by a time-bounded machine-readable register. Its gate rejects unregistered files, count changes, stale entries, review expiry, direct DOM style writes and increases above the `60 props / 10 files / 1 runtime style element` ratchet.
13. The classic bundled admin bootstrap no longer writes `document.documentElement.style`; it toggles the `dark` class while static CSS owns the light/dark color-scheme declarations.
14. The static modular Page Builder grid moved from a style attribute to a Tailwind grid class and is no longer part of the exception surface.
15. Persisted forum category colors are validated before persistence and normalized to the strict `#RGB`, `#RGBA`, `#RRGGBB` or `#RRGGBBAA` grammar before Rust UI rendering; CSS declaration injection is rejected or falls back instead of being concatenated into `background`.
16. The unreferenced legacy Page Builder `admin_canvas.rs` duplicate was removed after confirming it had no module declaration, path override or source reference.
17. Modular Page Builder layer indentation uses a bounded nine-step class scale instead of an inline `padding-left` declaration.
18. Page Builder hover, selection and insertion overlays use SVG geometry attributes instead of CSS positioning text.
19. Page Builder resize preview and handles use SVG geometry and a closed cursor-class map while retaining pointer capture.
20. Storefront and forum-admin category accents use a finite, build-time-visible class palette selected from validated hex colors and attach no CSS declaration to the DOM.
21. Page Builder custom viewport dimensions and continuous zoom use native SVG dimensions, `viewBox` and `foreignObject` geometry rather than CSS sizing or `transform:scale`.
22. The admin module build indicator uses native `<progress max="100">` and clamps transport progress to `0..=100`.
23. Tenant resolution is a typed enum with an exhaustive canonical resolver; unknown modes fail configuration deserialization and cannot reach a default-tenant catch-all.
24. `DefaultTenant` fallback is forbidden in production, rejected outside header mode and emits dedicated telemetry plus a warning only when it is actually selected.
25. HTTP and GraphQL WebSocket use one cache-aware tenant read-port loader with typed errors; transport code no longer queries tenant persistence or reconstructs `TenantContext` independently.
26. Operator routes, self-resolving handshakes and the global read-only registry catalog are represented by one segment-safe route policy rather than duplicated bypass lists.
27. Tenant runtime behavior is selected by an explicit `multi_tenant`, `single_tenant` or `development` profile; the development profile is forbidden in production.
28. Tenant resolution uses the dedicated `rustok_tenant_resolutions_total` metric with bounded transport, typed source and outcome labels rather than cache-operation telemetry.
29. Negative tenant isolation coverage rejects missing, malformed, unknown, conflicting and disabled tenant assertions across REST, GraphQL HTTP, GraphQL WebSocket and storefront paths.
30. Subdomain tenant resolution requires at least one configured base domain at bootstrap.
31. Production startup requires an explicit HTTPS deployment declaration, and HSTS flag parsing is normalized.
32. The `/v1/catalog*` bypass was reviewed and documented as a global read-only registry boundary; `/v2/catalog/*` mutation routes remain tenant-bound.
33. `modules.toml.example` and `docs/modules/overview.md` were synchronized with `modules.toml`, and an automated drift gate protects them.
34. The stale `quick-xml` advisory waivers were removed after confirming that the package is absent from the resolved `Cargo.lock` graph.
35. Three stale `rustls-webpki` waivers were removed because the resolved version is `0.103.13`, which meets all three patched thresholds.
36. Both `deny.toml` and `.cargo/audit.toml` ignores are governed by the same expiry-enforcing exception register.
37. Unused SeaORM migration CLI/MySQL and Postcard heapless defaults are disabled at workspace level and protected from member override by a repository gate.
38. A dedicated browser Playwright matrix runs smoke tests for `next-admin` and `next-frontend`.
39. Durable tenant cache generation publication aborts and logs an error rather than emitting timestamp zero on a pre-epoch clock anomaly.
40. Production JWT claims cannot use framework defaults; HS256 requires at least 64 bytes and rejects common placeholder or low-diversity secrets.

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
- Enforced CSP contains no `unsafe-eval`, no inline-script allowance, no blanket inline-style-element allowance, no plaintext production connection source and no inline-style-attribute allowance.
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

1. Complete `HARD-101` by migrating the registered Next boundary from 60 JSX style props in 10 files and one runtime style element to zero, capture cross-stack browser evidence, then enforce `style-src-attr 'none'`.
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
cargo test -p rustok-ui-core
cargo test -p rustok-forum-admin
cargo test -p rustok-forum-storefront
cargo test -p rustok-page-builder-admin
cargo test -p rustok-web
cargo test -p rustok-admin --features ssr app::security
cargo test -p rustok-admin --features ssr app::auth_ssr
cargo test -p rustok-storefront --features ssr
cargo test -p rustok-server services::app_router
cargo test -p rustok-server host::tests
cargo test -p rustok-server middleware::csp_reports
cargo test -p rustok-server middleware::security_headers
cargo test -p rustok-server middleware::tenant
cargo test -p rustok-server --test tenant_resolver_invariants_test
node scripts/verify/verify-csp-reporting-contract.mjs
node scripts/verify/verify-csp-inline-style-exceptions.mjs
node scripts/verify/verify-csp-next-style-boundary.mjs
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
| `HARD-101` CSP enforcement hardening | In progress; Rust `0/0`, Next `60 props / 10 files / 1 runtime style element` | Shared nonce `8492391`; storefront trusted JSON-LD `712168a`; embedded admin elements `531146a`/`65ae8c8`/`a20cde6`; main-server nonce policy `9b1b1af`/`700d4cb`; production WSS-only `8800f79`; standalone admin adapter `8b80543`/`611e50a`/`066a45b`; standalone style policy `d5defaf`; strict color grammar `95efed1`/`6917019`; Page Builder class/SVG migrations `314c320`/`8a97295`/`30e2647`/`d2a6e10`/`e485925`; shared accent adapters `b9fd978`/`a1e2eae`/`d62cb59`/`5e79a57`; storefront/admin forum class migrations `5f620f6`/`6adb7e7`; final native progress and Rust `0/0` ratchet `e250e42`/`3a61789`; classic admin style-write removal `bf8816e`/`cfc29c4`; Next register/gate `044d4d3`/`4ac34c2`; CI/master wiring `c789cd7`/`72e29d7`; inventory `7fff543`; Next migration and browser evidence remain |
| `HARD-102` CSP report-only and telemetry | Completed | Bounded collector `6c71c30`, minimized telemetry `0990b59`, report headers `ac93c41`, inventory `273ece5`/`8dbd47b`/`c495c1c`/`71522b8`/`cef4a41`/`e81bc42`/`7fff543`, gate `c7436f9`/`85e6e6a`/`389cb07`, middleware test `50ef318` |
| `HARD-103` Production HSTS contract | Completed | `822430e`, `3a9f936`; standalone admin production validation `8b80543`/`611e50a` |
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
