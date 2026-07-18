# RusTok — Verification Scripts

Automated platform checks integrated into the common verification workflow. Entry point for manual orchestration runs: [PLATFORM_VERIFICATION_PLAN.md](../../docs/verification/PLATFORM_VERIFICATION_PLAN.md).

## Quick Start

```bash
# Run ALL checks (compact output)
./scripts/verify/verify-all.sh

# Run ALL checks (full output)
./scripts/verify/verify-all.sh -v

# Run a single category
./scripts/verify/verify-all.sh tenant-isolation
./scripts/verify/verify-all.sh api-quality
./scripts/verify/verify-all.sh deployment-profiles

# Run a script directly (always full output)
./scripts/verify/verify-tenant-isolation.sh
./scripts/verify/verify-deployment-profiles.sh
./scripts/verify/verify-migration-smoke.sh
node scripts/verify/verify-flex-multilingual-contract.mjs
node scripts/verify/verify-flex-standalone-contract.mjs
node scripts/verify/verify-module-lifecycle-bypass-usage.mjs
node scripts/verify/verify-module-control-plane-write-path.mjs
node scripts/verify/verify-module-build-worker-isolation.mjs
node scripts/verify/verify-api-surface-contract.mjs
node scripts/verify/verify-axum-runtime.mjs
node scripts/verify/export-reference-artifacts.mjs artifacts/reference
node scripts/verify/verify-reference-artifacts.mjs artifacts/reference
node crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-parity.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-registry.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-fallback-profiles.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-toggle-profiles-consistency.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-error-catalog-binding.mjs pages
node crates/rustok-page-builder/scripts/verify/verify-page-builder-consumer-readiness.mjs pages
node scripts/verify/verify-ecommerce-fba-registries.mjs
```

## When to Run

| Situation | Command |
|-----------|---------|
| Before commit | `./scripts/verify/verify-all.sh` |
| After module refactoring | `./scripts/verify/verify-all.sh -v` |
| PR review | `./scripts/verify/verify-all.sh -v` |
| Added a new endpoint | `./scripts/verify/verify-all.sh api-quality` + `node scripts/verify/verify-api-surface-contract.mjs` |
| Touching Axum runtime boundaries | `node scripts/verify/verify-axum-runtime.mjs` |
| Exporting OpenAPI and GraphQL contracts | `node scripts/verify/export-reference-artifacts.mjs artifacts/reference` |
| Added a new event | `./scripts/verify/verify-all.sh events` |
| Anti-bypass drift check | `./scripts/verify/verify-all.sh anti-bypass` |
| Added a migration | `./scripts/verify/verify-all.sh tenant-isolation` + `./scripts/verify/verify-migration-smoke.sh`; in CI the same smoke is pinned as a separate job `migration-smoke` |
| Suspected RBAC gap | `./scripts/verify/verify-all.sh rbac-coverage` |
| Security audit | `./scripts/verify/verify-security.sh` |
| Deployment profile matrix check | `./scripts/verify/verify-all.sh deployment-profiles` |
| Flex multilingual contract drift check | `node scripts/verify/verify-flex-multilingual-contract.mjs` |
| No-compile guardrails check for standalone Flex Phase 5 | `node scripts/verify/verify-flex-standalone-contract.mjs` |
| Runtime-context/cache-key invariants check | `node scripts/verify/verify-runtime-context-invariants.mjs` |
| Large FFA UI migration gate | `npm run verify:ffa:ui:migration` |
| Sweep all `core_transport_ui` rows in readiness board | `node scripts/verify/verify-ffa-ui-boundary-sweep.mjs` |
| Sweep transport profiles for FFA surfaces | `node scripts/verify/verify-ffa-ui-transport-profile-sweep.mjs` |
| Inventory admin native/write boundary check | `node scripts/verify/verify-inventory-admin-boundary.mjs` |
| AI admin FFA boundary check | `node scripts/verify/verify-ai-admin-boundary.mjs` |
| AI Rig-only cutover drift check | `node scripts/verify/verify-ai-rig-cutover.mjs` |
| Tenant admin FFA boundary check | `node scripts/verify/verify-tenant-admin-boundary.mjs` |
| Lifecycle bypass helper prohibition in production | `node scripts/verify/verify-module-lifecycle-bypass-usage.mjs` |
| Provider/consumer parity check for page-builder contract | `node crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-parity.mjs` |
| Machine-readable registry page-builder vs manifests check | `node crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-registry.mjs` |
| Required fallback/toggle profiles for page-builder | `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fallback-profiles.mjs` |
| Toggle profile value consistency for page-builder | `node crates/rustok-page-builder/scripts/verify/verify-page-builder-toggle-profiles-consistency.mjs` |
| Full baseline gate page-builder FBA before Wave 0/Wave 1 | `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs` |
| Error catalog drift between provider/consumer manifest, backend and UI adapters | `node crates/rustok-page-builder/scripts/verify/verify-page-builder-error-catalog-binding.mjs pages` |
| Consumer module readiness check (`pages/forum`) | `node crates/rustok-page-builder/scripts/verify/verify-page-builder-consumer-readiness.mjs <slug>` |
| Ecommerce FBA provider registries and locked contract-test metadata | `node scripts/verify/verify-ecommerce-fba-registries.mjs` |

