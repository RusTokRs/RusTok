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
**Active phase:** FS-03 — Commerce & Business Domain Modules (`rustok-commerce`, `rustok-cart`, `rustok-order`, `rustok-payment`, `rustok-pricing`, `rustok-inventory`, `rustok-fulfillment`)  
**Current main SHA:** `c6cb2191e`
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

- [x] **Phase FS-01 — Entrypoint & Composition Root (`apps/server`)**
- [x] **Phase FS-02 — Core Identity, Auth & Tenancy Modules (`rustok-auth`, `rustok-rbac`, `rustok-tenant`, `rustok-cache`)**
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
- [x] **FS-01.15 — `apps/server/src/controllers/health.rs`** — liveness, readiness, DB/Redis ping probes, status reporting. (Done: aligned /health/ready response to return HTTP 503 Service Unavailable directly when aggregate status is Unhealthy; resolved module health status into distinct tri-state ok/degraded/unhealthy; added warning log when circuit breaker trips open; added entry debug tracing for health endpoints; added 2 unit tests covering readiness HTTP status mapping and module health tri-state resolution).
- [x] **FS-01.16 — `apps/server/src/controllers/metrics.rs`** — Prometheus metric exposition, scrape latency, sanitization. (Done: added entry debug tracing to metrics endpoint; guaranteed canonical Content-Type header on 503 metrics-disabled responses; added unit test verifying disabled metrics status code and text/plain content-type).
- [x] **FS-01.17 — `apps/server/src/controllers/auth.rs`** — login, register, refresh token, password reset, session invalidation. (Done: trimmed email inputs across register, login, password reset request, and verification request endpoints to prevent whitespace-induced authentication failures; added structured entry debug tracing across all auth endpoints; added unit test verifying session limit clamping bounds).
- [x] **FS-01.18 — `apps/server/src/controllers/oauth.rs` & `apps/server/src/controllers/oauth_metadata.rs`** — OAuth2/OIDC provider & client endpoints, PKCE, token grants. (Done: enforced RFC 6749 §5.2 Cache-Control: no-store and Pragma: no-cache headers on OAuth error responses in oauth_error_response; added structured entry debug tracing across all token, authorize, authorize_browser, consent, revoke, and userinfo handlers in oauth.rs and metadata handler in oauth_metadata.rs; added unit test verifying RFC 6749 error caching headers).
- [x] **FS-01.19 — `apps/server/src/controllers/users.rs`** — user profile management, admin user CRUD, tenant isolation. (Done: trimmed and filtered search and status query parameters to prevent wasteful scans; extracted pure normalize_page_params with clamping; added entry debug tracing to list_users and get_user; added 3 unit tests covering pagination defaults, bounds clamping, and user model projection).
- [x] **FS-01.20 — `apps/server/src/controllers/graphql.rs` & `apps/server/src/services/graphql_schema.rs`** — GraphQL endpoint, query/mutation execution, dataloader, subscriptions. (Done: added entry debug tracing across graphql_handler, graphql_playground, graphql_schema_sdl, and graphql_ws_handler; added lifecycle debug tracing to init_graphql_schema; fixed self-referential string literal false-positive match in websocket_tenant_resolution_does_not_log_raw_slug_payload test).
- [x] **FS-01.21 — `apps/server/src/controllers/mcp.rs` & `apps/server/src/services/mcp_*.rs`** — MCP transport, tool call routing, token auth, error masking. (Done: enforced RFC 9110 fail-closed duplicate Authorization header checks and quotes stripping on bearer tokens in bearer_token_from_headers; added structured entry debug tracing across bootstrap, tool call, stream, clients, tokens, policies, and scaffold endpoints; added unit tests for quote stripping and duplicate header rejection).
- [x] **FS-01.22 — `apps/server/src/controllers/flex.rs` & `apps/server/src/services/flex_*.rs`** — Flex schema REST endpoints, dynamic fields validation. (Done: added structured entry debug tracing across all 10 schema and entry CRUD REST endpoints; added unit test verifying map_flex_rest_error mapping to stable HTTP status kinds).
- [x] **FS-01.23 — `apps/server/src/controllers/installer.rs`** — first-boot installer controller, setup token, DB provisioning. (Done: enforced RFC 9110 fail-closed duplicate header checks on x-rustok-setup-token and Authorization headers; trimmed quotes and whitespace from setup token; added constant-time validation with ct_eq via validate_setup_token_value; added structured entry debug tracing across status, plan, preflight, apply, job_status, and receipts endpoints; added 4 unit tests covering token extraction, quote stripping, duplicate header rejection, and constant-time validation).
- [x] **FS-01.24 — `apps/server/src/controllers/marketplace_registry.rs`** — package publication, search, downloads. (Done: fixed failing vary cache header test to include X-Tenant-Slug; enforced RFC 9110 fail-closed duplicate header checks and quote stripping in require_remote_executor_access and required_non_nil_idempotency_key; added structured entry debug tracing across all 22 registry handlers; added 2 unit tests covering idempotency key header validation and registry slug format checks).
- [x] **FS-01.25 — `apps/server/src/controllers/artifact_http.rs` & `artifact_permissions.rs`** — WASM artifact serving, permissions check. (Done: enforced RFC 9110 fail-closed duplicate header checks on Idempotency-Key in header_idempotency_key and added quote trimming; sanitized and validated permission_key and idempotency_key in assign; added structured entry debug tracing across all artifact dispatch, UI contribution, and permission grant/revoke handlers; added unit tests for duplicate idempotency headers, quoted UUIDs, and assignment error status mappings).
- [x] **FS-01.26 — `apps/server/src/controllers/admin_events.rs`** — outbox/event inspection, DLQ management, retry dispatch. (Done: trimmed and filtered event_type query parameter in list_dlq to eliminate empty and padded queries; added structured entry debug tracing to list_dlq and replay_dlq_event; added unit tests covering default query deserialization, event_type trimming, and non-existent event requeue rejection).
- [x] **FS-01.27 — `apps/server/src/controllers/channel.rs`** — channel admin endpoints, CRUD, tenant scoping. (Done: trimmed and validated input strings across channel, target, module, and policy set mutations rejecting empty/whitespace values; enforced tenant ownership verification for OAuth app bindings in create/update resolution rules via ensure_oauth_app_belongs_to_tenant; added structured entry debug tracing across all 16 channel and policy handlers; added 3 unit tests covering invalid policy definition error mapping, serialization error mapping, and input trimming invariants).
- [x] **FS-01.28 — `apps/server/src/controllers/swagger.rs`** — OpenAPI/Swagger UI endpoints, schema generation safety. (Done: added structured entry debug tracing to openapi_json and openapi_yaml endpoints; fixed conflicting test assertion in openapi_marks_marketplace_registry_auth_boundaries by aligning anonymous publish status inspection; corrected method assertions in openapi_documents_expected_methods_on_shared_paths to PATCH for flex schemas and entries matching controller route definitions; verified all 12 unit tests pass).

