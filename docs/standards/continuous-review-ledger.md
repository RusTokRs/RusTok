## Scoped Flutter i18n audit — 2026-10-13

- User-requested package audit, independent of the sequential FS cycle below; no FS phase is marked complete.
- Base `main`: `85e83666947217e772ff234664692f61938c4d99`; working branch: `arena/7ca57eaa-rustok`. Not integrated.
- Owner/surface: `rustok_flutter_i18n`, core catalog preparation and locale/fallback primitives, package tests/docs, i18n CI and generated cache hygiene.
- Implemented fixes and compatibility notes: [wrapper audit](../../rustok_mobile/packages/rustok_flutter_i18n/README.md#engineering-audit--2026-10-13).
- Second pass: re-audited Riverpod 2.5.1 lifecycle against upstream source, corrected the first pass's build-listener revision gap, eliminated spurious loading/unmount for ready catalogs, and reviewed facade attribute/provenance boundaries, JSON transactionality and non-finite number formatting. Added minimum/stable SDK CI matrices (not executed here).
- Third pass: added shared resolver work/character budgets, parser nesting recovery, safe numeric/date boundaries, exact decimal percent scaling and immutable root resource snapshots. Added eleven core regressions and one widget regression; synchronized the numeric generator model without running it.
- Local main reference observed at this continuation: `1869b2d7c40c368af12e4fe37ea0a310fa301f8a`; no intervening changes in the two i18n packages or their CI workflow relative to the original base. Assigned branch retained; not integrated.
- Fourth pass: bounded Accept-Language parsing aligned with the Rust owner's raw-input limits, with a strict diagnostics API and a compatibility projection; canonical/default/script/exclusion-aware negotiation, UTF-8 locale bounds and stable extension serialization. Fifteen Dart regressions added, not run.
- Local main observed for the fourth pass: `232e9368d1fc5fe91df3f46f7f0c4e85acc69fd4`. Relative to the original base, the relevant paths only gained generated Flutter package caches/lockfile; no production changes needed reconciliation. Not integrated.
- Fifth pass: date locale normalization, en-GB profile, CLDR narrow/stand-alone names and short-year widths, partial-field widths, invalid options and signed proleptic years. Eleven core and one widget regression added (not run); Unicode data notice included. Local main remains `232e9368d1fc5fe91df3f46f7f0c4e85acc69fd4`, not integrated.
- Sixth pass: typed selector identity, nested NUMBER/DATETIME option inheritance/override, reference arguments, currency precision/padding, tiny fractional and exact modulo CLDR operands, and normalized Portuguese regions. Fourteen core and one widget regression added; Python mirrors synchronized, not executed. Compared resolver source with @fluent/bundle 0.19.1, retaining the explicitly documented raw-number plural and numeric-exact-first package policies. Local main remains unchanged; not integrated.
- Seventh pass: canonical number-symbol locale resolution (regional identity with script/variants/extensions, POSIX and alias normalization, invalid-tag fallback), synchronized generator template, and Fluent Unicode scalar escape replacement. Ten core regressions added for these fixes and reference-scope/catalog-fallback contracts, not run. No CLDR table regeneration or dependency changes. Local main remains `232e9368d1fc5fe91df3f46f7f0c4e85acc69fd4`; assigned branch retained, not integrated.
- Eighth pass: quote-aware number affix and fraction-pattern parsing, Unicode boundary-based currency spacing (including non-finite versus explicit whitespace), and Unicode alphabetic predicate. Eight core regressions added, not run. Python models/templates synchronized; no generators, tests or CLDR table regeneration executed. Local main unchanged at `232e9368d1fc5fe91df3f46f7f0c4e85acc69fd4`; not integrated.
- Ninth pass: region-implied script compatibility in sibling negotiation (shared fallback map); correct sr-ME -> Latn from cldr-core 48.2 likelySubtags data. Align typegen argument collection with resolver behavior and validate names at both direct/API CLI boundaries (including String shadowing). Nine core locale and four CLI regressions added, not run. Current local main is `fa5806bed53301c83af32b19be6532a4f60a6716`; its relevant-path diff from session HEAD contains only generated Flutter caches/lockfile, not production changes. Previous observed main object is no longer available locally, so comparison used session HEAD. Assigned branch retained; not integrated.
- Full likely-subtags implementation: replace the regional shortlist with all 7,788 cldr-core 48.2 mappings; UTS #35 release-48 maximization for canonical input; generated 37-script RTL metadata, region-aware direction, negotiation and generalized regional catalog candidates. Hash-pinned offline generator produced the two Dart tables; original CLDR JSON retained as a test corpus. Eight core scenarios, one full-corpus/idempotence test and one widget regression added, not executed. Full alias canonicalization, minimization and language-distance matching are separate unsupported operations, not claimed. Current local main remains `fa5806bed53301c83af32b19be6532a4f60a6716`; no integration. No Dart/Flutter tests or generator --check were run.
- Likely-subtags recheck: re-read UTS #35 release-48 lookup semantics and dependent catalog/direction paths; corrected unknown-marker fallback inconsistency (`Zzzz`/`ZZ`) while preserving literal exact catalogs and avoiding invented region catalogs. Generator now rejects output-output and input-output collisions before I/O. Added five Dart and two Python regressions, none run. Executed only the generator's deterministic artifact `--check` using the pinned corpus and scriptMetadata inputs: both generated files match. This is not execution of the Dart algorithm or test suite. Local main unchanged; no integration.
- Symmetric regional fallback continuation: corrected explicit-default-script requests skipping their equivalent regional catalog (en-Latn-GB -> en-GB). Equivalence is checked against the complete CLDR language-region inference and applied at each variant level before reduction, preserving exact priority and explicit conflicting scripts. The shared chain covers lookup/provenance, negotiation and lazy preparation; q=0 remains the documented canonical-prefix policy. Eight core regressions added, not executed. Local main remains `fa5806bed53301c83af32b19be6532a4f60a6716`; assigned branch retained, not integrated.
- Verification: source/call-site review, CLDR 48.2 source-data inspection, Tree-sitter syntax parsing of twenty-six changed/new Dart files, workflow YAML parsing, Python AST parsing of modified sources, and `git diff HEAD --check`. Tests intentionally not run; Dart/Flutter SDKs unavailable, so analyzer/compilation/formatting remain maintainer-owned. No claim of complete Fluent/CLDR conformance or runtime success.

## Deep Full-Stack Audit Cycle — 2026-10-13 (Fresh Start)

**Status:** ACTIVE  
**Active phase:** FS-01 — `apps/server` composition root & entrypoints  
**Current main SHA:** `845e6ea62`
**Active branch:** `main`

**Purpose:** sequential, top-down root-to-leaf audit of the entire repository starting from the host entrypoint and composition root (`apps/server`), propagating through domain modules down to shared foundation libraries.

### Deep Audit Execution Protocol — mandatory for every phase

Every phase and module track executes the strict closed-loop protocol defined by `AGENTS.md` §2.1:

1. **Discovery pass (CodeGraph & Call Flow):** read the complete in-scope production path end-to-end, including callers, persistence, configuration, transport adapters, failure paths, and adjacent boundaries. Per `AGENTS.md` §2.2, prioritize CodeGraph (`codegraph_explore` / CLI) when available for call graph exploration and blast-radius analysis before falling back to manual symbol search/read loops. Pattern scanners are supporting evidence only.
2. **Invariant map:** write down the business, security, tenancy, authorization, transaction, concurrency, retry/idempotency, lifecycle, and compatibility invariants that the path must preserve.
3. **Finding isolation:** group findings by root cause. Do not batch unrelated fixes merely because they touch the same file. Prefer one small coherent remediation unit at a time.
4. **Implementation pass:** fix the root cause, not the visible symptom. Do not redesign unrelated code during the same remediation unit.
5. **Immediate re-audit:** after every remediation unit, re-read the entire changed function/module and all direct callers/callees. Explicitly inspect what assumptions the change invalidated.
6. **Adjacent-boundary re-audit:** inspect the nearest upstream and downstream boundaries (transport, auth, tenant/channel/locale context, DB schema, events/outbox, cache, worker, UI/CLI contract as applicable).
7. **Regression audit:** compare the pre-change and post-change behavior as a reviewer would: removed behavior, newly reachable states, error mapping, fallback behavior, defaults, feature-flag interactions, concurrency behavior, and observability. Ask specifically: "What new bug could this change have introduced?"
8. **Fresh second pass:** repeat the audit against the modified area without relying on the original finding list. New findings discovered here are treated as first-class findings.
9. **Scoped verification:** run scoped static checks and tests (`cargo check -p <crate>`, `cargo test -p <crate>`); never claim tests, clippy, or gatekeeper success unless actually executed.
10. **Commit boundary:** commit cleanly on the dedicated iteration branch and update the ledger.

### Phase Granularity Rule — one primary production module per iteration

- Each audit iteration has exactly **one primary production module/component**.
- A single module may require multiple consecutive audit iterations. Multiple iterations on the same module are preferred over widening scope.
- Surrounding reads establish evidence for the primary module, not additional primary scope.
- Root cause, not symptom: do not leave a known defect as a follow-up.

### Canonical Trigger
The single canonical trigger to continue this audit sequence is:
`реализуй план аудита`

---

### Phase Backlog — Top-Down Sequence

- [ ] **Phase FS-01 — Entrypoint & Composition Root (`apps/server`)**
- [ ] **Phase FS-02 — Core Identity, Auth & Tenancy Modules (`rustok-auth`, `rustok-rbac`, `rustok-tenant`, `rustok-cache`)**
- [ ] **Phase FS-03 — Commerce & Business Domain Modules (`rustok-commerce`, `rustok-cart`, `rustok-order`, `rustok-payment`, `rustok-pricing`, `rustok-inventory`, `rustok-fulfillment`)**
- [ ] **Phase FS-04 — Content, Interaction & Social Modules (`rustok-blog`, `rustok-forum`, `rustok-content`, `rustok-notifications`, `rustok-feedback`)**
- [ ] **Phase FS-05 — Data Classification & Metadata Modules (`rustok-taxonomy`, `rustok-seo`, `rustok-translation`, `rustok-ai`)**
- [ ] **Phase FS-06 — Background Workers & Execution Fleet (`crates/workers/*`)**
- [ ] **Phase FS-07 — Shared Foundation Libraries (`crates/libs/*`: `rustok-api`, `rustok-core`, `rustok-events`, `rustok-runtime`, `rustok-telemetry`)**
- [ ] **Phase FS-08 — Frontend Applications & UI Packages (`apps/admin`, `apps/storefront`, `crates/ui/*`)**
- [ ] **Phase FS-09 — Operational Tooling & Build Utilities (`crates/utils/*`, `scripts/*`)**

---

### Active Phase: FS-01 — `apps/server` Module Tracks Queue

#### 1. Bootstrap, Host & Configuration
- [x] **FS-01.01 — `apps/server/src/main.rs` & `apps/server/src/bin/*`** — entrypoint, CLI arguments, signal handling, process lifecycle. (Done: structured fatal error logging before otel shutdown, service name env fallback, thread stack size minimum floor, testable argument parsing for `export_api_contracts` with 5 unit tests).
- [x] **FS-01.02 — `apps/server/src/host.rs` & `apps/server/src/host_authority.rs`** — host wiring, socket binding, TLS/HTTPS, shutdown signals. (Done: graceful shutdown sequence reordered so Axum stops HTTP traffic before draining background workers, validated DB min_connections <= max_connections, guarded against duplicate host authority headers fail-closed, fixed URL authority parsing in health checks).
- [x] **FS-01.03 — `apps/server/src/services/server_bootstrap.rs`** — bootstrap sequence, database pool initialization, migration execution, state validation. (Done: strengthened production secret validation with case-insensitive dev JWT matcher, extended database credentials pattern matching to forbid sample credentials and local SQLite fallback in production, case-insensitive superadmin password matcher, added unit tests).
- [x] **FS-01.04 — `apps/server/src/services/server_runtime_context.rs`** — shared server context, state injection, thread safety. (Done: atomic single-lock take with reference preservation on strong-count contention to prevent worker handle loss, implemented without_auth constructor and FromRef<DatabaseConnection> for Axum handler extraction, added unit tests).
- [x] **FS-01.05 — `apps/server/src/services/app_runtime.rs` & `apps/server/src/services/app_lifecycle.rs`** — runtime background worker lifecycle, cancellation, drain on shutdown. (Done: StopHandle::stop updated to use send_replace to guarantee state broadcast even without active receivers; added ModuleWorkSchedulerHandle and WorkflowCronSchedulerHandle to lifecycle management and context storage; connect_runtime_workers guards against starting workers when shutdown is already requested; rewritten shutdown_runtime_workers to drain all registered background workers concurrently with tokio::join!; strengthened stop checks in remote executor reaper and SEO loops; ensured TransactionalEventBus is initialized in registry-only mode; fixed payment migration multi-alter SQLite incompatibility; added 4 unit tests).
- [x] **FS-01.06 — `apps/server/src/routes.rs` & `apps/server/src/error.rs`** — global routing table, 404 fallback, top-level Axum error mapping. (Done: moved global build_application_router and canonical not_found_fallback to routes.rs; ensured unmounted/headless mode and unmatched admin routes fall back to structured JSON ErrorBody instead of plain text; mapped Database RecordNotFound to 404 not_found and sanitized other DB/cache errors to 500 without leaking credentials; added 5 unit tests).

#### 2. Middleware & Context Guards
- [x] **FS-01.07 — `apps/server/src/middleware/tenant.rs`** — multi-tenant header/host resolution, domain mapping, fail-closed tenant boundary. (Done: enforced structured JSON ErrorBody responses on fail-closed tenant resolution and loading errors; prevented RFC 9110 §7.2 duplicate Host and x-forwarded-host header smuggling; normalized base domain matching against mixed casing and trailing dots in subdomain routing; normalized trailing slashes in tenant route policy scopes; lowered high-frequency request tracing logs from info to debug; added 5 unit tests across tenant_resolution, tenant_route_policy, and tenant_tests).
- [x] **FS-01.08 — `apps/server/src/middleware/channel.rs`** — sales channel resolution, default fallback, header precedence. (Done: enforced cross-tenant boundary verification in channel resolution failing closed with 403 Forbidden; guarded against conflicting duplicate X-Channel-ID, X-Channel-Slug, and ?channel= query parameters; lowered per-request tracing from info to debug; added 3 unit tests in channel_tests verifying duplicate rejection and cross-tenant protection).
- [x] **FS-01.09 — `apps/server/src/middleware/locale.rs`** — locale header negotiation, fallback normalization, language validation. (Done: added BCP 47 primary language fallback negotiation for requested locales and tenant default locale in constrain_locale_to_tenant; replaced empty 500 status on tenant locale policy read failure with structured JSON HttpError::internal response; lowered high-frequency per-request tracing from info to debug; normalized content-language header emission using canonical header constant; added 2 unit tests covering regional-to-primary and default fallback).
- [x] **FS-01.10 — `apps/server/src/middleware/auth_context.rs`** — JWT/session authentication context extraction, bearer token parsing, claims validation. (Done: added RFC 9110 §11.6.2 fail-closed protection against duplicate Authorization headers; rejected nil tenant claims in verified_tenant_id_from_access_token; added symmetric debug tracing for request entry and next delegation; added 2 unit tests covering duplicate header detection and nil tenant token rejection).
- [x] **FS-01.11 — `apps/server/src/middleware/rate_limit.rs`** — IP/tenant/user rate limiting, sliding window, memory/Redis exhaustion protection. (Done: converted rate-limited 429 and backend-unavailable 503 responses to structured JSON ErrorBody payloads with standard content-type; added debug tracing on path rate limiter entry; added unit test verifying JSON content-type and backend unavailable response contract).
- [x] **FS-01.12 — `apps/server/src/middleware/security_headers.rs`** — CSP, HSTS, X-Frame-Options, CORS policy enforcement. (Done: broadened is_api_surface to match /metrics/, /openapi, and /catalog paths under strict API CSP; normalized header names using axum::http::header canonical constants; added debug tracing on security_headers entry; added 4 unit test assertions for additional API paths).
- [x] **FS-01.13 — `apps/server/src/middleware/metrics_auth.rs`** — Prometheus scrape endpoint authentication, bearer/token security. (Done: structured JSON 401/503 responses for health/api observability paths while preserving Prometheus text format for /metrics; RFC 9110 §11 fail-closed protection against conflicting duplicate Authorization headers; sanitized configured token by stripping surrounding quotes and whitespace; added entry debug tracing; added 2 unit tests covering quote stripping and duplicate header rejection).
- [x] **FS-01.14 — `apps/server/src/middleware/registry_publish_policy.rs` & `registry_artifact_access.rs`** — registry package access policies. (Done: normalized publish path handling against trailing slashes in publish policy; added support for kebab-case ownership aliases first-party and third-party; added fail-closed protection against conflicting duplicate x-rustok-runner-token headers; sanitized expected runner token stripping quotes and whitespace; added debug tracing for entry and metadata normalization; added 2 unit tests covering trailing slash handling, kebab-case aliases, and runner token duplicate rejection).

#### 3. Controllers & Transports
- [ ] **FS-01.15 — `apps/server/src/controllers/health.rs`** — liveness, readiness, DB/Redis ping probes, status reporting.
- [ ] **FS-01.16 — `apps/server/src/controllers/metrics.rs`** — Prometheus metric exposition, scrape latency, sanitization.
- [ ] **FS-01.17 — `apps/server/src/controllers/auth.rs`** — login, register, refresh token, password reset, session invalidation.
- [ ] **FS-01.18 — `apps/server/src/controllers/oauth.rs` & `apps/server/src/controllers/oauth_metadata.rs`** — OAuth2/OIDC provider & client endpoints, PKCE, token grants.
- [ ] **FS-01.19 — `apps/server/src/controllers/users.rs`** — user profile management, admin user CRUD, tenant isolation.
- [ ] **FS-01.20 — `apps/server/src/controllers/graphql.rs` & `apps/server/src/services/graphql_schema.rs`** — GraphQL endpoint, query/mutation execution, dataloader, subscriptions.
- [ ] **FS-01.21 — `apps/server/src/controllers/mcp.rs` & `apps/server/src/services/mcp_*.rs`** — MCP transport, tool call routing, token auth, error masking.
- [ ] **FS-01.22 — `apps/server/src/controllers/flex.rs` & `apps/server/src/services/flex_*.rs`** — Flex schema REST endpoints, dynamic fields validation.
- [ ] **FS-01.23 — `apps/server/src/controllers/installer.rs`** — first-boot installer controller, setup token, DB provisioning.
- [ ] **FS-01.24 — `apps/server/src/controllers/marketplace_registry.rs`** — package publication, search, downloads.
- [ ] **FS-01.25 — `apps/server/src/controllers/artifact_http.rs` & `artifact_permissions.rs`** — WASM artifact serving, permissions check.
- [ ] **FS-01.26 — `apps/server/src/controllers/admin_events.rs`** — outbox/event inspection, DLQ management, retry dispatch.
- [ ] **FS-01.27 — `apps/server/src/controllers/channel.rs`** — channel admin endpoints, CRUD, tenant scoping.
- [ ] **FS-01.28 — `apps/server/src/controllers/swagger.rs`** — OpenAPI/Swagger UI endpoints, schema generation safety.

#### 4. Event Bus, Outbox & Background Services in Server
- [ ] **FS-01.29 — `apps/server/src/services/event_bus.rs` & `module_event_dispatcher.rs`** — host event bus composition, module dispatch.
- [ ] **FS-01.30 — `apps/server/src/services/outbox_retention_worker.rs`** — outbox cleanup worker, batch retention pruning.
- [ ] **FS-01.31 — `apps/server/src/services/module_transition_watchdog.rs`** — module state machine watchdog, timeout recovery.
- [ ] **FS-01.32 — `apps/server/src/services/settings_service.rs`** — static & dynamic tenant settings loading and caching.
- [ ] **FS-01.33 — `apps/server/src/services/rbac_*.rs`** — server-level RBAC caching, runtime invalidation.

---

## Continuous Code Review & Remediation Ledger (ACRE)

Tracking persistent progress across cyclical review rounds for all modules in RusToK.

## Current Cycle Status
- **Active Round:** Round 1
- **Cycle Started:** `2026-10-13T20:15:00Z`
- **Progress:** `0 / 254` components audited (**0%**)
- **Total Workspace Codebase:** ~350,000 LOC across 254 modules/apps

---

## Components Review Status

| Status | Component | Category | Files | LOC | Last Audited | Notes |
|:---:|---|---|---:|---:|---|---|
| [ ] | [admin](../../apps/admin) | `apps` | 116 | 26,430 | None |  |
| [ ] | [dioxus-admin](../../apps/dioxus-admin) | `apps` | 1 | 13 | None |  |
| [ ] | [next-admin](../../apps/next-admin) | `apps` | 520 | 81,193 | None |  |
| [ ] | [blog](../../apps/next-admin/packages/blog) | `apps` | 14 | 1,909 | None |  |
| [ ] | [cache](../../apps/next-admin/packages/cache) | `apps` | 6 | 478 | None |  |
| [ ] | [commerce](../../apps/next-admin/packages/commerce) | `apps` | 11 | 2,737 | None |  |
| [ ] | [email](../../apps/next-admin/packages/email) | `apps` | 5 | 291 | None |  |
| [ ] | [events](../../apps/next-admin/packages/events) | `apps` | 6 | 350 | None |  |
| [ ] | [forum](../../apps/next-admin/packages/forum) | `apps` | 19 | 4,844 | None |  |
| [ ] | [iggy-connector](../../apps/next-admin/packages/iggy-connector) | `apps` | 5 | 367 | None |  |
| [ ] | [rbac](../../apps/next-admin/packages/rbac) | `apps` | 13 | 2,155 | None |  |
| [ ] | [rustok-ai](../../apps/next-admin/packages/rustok-ai) | `apps` | 1 | 4,055 | None |  |
| [ ] | [rustok-mcp](../../apps/next-admin/packages/rustok-mcp) | `apps` | 1 | 1,050 | None |  |
| [ ] | [rustok-product](../../apps/next-admin/packages/rustok-product) | `apps` | 37 | 11,066 | None |  |
| [ ] | [search](../../apps/next-admin/packages/search) | `apps` | 1 | 2,926 | None |  |
| [ ] | [translation](../../apps/next-admin/packages/translation) | `apps` | 4 | 5,449 | None |  |
| [ ] | [workflow](../../apps/next-admin/packages/workflow) | `apps` | 13 | 1,529 | None |  |
| [ ] | [next-frontend](../../apps/next-frontend) | `apps` | 134 | 22,793 | None |  |
| [ ] | [rustok-blog](../../apps/next-frontend/packages/rustok-blog) | `apps` | 16 | 1,678 | None |  |
| [ ] | [rustok-cart](../../apps/next-frontend/packages/rustok-cart) | `apps` | 7 | 2,142 | None |  |
| [ ] | [rustok-comments](../../apps/next-frontend/packages/rustok-comments) | `apps` | 2 | 123 | None |  |
| [ ] | [rustok-forum](../../apps/next-frontend/packages/rustok-forum) | `apps` | 15 | 3,593 | None |  |
| [ ] | [rustok-notifications](../../apps/next-frontend/packages/rustok-notifications) | `apps` | 7 | 916 | None |  |
| [ ] | [rustok-order](../../apps/next-frontend/packages/rustok-order) | `apps` | 5 | 1,361 | None |  |
| [ ] | [rustok-pricing](../../apps/next-frontend/packages/rustok-pricing) | `apps` | 7 | 904 | None |  |
| [ ] | [rustok-product](../../apps/next-frontend/packages/rustok-product) | `apps` | 10 | 2,252 | None |  |
| [ ] | [rustok-profiles](../../apps/next-frontend/packages/rustok-profiles) | `apps` | 7 | 780 | None |  |
| [ ] | [rustok-seo](../../apps/next-frontend/packages/rustok-seo) | `apps` | 5 | 1,459 | None |  |
| [ ] | [search](../../apps/next-frontend/packages/search) | `apps` | 1 | 681 | None |  |
| [ ] | [server](../../apps/server) | `apps` | 484 | 187,425 | None |  |
| [ ] | [storefront](../../apps/storefront) | `apps` | 34 | 5,987 | None |  |
| [ ] | [rustok-content-revisions](../../crates/integration/rustok-content-revisions) | `integration` | 6 | 903 | None |  |
| [ ] | [rustok-api](../../crates/libs/rustok-api) | `libs` | 35 | 7,230 | None |  |
| [ ] | [rustok-content-portability](../../crates/libs/rustok-content-portability) | `libs` | 6 | 617 | None |  |
| [ ] | [rustok-content-portability-api](../../crates/libs/rustok-content-portability-api) | `libs` | 6 | 920 | None |  |
| [ ] | [rustok-core](../../crates/libs/rustok-core) | `libs` | 65 | 17,586 | None |  |
| [ ] | [rustok-events](../../crates/libs/rustok-events) | `libs` | 34 | 12,066 | None |  |
| [ ] | [rustok-fba](../../crates/libs/rustok-fba) | `libs` | 1 | 82 | None |  |
| [ ] | [rustok-runtime](../../crates/libs/rustok-runtime) | `libs` | 4 | 2,423 | None |  |
| [ ] | [rustok-telemetry](../../crates/libs/rustok-telemetry) | `libs` | 15 | 3,340 | None |  |
| [ ] | [rustok-web](../../crates/libs/rustok-web) | `libs` | 2 | 742 | None |  |
| [ ] | [alloy](../../crates/modules/alloy) | `modules` | 77 | 25,206 | None |  |
| [ ] | [flex](../../crates/modules/flex) | `modules` | 51 | 17,159 | None |  |
| [ ] | [rustok-accounting](../../crates/modules/rustok-accounting) | `modules` | 13 | 417 | None |  |
| [ ] | [rustok-ai](../../crates/modules/rustok-ai) | `modules` | 74 | 42,424 | None |  |
| [ ] | [rustok-ai-alloy](../../crates/modules/rustok-ai-alloy) | `modules` | 1 | 384 | None |  |
| [ ] | [rustok-ai-athanor](../../crates/modules/rustok-ai-athanor) | `modules` | 2 | 548 | None |  |
| [ ] | [rustok-ai-content](../../crates/modules/rustok-ai-content) | `modules` | 1 | 326 | None |  |
| [ ] | [rustok-ai-media](../../crates/modules/rustok-ai-media) | `modules` | 1 | 71 | None |  |
| [ ] | [rustok-ai-order](../../crates/modules/rustok-ai-order) | `modules` | 1 | 255 | None |  |
| [ ] | [rustok-ai-product](../../crates/modules/rustok-ai-product) | `modules` | 1 | 307 | None |  |
| [ ] | [rustok-ai-translation](../../crates/modules/rustok-ai-translation) | `modules` | 1 | 1,655 | None |  |
| [ ] | [admin](../../crates/modules/rustok-ai/admin) | `modules` | 17 | 8,538 | None |  |
| [ ] | [rustok-auth](../../crates/modules/rustok-auth) | `modules` | 50 | 12,368 | None |  |
| [ ] | [admin](../../crates/modules/rustok-auth/admin) | `modules` | 18 | 6,501 | None |  |
| [ ] | [cli](../../crates/modules/rustok-auth/cli) | `modules` | 1 | 333 | None |  |
| [ ] | [rustok-blog](../../crates/modules/rustok-blog) | `modules` | 148 | 35,732 | None |  |
| [ ] | [admin](../../crates/modules/rustok-blog/admin) | `modules` | 16 | 5,248 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-blog/storefront) | `modules` | 14 | 3,289 | None |  |
| [ ] | [rustok-brand](../../crates/modules/rustok-brand) | `modules` | 22 | 3,399 | None |  |
| [ ] | [admin](../../crates/modules/rustok-brand/admin) | `modules` | 9 | 2,081 | None |  |
| [ ] | [rustok-cache](../../crates/modules/rustok-cache) | `modules` | 45 | 15,736 | None |  |
| [ ] | [admin](../../crates/modules/rustok-cache/admin) | `modules` | 7 | 926 | None |  |
| [ ] | [rustok-cart](../../crates/modules/rustok-cart) | `modules` | 73 | 18,625 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-cart/storefront) | `modules` | 18 | 4,545 | None |  |
| [ ] | [rustok-channel](../../crates/modules/rustok-channel) | `modules` | 47 | 11,368 | None |  |
| [ ] | [admin](../../crates/modules/rustok-channel/admin) | `modules` | 14 | 5,173 | None |  |
| [ ] | [rustok-comments](../../crates/modules/rustok-comments) | `modules` | 39 | 9,622 | None |  |
| [ ] | [rustok-comments-api](../../crates/modules/rustok-comments-api) | `modules` | 3 | 154 | None |  |
| [ ] | [rustok-comments-storefront-support](../../crates/modules/rustok-comments-storefront-support) | `modules` | 3 | 171 | None |  |
| [ ] | [admin](../../crates/modules/rustok-comments/admin) | `modules` | 8 | 1,382 | None |  |
| [ ] | [rustok-commerce](../../crates/modules/rustok-commerce) | `modules` | 297 | 129,394 | None |  |
| [ ] | [rustok-commerce-foundation](../../crates/modules/rustok-commerce-foundation) | `modules` | 19 | 929 | None |  |
| [ ] | [admin](../../crates/modules/rustok-commerce/admin) | `modules` | 21 | 5,889 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-commerce/storefront) | `modules` | 16 | 2,022 | None |  |
| [ ] | [rustok-content](../../crates/modules/rustok-content) | `modules` | 45 | 5,878 | None |  |
| [ ] | [rustok-content-orchestration](../../crates/modules/rustok-content-orchestration) | `modules` | 18 | 3,020 | None |  |
| [ ] | [rustok-customer](../../crates/modules/rustok-customer) | `modules` | 26 | 5,273 | None |  |
| [ ] | [admin](../../crates/modules/rustok-customer/admin) | `modules` | 9 | 2,779 | None |  |
| [ ] | [rustok-distribution](../../crates/modules/rustok-distribution) | `modules` | 34 | 15,714 | None |  |
| [ ] | [rustok-docs](../../crates/modules/rustok-docs) | `modules` | 21 | 2,957 | None |  |
| [ ] | [rustok-email](../../crates/modules/rustok-email) | `modules` | 6 | 784 | None |  |
| [ ] | [rustok-events-module](../../crates/modules/rustok-events-module) | `modules` | 9 | 571 | None |  |
| [ ] | [admin](../../crates/modules/rustok-events-module/admin) | `modules` | 8 | 508 | None |  |
| [ ] | [next-admin](../../crates/modules/rustok-events-module/next-admin) | `modules` | 6 | 278 | None |  |
| [ ] | [rustok-forms](../../crates/modules/rustok-forms) | `modules` | 17 | 1,877 | None |  |
| [ ] | [admin](../../crates/modules/rustok-forms/admin) | `modules` | 4 | 274 | None |  |
| [ ] | [rustok-forum](../../crates/modules/rustok-forum) | `modules` | 620 | 170,498 | None |  |
| [ ] | [admin](../../crates/modules/rustok-forum/admin) | `modules` | 35 | 13,457 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-forum/storefront) | `modules` | 26 | 7,437 | None |  |
| [ ] | [rustok-fulfillment](../../crates/modules/rustok-fulfillment) | `modules` | 76 | 26,191 | None |  |
| [ ] | [admin](../../crates/modules/rustok-fulfillment/admin) | `modules` | 8 | 1,916 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-fulfillment/storefront) | `modules` | 12 | 1,252 | None |  |
| [ ] | [rustok-groups](../../crates/modules/rustok-groups) | `modules` | 112 | 31,917 | None |  |
| [ ] | [admin](../../crates/modules/rustok-groups/admin) | `modules` | 34 | 8,525 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-groups/storefront) | `modules` | 17 | 3,119 | None |  |
| [ ] | [rustok-iggy](../../crates/modules/rustok-iggy) | `modules` | 32 | 8,270 | None |  |
| [ ] | [rustok-iggy-connector](../../crates/modules/rustok-iggy-connector) | `modules` | 17 | 4,593 | None |  |
| [ ] | [admin](../../crates/modules/rustok-iggy-connector/admin) | `modules` | 8 | 614 | None |  |
| [ ] | [next-admin](../../crates/modules/rustok-iggy-connector/next-admin) | `modules` | 5 | 367 | None |  |
| [ ] | [rustok-index](../../crates/modules/rustok-index) | `modules` | 137 | 58,160 | None |  |
| [ ] | [admin](../../crates/modules/rustok-index/admin) | `modules` | 9 | 2,912 | None |  |
| [ ] | [rustok-inventory](../../crates/modules/rustok-inventory) | `modules` | 35 | 12,536 | None |  |
| [ ] | [admin](../../crates/modules/rustok-inventory/admin) | `modules` | 10 | 4,750 | None |  |
| [ ] | [rustok-marketplace](../../crates/modules/rustok-marketplace) | `modules` | 10 | 1,485 | None |  |
| [ ] | [rustok-marketplace-allocation](../../crates/modules/rustok-marketplace-allocation) | `modules` | 12 | 1,486 | None |  |
| [ ] | [rustok-marketplace-balance](../../crates/modules/rustok-marketplace-balance) | `modules` | 29 | 6,779 | None |  |
| [ ] | [rustok-marketplace-commission](../../crates/modules/rustok-marketplace-commission) | `modules` | 16 | 2,662 | None |  |
| [ ] | [rustok-marketplace-ledger](../../crates/modules/rustok-marketplace-ledger) | `modules` | 28 | 6,426 | None |  |
| [ ] | [rustok-marketplace-listing](../../crates/modules/rustok-marketplace-listing) | `modules` | 31 | 6,455 | None |  |
| [ ] | [admin](../../crates/modules/rustok-marketplace-listing/admin) | `modules` | 9 | 1,911 | None |  |
| [ ] | [rustok-marketplace-payout](../../crates/modules/rustok-marketplace-payout) | `modules` | 13 | 1,702 | None |  |
| [ ] | [rustok-marketplace-seller](../../crates/modules/rustok-marketplace-seller) | `modules` | 48 | 10,566 | None |  |
| [ ] | [admin](../../crates/modules/rustok-marketplace-seller/admin) | `modules` | 13 | 2,387 | None |  |
| [ ] | [rustok-mcp](../../crates/modules/rustok-mcp) | `modules` | 26 | 6,939 | None |  |
| [ ] | [admin](../../crates/modules/rustok-mcp/admin) | `modules` | 7 | 1,873 | None |  |
| [ ] | [rustok-media](../../crates/modules/rustok-media) | `modules` | 44 | 13,117 | None |  |
| [ ] | [rustok-media-transport](../../crates/modules/rustok-media-transport) | `modules` | 6 | 1,882 | None |  |
| [ ] | [admin](../../crates/modules/rustok-media/admin) | `modules` | 10 | 2,125 | None |  |
| [ ] | [cli](../../crates/modules/rustok-media/cli) | `modules` | 1 | 175 | None |  |
| [ ] | [rustok-moderation](../../crates/modules/rustok-moderation) | `modules` | 36 | 9,132 | None |  |
| [ ] | [rustok-moderation-api](../../crates/modules/rustok-moderation-api) | `modules` | 3 | 766 | None |  |
| [ ] | [rustok-modules](../../crates/modules/rustok-modules) | `modules` | 242 | 131,456 | None |  |
| [ ] | [rustok-modules-translation](../../crates/modules/rustok-modules-translation) | `modules` | 1 | 822 | None |  |
| [ ] | [cli](../../crates/modules/rustok-modules/cli) | `modules` | 1 | 1,813 | None |  |
| [ ] | [rustok-navigation](../../crates/modules/rustok-navigation) | `modules` | 40 | 5,286 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-navigation/storefront) | `modules` | 9 | 495 | None |  |
| [ ] | [rustok-newsletter](../../crates/modules/rustok-newsletter) | `modules` | 23 | 2,474 | None |  |
| [ ] | [rustok-newsletter-api](../../crates/modules/rustok-newsletter-api) | `modules` | 4 | 445 | None |  |
| [ ] | [rustok-notifications](../../crates/modules/rustok-notifications) | `modules` | 75 | 22,126 | None |  |
| [ ] | [rustok-notifications-api](../../crates/modules/rustok-notifications-api) | `modules` | 5 | 1,166 | None |  |
| [ ] | [admin](../../crates/modules/rustok-notifications/admin) | `modules` | 6 | 127 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-notifications/storefront) | `modules` | 16 | 3,152 | None |  |
| [ ] | [rustok-order](../../crates/modules/rustok-order) | `modules` | 85 | 21,547 | None |  |
| [ ] | [admin](../../crates/modules/rustok-order/admin) | `modules` | 14 | 2,813 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-order/storefront) | `modules` | 12 | 2,171 | None |  |
| [ ] | [rustok-outbox](../../crates/modules/rustok-outbox) | `modules` | 26 | 4,758 | None |  |
| [ ] | [admin](../../crates/modules/rustok-outbox/admin) | `modules` | 8 | 493 | None |  |
| [ ] | [rustok-page-builder](../../crates/modules/rustok-page-builder) | `modules` | 108 | 30,260 | None |  |
| [ ] | [rustok-page-builder-storefront](../../crates/modules/rustok-page-builder-storefront) | `modules` | 3 | 1,200 | None |  |
| [ ] | [admin](../../crates/modules/rustok-page-builder/admin) | `modules` | 74 | 20,424 | None |  |
| [ ] | [rustok-pages](../../crates/modules/rustok-pages) | `modules` | 189 | 53,554 | None |  |
| [ ] | [admin](../../crates/modules/rustok-pages/admin) | `modules` | 25 | 6,626 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-pages/storefront) | `modules` | 17 | 4,732 | None |  |
| [ ] | [rustok-payment](../../crates/modules/rustok-payment) | `modules` | 88 | 23,137 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-payment/storefront) | `modules` | 11 | 1,315 | None |  |
| [ ] | [rustok-pricing](../../crates/modules/rustok-pricing) | `modules` | 48 | 19,082 | None |  |
| [ ] | [rustok-pricing-persistence](../../crates/modules/rustok-pricing-persistence) | `modules` | 5 | 204 | None |  |
| [ ] | [admin](../../crates/modules/rustok-pricing/admin) | `modules` | 14 | 7,971 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-pricing/storefront) | `modules` | 10 | 2,487 | None |  |
| [ ] | [rustok-product](../../crates/modules/rustok-product) | `modules` | 190 | 81,078 | None |  |
| [ ] | [rustok-product-bundles](../../crates/modules/rustok-product-bundles) | `modules` | 23 | 4,602 | None |  |
| [ ] | [admin](../../crates/modules/rustok-product-bundles/admin) | `modules` | 9 | 2,617 | None |  |
| [ ] | [rustok-product-catalog-service](../../crates/modules/rustok-product-catalog-service) | `modules` | 1 | 508 | None |  |
| [ ] | [rustok-product-relations](../../crates/modules/rustok-product-relations) | `modules` | 22 | 2,720 | None |  |
| [ ] | [admin](../../crates/modules/rustok-product-relations/admin) | `modules` | 9 | 1,527 | None |  |
| [ ] | [rustok-product-transport](../../crates/modules/rustok-product-transport) | `modules` | 8 | 1,704 | None |  |
| [ ] | [admin](../../crates/modules/rustok-product/admin) | `modules` | 29 | 21,028 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-product/storefront) | `modules` | 13 | 4,612 | None |  |
| [ ] | [rustok-profiles](../../crates/modules/rustok-profiles) | `modules` | 52 | 9,224 | None |  |
| [ ] | [rustok-profiles-api](../../crates/modules/rustok-profiles-api) | `modules` | 3 | 132 | None |  |
| [ ] | [cli](../../crates/modules/rustok-profiles/cli) | `modules` | 1 | 338 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-profiles/storefront) | `modules` | 9 | 1,463 | None |  |
| [ ] | [rustok-rbac](../../crates/modules/rustok-rbac) | `modules` | 56 | 13,098 | None |  |
| [ ] | [admin](../../crates/modules/rustok-rbac/admin) | `modules` | 8 | 970 | None |  |
| [ ] | [cli](../../crates/modules/rustok-rbac/cli) | `modules` | 1 | 243 | None |  |
| [ ] | [rustok-reactions](../../crates/modules/rustok-reactions) | `modules` | 8 | 2,925 | None |  |
| [ ] | [rustok-reactions-api](../../crates/modules/rustok-reactions-api) | `modules` | 3 | 1,222 | None |  |
| [ ] | [rustok-reactions-storefront](../../crates/modules/rustok-reactions-storefront) | `modules` | 6 | 615 | None |  |
| [ ] | [rustok-region](../../crates/modules/rustok-region) | `modules` | 39 | 8,566 | None |  |
| [ ] | [admin](../../crates/modules/rustok-region/admin) | `modules` | 8 | 2,944 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-region/storefront) | `modules` | 9 | 1,738 | None |  |
| [ ] | [rustok-search](../../crates/modules/rustok-search) | `modules` | 96 | 32,629 | None |  |
| [ ] | [admin](../../crates/modules/rustok-search/admin) | `modules` | 20 | 8,298 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-search/storefront) | `modules` | 11 | 3,960 | None |  |
| [ ] | [rustok-seo](../../crates/modules/rustok-seo) | `modules` | 82 | 35,387 | None |  |
| [ ] | [rustok-seo-admin-support](../../crates/modules/rustok-seo-admin-support) | `modules` | 6 | 2,627 | None |  |
| [ ] | [rustok-seo-targets](../../crates/modules/rustok-seo-targets) | `modules` | 1 | 1,428 | None |  |
| [ ] | [admin](../../crates/modules/rustok-seo/admin) | `modules` | 15 | 4,557 | None |  |
| [ ] | [panel](../../crates/modules/rustok-seo/panel) | `modules` | 6 | 2,754 | None |  |
| [ ] | [render](../../crates/modules/rustok-seo/render) | `modules` | 1 | 17 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-seo/storefront) | `modules` | 6 | 919 | None |  |
| [ ] | [rustok-social-graph](../../crates/modules/rustok-social-graph) | `modules` | 36 | 8,796 | None |  |
| [ ] | [rustok-social-graph-cli](../../crates/modules/rustok-social-graph-cli) | `modules` | 1 | 324 | None |  |
| [ ] | [rustok-tax](../../crates/modules/rustok-tax) | `modules` | 5 | 1,574 | None |  |
| [ ] | [rustok-taxonomy](../../crates/modules/rustok-taxonomy) | `modules` | 53 | 14,828 | None |  |
| [ ] | [rustok-tenant](../../crates/modules/rustok-tenant) | `modules` | 28 | 4,005 | None |  |
| [ ] | [admin](../../crates/modules/rustok-tenant/admin) | `modules` | 8 | 835 | None |  |
| [ ] | [rustok-translation](../../crates/modules/rustok-translation) | `modules` | 81 | 45,996 | None |  |
| [ ] | [rustok-translation-targets](../../crates/modules/rustok-translation-targets) | `modules` | 3 | 1,729 | None |  |
| [ ] | [admin](../../crates/modules/rustok-translation/admin) | `modules` | 9 | 14,236 | None |  |
| [ ] | [rustok-workflow](../../crates/modules/rustok-workflow) | `modules` | 52 | 7,300 | None |  |
| [ ] | [admin](../../crates/modules/rustok-workflow/admin) | `modules` | 14 | 1,434 | None |  |
| [ ] | [fly](../../crates/ui/fly) | `ui` | 114 | 39,582 | None |  |
| [ ] | [fly-browser](../../crates/ui/fly-browser) | `ui` | 9 | 816 | None |  |
| [ ] | [fly-leptos](../../crates/ui/fly-leptos) | `ui` | 5 | 1,691 | None |  |
| [ ] | [fly-ui](../../crates/ui/fly-ui) | `ui` | 20 | 5,225 | None |  |
| [ ] | [browser](../../crates/ui/fly/browser) | `ui` | 11 | 931 | None |  |
| [ ] | [dioxus](../../crates/ui/fly/dioxus) | `ui` | 1 | 77 | None |  |
| [ ] | [leptos](../../crates/ui/fly/leptos) | `ui` | 4 | 113 | None |  |
| [ ] | [ui](../../crates/ui/fly/ui) | `ui` | 20 | 5,495 | None |  |
| [ ] | [web](../../crates/ui/fly/web) | `ui` | 6 | 2,217 | None |  |
| [ ] | [leptos-auth](../../crates/ui/leptos-auth) | `ui` | 9 | 1,506 | None |  |
| [ ] | [leptos-ui](../../crates/ui/leptos-ui) | `ui` | 5 | 415 | None |  |
| [ ] | [leptos-ui-routing](../../crates/ui/leptos-ui-routing) | `ui` | 1 | 233 | None |  |
| [ ] | [rustok-forms](../../crates/ui/rustok-forms) | `ui` | 21 | 9,244 | None |  |
| [ ] | [dioxus](../../crates/ui/rustok-forms/dioxus) | `ui` | 6 | 2,222 | None |  |
| [ ] | [leptos](../../crates/ui/rustok-forms/leptos) | `ui` | 6 | 3,126 | None |  |
| [ ] | [rustok-graphql](../../crates/ui/rustok-graphql) | `ui` | 1 | 335 | None |  |
| [ ] | [rustok-grid](../../crates/ui/rustok-grid) | `ui` | 27 | 6,864 | None |  |
| [ ] | [dioxus](../../crates/ui/rustok-grid/dioxus) | `ui` | 8 | 1,726 | None |  |
| [ ] | [leptos](../../crates/ui/rustok-grid/leptos) | `ui` | 8 | 1,499 | None |  |
| [ ] | [rustok-ui](../../crates/ui/rustok-ui) | `ui` | 46 | 5,231 | None |  |
| [ ] | [rustok-ui-auth](../../crates/ui/rustok-ui-auth) | `ui` | 1 | 66 | None |  |
| [ ] | [rustok-ui-core](../../crates/ui/rustok-ui-core) | `ui` | 9 | 2,257 | None |  |
| [ ] | [rustok-ui-forms](../../crates/ui/rustok-ui-forms) | `ui` | 1 | 75 | None |  |
| [ ] | [rustok-ui-i18n](../../crates/ui/rustok-ui-i18n) | `ui` | 32 | 6,793 | None |  |
| [ ] | [rustok-ui-transport](../../crates/ui/rustok-ui-transport) | `ui` | 1 | 478 | None |  |
| [ ] | [dioxus](../../crates/ui/rustok-ui/dioxus) | `ui` | 20 | 1,299 | None |  |
| [ ] | [leptos](../../crates/ui/rustok-ui/leptos) | `ui` | 20 | 2,046 | None |  |
| [ ] | [rustok-build](../../crates/utils/rustok-build) | `utils` | 12 | 2,109 | None |  |
| [ ] | [rustok-build-publication](../../crates/utils/rustok-build-publication) | `utils` | 4 | 874 | None |  |
| [ ] | [rustok-build-source](../../crates/utils/rustok-build-source) | `utils` | 2 | 1,570 | None |  |
| [ ] | [rustok-cli](../../crates/utils/rustok-cli) | `utils` | 3 | 1,328 | None |  |
| [ ] | [rustok-cli-core](../../crates/utils/rustok-cli-core) | `utils` | 1 | 120 | None |  |
| [ ] | [rustok-cli-platform](../../crates/utils/rustok-cli-platform) | `utils` | 3 | 466 | None |  |
| [ ] | [rustok-cli-registry](../../crates/utils/rustok-cli-registry) | `utils` | 2 | 131 | None |  |
| [ ] | [rustok-installer](../../crates/utils/rustok-installer) | `utils` | 11 | 4,403 | None |  |
| [ ] | [rustok-installer-cli](../../crates/utils/rustok-installer-cli) | `utils` | 1 | 823 | None |  |
| [ ] | [rustok-installer-persistence](../../crates/utils/rustok-installer-persistence) | `utils` | 9 | 1,867 | None |  |
| [ ] | [rustok-migrations](../../crates/utils/rustok-migrations) | `utils` | 70 | 15,966 | None |  |
| [ ] | [rustok-module-sdk](../../crates/utils/rustok-module-sdk) | `utils` | 2 | 60 | None |  |
| [ ] | [rustok-module-template](../../crates/utils/rustok-module-template) | `utils` | 2 | 329 | None |  |
| [ ] | [rustok-secrets](../../crates/utils/rustok-secrets) | `utils` | 3 | 1,346 | None |  |
| [ ] | [rustok-starter](../../crates/utils/rustok-starter) | `utils` | 11 | 1,273 | None |  |
| [ ] | [rustok-storage](../../crates/utils/rustok-storage) | `utils` | 4 | 792 | None |  |
| [ ] | [rustok-test-utils](../../crates/utils/rustok-test-utils) | `utils` | 7 | 1,949 | None |  |
| [ ] | [serde-columnar-vendored](../../crates/utils/serde-columnar-vendored) | `utils` | 16 | 2,451 | None |  |
| [ ] | [utoipa-swagger-ui-vendored](../../crates/utils/utoipa-swagger-ui-vendored) | `utils` | 2 | 34 | None |  |
| [ ] | [rustok-artifact-node-agent](../../crates/workers/rustok-artifact-node-agent) | `workers` | 8 | 2,421 | None |  |
| [ ] | [rustok-artifact-node-controller](../../crates/workers/rustok-artifact-node-controller) | `workers` | 3 | 137 | None |  |
| [ ] | [rustok-artifact-node-reconciler](../../crates/workers/rustok-artifact-node-reconciler) | `workers` | 3 | 158 | None |  |
| [ ] | [rustok-artifact-node-transport](../../crates/workers/rustok-artifact-node-transport) | `workers` | 6 | 1,016 | None |  |
| [ ] | [rustok-module-build-dispatcher](../../crates/workers/rustok-module-build-dispatcher) | `workers` | 3 | 504 | None |  |
| [ ] | [rustok-module-build-transport](../../crates/workers/rustok-module-build-transport) | `workers` | 6 | 325 | None |  |
| [ ] | [rustok-module-build-worker](../../crates/workers/rustok-module-build-worker) | `workers` | 8 | 4,124 | None |  |
| [ ] | [rustok-registry-validation-worker](../../crates/workers/rustok-registry-validation-worker) | `workers` | 2 | 898 | None |  |
| [ ] | [rustok-sandbox](../../crates/workers/rustok-sandbox) | `workers` | 26 | 8,071 | None |  |
| [ ] | [rustok-sandbox-transport](../../crates/workers/rustok-sandbox-transport) | `workers` | 5 | 1,225 | None |  |
| [ ] | [rustok-sandbox-worker](../../crates/workers/rustok-sandbox-worker) | `workers` | 3 | 730 | None |  |
| [ ] | [rustok-static-distribution-worker](../../crates/workers/rustok-static-distribution-worker) | `workers` | 10 | 3,397 | None |  |
| [ ] | [rustok-verification-transport](../../crates/workers/rustok-verification-transport) | `workers` | 4 | 309 | None |  |
| [ ] | [rustok-verification-worker](../../crates/workers/rustok-verification-worker) | `workers` | 5 | 1,121 | None |  |
| [ ] | [rustok-worker-transport](../../crates/workers/rustok-worker-transport) | `workers` | 1 | 472 | None |  |

---

## Completed Rounds Archive
_No completed rounds yet. Round 1 is currently in progress._