Alternatively, the same checks are available via `npm run`:

```bash
npm run verify:page-builder:contract-parity
npm run verify:page-builder:fallback-profiles
npm run verify:page-builder:toggle-profiles
npm run verify:page-builder:fba:baseline
npm run verify:page-builder:error-catalog
npm run verify:page-builder:consumer:pages
npm run verify:page-builder:consumer:forum  # forum FW-2 + Wave 1 smoke/SLO/trace guardrail
npm run verify:ffa:ui:migration
npm run verify:ffa:ui:migration:boundary-sweep
npm run test:verify:ffa:ui:migration:boundary-sweep
npm run verify:ffa:ui:migration:transport-profile
npm run test:verify:ffa:ui:migration:transport-profile
npm run verify:ecommerce:fba-registries
npm run test:verify:ecommerce:fba-registries
```

## Script Descriptions

### `verify:ffa:ui:migration` / `verify-ffa-ui-boundary-sweep.mjs`
**FFA UI migration gate** — large source-level gate for module-owned UI migration to `core/transport/ui`.

What the aggregate `npm run verify:ffa:ui:migration` does:
- checks FFA migration contract docs and anti-over-extraction/doc patterns;
- runs a repository-wide sweep over all readiness board rows with `Structural shape: core_transport_ui`;
- checks the transport profile of each FFA surface: multi-adapter by code or documented single-adapter/owner-fragment state;
- runs module-specific boundary guardrails for modules with deeper local rules.

What the sweep additionally verifies:
- central readiness board in `docs/modules/registry.md` matches local `docs/implementation-plan.md` by FFA/FBA status and structural shape;
- each `core_transport_ui` surface has `core`, `transport` and `ui/leptos` layers;
- `core` remains free of Leptos/server-function imports;
- `ui/leptos` does not call raw `api::*` directly.

Example:

```bash
npm run verify:ffa:ui:migration
npm run verify:ffa:ui:migration:boundary-sweep
npm run test:verify:ffa:ui:migration:boundary-sweep
npm run verify:ffa:ui:migration:transport-profile
npm run test:verify:ffa:ui:migration:transport-profile
node scripts/verify/verify-ffa-ui-boundary-sweep.mjs
node scripts/verify/verify-ffa-ui-transport-profile-sweep.mjs
```

### `verify-ecommerce-fba-registries.mjs`
**Ecommerce FBA provider/consumer registry gate** — checks provider metadata for `payment`, `fulfillment`, `order`, `pricing`, `inventory` and consumer metadata for `commerce`.

What it does:
- verifies module-owned `contracts/*-fba-registry.json` against `rustok-module.toml`, `Cargo.toml`, `src/lib.rs`, `src/ports.rs`, local implementation plan, the central readiness board and evidence paths inside the registry;
- requires neutral `PortContext`/`PortError`, per-operation port declarations and an in-process provider implementation marker if declared in the registry;
- checks `contract_tests.status = planned_cases_locked`, presence of `in_process` + `remote_adapter_placeholder` profiles, a case for each port operation and baseline assertions `typed_port_error_mapping`/`context_deadline_preserved`;
- verifies that the planned fallback-smoke profile set covers all consumer fallback profiles so future runtime evidence does not diverge from provider/consumer metadata;
- verifies `crates/rustok-commerce/contracts/commerce-fba-registry.json` against provider registries so checkout orchestration does not reference outdated contract versions, profiles, degraded modes or fallback profiles.

