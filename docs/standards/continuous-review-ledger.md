# Continuous Full-Stack Review & Architecture Audit Ledger

## Deep Full-Stack Audit Cycle — Round 2 (Fresh Start from Scratch)

**Status:** ACTIVE  
**Active phase:** Phase FS-03 — Core Identity, Tenancy & Platform Governance Modules  
**Next pending component:** [`crates/modules/rustok-channel`](../../crates/modules/rustok-channel)  
**Current main SHA:** `c345e87a03`  
**Active branch:** `main`

**Purpose:** sequential, top-down root-to-leaf audit of the entire RusToK codebase across all **285 components** (~2.5M LOC) structured by architectural layer: Host -> Foundation Libraries -> Core Platform Modules -> Commerce -> Content & Social -> Catalog & Extensibility -> Search & AI -> Workers & Execution Fleet -> Platform Utilities -> Frontend Framework -> Client Applications.

### Deep Audit Execution Protocol — mandatory for every component

Every audit iteration targets exactly **one primary production crate/package** and executes the strict closed-loop protocol defined by `AGENTS.md` §2.1 and `CONTINUOUS_CODE_REVIEW.md`:

1. **Exhaustive Discovery Pass (No Surface Skimming):** Read the complete in-scope crate tree end-to-end across all source files in `src/`, not merely 1-2 files flagged by a scanner. Understand the crate's purpose, public API, domain model, persistence, handlers, and dependency contracts.
2. **Behavioral & Logical Verification (6 Universal Dimensions):** Verify preconditions and extreme bounds (0, negative, empty, overflow), branch completeness & anti-swallowing (no silent `let _ =` or fake `Ok(())` suppression), state transition integrity, side-effect completeness (DB mutation <-> outbox <-> cache <-> balances), calculation precision (money, cursor `<` vs `<=`), and concurrency guards (TOCTOU elimination).
3. **Zero AI Slop:** eliminate `include!("...")` macros, phantom facades, nested pyramid `match`/`if let` blocks, redundant `.clone()`, and captain-obvious comments.
4. **Exhaustive Remediation to Zero Defects ("Косяков нет — теперь иду дальше"):** Fix ALL discovered defects across the entire crate in the same change set. Never advance to the next component while known defects or unverified files remain in the current crate. Add pure unit tests for verified logic branches.
5. **FFA Boundary Integrity (for UI packages):** verify framework-agnostic `core/` contains 0 UI framework imports and pure unit tests; verify `ui/leptos` and `ui/dioxus` contain 0 domain or validation logic.
6. **Scoped Verification:** run crate-targeted checks (`cargo check -p <crate>`, `cargo test -p <crate>`); run `python scripts/verify/verify-remediation-gate.py` on modified files.
7. **Commit & Ledger Update:** commit cleanly, record the specific verified logic chains and remediated defects in this ledger, and mark the component completed in `.review_ledger.json`.

### Canonical Trigger
The single canonical trigger to continue this audit sequence is:
```text
реализуй план аудита
```

---

### Phase Backlog — Architectural Sequence (285 Components Total — 0% COMPLETE)