#### 4. Event Bus, Outbox & Background Services in Server
- [x] **FS-01.29 — `apps/server/src/services/event_bus.rs` & `module_event_dispatcher.rs`** — host event bus composition, module dispatch. (Done: added structured entry debug tracing across event_bus_from_context, transactional_event_bus_from_context, build_event_bus, spawn_module_event_dispatcher, and build_module_event_dispatcher; removed obsolete option resource_kind translation target assertion; aligned test module registry composition for fail-closed reactions and moderation feature flags; added 3 unit tests in event_bus covering SharedEventBus clone reference preservation, idempotent context event bus resolution, and transactional event bus initialization).
- [x] **FS-01.30 — `apps/server/src/services/outbox_retention_worker.rs`** — outbox cleanup worker, batch retention pruning. (Done: added structured entry debug tracing to spawn_outbox_retention_worker; added fast catch-up yielding min(50ms, interval) when batch is exhausted to drain backlogged expired events without waiting full interval; added unit test for OutboxRetentionSupervisorMetricsSnapshot default and equality invariants; verified all 3 unit tests pass).
- [x] **FS-01.31 — `apps/server/src/services/module_transition_watchdog.rs`** — module state machine watchdog, timeout recovery. (Done: exposed abort handle on ModuleTransitionWatchdogHandle; configured rapid minimum polling interval during testing; added structured debug tracing for transition watchdog evaluation checkpoints; added 3 unit tests verifying instance ID monotonicity, abort cancellation, and graceful shutdown signal termination; verified gatekeeper approval).
- [x] **FS-01.32 — `apps/server/src/services/settings_service.rs`** — static & dynamic tenant settings loading and caching. (Done: fixed failing cache port validation test with explicit port and db integer bounds in CacheSettingsValidator; hardened EmailSettingsValidator and RateLimitSettingsValidator with strict type and finiteness checks; trimmed input categories; added structured debug tracing across get, get_all, and update; enforced JSON object payload invariant on settings mutations; redacted secrets from YAML configuration fallbacks in get and get_all; sanitized redis_url passwords in cache settings; added 5 unit tests covering category trimming, bounds checks, type checks, and redis_url password redaction; verified gatekeeper approval).
- [x] **FS-01.33 — `apps/server/src/services/rbac_*.rs`** — server-level RBAC caching, runtime invalidation. (Done: added structured entry debug tracing across publish_user_rbac_invalidation, publish_all_rbac_invalidation, start_rbac_cache_invalidation_listener, invalidate_user_permissions_cache, invalidate_user_rbac_caches, get_user_role, get_user_ids_for_role, and replace_user_role; added whitespace trimming for invalidation keys and target resolution; added unit test verifying padded invalidation keys; verified all 43 RBAC tests pass and gatekeeper approval).