Unit guardrail for the verifier itself: `node scripts/verify/verify-ecommerce-fba-registries.test.mjs` or `npm run test:verify:ecommerce:fba-registries`.


### `verify-migration-smoke.sh`
**Wave 4 migration-safety smoke** — PostgreSQL apply-from-zero for server migrator.

What it does:
- creates a temporary PostgreSQL database via `RUSTOK_MIGRATION_SMOKE_ADMIN_URL` inside a Rust integration test, without depending on local `psql`;
- runs the ignored integration test `postgres_zero_migration_smoke_applies_from_empty_database`;
- applies `rustok_migrations::Migrator` from scratch and checks that no pending migrations remain;
- when `RUSTOK_MIGRATION_SMOKE_INCREMENTAL=1`, applies migrations one by one to separately check the incremental apply path; the shell script and Rust test both accept only `0`/`1`, so direct test runs do not bypass this validation;
- checks for representative platform/module tables (`tenants`, `product_variants`, `prices`, `inventory_items`, `channels`, `oauth_apps`, `blog_post_tags`, `forum_topic_tags`, `taxonomy_terms`);
- drops the temporary database from the Rust test, unless `RUSTOK_MIGRATION_SMOKE_KEEP_DB=1` is set.

Example:

```bash
RUSTOK_MIGRATION_SMOKE_ADMIN_URL=postgres://postgres:postgres@localhost:5432/postgres \
  ./scripts/verify/verify-migration-smoke.sh

RUSTOK_MIGRATION_SMOKE_INCREMENTAL=1 \
RUSTOK_MIGRATION_SMOKE_ADMIN_URL=postgres://postgres:postgres@localhost:5432/postgres \
  ./scripts/verify/verify-migration-smoke.sh
```

---


### `verify-api-surface-contract.mjs`
**API surface contract guardrail** — fast no-compile gate for the API surface plan.

What it checks:
- GraphQL schema composition uses build-time generated optional module root (`graphql_schema_codegen.rs`) instead of a manual list of optional modules;
- `apps/server/build.rs` reads `modules.toml` and package-local `rustok-module.toml` declarations for `[provides.graphql]` / `[provides.http]`;
- package-local manifests are synchronized with `modules.toml` slugs and have `[crate].entry_type`;
- modules publishing GraphQL/HTTP transport are represented in the central registry;
- API verification plan and README reference this no-compile evidence path.

Example:

```bash
node scripts/verify/verify-api-surface-contract.mjs
```

---


### `verify-runtime-context-invariants.mjs`
**Wave 6 runtime-context guardrail** — fast source-level gate for already fixed P0/P1 invariants without full Rust compilation.

What it checks:
- `ChannelCacheKey` contains OAuth/client and locale dimensions;
- `RequestFacts` gets `oauth_app_id` from `AuthContextExtension`, and `locale` from `ResolvedRequestLocale.effective_locale`;
- source-order middleware in `compose_application_router` preserves the actual execution order of Axum `locale -> auth_context -> channel`;
- tenant locale cache metrics export counter names with `_total` and gauge `rustok_tenant_locale_cache_entries`;
- `modules.toml` and central registry evidence preserve `pages -> [content, page_builder]`.

Example:

```bash
node scripts/verify/verify-runtime-context-invariants.mjs
./scripts/verify/verify-all.sh runtime-context-invariants
```

---

### `verify-axum-runtime.mjs`
**Axum runtime boundary guardrail** — fast source-level gate for the server and
standalone CLI entrypoints.

What it checks:
- the composed server host, bootstrap, router, and lifecycle boundaries exist;
- the standalone CLI and runtime composition entrypoints exist.

Example:

```bash
node scripts/verify/verify-axum-runtime.mjs
npm run verify:axum:runtime
```

---

### `verify-inventory-admin-boundary.mjs`
**Wave 5/Wave 6 inventory guardrail** — fast source-level gate for inventory-owned admin read/write boundary without full Rust compilation.

What it checks:
- `InventoryQuantityWriteResult` builds `inStock` from committed quantity and backorder policy;
- native `set_variant_quantity`/`adjust_variant_quantity` use internal mutation update result and do not do a separate pre-read variant policy;
- removed GraphQL fallback stays removed: no `src/transport.rs`, `rustok-graphql`, `CommerceGraphqlInventoryReadAdapter`, GraphQL runtime markers, token/tenant-slug fallback inputs or `mod transport`;
- admin API read facades fetch-bootstrap/products/product and write facades set/adjust/reserve/release/check-availability go through inventory-owned native facades without GraphQL fallback;
- native server-function endpoints for inventory read/write/validation surfaces remain declared;
- commerce storefront/public-channel callers use inventory-owned availability/projection facades instead of direct loaders/backorder branching;
- admin UI/locales describe the native inventory facade and docs mark current admin stock operations as native/API covered.