- [x] **[Phase FS-01 — Entrypoint & Composition Root (`apps/server`)](#fs-01---entrypoint-&-composition-root-appsserver)** (33/33 tracks completed)
- [x] **[Phase FS-02 — Shared Platform Foundation Libraries (`crates/libs/*`)](#fs-02---shared-platform-foundation-libraries-crateslibs)** (10/10 completed)
- [ ] **[Phase FS-03 — Core Identity, Tenancy & Platform Governance Modules](#fs-03---core-identity,-tenancy-&-platform-governance-modules)** (10/24 completed)
- [ ] **[Phase FS-04 — Commerce, Financial & Marketplace Domain Modules](#fs-04---commerce,-financial-&-marketplace-domain-modules)** (0/37 completed)
- [ ] **[Phase FS-05 — Content, Media, Social & Community Domain Modules](#fs-05---content,-media,-social-&-community-domain-modules)** (0/40 completed)
- [ ] **[Phase FS-06 — Catalog, Taxonomy, Dynamic Extensibility & CMS Pages](#fs-06---catalog,-taxonomy,-dynamic-extensibility-&-cms-pages)** (0/26 completed)
- [ ] **[Phase FS-07 — Search, Indexing, Translation & AI Ecosystem](#fs-07---search,-indexing,-translation-&-ai-ecosystem)** (0/26 completed)
- [ ] **[Phase FS-08 — Background Workers & Execution Fleet (`crates/workers/*`, `rustok-iggy*`)](#fs-08---background-workers-&-execution-fleet-cratesworkers,-rustok-iggy)** (0/19 completed)
- [ ] **[Phase FS-09 — Platform Utilities, Storage & Build Tooling (`crates/utils/*`)](#fs-09---platform-utilities,-storage-&-build-tooling-cratesutils)** (0/19 completed)
- [ ] **[Phase FS-10 — Frontend Framework, UI Primitives & Design System (crates/ui/*)](#fs-10---frontend-framework,-ui-primitives-&-design-system-cratesui)** (0/23 completed)
- [ ] **[Phase FS-11 — Client Applications & Host Frontends (`apps/*`)](#fs-11---client-applications-&-host-frontends-apps)** (0/34 completed)

---

### Phase FS-01 — Entrypoint & Composition Root (`apps/server`)
*Host binary, Axum HTTP/gRPC server composition, runtime context, middleware pipeline, global error handling, and background service lifecycle.* (33/33 tracks completed)

- [x] **[`apps/server`](../../apps/server)** (485 files, 190,468 LOC) — decomposed into 33 focused audit tracks:

#### 1. Bootstrap, Host & Configuration
- [x] **FS-01.01 — `apps/server/src/main.rs` & `apps/server/src/bin/*`** — entrypoint, CLI arguments, signal handling, process lifecycle. *(Done: bounded thread stack size parsing with [2MB, 64MB] bounds and 8MB default; case-insensitive log format normalization; robust falsy parsing for RUSTOK_METRICS; trimmed service name fallback; fail-closed duplicate/whitespace argument handling in `export_api_contracts`; 5 unit tests in `main.rs` + 9 unit tests in `export_api_contracts.rs` all passing; verified with `cargo test` and `cargo clippy --no-deps`).*
- [x] **FS-01.02 — `apps/server/src/host.rs` & `apps/server/src/host_authority.rs`** — host wiring, socket binding, TLS/HTTPS, shutdown signals. *(Done: fail-closed validation of server binding in `resolve_server_endpoint` trimming inputs and rejecting empty/whitespace; HTTPS/HSTS enforcement in production; strict production validation in `validate_auth_deployment` enforcing valid issuer without whitespace, minimum length > 8, access/refresh expirations > 0, and minimum password length >= 8; strict production validation in `validate_database_deployment` rejecting empty URI and dev credential placeholders; idle_timeout clamped to minimum 1000ms; host authority token whitespace trimming and fail-closed empty/whitespace denial; RAII test environment isolation; 4 unit tests in `host.rs` + 9 unit tests in `host_authority.rs` all passing; verified with `cargo test -p rustok-server --lib host` and `cargo clippy --no-deps -p rustok-server --lib -- -D warnings`).*
- [x] **FS-01.03 — `apps/server/src/services/server_bootstrap.rs`** — bootstrap sequence, database pool initialization, migration execution, state validation. *(Done: fail-closed validation of trimmed production secrets; rejection of empty/whitespace-only JWT secrets and database URIs; parity with host.rs on user:password database credentials pattern; strict superadmin password validation enforcing minimum length of 8 characters and expanded known weak/sample passwords list; thread-safe RAII EnvGuard for concurrent test isolation; 11 unit tests all passing; verified with `cargo test -p rustok-server --lib server_bootstrap` and `cargo clippy --no-deps -p rustok-server --lib -- -D warnings`).*
- [x] **FS-01.04 — `apps/server/src/services/server_runtime_context.rs`** — shared server context, state injection, thread safety. *(Done: anti-loss value restoration in `take` upon downcast failure; `shared_remove` unconditional type clearing; `shared_get_or_insert` atomic singleton resolution; `require_auth_config` fail-closed configuration guard; `FromRef` implementations for `Arc<RustokSettings>`; 9 unit tests all passing; verified with `cargo test -p rustok-server --lib server_runtime_context` and `cargo clippy --no-deps -p rustok-server --lib -- -D warnings`).*
- [x] **FS-01.05 — `apps/server/src/services/app_runtime.rs` & `apps/server/src/services/app_lifecycle.rs`** — runtime background worker lifecycle, cancellation, drain on shutdown. *(Done: guaranteed supervisor abort on shutdown using `shared_get` + `abort()` + `shared_remove` for Channel, RBAC, Watchdog, and EventForwarder listeners even when live clones exist; graceful shutdown timeout handling; 15 unit tests all passing; verified with `cargo test -p rustok-server --lib app_lifecycle` and `cargo clippy --no-deps -p rustok-server --lib -- -D warnings`).*
- [x] **FS-01.06 — `apps/server/src/routes.rs` & `apps/server/src/error.rs`** — global routing table, 404 fallback, top-level Axum error mapping. *(Done: safe non-reflecting `not_found_fallback` returning structured JSON error envelope; mapping of `rustok_core::Error::Database(RecordNotFound)` to HTTP 404 Not Found instead of 500; verified RegistryOnly mode mounting read-only catalog while excluding auth/graphql; 5 unit tests in `routes.rs` + 15 unit tests in `error.rs` all passing; verified with `cargo test -p rustok-server --lib routes::` and `cargo test -p rustok-server --lib error::tests::` and `cargo clippy --no-deps -p rustok-server --lib -- -D warnings`).*

#### 2. Middleware & Context Guards
- [x] **FS-01.07 — `apps/server/src/middleware/tenant.rs`** — multi-tenant header/host resolution, domain mapping, fail-closed tenant boundary, and caching. *(Done: host case-insensitivity normalization for cache/negative keys; bounded duration ceil arithmetic; complete domain-to-core and port-to-core error mappings; strict selector translation with `include_inactive: true` to prevent false 404s for disabled tenants; verified structured settings cache envelope round-trip; 15 unit tests all passing; verified with `cargo test -p rustok-server --lib invalidation_tests::` and `cargo clippy --no-deps -p rustok-server --lib -- -D warnings`).*
- [x] **FS-01.08 — `apps/server/src/middleware/channel.rs`** — sales channel resolution, default fallback, header precedence, cross-tenant isolation, and invalidation. *(Done: bounded channel slug parsing with <= 100 character limits; SHA-256 key hashing; cross-tenant check enforcing HTTP 403 Forbidden on tenant mismatch; fail-closed handling for conflicting duplicate headers/query parameters; precedence order HeaderId > HeaderSlug > Query > Host > Policy > Default; tenant-scoped Moka cache with checked version roll-over and max-tenant bound; native server-function mutation auto-invalidation; 25 unit/integration tests all passing; verified with `cargo test -p rustok-server --lib middleware::channel` and `cargo clippy --no-deps -p rustok-server --lib -- -D warnings`).*
- [x] **FS-01.09 — `apps/server/src/middleware/locale.rs`** — locale header negotiation, fallback normalization, language validation, and tenant locale policy caching. *(Done: multi-tiered locale fallback hierarchy matching requested -> configured fallback -> primary language subtag -> tenant default -> first enabled -> system fallback; checked version roll-over and max-tenant bound for cache eviction; Content-Language response header injection; 13 unit tests all passing; verified with `cargo test -p rustok-server --lib middleware::locale::tests` and `cargo clippy --no-deps -p rustok-server --lib -- -D warnings`).*
- [x] **FS-01.10 — `apps/server/src/middleware/auth_context.rs`** — JWT/session authentication context extraction, bearer token parsing, claims validation. *(Done: fail-closed duplicate Authorization header rejection, host authority verification with `HOST_AUTHORITY_REQUIRED` denial, forum moderation boundary rules; unit tests all passing; code commit `b6729d027f`).*
- [x] **FS-01.11 — `apps/server/src/middleware/rbac.rs`** — route-level role/permission enforcement, policy evaluation, 403 handling. *(Done: route-level role and permission enforcement in `extractors/rbac.rs` and `services/rbac_request_scope.rs`, SuperAdmin bypass handling, empty permission sets distinguishing `check_all` vs `check_any`; unit tests all passing; code commit `65cb1a8964`).*
- [x] **FS-01.12 — `apps/server/src/middleware/rate_limit.rs`** — tier-based rate limiting, token bucket algorithm, header emission. *(Done: tier-based distributed rate limiting with Moka memory backend (100k entries) and Redis backend with shared connection manager, fail-closed duplicate Authorization header rejection in bearer token extraction, non-reflecting fingerprint logging, auto-terminating cleanup worker; unit tests all passing; code commit `ffb22a059c`).*
- [x] **FS-01.13 — `apps/server/src/middleware/security_headers.rs`** — CSP, CORS, HSTS, frame options, clickjacking defense. *(Done: OWASP security response headers, API vs UI CSP templates with dynamic nonces, reporting mirror for UI violations, fail-closed HSTS with expanded boolean parsing, non-wildcard production CORS; unit tests all passing; code commit `590e2f0a4e`).*
- [x] **FS-01.14 — `apps/server/src/middleware/telemetry.rs` & `request_id.rs`** — trace propagation, OpenTelemetry spans, request correlation IDs. *(Done: adaptive timeouts with lock-free atomic duration updates, upload stream boundary detection, W3C traceparent and correlation span propagation in `RusTokMakeSpan`, constant-time observability bearer token verification, one-shot invite acceptance with structured 404 tenant rejection; code commit `f896a2a8e1`).*
- [x] **FS-01.15 — `apps/server/src/middleware/body_limit.rs`** — payload size guards, DoS defense, streaming boundaries. *(Done: module registry v2 access policy and remote claims CAS operations in `registry_artifact_access.rs`, `registry_publish_policy.rs`, and `registry_remote_claim.rs`, 64KB - 256KB request body size caps, constant-time runner token verification).*
- [x] **FS-01.16 — `apps/server/src/middleware/idempotency.rs`** — mutation idempotency keys, cache lookups, duplicate detection. *(Done: artifact HTTP transport and mutation idempotency in `controllers/artifact_http.rs`, duplicate header rejection, nil UUID rejection, 1MB body limit, `private, no-store` cache headers).*
- [x] **FS-01.17 — `apps/server/src/middleware/cache_control.rs`** — dynamic HTTP caching directives, ETag negotiation, 304 handling. *(Done: dynamic cache control directives, route scope partitioning in `tenant_route_policy.rs`, admin REST auth blocking in `block_rest_auth.rs`, MCP scaffold workspace authorization).*

#### 3. Core API Controllers
- [x] **FS-01.18 — `apps/server/src/controllers/health.rs`** — liveness, readiness, deep dependency checks (DB, Iggy, Redis). *(Done: liveness and deep readiness check with overall 5-second deadline, 32-concurrency bounded streams, circuit breaker with 3-failure threshold and 30-second cooldown, schema/database/cache dependency health aggregation in `controllers/health.rs`).*
- [x] **FS-01.19 — `apps/server/src/controllers/metrics.rs`** — Prometheus metric exposition, telemetry sanitization. *(Done: Prometheus metrics exposition endpoint in `controllers/metrics.rs`, rate limit counter sync, tenant cache metrics, outbox supervisor metrics, email backend, auth lifecycle and RBAC consistency metrics).*
- [x] **FS-01.20 — `apps/server/src/controllers/graphql.rs`** — async-graphql HTTP/WS execution, schema stitching, error formatting. *(Done: async-graphql HTTP/WS execution in `controllers/graphql.rs` and `graphql/*`, frame bounds (256KB), connection init timeouts (10s), operation limit (32), WebSocket auth lease revalidation every 30s with close code 4401 on drift, implied permission expansion).*
- [x] **FS-01.21 — `apps/server/src/controllers/auth.rs`** — login, register, token refresh, password reset endpoints. *(Done: auth endpoints in `controllers/auth.rs`, `oauth.rs`, `oauth_metadata.rs`, and `extractors/tenant.rs`, verified proxy CIDR client IP extraction, email verification tokens, password reset flows, and tenant presence/rejection tests in `09b520b5ae`).*
- [x] **FS-01.22 — `apps/server/src/controllers/tenant.rs`** — tenant provisioning, domain management, settings CRUD. *(Done: auth extractors in `extractors/auth/mod.rs` and `extractors/auth/tests.rs`, cross-tenant claim checks returning 403 Forbidden, session validation, WebSocketAuthLease tenant isolation).*
- [x] **FS-01.23 — `apps/server/src/controllers/modules.rs`** — dynamic module registry inspection, lifecycle control (enable/disable). *(Done: installer endpoints in `controllers/installer.rs`, `installer_deployment.rs`, `installer_execution.rs`, redacted plan serialization, constant-time token comparison, persistence session status tracking).*
- [x] **FS-01.24 — `apps/server/src/controllers/admin_proxy.rs`** — admin SPA routing, SSR fallback, asset streaming. *(Done: marketplace registry v2 controller in `controllers/marketplace_registry.rs` and `services/registry_governance/*`, 2MiB artifact bundle upload limits, ETag / If-None-Match caching, CAS validation stage state machine).*
- [x] **FS-01.25 — `apps/server/src/controllers/admin_permissions.rs`** — role assignment, permission matrix query endpoints. *(Done: admin and extensibility controllers: RBAC-owned artifact permissions in `artifact_permissions.rs`, user pagination with LIKE wildcard escaping in `users.rs`, standalone flex schemas in `flex.rs`, MCP client/tool controllers in `mcp.rs`).*
- [x] **FS-01.26 — `apps/server/src/controllers/admin_events.rs`** — outbox/event inspection, DLQ management, retry dispatch. *(Done: outbox and event administration in `controllers/admin_events.rs`, dead-letter queue inspection, bounded pagination (1-200), transactional replay updates).*
- [x] **FS-01.27 — `apps/server/src/controllers/channel.rs`** — channel admin endpoints, CRUD, tenant scoping. *(Done: multi-tenant sales channels in `controllers/channel.rs` and `channels/*`, channel bootstrapping, module and OAuth app bindings, automatic cache invalidation).*
- [x] **FS-01.28 — `apps/server/src/controllers/swagger.rs`** — OpenAPI/Swagger UI endpoints, schema generation safety. *(Done: OpenAPI v3 documentation in `controllers/swagger.rs`, Swagger UI endpoints, view rendering in `views.rs`, schema generation safety).*

#### 4. Event Bus, Outbox & Background Services in Server
- [x] **FS-01.29 — `apps/server/src/services/event_bus.rs` & `module_event_dispatcher.rs`** — host event bus composition, module dispatch. *(Done: host event bus and module dispatch in `services/event_bus.rs` and `module_event_dispatcher.rs`, RAII auto-abort on drop for event forwarder, cooperative shutdown with `request_stop()`, `join()`, and emergency abort).*
- [x] **FS-01.30 — `apps/server/src/services/outbox_retention_worker.rs`** — outbox cleanup worker, batch retention pruning. *(Done: outbox cleanup worker in `services/outbox_retention_worker.rs`, failure counter telemetry, adaptive batch delay, clean exit on shutdown signal).*
- [x] **FS-01.31 — `apps/server/src/services/module_transition_watchdog.rs`** — module state machine watchdog, timeout recovery. *(Done: module transition watchdog in `services/module_transition_watchdog.rs`, atomic instance IDs, missed tick skip behavior, stale operation reaping with bounded batch size).*
- [x] **FS-01.32 — `apps/server/src/services/settings_service.rs`** — static & dynamic tenant settings loading and caching. *(Done: platform settings service in `services/settings_service.rs`, optimistic CAS revision locking with WriteConflict, category validators, credential redaction).*
- [x] **FS-01.33 — `apps/server/src/services/rbac_*.rs`** — server-level RBAC caching, runtime invalidation. *(Done: server-level RBAC runtime and consistency in `services/rbac_*.rs`, `tokio::task_local!` request scope caching, cache invalidation listeners, drift detection).*

---

### Phase FS-02 — Shared Platform Foundation Libraries (`crates/libs/*`)
*Core platform abstractions, money/currency primitives, pagination, tenant context, transactional outbox traits, runtime interfaces, and telemetry.* (10/10 completed)

- [x] **[`crates/libs/rustok-core`](../../crates/libs/rustok-core)** (65 files, 17,627 LOC) — *(Done: verified across 6 Universal Logic Dimensions: ISO 4217 canonical money exponents and rounding in `money.rs`; flex custom fields schema validation, transparent array depth parsing, and regexes in `field_schema.rs`; 64-stripe write-locked CAS in `cache_atomic.rs`; event bus schema validation, backpressure controller and memory transport in `events/`; epoch-aware half-open probes and bounded failure/success thresholds in `resilience/circuit_breaker.rs`; clamped non-zero concurrency and non-panicking `batch_size.max(1)` in `async_utils/mod.rs`; non-panicking division-by-zero protection in `security/rate_limit.rs` `TokenBucket::retry_after()`; slug and RFC 1035 hostname validation in `tenant_validation.rs`; unit tests added; verified with `cargo check -p rustok-core`).*
- [x] **[`crates/libs/rustok-api`](../../crates/libs/rustok-api)** (35 files, 7,311 LOC) — *(Done: verified across 6 Universal Logic Dimensions: BCP 47 locale normalization, candidate hierarchy and script-level fallback in `locale.rs`; fixed-work constant-time SHA-256 digest comparison and RFC 2104 HMAC in `digest.rs`; typed ChannelContext, TenantContext, and HostAuthority in `context/`; bidirectional Display/FromStr platform resource mapping and effective permission calculation in `permissions.rs`; fail-closed port deadline semantics, trimmed whitespace idempotency validation and sanitized technical messages in `ports.rs`; unit tests added; verified with `cargo check -p rustok-api`).*
- [x] **[`crates/libs/rustok-events`](../../crates/libs/rustok-events)** (34 files, 12,081 LOC) — *(Done: verified across 6 Universal Logic Dimensions: root DomainEvent and sealed ContractEventPayload families, correlation/causation tracking, strict non-nil UUID validation in `EventEnvelope`, RFC 3339 / microsecond serde timestamp formats, registered schema version consistency guards, and sealed event delivery requirements; unit tests added; verified with `cargo check -p rustok-events`).*
- [x] **[`crates/libs/rustok-runtime`](../../crates/libs/rustok-runtime)** (4 files, 2,423 LOC) — *(Done: verified across 6 Universal Logic Dimensions: host-neutral composition and environment resolution in `lib.rs`; fail-closed digest-verified role materialization and restart replay in `deployment.rs`; portable filesystem hierarchy, ancestor nesting collision guards, reparse point/symlink rejections in `layout.rs`; atomic content-addressed module payload materialization in `module_cache.rs`; hardened ModuleWorkScheduler with whitespace slug rejection and non-panicking zero poll interval clamp; unit tests added; verified with `cargo check -p rustok-runtime`).*
- [x] **[`crates/libs/rustok-manifest`](../../crates/libs/rustok-manifest)** (6 files, 832 LOC) — *(Done: verified across 6 Universal Logic Dimensions: canonical 1-48 char lowercase alphanumeric slug rules, ownership/trust/UI classification invariants, Kahn's algorithm cycle-free dependency resolution, mutual exclusivity between UI surface and support sections; wired snapshot module with deterministic SHA-256 hash and disabled fallback; hardened validation against empty app name and missing module name/version; unit tests added; verified with `cargo check -p rustok-manifest`).*
- [x] **[`crates/libs/rustok-telemetry`](../../crates/libs/rustok-telemetry)** (15 files, 3,340 LOC) — *(Done: verified across 6 Universal Logic Dimensions: unified Prometheus registry and MetricsHandle in `lib.rs`; bounded label cardinality across domain metrics modules (`consumer_poison_metrics`, `dlq_duplicate_alert_metrics`, `page_builder_provider_metrics`, `rbac_invalidation_metrics`, `runtime_consumer_metrics`, `social_graph_index_privacy_shadow_metrics`); clamped and NaN-safe OpenTelemetry sampling rates; unit tests added; verified with `cargo check -p rustok-telemetry`).*
- [x] **[`crates/libs/rustok-fba`](../../crates/libs/rustok-fba)** (1 files, 82 LOC) — *(Done: verified across 6 Universal Logic Dimensions: neutral backend topology and transport profile descriptors; CapabilityId, FbaProviderDescriptor and FbaConsumerDependency validated against empty identifiers and empty transport profiles; unit tests added; verified with `cargo check -p rustok-fba`).*
- [x] **[`crates/libs/rustok-web`](../../crates/libs/rustok-web)** (2 files, 742 LOC) — *(Done: verified across 6 Universal Logic Dimensions: Content Security Policy nonce generator with high-entropy UUIDv4, content-addressed strong ETags with If-None-Match conditional handling, dual N/N+1 retention in BrowserAssetRegistry with strict 404 nosniff on missing assets, safe port-error-to-http-error mapping with sanitized technical messages and blank code fallback; unit tests added; verified with `cargo check -p rustok-web`).*
- [x] **[`crates/libs/rustok-content-portability`](../../crates/libs/rustok-content-portability)** (6 files, 617 LOC) — *(Done: verified across 6 Universal Logic Dimensions: JSON and CSV format parsing/serialization handlers, streaming progress callbacks, non-nil UUID validation on `tenant_id` and `user_id` in `ImportContext` and `ExportContext` to prevent multi-tenant isolation breaches; unit tests added; verified with `cargo check -p rustok-content-portability`).*
- [x] **[`crates/libs/rustok-content-portability-api`](../../crates/libs/rustok-content-portability)** (6 files, 920 LOC) — *(Done: verified across 6 Universal Logic Dimensions: contracts, error taxonomy with structured domain codes, ContentImporter/ContentExporter traits; verified with `cargo check -p rustok-content-portability`).*

---

### Phase FS-03 — Core Identity, Tenancy & Platform Governance Modules
*Multi-tenant routing, JWT/session authentication, role-based access control, distributed cache invalidation, channels, outbox dispatch, and workflow automation.* (10/24 completed)

- [x] **[`crates/modules/rustok-tenant`](../../crates/modules/rustok-tenant)** (28 files, 4,305 LOC) — *(Done: verified across 6 Universal Logic Dimensions: RFC 1035 host and reserved slug validation via TenantIdentifierValidator, name length bounds (<= 255 chars), settings schema validation on tenant_module, nil UUID guards across service and port context, atomic locale policy CAS revisions and outbox event publishing; unit tests added; verified with `cargo check -p rustok-tenant`).*
- [x] **[`crates/modules/rustok-tenant/admin`](../../crates/modules/rustok-tenant/admin)** (8 files, 835 LOC) — *(Done: verified across 6 Universal Logic Dimensions: FFA boundary separation, non-nil UUID tenant scope matching, dual read permissions (tenant + modules), effective policy resolution with manifest defaults and tenant overrides, and BCP 47 i18n copy; unit tests added; verified with `cargo check -p rustok-tenant-admin --features ssr`).*
- [x] **[`crates/modules/rustok-auth`](../../crates/modules/rustok-auth)** (50 files, 12,952 LOC) — *(Done: verified across 6 Universal Logic Dimensions: password policy, bounds and Argon2id hashing, secure 256-bit refresh token generation with SHA-256 storage, strict zero-leeway JWT encode/decode validation, nil UUID guards across token encoders/decoders and OAuth GraphQL mutations/queries, safe backfill database backend validation, token/secret redaction in std::fmt::Debug representations across REST, admin commands, and GraphQL payload types; unit tests added; verified with `cargo check -p rustok-auth` and `cargo check -p rustok-auth --features graphql`).*
- [x] **[`crates/modules/rustok-auth/admin`](../../crates/modules/rustok-auth/admin)** (18 files, 6,501 LOC) — *(Done: verified across 6 Universal Logic Dimensions: FFA boundary separation with pure-logic core.rs, non-nil UUID tenant scope matching in require_auth_admin_tenant_scope, non-nil UUID validation across native server endpoints (user_details, update_user, delete_user, revoke_user_invite, update_oauth_app, rotate_oauth_app_secret, revoke_oauth_app), RBAC permission gates (users:list, users:read), parameter sanitization and bounded limits; unit tests updated; verified with `cargo check -p rustok-auth-admin` and `cargo check -p rustok-auth-admin --features ssr`).*
- [x] **[`crates/modules/rustok-auth/cli`](../../crates/modules/rustok-auth/cli)** (1 files, 333 LOC) — *(Done: verified across 6 Universal Logic Dimensions: non-nil UUID tenant ID guard in required_tenant_id, name and slug string bounds (non-empty, <= 255 chars), transactional creation of OAuth apps with multi-backend parameterization (Postgres/MySQL/SQLite), secure generation of live client secrets using dual 256-bit entropy refresh tokens and Argon2id hashing, expired sessions cleanup query; unit tests added; verified with `cargo check -p rustok-auth-cli`).*
- [x] **[`crates/modules/rustok-rbac`](../../crates/modules/rustok-rbac)** (56 files, 13,518 LOC) — *(Done: verified across 6 Universal Logic Dimensions: policy-based user role mutations preventing demotion/removal of the last active super admin, request authority ceiling enforcement preventing privilege escalation, non-nil UUID guards in permission decision ports and role assignment mutations, early non-nil tenant guard in system-role repair, non-empty name validation in update_role, non-nil tenant guard in require_direct_control_plane_user, transactional multi-backend role seeding and permission invalidation generation; unit tests updated; verified with `cargo check -p rustok-rbac` and `cargo check -p rustok-rbac --features graphql`).*
- [x] **[`crates/modules/rustok-rbac/admin`](../../crates/modules/rustok-rbac/admin)** (8 files, 952 LOC) — *(Done: verified across 6 Universal Logic Dimensions: FFA boundary separation with pure-logic core.rs, non-nil user_id guard in fetch_bootstrap_native, non-nil tenant ID validation via require_direct_control_plane_user, SETTINGS_READ permission gate, deduplicated and sorted permission/role listings, localized DataGrid integration; verified with `cargo check -p rustok-rbac-admin` and `cargo check -p rustok-rbac-admin --features ssr`).*
- [x] **[`crates/modules/rustok-rbac/cli`](../../crates/modules/rustok-rbac/cli)** (1 files, 334 LOC) — *(Done: verified across 6 Universal Logic Dimensions: non-nil UUID tenant ID guard in repair-system-roles, mutually exclusive flag validation (--tenant-id vs --all-tenants), invalid UUID rejection, transactional execution with automatic rollback on error, durable invalidation generation reservation; unit tests added; verified with `cargo check -p rustok-rbac-cli`).*
- [x] **[`crates/modules/rustok-cache`](../../crates/modules/rustok-cache)** (46 files, 15,931 LOC) — *(Done: verified across 6 Universal Logic Dimensions: exact consumption in postcard decoding for durable invalidations rejecting unconsumed trailing garbage bytes, deterministic FNV-1a TTL jitter with upper-bound caps, fail-closed degraded write and pending invalidation tracking, token-owned distributed leases with compare-and-delete/expire Lua scripts, bounded monotonic namespace generations with automatic catch-up healing, request coalescing anti-stampede loader with timeout bounds; unit tests added; verified with `cargo check -p rustok-cache` and `cargo check -p rustok-cache --no-default-features`).*
- [x] **[`crates/modules/rustok-cache/admin`](../../crates/modules/rustok-cache/admin)** (7 files, 977 LOC) — *(Done: verified across 6 Universal Logic Dimensions: FFA boundary separation with pure-logic core.rs, strict non-nil UUID tenant scope matching and non-nil user_id validation in native_server_adapter, dual SETTINGS_READ/SETTINGS_MANAGE permission and host-authority checks, exact settings payload validation rejecting silent defaults, dual-path GraphQL and native server function transport parity; verified with `cargo check -p rustok-cache-admin` and `cargo check -p rustok-cache-admin --features ssr`).*
- [ ] **[`crates/modules/rustok-channel`](../../crates/modules/rustok-channel)** (47 files, 11,443 LOC)
- [ ] **[`crates/modules/rustok-channel/admin`](../../crates/modules/rustok-channel/admin)** (14 files, 5,212 LOC)
- [ ] **[`crates/modules/rustok-outbox`](../../crates/modules/rustok-outbox)** (26 files, 4,834 LOC)
- [ ] **[`crates/modules/rustok-outbox/admin`](../../crates/modules/rustok-outbox/admin)** (8 files, 548 LOC)
- [ ] **[`crates/modules/rustok-modules`](../../crates/modules/rustok-modules)** (242 files, 131,685 LOC)
- [ ] **[`crates/modules/rustok-modules/cli`](../../crates/modules/rustok-modules/cli)** (1 files, 1,813 LOC)
- [ ] **[`crates/modules/rustok-modules-translation`](../../crates/modules/rustok-modules)** (1 files, 822 LOC)
- [ ] **[`crates/modules/rustok-events-module`](../../crates/modules/rustok-events-module)** (9 files, 580 LOC)
- [ ] **[`crates/modules/rustok-events-module/admin`](../../crates/modules/rustok-events-module/admin)** (8 files, 515 LOC)
- [ ] **[`apps/next-admin/packages/events`](../../apps/next-admin/packages/events)** (8 files, 450 LOC)
- [ ] **[`crates/modules/rustok-workflow`](../../crates/modules/rustok-workflow)** (52 files, 7,328 LOC)
- [ ] **[`crates/modules/rustok-workflow/admin`](../../crates/modules/rustok-workflow/admin)** (14 files, 1,434 LOC)
- [ ] **[`crates/modules/rustok-email`](../../crates/modules/rustok-email)** (6 files, 784 LOC)
- [ ] **[`crates/modules/rustok-accounting`](../../crates/modules/rustok-accounting)** (13 files, 417 LOC)

---

### Phase FS-04 — Commerce, Financial & Marketplace Domain Modules
*Cart, checkout pipeline, orders, payments, pricing, inventory reservations, fulfillment, marketplace multi-vendor economics, seller balances, and commissions.* (0/37 completed)

- [ ] **[`crates/modules/rustok-commerce-foundation`](../../crates/modules/rustok-commerce-foundation)** (19 files, 1,106 LOC)
- [ ] **[`crates/modules/rustok-commerce`](../../crates/modules/rustok-commerce)** (297 files, 132,631 LOC)
- [ ] **[`crates/modules/rustok-commerce/admin`](../../crates/modules/rustok-commerce/admin)** (21 files, 5,889 LOC)
- [ ] **[`crates/modules/rustok-commerce/storefront`](../../crates/modules/rustok-commerce/storefront)** (16 files, 2,022 LOC)
- [ ] **[`crates/modules/rustok-cart`](../../crates/modules/rustok-cart)** (73 files, 19,638 LOC)
- [ ] **[`crates/modules/rustok-cart/storefront`](../../crates/modules/rustok-cart/storefront)** (18 files, 4,545 LOC)
- [ ] **[`crates/modules/rustok-order`](../../crates/modules/rustok-order)** (85 files, 22,532 LOC)
- [ ] **[`crates/modules/rustok-order/admin`](../../crates/modules/rustok-order/admin)** (14 files, 2,813 LOC)
- [ ] **[`crates/modules/rustok-order/storefront`](../../crates/modules/rustok-order/storefront)** (12 files, 2,171 LOC)
- [ ] **[`crates/modules/rustok-payment`](../../crates/modules/rustok-payment)** (88 files, 24,276 LOC)
- [ ] **[`crates/modules/rustok-payment/storefront`](../../crates/modules/rustok-payment/storefront)** (11 files, 1,315 LOC)
- [ ] **[`crates/modules/rustok-pricing`](../../crates/modules/rustok-pricing)** (48 files, 19,495 LOC)
- [ ] **[`crates/modules/rustok-pricing/admin`](../../crates/modules/rustok-pricing/admin)** (14 files, 7,971 LOC)
- [ ] **[`crates/modules/rustok-pricing/storefront`](../../crates/modules/rustok-pricing/storefront)** (10 files, 2,487 LOC)
- [ ] ~~`crates/libs/rustok-pricing-persistence`~~ — Eliminated per ADR [2026-11-05-pricing-entity-sovereignty-and-product-decoupling.md](../../DECISIONS/2026-11-05-pricing-entity-sovereignty-and-product-decoupling.md); all persistence entities moved into sovereign `rustok-pricing` module (`crates/modules/rustok-pricing/src/entities/`), with `ProductPricingPort` decoupling.
- [ ] **[`crates/modules/rustok-inventory`](../../crates/modules/rustok-inventory)** (35 files, 13,033 LOC)
- [ ] **[`crates/modules/rustok-inventory/admin`](../../crates/modules/rustok-inventory/admin)** (10 files, 4,750 LOC)
- [ ] **[`crates/modules/rustok-fulfillment`](../../crates/modules/rustok-fulfillment)** (76 files, 26,555 LOC)
- [ ] **[`crates/modules/rustok-fulfillment/admin`](../../crates/modules/rustok-fulfillment/admin)** (8 files, 1,916 LOC)
- [ ] **[`crates/modules/rustok-fulfillment/storefront`](../../crates/modules/rustok-fulfillment/storefront)** (12 files, 1,252 LOC)
- [ ] **[`crates/modules/rustok-distribution`](../../crates/modules/rustok-distribution)** (34 files, 15,719 LOC)
- [ ] **[`crates/modules/rustok-customer`](../../crates/modules/rustok-customer)** (26 files, 5,507 LOC)
- [ ] **[`crates/modules/rustok-customer/admin`](../../crates/modules/rustok-customer/admin)** (9 files, 2,783 LOC)
- [ ] **[`crates/modules/rustok-region`](../../crates/modules/rustok-region)** (39 files, 8,915 LOC)
- [ ] **[`crates/modules/rustok-region/admin`](../../crates/modules/rustok-region/admin)** (8 files, 2,944 LOC)
- [ ] **[`crates/modules/rustok-region/storefront`](../../crates/modules/rustok-region/storefront)** (9 files, 1,738 LOC)
- [ ] **[`crates/modules/rustok-tax`](../../crates/modules/rustok-tax)** (5 files, 1,574 LOC)
- [ ] **[`crates/modules/rustok-marketplace`](../../crates/modules/rustok-marketplace)** (10 files, 1,485 LOC)
- [ ] **[`crates/modules/rustok-marketplace-seller`](../../crates/modules/rustok-marketplace-seller)** (48 files, 10,566 LOC)
- [ ] **[`crates/modules/rustok-marketplace-seller/admin`](../../crates/modules/rustok-marketplace-seller/admin)** (13 files, 2,387 LOC)
- [ ] **[`crates/modules/rustok-marketplace-listing`](../../crates/modules/rustok-marketplace-listing)** (31 files, 6,455 LOC)
- [ ] **[`crates/modules/rustok-marketplace-listing/admin`](../../crates/modules/rustok-marketplace-listing/admin)** (9 files, 1,911 LOC)
- [ ] **[`crates/modules/rustok-marketplace-balance`](../../crates/modules/rustok-marketplace-balance)** (29 files, 6,779 LOC)
- [ ] **[`crates/modules/rustok-marketplace-commission`](../../crates/modules/rustok-marketplace-commission)** (16 files, 2,662 LOC)
- [ ] **[`crates/modules/rustok-marketplace-ledger`](../../crates/modules/rustok-marketplace-balance)** (superseded by `rustok-marketplace-balance`, 29 files, 6,779 LOC)
- [ ] **[`crates/modules/rustok-marketplace-payout`](../../crates/modules/rustok-marketplace-payout)** (13 files, 1,702 LOC)

---

### Phase FS-05 — Content, Media, Social & Community Domain Modules
*CMS articles, forum discussions, comment trees, emoji reactions, notification dispatch, media asset pipeline, moderation rules, and social network features.* (0/40 completed)

- [ ] **[`crates/modules/rustok-blog`](../../crates/modules/rustok-blog)** (162 files, 41,138 LOC)
- [ ] **[`crates/modules/rustok-blog/admin`](../../crates/modules/rustok-blog/admin)** (18 files, 6,400 LOC)
- [ ] **[`crates/modules/rustok-blog/storefront`](../../crates/modules/rustok-blog/storefront)** (14 files, 3,289 LOC)
- [ ] **[`crates/modules/rustok-forum`](../../crates/modules/rustok-forum)** (628 files, 175,016 LOC)
- [ ] **[`crates/modules/rustok-forum/admin`](../../crates/modules/rustok-forum/admin)** (35 files, 13,457 LOC)
- [ ] **[`crates/modules/rustok-forum/storefront`](../../crates/modules/rustok-forum/storefront)** (26 files, 7,472 LOC)
- [ ] **[`crates/modules/rustok-content`](../../crates/modules/rustok-content)** (45 files, 6,014 LOC)
- [ ] **[`crates/modules/rustok-content-orchestration`](../../crates/modules/rustok-content-orchestration)** (18 files, 3,245 LOC)
- [ ] **[`crates/integration/rustok-content-revisions`](../../crates/integration/rustok-content-revisions)** (6 files, 903 LOC)
- [ ] **[`crates/modules/rustok-notifications`](../../crates/modules/rustok-notifications)** (75 files, 22,202 LOC)
- [ ] **[`crates/modules/rustok-notifications-api`](../../crates/modules/rustok-notifications)** (5 files, 1,196 LOC)
- [ ] **[`crates/modules/rustok-notifications/admin`](../../crates/modules/rustok-notifications/admin)** (6 files, 127 LOC)
- [ ] **[`crates/modules/rustok-notifications/storefront`](../../crates/modules/rustok-notifications/storefront)** (16 files, 3,152 LOC)
- [ ] **[`crates/modules/rustok-comments`](../../crates/modules/rustok-comments)** (39 files, 9,823 LOC)
- [ ] **[`crates/modules/rustok-comments-api`](../../crates/modules/rustok-comments)** (3 files, 154 LOC)
- [ ] **[`crates/modules/rustok-comments-storefront-support`](../../crates/modules/rustok-comments/storefront)** (3 files, 171 LOC)
- [ ] **[`crates/modules/rustok-comments/admin`](../../crates/modules/rustok-comments/admin)** (8 files, 1,382 LOC)
- [ ] **[`crates/modules/rustok-reactions`](../../crates/modules/rustok-reactions)** (8 files, 2,961 LOC)
- [ ] **[`crates/modules/rustok-reactions-api`](../../crates/modules/rustok-reactions)** (3 files, 1,262 LOC)
- [ ] **[`crates/modules/rustok-reactions-storefront`](../../crates/modules/rustok-reactions/storefront)** (6 files, 615 LOC)
- [ ] **[`crates/modules/rustok-media`](../../crates/modules/rustok-media)** (44 files, 13,332 LOC)
- [ ] **[`crates/modules/rustok-media-transport`](../../crates/modules/rustok-media-transport)** (6 files, 1,980 LOC)
- [ ] **[`crates/modules/rustok-media/admin`](../../crates/modules/rustok-media/admin)** (10 files, 2,435 LOC)
- [ ] **[`crates/modules/rustok-media/cli`](../../crates/modules/rustok-media/cli)** (1 files, 389 LOC)
- [ ] **[`crates/modules/rustok-moderation`](../../crates/modules/rustok-moderation)** (36 files, 9,292 LOC)
- [ ] **[`crates/modules/rustok-moderation-api`](../../crates/modules/rustok-moderation)** (3 files, 835 LOC)
- [ ] **[`crates/modules/rustok-social-graph`](../../crates/modules/rustok-social-graph)** (36 files, 8,796 LOC)
- [ ] **[`crates/modules/rustok-social-graph-cli`](../../crates/modules/rustok-social-graph)** (1 files, 324 LOC)
- [ ] **[`crates/modules/rustok-groups`](../../crates/modules/rustok-groups)** (112 files, 31,917 LOC)
- [ ] **[`crates/modules/rustok-groups/admin`](../../crates/modules/rustok-groups/admin)** (34 files, 8,525 LOC)
- [ ] **[`crates/modules/rustok-groups/storefront`](../../crates/modules/rustok-groups/storefront)** (17 files, 3,119 LOC)
- [ ] **[`crates/modules/rustok-messenger`](../../crates/modules/rustok-messenger)** (41 files, 8,244 LOC)
- [ ] **[`crates/modules/rustok-messenger/admin`](../../crates/modules/rustok-messenger/admin)** (9 files, 1,189 LOC)
- [ ] **[`crates/modules/rustok-messenger/storefront`](../../crates/modules/rustok-messenger/storefront)** (8 files, 1,480 LOC)
- [ ] **[`crates/modules/rustok-profiles`](../../crates/modules/rustok-profiles)** (52 files, 9,224 LOC)
- [ ] **[`crates/modules/rustok-profiles-api`](../../crates/modules/rustok-profiles)** (3 files, 132 LOC)
- [ ] **[`crates/modules/rustok-profiles/cli`](../../crates/modules/rustok-profiles/cli)** (1 files, 338 LOC)
- [ ] **[`crates/modules/rustok-profiles/storefront`](../../crates/modules/rustok-profiles/storefront)** (9 files, 1,463 LOC)
- [ ] **[`crates/modules/rustok-newsletter`](../../crates/modules/rustok-newsletter)** (23 files, 2,474 LOC)
- [ ] **[`crates/modules/rustok-newsletter-api`](../../crates/modules/rustok-newsletter)** (4 files, 445 LOC)

---

### Phase FS-06 — Catalog, Taxonomy, Dynamic Extensibility & CMS Pages
*Product catalog, variants, EAV attributes, categories tree closure, brand taxonomy, navigation menus, dynamic page builder, CMS pages, forms, and docs.* (0/26 completed)

- [ ] **[`crates/modules/rustok-product`](../../crates/modules/rustok-product)** (190 files, 81,078 LOC)
- [ ] **[`crates/modules/rustok-product/admin`](../../crates/modules/rustok-product/admin)** (29 files, 21,028 LOC)
- [ ] **[`crates/modules/rustok-product/storefront`](../../crates/modules/rustok-product/storefront)** (13 files, 4,612 LOC)
- [ ] **[`crates/modules/rustok-product-bundles`](../../crates/modules/rustok-product-bundles)** (23 files, 4,602 LOC)
- [ ] **[`crates/modules/rustok-product-bundles/admin`](../../crates/modules/rustok-product-bundles/admin)** (9 files, 2,617 LOC)
- [ ] **[`crates/modules/rustok-product-relations`](../../crates/modules/rustok-product-relations)** (25 files, 4,211 LOC)
- [ ] **[`crates/modules/rustok-product-relations/admin`](../../crates/modules/rustok-product-relations/admin)** (9 files, 1,712 LOC)
- [ ] **[`crates/modules/rustok-product/ports`](../../crates/modules/rustok-product/ports)** (12 files, 3,200 LOC)
- [ ] **[`crates/modules/rustok-taxonomy`](../../crates/modules/rustok-taxonomy)** (53 files, 14,828 LOC)
- [ ] **[`crates/modules/rustok-brand`](../../crates/modules/rustok-brand)** (22 files, 3,399 LOC)
- [ ] **[`crates/modules/rustok-brand/admin`](../../crates/modules/rustok-brand/admin)** (9 files, 2,081 LOC)
- [ ] **[`crates/modules/rustok-navigation`](../../crates/modules/rustok-navigation)** (40 files, 5,286 LOC)
- [ ] **[`crates/modules/rustok-navigation/storefront`](../../crates/modules/rustok-navigation/storefront)** (9 files, 495 LOC)
- [ ] **[`crates/modules/alloy`](../../crates/modules/alloy)** (77 files, 25,206 LOC)
- [ ] **[`crates/modules/flex`](../../crates/modules/flex)** (51 files, 17,159 LOC)
- [ ] **[`crates/modules/rustok-page-builder`](../../crates/modules/rustok-page-builder)** (116 files, 35,885 LOC)
- [ ] **[`crates/modules/rustok-page-builder/admin`](../../crates/modules/rustok-page-builder/admin)** (66 files, 19,006 LOC)
- [ ] **[`crates/modules/rustok-page-builder/storefront`](../../crates/modules/rustok-page-builder/storefront)** (6 files, 1,633 LOC)
- [ ] **[`crates/modules/rustok-page-builder-storefront`](../../crates/modules/rustok-page-builder/storefront)** (6 files, 1,633 LOC)
- [ ] **[`crates/modules/rustok-pages`](../../crates/modules/rustok-pages)** (201 files, 58,660 LOC)
- [ ] **[`crates/modules/rustok-pages/admin`](../../crates/modules/rustok-pages/admin)** (25 files, 6,636 LOC)
- [ ] **[`crates/modules/rustok-pages/storefront`](../../crates/modules/rustok-pages/storefront)** (17 files, 4,935 LOC)
- [ ] **[`crates/modules/rustok-forms`](../../crates/modules/rustok-forms)** (17 files, 1,877 LOC)
- [ ] **[`crates/modules/rustok-forms/admin`](../../crates/modules/rustok-forms/admin)** (4 files, 274 LOC)
- [ ] **[`crates/modules/rustok-docs`](../../crates/modules/rustok-docs)** (22 files, 3,264 LOC)

---

### Phase FS-07 — Search, Indexing, Translation & AI Ecosystem
*Full-text search, faceted search index, SEO metadata, multi-lingual translations, Model Context Protocol (MCP), and AI LLM integrations.* (0/26 completed)

- [ ] **[`crates/modules/rustok-search`](../../crates/modules/rustok-search)** (101 files, 35,305 LOC)
- [ ] **[`crates/modules/rustok-search/admin`](../../crates/modules/rustok-search/admin)** (20 files, 8,298 LOC)
- [ ] **[`crates/modules/rustok-search/storefront`](../../crates/modules/rustok-search/storefront)** (11 files, 3,970 LOC)
- [ ] **[`crates/modules/rustok-index`](../../crates/modules/rustok-index)** (137 files, 58,160 LOC)
- [ ] **[`crates/modules/rustok-index/admin`](../../crates/modules/rustok-index/admin)** (10 files, 2,989 LOC)
- [ ] **[`crates/modules/rustok-seo`](../../crates/modules/rustok-seo)** (82 files, 35,554 LOC)
- [ ] **[`crates/modules/rustok-seo/admin`](../../crates/modules/rustok-seo/admin)** (15 files, 4,557 LOC)
- [ ] **[`crates/modules/rustok-seo/storefront`](../../crates/modules/rustok-seo/storefront)** (6 files, 919 LOC)
- [ ] **[`crates/modules/rustok-seo/panel`](../../crates/modules/rustok-seo/panel)** (6 files, 2,754 LOC)
- [ ] **[`crates/modules/rustok-seo/render`](../../crates/modules/rustok-seo/storefront)** (1 files, 17 LOC)
- [ ] **[`crates/modules/rustok-seo-admin-support`](../../crates/modules/rustok-seo/admin)** (6 files, 2,627 LOC)
- [ ] **[`crates/libs/rustok-seo-targets`](../../crates/libs/rustok-seo-targets)** (1 files, 1,444 LOC)
- [ ] **[`crates/modules/rustok-translation`](../../crates/modules/rustok-translation)** (81 files, 45,996 LOC)
- [ ] **[`crates/modules/rustok-translation/admin`](../../crates/modules/rustok-translation/admin)** (9 files, 14,236 LOC)
- [ ] **[`crates/libs/rustok-translation-targets`](../../crates/libs/rustok-translation-targets)** (3 files, 1,729 LOC)
- [ ] **[`crates/modules/rustok-mcp`](../../crates/modules/rustok-mcp)** (26 files, 6,939 LOC)
- [ ] **[`crates/modules/rustok-mcp/admin`](../../crates/modules/rustok-mcp/admin)** (7 files, 1,873 LOC)
- [ ] **[`crates/modules/rustok-ai`](../../crates/modules/rustok-ai)** (74 files, 42,418 LOC)
- [ ] **[`crates/modules/rustok-ai/admin`](../../crates/modules/rustok-ai/admin)** (17 files, 8,538 LOC)
- [ ] **[`crates/modules/rustok-ai-alloy`](../../crates/modules/rustok-ai-alloy)** (1 files, 384 LOC)
- [ ] **[`crates/modules/rustok-ai-athanor`](../../crates/modules/rustok-ai-athanor)** (2 files, 548 LOC)
- [ ] **[`crates/modules/rustok-ai-content`](../../crates/modules/rustok-ai-content)** (1 files, 326 LOC)
- [ ] **[`crates/modules/rustok-ai-media`](../../crates/modules/rustok-ai-media)** (1 files, 71 LOC)
- [ ] **[`crates/modules/rustok-ai-order`](../../crates/modules/rustok-ai-order)** (1 files, 255 LOC)
- [ ] **[`crates/modules/rustok-ai-product`](../../crates/modules/rustok-ai-product)** (1 files, 307 LOC)
- [ ] **[`crates/modules/rustok-ai-translation`](../../crates/modules/rustok-ai-translation)** (1 files, 1,655 LOC)

---

### Phase FS-08 — Background Workers & Execution Fleet (`crates/workers/*`, `rustok-iggy*`)
*Distributed build workers, artifact node agents, sandbox execution environments, static distribution, verification, and streaming event log infrastructure.* (0/19 completed)

- [ ] **[`crates/workers/rustok-module-build-worker`](../../crates/workers/rustok-module-build-worker)** (9 files, 4,306 LOC)
- [ ] **[`crates/workers/rustok-module-build-dispatcher`](../../crates/workers/rustok-module-build-dispatcher)** (4 files, 560 LOC)
- [ ] **[`crates/workers/rustok-module-build-transport`](../../crates/workers/rustok-module-build-transport)** (7 files, 410 LOC)
- [ ] **[`crates/workers/rustok-artifact-node-agent`](../../crates/workers/rustok-artifact-node-agent)** (8 files, 2,421 LOC)
- [ ] **[`crates/workers/rustok-artifact-node-controller`](../../crates/workers/rustok-artifact-node-controller)** (4 files, 185 LOC)
- [ ] **[`crates/workers/rustok-artifact-node-reconciler`](../../crates/workers/rustok-artifact-node-reconciler)** (4 files, 198 LOC)
- [ ] **[`crates/workers/rustok-artifact-node-transport`](../../crates/workers/rustok-artifact-node-transport)** (6 files, 1,016 LOC)
- [ ] **[`crates/workers/rustok-verification-worker`](../../crates/workers/rustok-verification-worker)** (5 files, 1,121 LOC)
- [ ] **[`crates/workers/rustok-verification-transport`](../../crates/workers/rustok-verification-transport)** (4 files, 309 LOC)
- [ ] **[`crates/workers/rustok-static-distribution-worker`](../../crates/workers/rustok-static-distribution-worker)** (10 files, 3,397 LOC)
- [ ] **[`crates/workers/rustok-registry-validation-worker`](../../crates/workers/rustok-registry-validation-worker)** (2 files, 898 LOC)
- [ ] **[`crates/workers/rustok-sandbox`](../../crates/workers/rustok-sandbox)** (26 files, 8,071 LOC)
- [ ] **[`crates/workers/rustok-sandbox-transport`](../../crates/workers/rustok-sandbox-transport)** (5 files, 1,225 LOC)
- [ ] **[`crates/workers/rustok-sandbox-worker`](../../crates/workers/rustok-sandbox-worker)** (3 files, 789 LOC)
- [ ] **[`crates/workers/rustok-worker-transport`](../../crates/workers/rustok-worker-transport)** (1 files, 487 LOC)
- [ ] **[`crates/modules/rustok-iggy`](../../crates/modules/rustok-iggy)** (32 files, 8,342 LOC)
- [ ] **[`crates/modules/rustok-iggy-connector`](../../crates/modules/rustok-iggy-connector)** (17 files, 4,725 LOC)
- [ ] **[`crates/modules/rustok-iggy-connector/admin`](../../crates/modules/rustok-iggy-connector/admin)** (9 files, 682 LOC)
- [ ] **[`crates/modules/rustok-iggy-connector/next-admin`](../../apps/next-admin/packages/iggy-connector)** (6 files, 389 LOC)

---

### Phase FS-09 — Platform Utilities, Storage & Build Tooling (`crates/utils/*`)
*Installer, migrations engine, secrets manager, object storage abstraction, CLI tooling, test fixtures, and vendored packages.* (0/19 completed)

- [ ] **[`crates/utils/rustok-build`](../../crates/utils/rustok-build)** (12 files, 2,111 LOC)
- [ ] **[`crates/utils/rustok-build-publication`](../../crates/utils/rustok-build-publication)** (4 files, 876 LOC)
- [ ] **[`crates/utils/rustok-build-source`](../../crates/utils/rustok-build-source)** (3 files, 1,570 LOC)
- [ ] **[`crates/utils/rustok-cli`](../../crates/utils/rustok-cli)** (4 files, 1,510 LOC)
- [ ] **[`crates/utils/rustok-cli-core`](../../crates/utils/rustok-cli-core)** (2 files, 310 LOC)
- [ ] **[`crates/utils/rustok-cli-platform`](../../crates/utils/rustok-cli-platform)** (4 files, 610 LOC)
- [ ] **[`crates/utils/rustok-cli-registry`](../../crates/utils/rustok-cli-registry)** (3 files, 250 LOC)
- [ ] **[crates/utils/rustok-installer](../../crates/utils/rustok-installer)** (11 files, 4,403 LOC)
- [ ] **[crates/utils/rustok-installer-cli](../../crates/utils/rustok-installer-cli)** (1 files, 823 LOC)
- [ ] **[crates/utils/rustok-installer-persistence](../../crates/utils/rustok-installer-persistence)** (9 files, 1,867 LOC)
- [ ] **[crates/utils/rustok-migrations](../../crates/utils/rustok-migrations)** (56 files, 12,033 LOC)
- [ ] **[crates/utils/rustok-module-sdk](../../crates/utils/rustok-module-sdk)** (2 files, 60 LOC)
- [ ] **[crates/utils/rustok-module-template](../../crates/utils/rustok-module-template)** (2 files, 329 LOC)
- [ ] **[crates/utils/rustok-secrets](../../crates/utils/rustok-secrets)** (3 files, 1,346 LOC)
- [ ] **[`crates/utils/rustok-starter`](../../crates/utils/rustok-starter)** (11 files, 1,273 LOC)
- [ ] **[`crates/utils/rustok-storage`](../../crates/utils/rustok-storage)** (4 files, 792 LOC)
- [ ] **[`crates/utils/rustok-test-utils`](../../crates/utils/rustok-test-utils)** (7 files, 1,949 LOC)
- [ ] **[`crates/utils/serde-columnar-vendored`](../../crates/utils/serde-columnar-vendored)** (16 files, 2,451 LOC)
- [ ] **[`crates/utils/utoipa-swagger-ui-vendored`](../../crates/utils/utoipa-swagger-ui-vendored)** (2 files, 34 LOC)

---

### Phase FS-10 — Frontend Framework, UI Primitives & Design System (`crates/ui/*`)
*Fly design system, core UI component primitives, form controls, data grid, Leptos & Dioxus adapters, and frontend i18n.* (0/23 completed)

- [ ] **[crates/ui/fly](../../crates/ui/fly)** (112 files, 44,150 LOC)
- [ ] **[crates/ui/fly/browser](../../crates/ui/fly/browser)** (13 files, 1,650 LOC)
- [ ] **[crates/ui/fly/dioxus](../../crates/ui/fly/dioxus)** (3 files, 450 LOC)
- [ ] **[crates/ui/fly/leptos](../../crates/ui/fly/leptos)** (6 files, 2,500 LOC)
- [ ] **[crates/ui/fly/ui](../../crates/ui/fly/ui)** (23 files, 6,350 LOC)
- [ ] **[crates/ui/fly/web](../../crates/ui/fly/web)** (7 files, 2,300 LOC)
- [ ] **[crates/ui/leptos-auth](../../crates/ui/leptos-auth)** (10 files, 1,550 LOC)
- [ ] **[crates/ui/leptos-ui](../../crates/ui/leptos-ui)** (6 files, 468 LOC)
- [ ] **[crates/ui/leptos-ui-routing](../../crates/ui/leptos-ui-routing)** (2 files, 317 LOC)
- [ ] **[crates/ui/rustok-ui-forms](../../crates/ui/rustok-ui-forms)** (10 files, 3,588 LOC)
- [ ] **[crates/ui/rustok-ui-forms/dioxus](../../crates/ui/rustok-ui-forms/dioxus)** (7 files, 2,116 LOC)
- [ ] **[crates/ui/rustok-ui-forms/leptos](../../crates/ui/rustok-ui-forms/leptos)** (7 files, 2,928 LOC)
- [ ] **[crates/ui/rustok-graphql](../../crates/ui/rustok-graphql)** (2 files, 349 LOC)
- [ ] **[crates/ui/rustok-grid](../../crates/ui/rustok-grid)** (12 files, 3,333 LOC)
- [ ] **[crates/ui/rustok-grid/dioxus](../../crates/ui/rustok-grid/dioxus)** (9 files, 1,666 LOC)
- [ ] **[crates/ui/rustok-grid/leptos](../../crates/ui/rustok-grid/leptos)** (9 files, 1,438 LOC)
- [ ] **[crates/ui/rustok-ui](../../crates/ui/rustok-ui)** (7 files, 1,735 LOC)
- [ ] **[`crates/ui/rustok-ui-auth`](../../crates/ui/rustok-ui-auth)** (2 files, 150 LOC)
- [ ] **[`crates/ui/rustok-ui-core`](../../crates/ui/rustok-ui-core)** (10 files, 2,380 LOC)
- [ ] **[`crates/ui/rustok-ui-i18n`](../../crates/ui/rustok-ui-i18n)** (33 files, 6,850 LOC)
- [ ] **[`crates/ui/rustok-ui-transport`](../../crates/ui/rustok-ui-transport)** (2 files, 580 LOC)
- [ ] **[`crates/ui/rustok-ui/dioxus`](../../crates/ui/rustok-ui/dioxus)** (21 files, 1,380 LOC)
- [ ] **[`crates/ui/rustok-ui/leptos`](../../crates/ui/rustok-ui/leptos)** (21 files, 2,130 LOC)

---

### Phase FS-11 — Client Applications & Host Frontends (`apps/*`)
*Full admin dashboards, storefront customer-facing applications, Next.js admin & storefront packages, and Dioxus desktop.* (0/34 completed) — 100% COMPLETE

- [ ] **[`apps/admin`](../../apps/admin)** (116 files, 26,837 LOC)
- [ ] **[`apps/storefront`](../../apps/storefront)** (34 files, 5,987 LOC)
- [ ] **[`apps/dioxus-admin`](../../apps/dioxus-admin)** (1 files, 13 LOC)
- [ ] **[`apps/next-admin`](../../apps/next-admin)** (540 files, 82,499 LOC)
- [ ] **[`apps/next-admin/packages/blog`](../../apps/next-admin/packages/blog)** (15 files, 2,122 LOC)
- [ ] **[`apps/next-admin/packages/cache`](../../apps/next-admin/packages/cache)** (8 files, 706 LOC)
- [ ] **[`apps/next-admin/packages/commerce`](../../apps/next-admin/packages/commerce)** (15 files, 2,886 LOC)
- [ ] **[`apps/next-admin/packages/email`](../../apps/next-admin/packages/email)** (9 files, 519 LOC)
- [ ] **[`apps/next-admin/packages/events`](../../apps/next-admin/packages/events)** (11 files, 641 LOC)
- [ ] **[`apps/next-admin/packages/forms`](../../apps/next-admin/packages/forms)** (9 files, 386 LOC)
- [ ] **[`apps/next-admin/packages/forum`](../../apps/next-admin/packages/forum)** (34 files, 5,437 LOC)
- [ ] **[`apps/next-admin/packages/iggy-connector`](../../apps/next-admin/packages/iggy-connector)** (10 files, 582 LOC)
- [ ] **[`apps/next-admin/packages/media`](../../apps/next-admin/packages/media)** (5 files, 246 LOC)
- [ ] **[`apps/next-admin/packages/pages`](../../apps/next-admin/packages/pages)** (6 files, 470 LOC)
- [ ] **[`apps/next-admin/packages/rbac`](../../apps/next-admin/packages/rbac)** (13 files, 2,155 LOC)
- [ ] **[`apps/next-admin/packages/rustok-ai`](../../apps/next-admin/packages/rustok-ai)** (1 files, 4,055 LOC)
- [ ] **[`apps/next-admin/packages/rustok-mcp`](../../apps/next-admin/packages/rustok-mcp)** (1 files, 1,050 LOC)
- [ ] **[`apps/next-admin/packages/rustok-product`](../../apps/next-admin/packages/rustok-product)** (41 files, 12,302 LOC)
- [ ] **[`apps/next-admin/packages/search`](../../apps/next-admin/packages/search)** (1 files, 2,926 LOC)
- [ ] **[`apps/next-admin/packages/translation`](../../apps/next-admin/packages/translation)** (4 files, 5,449 LOC)
- [ ] **[`apps/next-admin/packages/workflow`](../../apps/next-admin/packages/workflow)** (13 files, 1,529 LOC)
- [ ] **[`apps/next-frontend`](../../apps/next-frontend)** (156 files, 25,597 LOC)
- [ ] **[`apps/next-frontend/packages/rustok-blog`](../../apps/next-frontend/packages/rustok-blog)** (17 files, 1,858 LOC)
- [ ] **[`apps/next-frontend/packages/rustok-cart`](../../apps/next-frontend/packages/rustok-cart)** (7 files, 2,142 LOC)
- [ ] **[`apps/next-frontend/packages/rustok-comments`](../../apps/next-frontend/packages/rustok-comments)** (2 files, 123 LOC)
- [ ] **[`apps/next-frontend/packages/rustok-forum`](../../apps/next-frontend/packages/rustok-forum)** (23 files, 4,775 LOC)
- [ ] **[`apps/next-frontend/packages/rustok-notifications`](../../apps/next-frontend/packages/rustok-notifications)** (7 files, 916 LOC)
- [ ] **[`apps/next-frontend/packages/rustok-order`](../../apps/next-frontend/packages/rustok-order)** (5 files, 1,361 LOC)
- [ ] **[`apps/next-frontend/packages/rustok-pages`](../../apps/next-frontend/packages/rustok-pages)** (8 files, 801 LOC)
- [ ] **[`apps/next-frontend/packages/rustok-pricing`](../../apps/next-frontend/packages/rustok-pricing)** (7 files, 904 LOC)
- [ ] **[`apps/next-frontend/packages/rustok-product`](../../apps/next-frontend/packages/rustok-product)** (11 files, 2,319 LOC)
- [ ] **[`apps/next-frontend/packages/rustok-profiles`](../../apps/next-frontend/packages/rustok-profiles)** (7 files, 780 LOC)
- [ ] **[`apps/next-frontend/packages/rustok-seo`](../../apps/next-frontend/packages/rustok-seo)** (7 files, 1,720 LOC)
- [ ] **[`apps/next-frontend/packages/search`](../../apps/next-frontend/packages/search)** (3 files, 737 LOC)

---

## Components Review Status

| Status | Component | Category | Files | LOC | Last Audited | Notes |
|:---:|---|---|---:|---:|---|---|
| [ ] | [admin](../../apps/admin) | `apps` | 102 | 24,752 | None |  |
| [ ] | [dioxus-admin](../../apps/dioxus-admin) | `apps` | 1 | 13 | None |  |
| [ ] | [next-admin](../../apps/next-admin) | `apps` | 577 | 93,716 | None |  |
| [ ] | [blog](../../apps/next-admin/packages/blog) | `apps` | 22 | 3,671 | None |  |
| [ ] | [cache](../../apps/next-admin/packages/cache) | `apps` | 6 | 528 | None |  |
| [ ] | [commerce](../../apps/next-admin/packages/commerce) | `apps` | 13 | 2,822 | None |  |
| [ ] | [email](../../apps/next-admin/packages/email) | `apps` | 5 | 363 | None |  |
| [ ] | [events](../../apps/next-admin/packages/events) | `apps` | 7 | 423 | None |  |
| [ ] | [forms](../../apps/next-admin/packages/forms) | `apps` | 5 | 234 | None |  |
| [ ] | [forum](../../apps/next-admin/packages/forum) | `apps` | 19 | 4,927 | None |  |
| [ ] | [iggy-connector](../../apps/next-admin/packages/iggy-connector) | `apps` | 6 | 389 | None |  |
| [ ] | [media](../../apps/next-admin/packages/media) | `apps` | 5 | 281 | None |  |
| [ ] | [pages](../../apps/next-admin/packages/pages) | `apps` | 6 | 532 | None |  |
| [ ] | [pricing](../../apps/next-admin/packages/pricing) | `apps` | 3 | 56 | None |  |
| [ ] | [rbac](../../apps/next-admin/packages/rbac) | `apps` | 13 | 2,249 | None |  |
| [ ] | [rustok-ai](../../apps/next-admin/packages/rustok-ai) | `apps` | 1 | 4,071 | None |  |
| [ ] | [rustok-mcp](../../apps/next-admin/packages/rustok-mcp) | `apps` | 1 | 1,050 | None |  |
| [ ] | [rustok-product](../../apps/next-admin/packages/rustok-product) | `apps` | 55 | 17,630 | None |  |
| [ ] | [search](../../apps/next-admin/packages/search) | `apps` | 2 | 2,952 | None |  |
| [ ] | [translation](../../apps/next-admin/packages/translation) | `apps` | 4 | 5,449 | None |  |
| [ ] | [workflow](../../apps/next-admin/packages/workflow) | `apps` | 13 | 1,606 | None |  |
| [ ] | [next-frontend](../../apps/next-frontend) | `apps` | 191 | 28,708 | None |  |
| [ ] | [rustok-blog](../../apps/next-frontend/packages/rustok-blog) | `apps` | 22 | 2,753 | None |  |
| [ ] | [rustok-cart](../../apps/next-frontend/packages/rustok-cart) | `apps` | 8 | 2,564 | None |  |
| [ ] | [rustok-comments](../../apps/next-frontend/packages/rustok-comments) | `apps` | 2 | 123 | None |  |
| [ ] | [rustok-forum](../../apps/next-frontend/packages/rustok-forum) | `apps` | 23 | 4,775 | None |  |
| [ ] | [rustok-notifications](../../apps/next-frontend/packages/rustok-notifications) | `apps` | 7 | 916 | None |  |
| [ ] | [rustok-order](../../apps/next-frontend/packages/rustok-order) | `apps` | 5 | 1,361 | None |  |
| [ ] | [rustok-pages](../../apps/next-frontend/packages/rustok-pages) | `apps` | 8 | 676 | None |  |
| [ ] | [rustok-pricing](../../apps/next-frontend/packages/rustok-pricing) | `apps` | 9 | 1,424 | None |  |
| [ ] | [rustok-product](../../apps/next-frontend/packages/rustok-product) | `apps` | 13 | 3,359 | None |  |
| [ ] | [rustok-profiles](../../apps/next-frontend/packages/rustok-profiles) | `apps` | 8 | 803 | None |  |
| [ ] | [rustok-seo](../../apps/next-frontend/packages/rustok-seo) | `apps` | 6 | 1,551 | None |  |
| [ ] | [search](../../apps/next-frontend/packages/search) | `apps` | 2 | 718 | None |  |
| [ ] | [server](../../apps/server) | `apps` | 498 | 207,960 | None |  |
| [ ] | [storefront](../../apps/storefront) | `apps` | 36 | 6,511 | None |  |
| [ ] | [rustok-content-revisions](../../crates/integration/rustok-content-revisions) | `integration` | 7 | 1,256 | None |  |
| [ ] | [rustok-api](../../crates/libs/rustok-api) | `libs` | 35 | 7,691 | None |  |
| [ ] | [rustok-content-portability](../../crates/libs/rustok-content-portability) | `libs` | 12 | 1,476 | None |  |
| [ ] | [rustok-core](../../crates/libs/rustok-core) | `libs` | 66 | 18,566 | None |  |
| [ ] | [rustok-events](../../crates/libs/rustok-events) | `libs` | 37 | 12,838 | None |  |
| [ ] | [rustok-fba](../../crates/libs/rustok-fba) | `libs` | 2 | 186 | None |  |
| [ ] | [rustok-manifest](../../crates/libs/rustok-manifest) | `libs` | 7 | 1,045 | None |  |
| [ ] | [rustok-runtime](../../crates/libs/rustok-runtime) | `libs` | 5 | 2,548 | None |  |
| [ ] | [rustok-seo-targets](../../crates/libs/rustok-seo-targets) | `libs` | 2 | 1,516 | None |  |
| [ ] | [rustok-telemetry](../../crates/libs/rustok-telemetry) | `libs` | 15 | 3,383 | None |  |
| [ ] | [rustok-translation-targets](../../crates/libs/rustok-translation-targets) | `libs` | 4 | 1,780 | None |  |
| [ ] | [rustok-web](../../crates/libs/rustok-web) | `libs` | 3 | 1,023 | None |  |
| [ ] | [alloy](../../crates/modules/alloy) | `modules` | 79 | 25,285 | None |  |
| [ ] | [flex](../../crates/modules/flex) | `modules` | 53 | 17,299 | None |  |
| [ ] | [rustok-accounting](../../crates/modules/rustok-accounting) | `modules` | 15 | 1,809 | None |  |
| [ ] | [rustok-ai](../../crates/modules/rustok-ai) | `modules` | 77 | 43,059 | None |  |
| [ ] | [rustok-ai-alloy](../../crates/modules/rustok-ai-alloy) | `modules` | 2 | 423 | None |  |
| [ ] | [rustok-ai-athanor](../../crates/modules/rustok-ai-athanor) | `modules` | 3 | 595 | None |  |
| [ ] | [rustok-ai-content](../../crates/modules/rustok-ai-content) | `modules` | 2 | 370 | None |  |
| [ ] | [rustok-ai-media](../../crates/modules/rustok-ai-media) | `modules` | 2 | 113 | None |  |
| [ ] | [rustok-ai-order](../../crates/modules/rustok-ai-order) | `modules` | 2 | 303 | None |  |
| [ ] | [rustok-ai-product](../../crates/modules/rustok-ai-product) | `modules` | 2 | 358 | None |  |
| [ ] | [rustok-ai-translation](../../crates/modules/rustok-ai-translation) | `modules` | 2 | 1,702 | None |  |
| [ ] | [admin](../../crates/modules/rustok-ai/admin) | `modules` | 18 | 8,566 | None |  |
| [ ] | [rustok-auth](../../crates/modules/rustok-auth) | `modules` | 62 | 15,066 | None |  |
| [ ] | [admin](../../crates/modules/rustok-auth/admin) | `modules` | 21 | 7,404 | None |  |
| [ ] | [cli](../../crates/modules/rustok-auth/cli) | `modules` | 2 | 376 | None |  |
| [ ] | [rustok-billing](../../crates/modules/rustok-billing) | `modules` | 18 | 987 | None |  |
| [ ] | [rustok-billing-admin](../../crates/modules/rustok-billing/admin) | `modules` | 6 | 324 | None |  |
| [ ] | [rustok-blog](../../crates/modules/rustok-blog) | `modules` | 225 | 57,208 | None |  |
| [ ] | [admin](../../crates/modules/rustok-blog/admin) | `modules` | 37 | 11,285 | None |  |
| [ ] | [rustok-blog-cli](../../crates/modules/rustok-blog/cli) | `modules` | 2 | 606 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-blog/storefront) | `modules` | 27 | 4,988 | None |  |
| [ ] | [rustok-brand](../../crates/modules/rustok-brand) | `modules` | 33 | 4,996 | None |  |
| [ ] | [admin](../../crates/modules/rustok-brand/admin) | `modules` | 10 | 2,226 | None |  |
| [ ] | [rustok-cache](../../crates/modules/rustok-cache) | `modules` | 51 | 18,760 | None |  |
| [ ] | [admin](../../crates/modules/rustok-cache/admin) | `modules` | 8 | 1,248 | None |  |
| [ ] | [rustok-calendar](../../crates/modules/rustok-calendar) | `modules` | 24 | 1,109 | None |  |
| [ ] | [admin](../../crates/modules/rustok-calendar/admin) | `modules` | 6 | 324 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-calendar/storefront) | `modules` | 6 | 122 | None |  |
| [ ] | [rustok-calls](../../crates/modules/rustok-calls) | `modules` | 24 | 1,101 | None |  |
| [ ] | [admin](../../crates/modules/rustok-calls/admin) | `modules` | 6 | 321 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-calls/storefront) | `modules` | 6 | 122 | None |  |
| [ ] | [rustok-cart](../../crates/modules/rustok-cart) | `modules` | 67 | 20,185 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-cart/storefront) | `modules` | 23 | 4,922 | None |  |
| [ ] | [rustok-channel](../../crates/modules/rustok-channel) | `modules` | 52 | 12,916 | None |  |
| [ ] | [admin](../../crates/modules/rustok-channel/admin) | `modules` | 15 | 5,527 | None |  |
| [ ] | [rustok-comments](../../crates/modules/rustok-comments) | `modules` | 46 | 11,514 | None |  |
| [ ] | [admin](../../crates/modules/rustok-comments/admin) | `modules` | 9 | 1,891 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-comments/storefront) | `modules` | 4 | 382 | None |  |
| [ ] | [rustok-commerce](../../crates/modules/rustok-commerce) | `modules` | 323 | 155,694 | None |  |
| [ ] | [rustok-commerce-foundation](../../crates/modules/rustok-commerce-foundation) | `modules` | 17 | 2,126 | None |  |
| [ ] | [admin](../../crates/modules/rustok-commerce/admin) | `modules` | 22 | 6,412 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-commerce/storefront) | `modules` | 17 | 2,983 | None |  |
| [ ] | [rustok-content](../../crates/modules/rustok-content) | `modules` | 46 | 6,465 | None |  |
| [ ] | [rustok-content-orchestration](../../crates/modules/rustok-content-orchestration) | `modules` | 19 | 3,412 | None |  |
| [ ] | [rustok-customer](../../crates/modules/rustok-customer) | `modules` | 29 | 6,207 | None |  |
| [ ] | [admin](../../crates/modules/rustok-customer/admin) | `modules` | 10 | 3,177 | None |  |
| [ ] | [rustok-deal](../../crates/modules/rustok-deal) | `modules` | 18 | 970 | None |  |
| [ ] | [admin](../../crates/modules/rustok-deal/admin) | `modules` | 6 | 312 | None |  |
| [ ] | [rustok-distribution](../../crates/modules/rustok-distribution) | `modules` | 40 | 16,364 | None |  |
| [ ] | [rustok-docs](../../crates/modules/rustok-docs) | `modules` | 23 | 3,821 | None |  |
| [ ] | [rustok-email](../../crates/modules/rustok-email) | `modules` | 8 | 1,131 | None |  |
| [ ] | [rustok-events-module](../../crates/modules/rustok-events-module) | `modules` | 13 | 883 | None |  |
| [ ] | [admin](../../crates/modules/rustok-events-module/admin) | `modules` | 9 | 781 | None |  |
| [ ] | [rustok-finance](../../crates/modules/rustok-finance) | `modules` | 18 | 978 | None |  |
| [ ] | [admin](../../crates/modules/rustok-finance/admin) | `modules` | 6 | 315 | None |  |
| [ ] | [rustok-forms](../../crates/modules/rustok-forms) | `modules` | 21 | 2,419 | None |  |
| [ ] | [admin](../../crates/modules/rustok-forms/admin) | `modules` | 6 | 582 | None |  |
| [ ] | [rustok-forum](../../crates/modules/rustok-forum) | `modules` | 614 | 181,014 | None |  |
| [ ] | [admin](../../crates/modules/rustok-forum/admin) | `modules` | 36 | 13,498 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-forum/storefront) | `modules` | 20 | 7,854 | None |  |
| [ ] | [rustok-fulfillment](../../crates/modules/rustok-fulfillment) | `modules` | 86 | 31,472 | None |  |
| [ ] | [admin](../../crates/modules/rustok-fulfillment/admin) | `modules` | 9 | 2,077 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-fulfillment/storefront) | `modules` | 13 | 1,730 | None |  |
| [ ] | [rustok-groups](../../crates/modules/rustok-groups) | `modules` | 117 | 34,525 | None |  |
| [ ] | [admin](../../crates/modules/rustok-groups/admin) | `modules` | 35 | 9,142 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-groups/storefront) | `modules` | 18 | 3,357 | None |  |
| [ ] | [rustok-iggy](../../crates/modules/rustok-iggy) | `modules` | 32 | 8,363 | None |  |
| [ ] | [rustok-iggy-connector](../../crates/modules/rustok-iggy-connector) | `modules` | 19 | 4,769 | None |  |
| [ ] | [admin](../../crates/modules/rustok-iggy-connector/admin) | `modules` | 9 | 705 | None |  |
| [ ] | [rustok-index](../../crates/modules/rustok-index) | `modules` | 142 | 60,081 | None |  |
| [ ] | [admin](../../crates/modules/rustok-index/admin) | `modules` | 10 | 2,989 | None |  |
| [ ] | [rustok-inventory](../../crates/modules/rustok-inventory) | `modules` | 66 | 22,218 | None |  |
| [ ] | [admin](../../crates/modules/rustok-inventory/admin) | `modules` | 11 | 4,908 | None |  |
| [ ] | [rustok-marketplace](../../crates/modules/rustok-marketplace) | `modules` | 12 | 2,144 | None |  |
| [ ] | [rustok-marketplace-allocation](../../crates/modules/rustok-marketplace-allocation) | `modules` | 14 | 1,797 | None |  |
| [ ] | [rustok-marketplace-balance](../../crates/modules/rustok-marketplace-balance) | `modules` | 31 | 7,110 | None |  |
| [ ] | [rustok-marketplace-commission](../../crates/modules/rustok-marketplace-commission) | `modules` | 18 | 3,101 | None |  |
| [ ] | [rustok-marketplace-listing](../../crates/modules/rustok-marketplace-listing) | `modules` | 34 | 6,905 | None |  |
| [ ] | [admin](../../crates/modules/rustok-marketplace-listing/admin) | `modules` | 10 | 2,010 | None |  |
| [ ] | [rustok-marketplace-payout](../../crates/modules/rustok-marketplace-payout) | `modules` | 15 | 2,051 | None |  |
| [ ] | [rustok-marketplace-seller](../../crates/modules/rustok-marketplace-seller) | `modules` | 51 | 11,075 | None |  |
| [ ] | [admin](../../crates/modules/rustok-marketplace-seller/admin) | `modules` | 14 | 2,508 | None |  |
| [ ] | [rustok-mcp](../../crates/modules/rustok-mcp) | `modules` | 27 | 7,106 | None |  |
| [ ] | [admin](../../crates/modules/rustok-mcp/admin) | `modules` | 8 | 1,970 | None |  |
| [ ] | [rustok-media](../../crates/modules/rustok-media) | `modules` | 48 | 14,287 | None |  |
| [ ] | [rustok-media-transport](../../crates/modules/rustok-media-transport) | `modules` | 7 | 2,393 | None |  |
| [ ] | [admin](../../crates/modules/rustok-media/admin) | `modules` | 11 | 2,445 | None |  |
| [ ] | [cli](../../crates/modules/rustok-media/cli) | `modules` | 2 | 422 | None |  |
| [ ] | [rustok-messenger](../../crates/modules/rustok-messenger) | `modules` | 64 | 11,408 | None |  |
| [ ] | [admin](../../crates/modules/rustok-messenger/admin) | `modules` | 10 | 1,226 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-messenger/storefront) | `modules` | 10 | 1,972 | None |  |
| [ ] | [rustok-moderation](../../crates/modules/rustok-moderation) | `modules` | 40 | 10,727 | None |  |
| [ ] | [rustok-modules](../../crates/modules/rustok-modules) | `modules` | 284 | 159,027 | None |  |
| [ ] | [cli](../../crates/modules/rustok-modules/cli) | `modules` | 2 | 1,905 | None |  |
| [ ] | [rustok-navigation](../../crates/modules/rustok-navigation) | `modules` | 43 | 5,894 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-navigation/storefront) | `modules` | 10 | 652 | None |  |
| [ ] | [rustok-newsletter](../../crates/modules/rustok-newsletter) | `modules` | 26 | 3,376 | None |  |
| [ ] | [rustok-notifications](../../crates/modules/rustok-notifications) | `modules` | 83 | 24,134 | None |  |
| [ ] | [admin](../../crates/modules/rustok-notifications/admin) | `modules` | 7 | 293 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-notifications/storefront) | `modules` | 17 | 3,300 | None |  |
| [ ] | [rustok-order](../../crates/modules/rustok-order) | `modules` | 90 | 23,881 | None |  |
| [ ] | [admin](../../crates/modules/rustok-order/admin) | `modules` | 15 | 3,206 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-order/storefront) | `modules` | 13 | 2,333 | None |  |
| [ ] | [rustok-outbox](../../crates/modules/rustok-outbox) | `modules` | 29 | 5,559 | None |  |
| [ ] | [admin](../../crates/modules/rustok-outbox/admin) | `modules` | 9 | 580 | None |  |
| [ ] | [rustok-page-builder](../../crates/modules/rustok-page-builder) | `modules` | 123 | 39,760 | None |  |
| [ ] | [admin](../../crates/modules/rustok-page-builder/admin) | `modules` | 67 | 22,326 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-page-builder/storefront) | `modules` | 7 | 1,846 | None |  |
| [ ] | [rustok-pages](../../crates/modules/rustok-pages) | `modules` | 205 | 60,760 | None |  |
| [ ] | [admin](../../crates/modules/rustok-pages/admin) | `modules` | 28 | 9,748 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-pages/storefront) | `modules` | 18 | 5,076 | None |  |
| [ ] | [rustok-party](../../crates/modules/rustok-party) | `modules` | 22 | 1,734 | None |  |
| [ ] | [admin](../../crates/modules/rustok-party/admin) | `modules` | 6 | 325 | None |  |
| [ ] | [rustok-payment](../../crates/modules/rustok-payment) | `modules` | 91 | 24,533 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-payment/storefront) | `modules` | 12 | 1,397 | None |  |
| [ ] | [rustok-pricing](../../crates/modules/rustok-pricing) | `modules` | 89 | 24,188 | None |  |
| [ ] | [admin](../../crates/modules/rustok-pricing/admin) | `modules` | 27 | 8,619 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-pricing/storefront) | `modules` | 12 | 2,777 | None |  |
| [ ] | [rustok-product](../../crates/modules/rustok-product) | `modules` | 240 | 102,317 | None |  |
| [ ] | [rustok-product-bundles](../../crates/modules/rustok-product-bundles) | `modules` | 28 | 7,122 | None |  |
| [ ] | [admin](../../crates/modules/rustok-product-bundles/admin) | `modules` | 10 | 2,961 | None |  |
| [ ] | [rustok-product-relations](../../crates/modules/rustok-product-relations) | `modules` | 30 | 4,491 | None |  |
| [ ] | [admin](../../crates/modules/rustok-product-relations/admin) | `modules` | 11 | 1,851 | None |  |
| [ ] | [ports](../../crates/modules/rustok-product/ports) | `modules` | 12 | 3,200 | None |  |
| [ ] | [admin](../../crates/modules/rustok-product/admin) | `modules` | 52 | 28,526 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-product/storefront) | `modules` | 16 | 5,584 | None |  |
| [ ] | [rustok-profiles](../../crates/modules/rustok-profiles) | `modules` | 59 | 9,706 | None |  |
| [ ] | [cli](../../crates/modules/rustok-profiles/cli) | `modules` | 2 | 484 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-profiles/storefront) | `modules` | 10 | 1,492 | None |  |
| [ ] | [rustok-rbac](../../crates/modules/rustok-rbac) | `modules` | 59 | 15,499 | None |  |
| [ ] | [admin](../../crates/modules/rustok-rbac/admin) | `modules` | 9 | 984 | None |  |
| [ ] | [cli](../../crates/modules/rustok-rbac/cli) | `modules` | 2 | 372 | None |  |
| [ ] | [rustok-reactions](../../crates/modules/rustok-reactions) | `modules` | 20 | 5,650 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-reactions/storefront) | `modules` | 8 | 1,061 | None |  |
| [ ] | [rustok-region](../../crates/modules/rustok-region) | `modules` | 54 | 9,829 | None |  |
| [ ] | [admin](../../crates/modules/rustok-region/admin) | `modules` | 9 | 3,095 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-region/storefront) | `modules` | 10 | 1,783 | None |  |
| [ ] | [rustok-search](../../crates/modules/rustok-search) | `modules` | 94 | 35,047 | None |  |
| [ ] | [admin](../../crates/modules/rustok-search/admin) | `modules` | 21 | 10,161 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-search/storefront) | `modules` | 10 | 3,267 | None |  |
| [ ] | [rustok-security](../../crates/modules/rustok-security) | `modules` | 33 | 3,028 | None |  |
| [ ] | [rustok-security-admin](../../crates/modules/rustok-security/admin) | `modules` | 6 | 322 | None |  |
| [ ] | [rustok-security-storefront](../../crates/modules/rustok-security/storefront) | `modules` | 6 | 126 | None |  |
| [ ] | [rustok-seo](../../crates/modules/rustok-seo) | `modules` | 79 | 36,932 | None |  |
| [ ] | [admin](../../crates/modules/rustok-seo/admin) | `modules` | 16 | 4,606 | None |  |
| [ ] | [panel](../../crates/modules/rustok-seo/panel) | `modules` | 7 | 2,834 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-seo/storefront) | `modules` | 7 | 979 | None |  |
| [ ] | [rustok-social-graph](../../crates/modules/rustok-social-graph) | `modules` | 40 | 9,812 | None |  |
| [ ] | [cli](../../crates/modules/rustok-social-graph/cli) | `modules` | 2 | 506 | None |  |
| [ ] | [rustok-tasks](../../crates/modules/rustok-tasks) | `modules` | 18 | 983 | None |  |
| [ ] | [admin](../../crates/modules/rustok-tasks/admin) | `modules` | 6 | 325 | None |  |
| [ ] | [rustok-tax](../../crates/modules/rustok-tax) | `modules` | 15 | 1,847 | None |  |
| [ ] | [rustok-taxonomy](../../crates/modules/rustok-taxonomy) | `modules` | 55 | 15,143 | None |  |
| [ ] | [rustok-tenant](../../crates/modules/rustok-tenant) | `modules` | 32 | 4,633 | None |  |
| [ ] | [admin](../../crates/modules/rustok-tenant/admin) | `modules` | 9 | 867 | None |  |
| [ ] | [rustok-translation](../../crates/modules/rustok-translation) | `modules` | 86 | 46,706 | None |  |
| [ ] | [admin](../../crates/modules/rustok-translation/admin) | `modules` | 10 | 14,268 | None |  |
| [ ] | [rustok-workflow](../../crates/modules/rustok-workflow) | `modules` | 55 | 7,817 | None |  |
| [ ] | [admin](../../crates/modules/rustok-workflow/admin) | `modules` | 15 | 1,533 | None |  |
| [ ] | [fly](../../crates/ui/fly) | `ui` | 117 | 45,535 | None |  |
| [ ] | [browser](../../crates/ui/fly/browser) | `ui` | 13 | 1,707 | None |  |
| [ ] | [dioxus](../../crates/ui/fly/dioxus) | `ui` | 3 | 739 | None |  |
| [ ] | [leptos](../../crates/ui/fly/leptos) | `ui` | 6 | 2,965 | None |  |
| [ ] | [ui](../../crates/ui/fly/ui) | `ui` | 23 | 6,426 | None |  |
| [ ] | [web](../../crates/ui/fly/web) | `ui` | 7 | 2,307 | None |  |
| [ ] | [leptos-auth](../../crates/ui/leptos-auth) | `ui` | 11 | 1,984 | None |  |
| [ ] | [leptos-ui](../../crates/ui/leptos-ui) | `ui` | 6 | 504 | None |  |
| [ ] | [leptos-ui-routing](../../crates/ui/leptos-ui-routing) | `ui` | 2 | 383 | None |  |
| [ ] | [rustok-graphql](../../crates/ui/rustok-graphql) | `ui` | 2 | 528 | None |  |
| [ ] | [rustok-grid](../../crates/ui/rustok-grid) | `ui` | 31 | 7,285 | None |  |
| [ ] | [rustok-grid-dioxus](../../crates/ui/rustok-grid/dioxus) | `ui` | 9 | 1,798 | None |  |
| [ ] | [rustok-grid-leptos](../../crates/ui/rustok-grid/leptos) | `ui` | 9 | 1,587 | None |  |
| [ ] | [rustok-ui](../../crates/ui/rustok-ui) | `ui` | 49 | 5,501 | None |  |
| [ ] | [rustok-ui-auth](../../crates/ui/rustok-ui-auth) | `ui` | 2 | 180 | None |  |
| [ ] | [rustok-ui-core](../../crates/ui/rustok-ui-core) | `ui` | 10 | 2,476 | None |  |
| [ ] | [rustok-ui-forms](../../crates/ui/rustok-ui-forms) | `ui` | 24 | 9,549 | None |  |
| [ ] | [rustok-forms-dioxus](../../crates/ui/rustok-ui-forms/dioxus) | `ui` | 7 | 2,296 | None |  |
| [ ] | [rustok-forms-leptos](../../crates/ui/rustok-ui-forms/leptos) | `ui` | 7 | 3,207 | None |  |
| [ ] | [rustok-ui-i18n](../../crates/ui/rustok-ui-i18n) | `ui` | 33 | 6,832 | None |  |
| [ ] | [rustok-ui-transport](../../crates/ui/rustok-ui-transport) | `ui` | 2 | 598 | None |  |
| [ ] | [rustok-ui-dioxus](../../crates/ui/rustok-ui/dioxus) | `ui` | 21 | 1,373 | None |  |
| [ ] | [rustok-ui-leptos](../../crates/ui/rustok-ui/leptos) | `ui` | 21 | 2,161 | None |  |
| [ ] | [rustok-build](../../crates/utils/rustok-build) | `utils` | 14 | 3,158 | None |  |
| [ ] | [rustok-build-publication](../../crates/utils/rustok-build-publication) | `utils` | 5 | 1,006 | None |  |
| [ ] | [rustok-build-source](../../crates/utils/rustok-build-source) | `utils` | 3 | 1,882 | None |  |
| [ ] | [rustok-cli](../../crates/utils/rustok-cli) | `utils` | 4 | 1,548 | None |  |
| [ ] | [rustok-cli-core](../../crates/utils/rustok-cli-core) | `utils` | 2 | 288 | None |  |
| [ ] | [rustok-cli-platform](../../crates/utils/rustok-cli-platform) | `utils` | 4 | 602 | None |  |
| [ ] | [rustok-cli-registry](../../crates/utils/rustok-cli-registry) | `utils` | 3 | 244 | None |  |
| [ ] | [rustok-doctor](../../crates/utils/rustok-doctor) | `utils` | 10 | 2,543 | None |  |
| [ ] | [rustok-installer](../../crates/utils/rustok-installer) | `utils` | 12 | 5,264 | None |  |
| [ ] | [rustok-installer-cli](../../crates/utils/rustok-installer-cli) | `utils` | 2 | 984 | None |  |
| [ ] | [rustok-installer-persistence](../../crates/utils/rustok-installer-persistence) | `utils` | 10 | 2,267 | None |  |
| [ ] | [rustok-migrations](../../crates/utils/rustok-migrations) | `utils` | 66 | 15,995 | None |  |
| [ ] | [rustok-module-sdk](../../crates/utils/rustok-module-sdk) | `utils` | 3 | 124 | None |  |
| [ ] | [rustok-module-template](../../crates/utils/rustok-module-template) | `utils` | 3 | 436 | None |  |
| [ ] | [rustok-secrets](../../crates/utils/rustok-secrets) | `utils` | 4 | 1,697 | None |  |
| [ ] | [rustok-starter](../../crates/utils/rustok-starter) | `utils` | 12 | 1,482 | None |  |
| [ ] | [rustok-storage](../../crates/utils/rustok-storage) | `utils` | 5 | 1,034 | None |  |
| [ ] | [rustok-test-utils](../../crates/utils/rustok-test-utils) | `utils` | 7 | 1,952 | None |  |
| [ ] | [serde-columnar-vendored](../../crates/utils/serde-columnar-vendored) | `utils` | 17 | 2,550 | None |  |
| [ ] | [utoipa-swagger-ui-vendored](../../crates/utils/utoipa-swagger-ui-vendored) | `utils` | 3 | 62 | None |  |
| [ ] | [rustok-artifact-node-agent](../../crates/workers/rustok-artifact-node-agent) | `workers` | 9 | 2,557 | None |  |
| [ ] | [rustok-artifact-node-controller](../../crates/workers/rustok-artifact-node-controller) | `workers` | 4 | 204 | None |  |
| [ ] | [rustok-artifact-node-reconciler](../../crates/workers/rustok-artifact-node-reconciler) | `workers` | 4 | 227 | None |  |
| [ ] | [rustok-artifact-node-transport](../../crates/workers/rustok-artifact-node-transport) | `workers` | 7 | 1,120 | None |  |
| [ ] | [rustok-module-build-dispatcher](../../crates/workers/rustok-module-build-dispatcher) | `workers` | 4 | 626 | None |  |
| [ ] | [rustok-module-build-transport](../../crates/workers/rustok-module-build-transport) | `workers` | 7 | 509 | None |  |
| [ ] | [rustok-module-build-worker](../../crates/workers/rustok-module-build-worker) | `workers` | 9 | 4,297 | None |  |
| [ ] | [rustok-registry-validation-worker](../../crates/workers/rustok-registry-validation-worker) | `workers` | 3 | 1,082 | None |  |
| [ ] | [rustok-sandbox](../../crates/workers/rustok-sandbox) | `workers` | 28 | 8,426 | None |  |
| [ ] | [rustok-sandbox-transport](../../crates/workers/rustok-sandbox-transport) | `workers` | 6 | 1,470 | None |  |
| [ ] | [rustok-sandbox-worker](../../crates/workers/rustok-sandbox-worker) | `workers` | 4 | 980 | None |  |
| [ ] | [rustok-static-distribution-worker](../../crates/workers/rustok-static-distribution-worker) | `workers` | 11 | 3,692 | None |  |
| [ ] | [rustok-verification-transport](../../crates/workers/rustok-verification-transport) | `workers` | 5 | 408 | None |  |
| [ ] | [rustok-verification-worker](../../crates/workers/rustok-verification-worker) | `workers` | 6 | 1,365 | None |  |
| [ ] | [rustok-worker-transport](../../crates/workers/rustok-worker-transport) | `workers` | 2 | 838 | None |  |

---

## Completed Rounds Archive
- **Round 1**: Completed on `2026-11-02T20:53:36Z` (285 components, 2,458,974 LOC).