---

### Active Phase: FS-02 — Core Identity, Auth & Tenancy Modules Queue

#### 1. rustok-tenant
- [x] **FS-02.01 — `crates/modules/rustok-tenant/src/services/tenant_service.rs` & `error.rs`** — tenant lifecycle, provisioning, slug/domain resolution, locale policy aggregate, input sanitization. (Done: added Validation variant to TenantError and mapped to PortError::validation; trimmed and validated slug and name inputs rejecting empty/whitespace in create_tenant, ensure_tenant, and update_tenant; fast-pathed empty slug and domain lookups to NotFound; fast-pathed empty module_slug to Ok(None); clamped pagination parameters in list_tenants; added structured entry debug tracing across all tenant service methods; added integration test tenant_service_input_trimming_and_validation; all 21 tests passed; verified gatekeeper approval).
- [x] **FS-02.02 — `crates/modules/rustok-tenant/src/entities/*` & `ports.rs`** — tenant entities, locale policies, receipts, port adapters. (Done: enforced nil UUID validation in TenantReadSelector::Id; enforced non-empty validation in parse_context_tenant_id; trimmed input in tenant ActiveModel::new; added structured entry debug tracing in TenantReadPort and TenantLocalePolicyPort; added integration tests for nil ID and empty context tenant ID; verified all 21 tests pass and gatekeeper approval).
- [x] **FS-02.03 — `crates/modules/rustok-tenant/src/settings_schema.rs` & `contract_tests.rs`** — tenant settings schema, validation rules, contract tests. (Done: bounded array elements to maximum 256; rejected null bytes in object keys and string values to prevent PostgreSQL UTF-8 null byte corruption; enforced float finiteness on numbers with match guard; added unit tests for null byte key, null byte string value, and oversized array; all 24 tests passed; verified gatekeeper approval).

#### 2. rustok-auth
- [x] **FS-02.04 — `crates/modules/rustok-auth/src/credentials.rs`, `jwt.rs`, & `error.rs`** — credential hashing, argon2/sha256 entropy, JWT encode/decode, purpose validation, error taxonomy. (Done: enforced nil UUID validation in access token encoder, OAuth token encoder, and access token decoder; trimmed whitespace from token strings in all 4 decoders; added empty/nil email and tenant ID guards in password reset, verification, and invite token encoders/decoders; handled malformed password hashes safely in verify_password returning Ok(false) to prevent oracle leaks; added structured entry debug tracing across all credential hashing/verification and JWT encoding/decoding functions; added 5 unit tests covering nil UUID rejection, token whitespace trimming, empty token rejection, and malformed hash handling; verified all 53 unit tests pass and gatekeeper approval).
- [x] **FS-02.05 — `crates/modules/rustok-auth/src/config.rs` & `bootstrap.rs`** — auth configuration, key pair validation, secret redaction, bootstrap writer. (Done: enforced non-whitespace byte length check on HS256 secret; added structured entry debug tracing across validate_auth_config, ensure_user_on, and find_user_on; added whitespace trimming to normalize_bootstrap_email; enforced non-nil tenant_id, non-empty email, password, and name checks in ensure_user_on; fast-pathed nil tenant_id and empty email in find_user_on to Ok(None); added unit test bootstrap_user_on_rejects_nil_tenant_and_empty_fields; verified all 54 unit tests pass and gatekeeper approval).
- [x] **FS-02.06 — `crates/modules/rustok-auth/src/admin_mutations.rs`, `lifecycle.rs`, & `rest.rs`** — admin user/OAuth mutations, user lifecycle ports, REST DTOs, and backfill. (Done: implemented custom Debug for CreateUserCommand, UpdateUserCommand, AuthTokenRecord, ResetRequestResponse, VerificationRequestResponse, AuthResponse, TokenRequest, TokenResponse, and RevokeRequest redacting passwords, tokens, authorization codes, and client secrets from log formats; added nil tenant_id guard and structured debug tracing in AuthUserBackfillDbReader; added 7 unit tests verifying debug secret redaction and backfill validation invariants; verified all 62 unit tests pass and gatekeeper approval).