Example:

```bash
node scripts/verify/verify-inventory-admin-boundary.mjs
./scripts/verify/verify-all.sh inventory-admin-boundary
node scripts/verify/verify-inventory-admin-boundary.test.mjs
```

---



### `verify-ai-domain-verticals.mjs`
**AI domain vertical ownership guardrail** — fast source-level gate for `rustok-ai-product`, `rustok-ai-content` and `rustok-ai-order` without Rust compilation.

What it checks:
- product/content/order support crates own task/tool constants, descriptor registration APIs and generated payload validators;
- runtime composition in `rustok-ai` consumes domain-owned registration APIs instead of hard-coded slug literals;
- content moderation sensitive-tool defaults merge into runtime policy via `content_ai_sensitive_tools`;
- local implementation plans and `rustok-ai` plan document the compile-free evidence gate.

Example:

```bash
npm run verify:ai:domain-verticals
./scripts/verify/verify-all.sh ai-domain-verticals
```

---

### `verify-ai-admin-boundary.mjs` / `verify-tenant-admin-boundary.mjs`
**FFA admin guardrails** — fast source-level checks for module-owned admin UI splits without full Rust compilation.

What they check:
- crate root wires `core`, `transport` and explicit `ui/leptos.rs` adapters;
- Leptos adapters consume module-owned transport facades instead of pre-FFA `api::` calls;
- `core.rs` stays Leptos/server-function/runtime free;
- native server-function endpoints stay inside `transport/native_server_adapter.rs`;
- old flat `api.rs` facades do not return for completed slices.

Example:

```bash
npm run verify:ai:admin-boundary
npm run verify:tenant:admin-boundary
npm run verify:ffa:ui:migration
```

---

### `verify-ai-rig-cutover.mjs`

**AI Rig cutover guardrail** — fast source-level evidence for the pinned
Rig-only provider boundary without Rust compilation.

What it checks:

- provider catalog and normalized stream cassettes remain pinned to Rig 0.39.0;
- cassettes retain OpenAI-compatible, Anthropic, Gemini, cloud-auth, and local-target families;
- typed `ProviderIntegration` dispatch and its snapshot test remain present;
- removed legacy provider adapters, `AiRuntime`, and plaintext-secret storage markers do not return.

Example:

```bash
npm run verify:ai:rig-cutover
```

---

### `verify-tenant-isolation.sh`
**Phase 19.1 + 5** — Multi-tenancy safety

What it looks for:
- `.all(&db)` without `.filter(tenant_id)` — loading another tenant's data
- `find_by_id` without tenant_id check — accessing another tenant's resource by ID
- `DELETE` without tenant_id filter — deleting another tenant's data
- Migrations: each domain table has a `tenant_id` column
- SeaORM entities: `pub tenant_id` in Model struct
- Raw SQL strings (SQL injection risk)
- Hard DELETE without soft-delete (archival)

**Severity:** CRITICAL. Violation = data leak between tenants.

---

### `verify-unsafe-code.sh`
**Phase 19.1 + 19.3** — Runtime safety

What it looks for:
- `.unwrap()` — panic on None/Err
- `.expect()` — panic with message (review each)
- `panic!()` — explicit panic
- `todo!()` / `unimplemented!()` — incomplete code
- `std::thread::sleep` — blocking tokio runtime
- `std::fs::` — blocking I/O in async
- `block_on()` — deadlock in async context
- `println!` / `eprintln!` — should be tracing::
- `unreachable!()` — is it justified?
- `static` / `lazy_static!` / `once_cell::Lazy` — should use AppContext
- `unwrap_or("default")` for secrets — unsafe fallback

**Severity:** HIGH. Panic crashes the entire tokio runtime.

---

### `verify-rbac-coverage.sh`
**Phase 19.2** — Authorization coverage

What it looks for:
- REST handlers without RBAC extractors (`Require*`, `Permission`)
- GraphQL mutations without permission checks
- GraphQL queries without auth context
- Auth middleware registered in the router

**Severity:** CRITICAL. Missing RBAC = privilege escalation.

---

### `verify-api-quality.sh`
**Phase 19.12–19.14** — API correctness

What it looks for:

**GraphQL:**
- N+1 queries — direct DB access in resolvers (should be DataLoader)
- `MergedObject` — modular schema (not monolithic)
- String errors — should be error extensions
- `TenantContext` — in every resolver
- Pagination in list queries

**REST:**
- `#[utoipa::path]` — OpenAPI annotation on every endpoint
- HTTP status codes: 201 for POST, 204 for DELETE
- Input validation via `validator::Validate`
- Rate limiting on auth endpoints
- CORS middleware

**Parity:**
- Auth operations available via both REST and GraphQL
- Single `AuthLifecycleService` (not duplicated logic)
- Business logic not in controllers/resolvers

**Severity:** HIGH. N+1 = 50x latency. Missing OpenAPI = no documentation.

---

### `verify-events.sh`
**Phase 6 + 19.1** — Event system integrity

What it looks for:
- `publish()` without `_in_tx` — data saved, event lost
- `tenant_id` in every DomainEvent struct
- Event handlers registered
- Outbox pattern implemented
- DLQ (Dead Letter Queue) exists
- Event versioning
- Idempotency guards in handlers
- Transport config (not "memory" in production)
- `#[derive(Serialize, Deserialize)]` on event structs

**Severity:** CRITICAL. publish without _in_tx = event loss on rollback.

---

### `verify-code-quality.sh`
**Phase 19.4–19.11** — Code health

What it looks for:

**Security:**
- PII in logs (password, email, token in tracing)
- Hardcoded secrets in code
- `.env` files in git
- Entities returned directly in API (should be Response DTOs)

**Metrics:**
- Files > 500 lines
- Functions > 60 lines (top 10)
- Functions with > 5 arguments

**Dependencies:**
- `rustok-core` does not depend on domain crates
- Domain crates do not depend on each other
- `rustok-test-utils` only in `[dev-dependencies]`

**Error handling:**
- `thiserror` in domain crates (not `anyhow`)
- String-based status checks (should be enum)

**Observability:**
- `#[instrument]` decorator on service methods
- Structured logging fields (not string interpolation)

**Type safety:**
- Newtype IDs (`TenantId`, `UserId`), not bare `Uuid`

**Severity:** HIGH. PII in logs = GDPR violation.

---

### `verify-security.sh`
**Phase 18** — Security audit

What it looks for:
- Argon2 for password hashing (not MD5/SHA256/bcrypt)
- Security headers (CSP, X-Frame-Options, HSTS) in middleware
- SSRF protection (allowlist for external HTTP requests)
- `zeroize` for sensitive data in memory
- JWT secret via env var (no fallback defaults)
- Token invalidation on password change

**Severity:** CRITICAL. Weak hashing = compromise of all passwords.

---

### `verify-architecture.sh`
**Phase 1 + 5** — Architectural compliance

What it looks for:
- Module dependencies: `dependencies()` trait matches `modules.toml`
- Axum composition: all routes are assembled through the server router boundary
- Module registry: all modules registered via `build_registry()`
- Core modules not toggleable (`ModuleKind::Core`)
- MCP tools use `McpToolResponse` (not raw JSON)
- Controller return types: `rustok-web` HTTP errors (not custom transport contracts)
- Dependency guard (`cargo metadata` + allow/deny):
  - backend apps (current config: `rustok-server`) → only `rustok-*` crate dependencies (except explicit infra exceptions)
  - deny new cross-domain `rustok-* -> rustok-*` edges outside allow-list

---

### `verify-page-builder-contract-parity.mjs`
**Page Builder FBA baseline** — Provider/consumer version parity

What it checks:
- `builder_contract_version` between `rustok-page-builder` (provider) and `rustok-pages` (consumer);
- `consumer_min_version` in the provider manifest and the condition `consumer.builder_contract_version >= provider.consumer_min_version`;
- `contract_version` in the consumer manifest relative to the provider version.

**Severity:** HIGH. Contract version drift blocks safe rollout between Wave 0/Wave 1.

---


### `verify-page-builder-contract-registry.mjs`
**Page Builder FBA baseline** — Machine-readable registry anti-drift