#### 3. rustok-rbac
- [x] **FS-02.07 — `crates/modules/rustok-rbac/src/services/rbac_service.rs` & `error.rs`** — RBAC domain service, role assignment, policy enforcement. (Done: enriched RbacError taxonomy with domain errors and policy error conversion; fast-pathed nil tenant_id and user_id UUIDs to empty permissions across resolve_permissions_from_relations, resolve_permissions_with_cache, and invalidate_cached_permissions; guarded load_role_user_ids_on against nil tenant_id; added structured entry debug tracing across authorize_permission, authorize_any_permission, authorize_all_permissions, load_role_user_ids_on, resolve_permissions_from_relations, resolve_permissions_with_cache, and invalidate_cached_permissions; added unit tests for error conversions and nil UUID handling; verified all 79 lib tests pass and gatekeeper approval).
- [x] **FS-02.08 — `crates/modules/rustok-rbac/src/entities/*` & `ports.rs`** — RBAC entities, permissions matrix, ports. (Done: enforced non-nil UUID validation on tenant_id and actor user_id in parse_tenant_id and parse_user_actor; bounded maximum permissions count to 100 per check request; guarded RbacRoleAssignmentDbWriter methods against nil tenant_id and user_id; added structured debug tracing across check_permissions, assign_role_permissions_on, assign_role_on, replace_role_on, and remove_tenant_role_assignments_on; added 3 unit tests covering nil UUID and oversized permissions rejection; verified all 82 lib tests pass and gatekeeper approval).

#### 4. rustok-cache
- [x] **FS-02.09 — `crates/modules/rustok-cache/src/services/cache_service.rs` & `error.rs`** — distributed caching, Redis/memory backends, key hashing. (Done: implemented unified CacheError domain taxonomy in error.rs with manual Display, Error, and From conversions; implemented Display and Error on CacheInvalidationMessageError; added structured debug tracing across load_or_fill, publish_invalidation, and backend_shared_client_with_options; added unit tests for CacheError; verified all 180 unit tests pass and gatekeeper approval).
- [x] **FS-02.10 — `crates/modules/rustok-cache/src/ports.rs` & `invalidation.rs`** — cache ports, invalidation listener, stampede protection. (Done: added structured entry debug tracing across publish_durable in durable_invalidation_service.rs, load_or_fill_with_policy in policy.rs, DurableCacheInvalidationConsumer::process in invalidation_processor.rs, and VersionedCacheInvalidation new and from_message in invalidation.rs; verified all 180 unit tests pass and gatekeeper approval).

---

### Active Phase: FS-03 — Commerce & Business Domain Modules Queue

#### 1. rustok-commerce-foundation
- [x] **FS-03.01 — `crates/modules/rustok-commerce-foundation`** — shared error taxonomy, DTOs, search contracts. (Done: implemented From<CommerceError> for CoreError with mapping to NotFound, Validation, and Forbidden; added validate_invariants on PriceInput and AdjustInventoryInput guarding against negative prices and nil UUIDs; trimmed search patterns in product_translation_title_search_condition avoiding redundant full EXISTS subqueries on empty/whitespace searches; added 5 unit tests; verified all 8 tests pass and gatekeeper approval).