What it checks:
- `crates/rustok-page-builder/contracts/page-builder-fba-registry.json` exists and has `schema_version = 1`;
- provider metadata (`contract`, `builder_contract_version`, `consumer_min_version`, capabilities) matches `rustok-page-builder/rustok-module.toml`;
- the selected consumer (`pages` or `forum`) matches the registry by `contract_version`, `builder_contract_version`, `consumer_min_version` and capabilities;
- consumer version is not below provider `consumer_min_version`.

**Severity:** HIGH. Registry drift blocks Wave 0/Wave 1 promotion because contract freeze becomes unverifiable.

### `verify-page-builder-fallback-profiles.mjs`
**Page Builder FBA baseline** — Required fallback/toggle structure

What it checks:
- presence of sections `fba.builder_consumer.degraded_modes` and `fba.builder_consumer.toggle_profiles`;
- required degraded mode keys and profiles (`all_on/publish_off/preview_off/builder_off`);
- presence of required toggle flags and typed degraded-mode for publish-disable path.

**Severity:** HIGH. Missing fallback structure leads to uncontrollable degradation when capability is disabled.

---

### `verify-page-builder-toggle-profiles-consistency.mjs`
**Page Builder FBA baseline** — Toggle profile value consistency

What it checks:
- that in each profile (`all_on/publish_off/preview_off/builder_off`) flags have the expected boolean combinations;
- that dry-run rollout semantics remain deterministic.

**Severity:** HIGH. Inconsistent profiles make tenant-toggle rollout unpredictable.

---

### `verify-page-builder-fba-baseline.mjs`
**Page Builder FBA baseline** — Aggregate gate

What it does:
- sequentially runs:
  1) `verify-page-builder-contract-parity.mjs`,
  2) `verify-page-builder-contract-registry.mjs <module-slug>`,
  3) `verify-page-builder-consumer-readiness.mjs <module-slug>` (default `pages` in the aggregator),
  4) `verify-page-builder-fallback-profiles.mjs <module-slug>`,
  5) `verify-page-builder-toggle-profiles-consistency.mjs <module-slug>`,
- returns non-zero exit code on any step failure.

**Severity:** GATE. This is the canonical baseline check before promotion to the next rollout wave.

---

### `verify-page-builder-consumer-readiness.mjs`
**Page Builder FBA baseline** — consumer module readiness check

What it checks:
- presence of `rustok-module.toml` and `docs/implementation-plan.md` for the consumer module;
- presence of dependency/consumer contract markers (`page_builder`/`builder_consumer`, `contract_version`, `builder_contract_version`);
- presence of `Execution checkpoint` and FBA/page-builder readiness notes in the implementation-plan;
- for `pages`: rollout policy markers in manifest/docs for `control_plane_builder_wave_audit`, before/after snapshots, keep/rollback decision, owner sign-off, SLO rollback triggers, pilot smoke `preview -> properties -> publish(dry)` and rollback target <= 10 minutes without redeploy;
- for `forum`: FW-2 fallback matrix, FW-4/Wave 1 rollout evidence, numeric SLO metrics, forum-owned trace keys, approvals/waivers, monthly refresh policy, timeliness by dates (`created_at`, `next_due_at`, `max_age_days`) and non-empty form of mandatory refresh sections before builder-consumer rollout.

Supported slugs:
- `pages`
- `forum`

**Severity:** MEDIUM. The script checks structural readiness before including a module in a rollout wave.
  - deny nested imports of internal modules without explicit permission

**Severity:** CRITICAL. Module outside registry = fails health check.

---

### `verify-forum-wave-evidence-freshness.mjs`
**Forum Page Builder consumer freshness** — focused no-compile gate for live Wave 1 evidence

What it checks:
- `forum-wave1-rollout-evidence.json` refers to `forum`, `wave=1`, `mode=live`;
- monthly refresh policy enforces `npm run verify:page-builder:consumer:forum`;
- stale evidence blocks rollout until evidence is refreshed;
- `next_due_at` is after `created_at`, fits within `max_age_days` and is not in the past at gate runtime;
- mandatory refresh sections declared in policy are actually present in the packet and non-empty (`waivers` allowed as an empty array).

**Severity:** GATE for forum builder-consumer rollout. The script does not run Rust/Leptos compilation.

---

### `verify-deployment-profiles.sh`
Smoke-check of supported build surfaces:

- `monolith` — default feature set + startup smoke
- `server+admin` — `--no-default-features --features redis-cache,embed-admin`
- `headless-api` — `--no-default-features --features redis-cache`
- `registry-only` — runtime host mode `RUSTOK_RUNTIME_HOST_MODE=registry_only` on top of the minimal headless feature-profile (`--no-default-features --features redis-cache`)

The script runs `cargo check` and profile-specific smoke-test router/startup for each configuration. For
`registry-only` it additionally checks env override `RUSTOK_RUNTIME_HOST_MODE=registry_only`,
narrowed runtime surface and reduced OpenAPI so the deployment contract for a read-only catalog host does not
drift between docs and actual runtime.
Additionally for `registry-only` the matrix already holds `GET /v1/catalog/{slug}` detail-path,
cache-contract via `ETag` / `If-None-Match` and negative smoke on write routes
`POST /v2/catalog/publish`, `POST /v2/catalog/publish/{request_id}/validate`,
`POST /v2/catalog/publish/{request_id}/stages`,
`POST /v2/catalog/publish/{request_id}/request-changes`,
`POST /v2/catalog/publish/{request_id}/hold`,
`POST /v2/catalog/publish/{request_id}/resume`,
`POST /v2/catalog/runner/claim`, `POST /v2/catalog/owner-transfer` and
`POST /v2/catalog/yank`.

For an already deployed dedicated host the same script now supports optional external smoke:

```bash
RUSTOK_REGISTRY_BASE_URL=https://modules.rustok.dev \
RUSTOK_REGISTRY_SMOKE_SLUG=blog \
RUSTOK_REGISTRY_EVIDENCE_DIR=./tmp/modules-rustok-dev-smoke \
./scripts/verify/verify-deployment-profiles.sh
```

The PowerShell variant supports the same contract via env vars
`RUSTOK_REGISTRY_BASE_URL`, optional `RUSTOK_REGISTRY_SMOKE_SLUG` and optional
`RUSTOK_REGISTRY_EVIDENCE_DIR`. If evidence dir is set, external smoke saves
`runtime-*`, `catalog-*`, `openapi-*` snapshots and `registry-smoke-metadata.txt` there, and negative
smoke covers the same expanded V2 surface (`publish`, `validate`, `stages`,
`request-changes`, `hold`, `resume`, `runner/claim`, `owner-transfer`, `yank`).

For Windows / PowerShell use `./scripts/verify/verify-deployment-profiles.ps1`: it
covers the same deployment profile matrix when `bash` is not available in the local environment.

**Severity:** HIGH. Broken profile matrix = build contract documented but not reproducible.

---


### `verify-anti-bypass.sh`
**Phase 19.15** — Anti-bypass audit

What it looks for (candidates for manual review):
- Duplication of domain rule validation in `apps/server` and frontend-adapter layer
- Manual event publishing in app layer instead of module service
- Direct queries to domain tables bypassing crate API
- Orchestration-only control signatures (domain service calls)

Modes:
- `--manual-review` — extended candidate output
- `--strict` — found candidates are treated as errors (use in CI gate when needed)

Important: anti-bypass audit does not require "blindly moving everything to modules". Candidate review is done manually, considering the allowed platform/core layer and frontend-library layer.

**Severity:** MEDIUM→HIGH. Goal — systematically catch drift and record migration-task with correct target layer: domain logic → `crates/rustok-<domain>`, platform/core orchestration → `apps/server` + `crates/rustok-core`, frontend duplication → custom frontend libraries.

---
### `verify-flex-multilingual-contract.mjs`
Focused repo-side guardrail for the live Flex multilingual contract.

What it looks for:
- cleanup migration `m20260410_000001_cleanup_flex_attached_legacy_inline_metadata` is wired into the canonical server migrator;
- standalone runtime does not revert to inline localized fallback in `flex_entries.data`;
- attached runtime does not revert to inline localized fallback in donor `metadata`;
- `crates/flex` docs continue to document migration-based cleanup as the canonical path.

**Severity:** HIGH. Reverting to inline localized fallback would again scatter the single multilingual storage contract.

---
### `verify-storefront-module-routes.mjs`
Repo-side contract for storefront routes of modular UI surfaces.

What it checks:
- manifest metadata of storefront UI modules is synchronized with route wiring;
- route keys do not fall outside the expected contract-surface;
- drift between module-owned route map and host wiring is recorded as an error.

**Severity:** HIGH. Drift in storefront route contract breaks navigation and modular UI integration.

---
### `verify-i18n-contract.mjs`
Repo-side guardrail for the platform i18n contract.