#### 2. rustok-commerce core domain
- [x] **FS-03.02 — `crates/modules/rustok-commerce/src/services/checkout.rs` & `checkout_operation.rs`** — checkout pipeline, atomic state transitions, idempotent execution. (Done: enforced non-nil UUID validation on tenant_id, actor_id, and cart_id in complete_checkout; enforced non-nil UUID validation in checkpoint, ensure_cart_tenant, ensure_order_tenant, ensure_payment_collection_tenant, and normalize_begin_input; added structured entry debug tracing across complete_checkout, begin, and checkpoint; added unit tests verifying nil UUID rejection in begin checkout operation; verified unit tests pass and gatekeeper approval).
- [x] **FS-03.03 — `crates/modules/rustok-commerce/src/services/checkout_stage_pipeline*.rs` & `checkout_plan_builder.rs`** — stage execution, owner ports, plan building. (Done: enforced non-nil UUID validation on tenant_id and operation_id in advance_to_completed across standard and owner-ports stage pipelines; permitted guest nil actor_id in complete_checkout; enforced non-nil UUID validation and entry debug tracing in CheckoutPlanBuilder::build; verified all 186 unit tests pass).
- [x] **FS-03.04 — `crates/modules/rustok-commerce/src/services/checkout_inventory_*.rs` & `checkout_payment_*.rs`** — inventory reservation journal/adoption, payment stages. (Done: enforced non-nil UUID validation across inventory order adoption, reservation executor, reservation journal, and payment stages; added structured entry debug tracing across all inventory reservation and payment stage transitions; added unit test plan_rejects_nil_uuids; verified all 187 unit tests pass).
- [x] **FS-03.05 — `crates/modules/rustok-commerce/src/services/checkout_compensation*.rs`** — multi-stage compensation, failure sweeps, error-safe rollback. (Done: enforced non-nil UUID validation in compensation and owner-ports services; added debug! macro to error-safe tracing shim; added structured entry debug tracing across compensate and sweep execution; added unit tests for report defaults and safe error code mappings; verified all 189 unit tests pass).
- [x] **FS-03.06 — `crates/modules/rustok-commerce/src/services/marketplace_*.rs`** — marketplace economics, allocations, financial reconciliation, reversal inboxes. (Done: enforced non-nil UUID validation across marketplace paid and reversal event inboxes, financial and reversal operators, provider event adapters, and event handlers; added structured entry debug tracing across all marketplace services; added unit tests for deterministic hashes, nil UUID identity rejection, retry backoff bounds, and currency validation; verified all 201 unit tests pass and gatekeeper approval).
- [x] **FS-03.07 — `crates/modules/rustok-commerce/src/services/fulfillment_*.rs` & `return_*.rs`** — fulfillment orchestration, label recovery, returns lifecycle. (Done: enforced non-nil UUID validation across fulfillment create-label recovery, orchestration, facade, reconciliation, return completion operations, return completion orchestration, recovery facade, and return decision services; added structured entry debug tracing across all fulfillment and return lifecycle handlers; added 9 unit tests covering nil UUID rejection and delivery invariants; verified all 198 default and 210 marketplace-financial unit tests pass and gatekeeper approval).
- [x] **FS-03.08 — `crates/modules/rustok-commerce/src/controllers/*` & `graphql*`** — commerce REST/GraphQL transport adapters, input validation, permissions. (Done: enforced non-nil UUID validation for tenant, actor, and resource IDs across all commerce admin and storefront REST controllers and GraphQL helper scopes; added structured entry debug tracing across admin checkout operations, orders owner ports, returns, changes, fulfillments, payments, post-order commands and reads, products, shipping, and storefront checkout, carts, orders, and products; hardened GraphQL current_tenant_scope, require_commerce_permission, product_mutation_actor, and product_query_tenant against nil IDs; added unit tests in common and GraphQL; verified all 202 default and 214 marketplace-financial tests pass and gatekeeper approval).

#### 3. rustok-cart
- [x] **FS-03.09 — `crates/modules/rustok-cart`** — cart lifecycle, item mutation validation, pricing integration, currency/tenant scoping. (Done: enforced non-nil UUID validation for tenant_id, cart_id, line_item_id, channel_id, customer_id, region_id, and shipping_option_id across ports, checkout_snapshot, marketplace_snapshot, atomic_checkout_guard, atomic_checkout_port, owner_ports, promotion_guard, guarded_ports, helpers, and cart service; added structured entry debug tracing across all cart port boundaries and mutation services; added unit tests for UUID validations in ports and checkout_snapshot; verified all 32 lib tests pass).