What it checks:
- key i18n contract rules remain consistent in source/documentation;
- no regression in canonical locale handling paths for server-owned contract.

**Severity:** HIGH. i18n contract drift quickly leads to inconsistent locale fallback and UI/Server mismatch.

---
### `verify-ui-i18n-parity.mjs`
i18n parity check between module-owned UI and host-runtime expectations.

What it checks:
- module UI wiring does not diverge from host-provided locale contract;
- key surface points do not bypass the canonical locale provider.
- JSON bundles `rustok-product/admin` and `rustok-product/storefront` are included in the common key-parity scan without exceptions.

**Severity:** HIGH. Parity violation leads to i18n fragmentation and behavioral differences between surfaces.

---
### `verify-module-control-plane-write-path.mjs`
Guardrail against direct writes to module control-plane aggregates. The server,
installer persistence adapter, and module build/verification worker or
transport crates may map authenticated requests and supply host adapters, but
writes to composition, lifecycle, artifact installation/data, build, and
registry governance tables must remain in `rustok-modules` owner services. It
also rejects direct construction of extracted owner SeaORM services outside
`rustok-modules`; production composition roots must obtain them through
`ModuleControlPlane`.

---
### `verify-module-build-worker-isolation.mjs`
Guardrail for the isolated untrusted module build worker. It rejects direct
tenant-database, platform-storage, and general-secret dependencies or APIs in
the worker crate. It also requires the fixed untrusted runner to clear its
environment, be killed on drop, and receive no database or credential values.
It additionally rejects server-local module build worker/delivery paths and
requires the independent dispatcher to use the mTLS remote worker after a
readiness check. The worker must require and validate OCI job receipt evidence
bound to the immutable build request and configured hardened runtime.

---
### `verify-module-lifecycle-bypass-usage.mjs`
Guardrail against using the lifecycle bypass helper in production/runtime paths.

What it checks:
- the lifecycle bypass helper does not leak into production paths;
- forbidden usage is recorded as a contract violation.

**Severity:** HIGH. Bypass in production violates lifecycle governance and publish/runtime safety.

---
### `verify-all.sh`
**Master runner** — runs all `verify-*.sh` and key `verify-*.mjs` with a final report.
In non-verbose mode the runner tries to show a compact summary, and on failure prints
explicit `error/failed/violation` lines (with tail output fallback) so errors are not lost.

```
╔══════════════════════════════════════════════╗
║   Verification Report                        ║
╚══════════════════════════════════════════════╝

  PASS Tenant Isolation
  PASS Unsafe Code Patterns
  FAIL RBAC Coverage (2 error(s))
  PASS API Quality (REST + GraphQL)
  PASS Event System
  PASS Code Quality
  PASS Security
  PASS Architecture

  Total: 15 suites | 14 passed | 1 failed
```

> Note: the number of suites in the example is illustrative and should match
> the current `SCRIPTS` list in `verify-all.sh`.

## Result Interpretation

| Symbol | Meaning | Action |
|--------|---------|--------|
| `✓` (green) | Check passed | Nothing to do |
| `!` (yellow) | Warning — manual review | Review manually, may be OK |
| `✗` (red) | Error — violation | Must fix |

**Exit codes:**
- `0` — all checks passed
- `N` — aggregated error count (errors, not warnings)
- `255` — more than 255 errors (process exit code limit)

## Adding Scripts

To add a new check:

1. Find the appropriate script by category
2. Add a section with header/pass/fail/warn
3. Update this README

```bash
# New check template
header "N. Check description"
count=$(grep -rn 'PATTERN' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | wc -l)
if [[ $count -eq 0 ]]; then
    pass "Success description"
else
    fail "$count violation(s):"
    grep -rn 'PATTERN' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | head -10
fi
```

## Related Documents

- [Platform Verification Plan](../../docs/verification/PLATFORM_VERIFICATION_PLAN.md) — master plan for periodic runs
- [Quality and Operations Verification Plan](../../docs/verification/platform-quality-operations-verification-plan.md) — detailed test block, observability, CI/CD, security and quality checks
- [Forbidden Actions](../../docs/standards/forbidden-actions.md) — prohibitions with examples
- [Patterns vs Antipatterns](../../docs/standards/patterns-vs-antipatterns.md) — comparison
- [Known Pitfalls](../../docs/ai/KNOWN_PITFALLS.md) — common AI agent mistakes