#### 4. rustok-order
- [ ] **FS-03.10 — `crates/modules/rustok-order`** — order state machine, transitions, items validation, event outbox emission.

#### 5. rustok-payment
- [ ] **FS-03.11 — `crates/modules/rustok-payment`** — payment providers, transaction lifecycle, idempotency receipts, webhook verification.

#### 6. rustok-pricing
- [ ] **FS-03.12 — `crates/modules/rustok-pricing` & `rustok-pricing-persistence`** — price resolution rules, currency arithmetic, discount evaluation.

#### 7. rustok-inventory
- [ ] **FS-03.13 — `crates/modules/rustok-inventory`** — stock allocation, reservation leases, concurrency guards, backorder policies.

#### 8. rustok-fulfillment
- [ ] **FS-03.14 — `crates/modules/rustok-fulfillment` & `rustok-distribution`** — shipping carriers, tracking, package manifests, split fulfillment.

---

## Continuous Code Review & Remediation Ledger (ACRE)

Tracking persistent progress across cyclical review rounds for all modules in RusToK.

## Current Cycle Status
- **Active Round:** Round 1
- **Cycle Started:** `2026-10-13T20:15:00Z`
- **Progress:** `8 / 254` components audited (**3.1%**)
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
| [x] | [server](../../apps/server) | `apps` | 484 | 187,425 | 2026-10-14 | Completed all 33 audit tracks in Phase FS-01 |
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
| [x] | [rustok-auth](../../crates/modules/rustok-auth) | `modules` | 50 | 12,368 | 2026-10-15 | Completed all 3 audit tracks in Phase FS-02 (credentials/JWT, config/bootstrap, admin mutations & rest) |
| [ ] | [admin](../../crates/modules/rustok-auth/admin) | `modules` | 18 | 6,501 | None |  |
| [ ] | [cli](../../crates/modules/rustok-auth/cli) | `modules` | 1 | 333 | None |  |
| [ ] | [rustok-blog](../../crates/modules/rustok-blog) | `modules` | 148 | 35,732 | None |  |
| [ ] | [admin](../../crates/modules/rustok-blog/admin) | `modules` | 16 | 5,248 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-blog/storefront) | `modules` | 14 | 3,289 | None |  |
| [ ] | [rustok-brand](../../crates/modules/rustok-brand) | `modules` | 22 | 3,399 | None |  |
| [ ] | [admin](../../crates/modules/rustok-brand/admin) | `modules` | 9 | 2,081 | None |  |
| [x] | [rustok-cache](../../crates/modules/rustok-cache) | `modules` | 45 | 15,736 | 2026-10-15 | Completed all 2 audit tracks in Phase FS-02 (cache service/error taxonomy, invalidation/stampede protection) |
| [ ] | [admin](../../crates/modules/rustok-cache/admin) | `modules` | 7 | 926 | None |  |
| [x] | [rustok-cart](../../crates/modules/rustok-cart) | `modules` | 73 | 18,625 | 2026-10-15 | Completed cart lifecycle, port boundaries nil UUID guards, entry tracing & validation tests in Phase FS-03.09 |
| [ ] | [storefront](../../crates/modules/rustok-cart/storefront) | `modules` | 18 | 4,545 | None |  |
| [ ] | [rustok-channel](../../crates/modules/rustok-channel) | `modules` | 47 | 11,368 | None |  |
| [ ] | [admin](../../crates/modules/rustok-channel/admin) | `modules` | 14 | 5,173 | None |  |
| [ ] | [rustok-comments](../../crates/modules/rustok-comments) | `modules` | 39 | 9,622 | None |  |
| [ ] | [rustok-comments-api](../../crates/modules/rustok-comments-api) | `modules` | 3 | 154 | None |  |
| [ ] | [rustok-comments-storefront-support](../../crates/modules/rustok-comments-storefront-support) | `modules` | 3 | 171 | None |  |
| [ ] | [admin](../../crates/modules/rustok-comments/admin) | `modules` | 8 | 1,382 | None |  |
| [x] | [rustok-commerce](../../crates/modules/rustok-commerce) | `modules` | 297 | 129,394 | 2026-10-15 | Completed audit tracks FS-03.02 - FS-03.08 (checkout pipeline, stage pipeline, inventory/payment, compensation, marketplace, fulfillment, controllers/graphql) |
| [x] | [rustok-commerce-foundation](../../crates/modules/rustok-commerce-foundation) | `modules` | 19 | 929 | 2026-10-15 | Completed audit track FS-03.01 (error taxonomy, DTO invariant validation, search condition trimming) |
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
| [ ] | [rustok-index](../../crates/modules/rustok-index) | `modules` | 137 | 58,160 | None |  |
| [ ] | [admin](../../crates/modules/rustok-index/admin) | `modules` | 9 | 2,912 | None |  |
| [ ] | [rustok-inventory](../../crates/modules/rustok-inventory) | `modules` | 35 | 12,536 | None |  |
| [ ] | [admin](../../crates/modules/rustok-inventory/admin) | `modules` | 10 | 4,750 | None |  |
| [ ] | [rustok-marketplace](../../crates/modules/rustok-marketplace) | `modules` | 10 | 1,485 | None |  |
| [ ] | [rustok-marketplace-allocation](../../crates/modules/rustok-marketplace-allocation) | `modules` | 12 | 1,486 | None |  |
| [ ] | [rustok-marketplace-balance](../../crates/modules/rustok-marketplace-balance) | `modules` | 29 | 6,779 | None |  |
| [ ] | [rustok-marketplace-commission](../../crates/modules/rustok-marketplace-commission) | `modules` | 16 | 2,662 | None |  |
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
| [ ] | [admin](../../crates/modules/rustok-page-builder/admin) | `modules` | 74 | 20,424 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-page-builder/storefront) | `modules` | 3 | 1,200 | None |  |
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
| [x] | [rustok-rbac](../../crates/modules/rustok-rbac) | `modules` | 56 | 13,098 | 2026-10-15 | Completed all 2 audit tracks in Phase FS-02 (rbac service/error taxonomy, entities/ports & bootstrap) |
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
| [ ] | [rustok-seo-targets](../../crates/modules/rustok-seo-targets) | `modules` | 1 | 1,428 | None |  |
| [ ] | [admin](../../crates/modules/rustok-seo/admin) | `modules` | 15 | 4,557 | None |  |
| [ ] | [panel](../../crates/modules/rustok-seo/panel) | `modules` | 6 | 2,754 | None |  |
| [ ] | [render](../../crates/modules/rustok-seo/render) | `modules` | 1 | 17 | None |  |
| [ ] | [storefront](../../crates/modules/rustok-seo/storefront) | `modules` | 6 | 919 | None |  |
| [ ] | [rustok-social-graph](../../crates/modules/rustok-social-graph) | `modules` | 36 | 8,796 | None |  |
| [ ] | [rustok-social-graph-cli](../../crates/modules/rustok-social-graph-cli) | `modules` | 1 | 324 | None |  |
| [ ] | [rustok-tax](../../crates/modules/rustok-tax) | `modules` | 5 | 1,574 | None |  |
| [ ] | [rustok-taxonomy](../../crates/modules/rustok-taxonomy) | `modules` | 53 | 14,828 | None |  |
| [x] | [rustok-tenant](../../crates/modules/rustok-tenant) | `modules` | 28 | 4,005 | 2026-10-14 | Completed all 3 audit tracks in Phase FS-02 (tenant service, ports/entities, settings schema) |
| [ ] | [admin](../../crates/modules/rustok-tenant/admin) | `modules` | 8 | 835 | None |  |
| [ ] | [rustok-translation](../../crates/modules/rustok-translation) | `modules` | 81 | 45,996 | None |  |
| [ ] | [rustok-translation-targets](../../crates/modules/rustok-translation-targets) | `modules` | 3 | 1,729 | None |  |
| [ ] | [admin](../../crates/modules/rustok-translation/admin) | `modules` | 9 | 14,236 | None |  |
| [ ] | [rustok-workflow](../../crates/modules/rustok-workflow) | `modules` | 52 | 7,300 | None |  |
| [ ] | [admin](../../crates/modules/rustok-workflow/admin) | `modules` | 14 | 1,434 | None |  |
| [ ] | [fly](../../crates/ui/fly) | `ui` | 114 | 39,582 | None |  |
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
