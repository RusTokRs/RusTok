---
id: doc://docs/AI_CONTEXT.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# AI Context for RusToK

Mandatory starting context for AI sessions.

## Reading Order

1. `docs/index.md`
2. `docs/AI_CONTEXT.md`
3. `README.md` and `CRATE_API.md` of the target component
4. For backend module changes: `docs/backend/module-backend-architecture.md`,
   `docs/backend/module-backend-implementation.md` and
   `docs/backend/module-backend-verification.md`
5. For event changes: `crates/rustok-outbox/docs/README.md` and `docs/architecture/event-flow-contract.md`

## Terminology

### Platform modules

For platform modules, there are only two statuses:

- `Core`
- `Optional`

The source of truth for module composition is `modules.toml`.

### Crates

`crate` is a technical packaging unit in Cargo. Not every crate in `crates/` is automatically a platform module.

In `crates/`, there are:

- module-crates
- shared libraries
- infrastructure/support crates

### Important Rule

Do not mix:

- **module status** (`Core` / `Optional`)
- **wiring method** (`ModuleRegistry`, bootstrap, codegen, host wiring)
- **packaging form** (`crate`)

`rustok-outbox` is a `Core` module. The fact that event runtime uses it directly does not make it a separate component type.

## Current Platform Baseline

### Core modules

- `auth`
- `cache`
- `channel`
- `email`
- `index`
- `search`
- `outbox`
- `tenant`
- `rbac`

### Optional modules

- `content`
- `commerce`
- `blog`
- `comments`
- `forum`
- `pages`
- `taxonomy`
- `media`
- `workflow`

## Common Invariants

- Platform modules must remain consistent between `modules.toml`, `build_registry()` and manifest validation.
- For write-flow with cross-module events, the transactional outbox is used.
- Tenant isolation and RBAC are mandatory in the service layer.
- Events and handlers must remain compatible with `DomainEvent` / `EventEnvelope`.
- For Leptos host applications and module-owned Leptos UI packages, the default internal data layer is built via `#[server]` functions.
- GraphQL remains a mandatory parallel contract: for headless clients, Next.js UI and selected GraphQL paths in Leptos where the surface supports headless/CSR execution.
- New Leptos UI code must not be born as a GraphQL-only path if a native `#[server]` layer is possible for the scenario.
- For the core wave, the source of truth for module composition and coverage is `modules.toml`, not outdated lists in local notes.
- In the current core wave, `auth`, `search` and `channel` are considered active dual-path work; `cache` and `email` are already covered by host-level build-profile-selected native surfaces; `index`, `outbox`, `tenant` and `rbac` now also have module-owned Leptos admin surfaces with native `#[server]` bootstrap.
- Leptos core surfaces use build-profile-selected native and GraphQL paths where both contracts exist; `rustok-channel-admin` may keep its REST secondary path while that REST client is supported in parallel.

## Historical Loco Replacement Inventory (Archived)

This section preserves the pre-Axum replacement inventory only. Some code
examples and API names below belong to the historical Loco host and are not
implementation guidance. Use the [Loco RS Exit Plan](./architecture/loco-exit-plan.md),
the backend guides, and current component-local documentation for active work.
Do not recreate any parallel framework adapter from this inventory.

| Loco Subsystem | Replaced by | What to do | What NOT to do |
|---|---|---|---|
| `ctx.config.auth` / JWT middleware | `rustok-auth` (`crates/rustok-auth`) | Use `auth_config_from_ctx(ctx)` → `encode_access_token` / `decode_access_token` from `apps/server/src/auth.rs` | Do not use `loco_rs::prelude::auth::JWT` directly; do not implement custom JWT outside `rustok-auth` |
| `ctx.config.cache` / Loco cache config | `rustok-cache` (`crates/rustok-cache`) | Get `CacheService` from `ctx.shared_store.get::<CacheService>()` — it is initialized in `bootstrap_app_runtime` | Do not read `REDIS_URL` manually in modules; do not create `redis::Client` directly; do not ignore `ctx.config.cache` in favor of a self-managed connection |
| Loco Mailer (`ctx.mailer`) / SMTP | `rustok-email` + `apps/server/src/services/email.rs` | Use `email_service_from_ctx(ctx, locale)` — returns a localized `BuiltInAuthEmailSender` for built-in auth mailers; provider is selected via `settings.rustok.email.provider` | Do not call `ctx.mailer` directly in handlers; do not create `AsyncSmtpTransport` outside the email service; do not extract email into a separate platform module |
| Loco Storage abstraction | `rustok-storage` (`crates/rustok-storage`) | Get `StorageService` from `ctx.shared_store.get::<StorageService>()`; upload files through it | Do not create adhoc upload backends in controllers; do not add parallel storage paths outside `rustok-storage` |
| Loco Queue / Workers | `rustok-outbox` — not a direct replacement, but a separate layer for transactional event delivery. Loco Queue (Sidekiq) and Outbox solve different problems. | For domain events with atomicity guarantees: `publish_in_tx` via `TransactionalEventBus`. For background runtime workers, use own lifecycle; for maintenance flows, the target layer is a separate `rustok-cli` over `rustok-cli-core`. | Do not duplicate event delivery-path via Loco Queue; do not create `rustok-jobs` on top of outbox — they solve different problems. ADR: `DECISIONS/2026-03-11-queue-runtime-source-of-truth-outbox.md` |
| Loco Channels (WebSocket) | Custom Axum WebSocket in `apps/server` | Use existing WS handlers | Do not use `loco_rs::controller::channels` — incompatible with custom auth-handshake |

**Current status of Loco migration:**
- Active roadmap: [`docs/architecture/loco-exit-plan.md`](./architecture/loco-exit-plan.md).
- Architectural decision: [`DECISIONS/2026-07-02-axum-runtime-and-ops-cli-boundary.md`](../DECISIONS/2026-07-02-axum-runtime-and-ops-cli-boundary.md).
- New server-owned services must not accept `loco_rs::app::AppContext`; use `ServerRuntimeContext` or narrow typed contexts.
- Module-owned Leptos server functions must consume `rustok_api::HostRuntimeContext`, not `AppContext`.
- Loco `Hooks`, `AppContext`, `Config`, tasks and initializers remain only as the current cutover inventory, not as the target architecture.
- Maintenance/CLI flows of the target state go through a separate `rustok-cli` and module-local `cli/` adapters, not through `cargo loco task` and not through the production server binary.
- New backend helpers must use the dedicated foundation crates: `rustok-runtime` for executable runtime access helpers, `rustok-web` for Axum response/error/extractor helpers, `rustok-fba` for FBA metadata, and `rustok-cli-core` for CLI provider contracts. Do not put these concerns back into `rustok-api` or `apps/server`.
- New HTTP response formatting must use `rustok_web::json_response` or another `rustok-web` helper. Do not add new `loco_rs::controller::format` usage.
- Backend module layout is fixed: domain/application code in `crates/rustok-<module>/src`,
  contract/evidence artifacts in `contracts/`, local plan/evidence in `docs/`, optional
  command adapters in module-local `cli/`, and route/runtime composition in `apps/server`.
  The production HTTP server must not link module CLI adapters.

**Loco Queue (Sidekiq/Redis) is not connected and not needed.** Reasons:
- Background workers run as direct tokio tasks: outbox relay (`OutboxRelayWorkerHandle`), build worker (`BuildWorkerHandle`), index/search dispatchers, workflow cron.
- Outbox pattern is architecturally better than Sidekiq for domain events — guarantees atomicity.
- Current Loco Tasks are legacy inventory; new maintenance flows are designed for a separate `rustok-cli`.
- For decoupling slow operations from HTTP requests, `tokio::spawn` is used (e.g., sending email in `forgot_password`).
- If a push-based queue with retry is needed — consider extending the outbox relay, not connecting Sidekiq.

Full matrix: [`apps/server/docs/LOCO_FEATURE_SUPPORT.md`](../apps/server/docs/LOCO_FEATURE_SUPPORT.md)

---

## Important Crates

### `crates/rustok-core`

Platform contracts: `RusToKModule`, `ModuleRegistry`, permissions, events, health, metrics.

### `crates/rustok-events`

Canonical event contract layer on top of the platform event model.

### `crates/rustok-auth`

`Core` authentication module: JWT (HS256 and RS256), Argon2 password hashing, refresh tokens, password reset, invite, email verification tokens. It replaces the historical Loco JWT helper and is composed through `apps/server/src/auth.rs` with RusToK-owned runtime/settings contracts.

Algorithm is selected via `AuthConfig::algorithm: JwtAlgorithm`:
- `JwtAlgorithm::HS256` (default) — symmetric, `AuthConfig::secret`
- `JwtAlgorithm::RS256` — asymmetric, `AuthConfig::with_rs256(private_pem, public_pem)`

Server runtime reads auth overrides only through `settings.rustok.auth` in
`apps/server/src/auth.rs`: `algorithm`, `rsa_private_key_env`,
`rsa_public_key_env`, `rsa_private_key_pem`, and `rsa_public_key_pem`.
`HS256` remains the default. `RS256` requires both RSA keys and must fail
config assembly instead of silently downgrading to `HS256`.

### `crates/rustok-cache`

`Core` cache management module: Redis client (single connection point), in-memory fallback (Moka), `CacheService::health()` with PING check. **Replaces** `ctx.config.cache`. Initialized in `bootstrap_app_runtime`, available via `ctx.shared_store.get::<CacheService>()`.

Redis URL is specified via (in priority order):
1. `settings.rustok.cache.redis_url` in YAML
2. env `RUSTOK_REDIS_URL`
3. env `REDIS_URL`

### `crates/rustok-email`

`Core` email delivery module: SMTP via lettre and Tera templates. Factory
`email_service_from_ctx(ctx, locale)` in `apps/server/src/services/email.rs`
selects `smtp` or explicit `none`; SMTP transport is cached in the typed server
runtime via `SharedSmtpEmailService`.

Two public traits:
- `BuiltInAuthEmailSender` in `apps/server/src/services/email.rs` — localized runtime contract for built-in auth email flows (password reset + email verification)
- `TransactionalEmailSender` — general contract for any transactional email by template ID (`"{module}/{action}"`, e.g. `"commerce/order_confirmed"`). Modules register templates via `EmailTemplateProvider`; `SmtpEmailSender::with_provider()` connects the provider.

### `crates/rustok-storage`

Infrastructure storage crate: `StorageBackend` trait, `LocalStorage`, `StorageService`. **Replaces** Loco Storage abstraction. Initialized in `bootstrap_app_runtime` (feature `mod-media`), available via `ctx.shared_store.get::<StorageService>()`. S3 backend is declared in Cargo.toml features, but not implemented.

### `crates/rustok-outbox`

`Core` module transactional outbox: `TransactionalEventBus`, `OutboxTransport`, `OutboxRelay`, `SysEventsMigration`. **Replaces** Loco Queue / Workers. ADR: `DECISIONS/2026-03-11-queue-runtime-source-of-truth-outbox.md`.

### Backend foundation crates

- `crates/rustok-runtime`: executable runtime helpers such as typed host shared-handle lookup.
- `crates/rustok-web`: Axum HTTP boundary helpers such as JSON response mapping and HTTP error envelopes.
- `crates/rustok-fba`: FBA provider/consumer metadata and backend topology descriptors.
- `crates/rustok-cli-core`: stable command/provider contracts for the future `rustok-cli` and module-local `cli/` adapters.

For module backend implementation, read `docs/backend/README.md`.

### `crates/rustok-tenant`

`Core` module multi-tenant lifecycle and module enablement.

### `crates/rustok-rbac`

`Core` module authorization, roles, policies and permission resolution.

### `crates/rustok-content` / `commerce` / `blog` / `forum` / `pages` / `media` / `workflow`

Optional domain modules and their transport/UI surfaces.

## Known False Compilation Errors

When running `cargo check -p rustok-server` without a built frontend, errors appear:

```
error: #[derive(RustEmbed)] folder 'apps/admin/dist' does not exist
error[E0599]: no function or associated item named `get` found for struct `AdminAssets`
```

**These are not bugs** — this is expected behavior of the `embed-admin-assets` feature. The feature requires prior build of `apps/admin/dist` (`trunk build` or `npm run build`). In CI/dev environments without frontend artifacts, the feature is disabled by default, and `/admin/*` returns `503`. Errors in `app_router.rs` when checking code without artifacts are normal — ignore them.

Check only errors in changed files; errors in `services/app_router.rs` / `AdminAssets` are not related to server logic.

## Do / Don't

### Do

- Use only actually existing APIs from code and docs.
- For domain write-flows with events, use `publish_in_tx` when atomic publish is needed.
- Check that docs reflect the current code, not old architectural assumptions.
- For Leptos UI, first design a local API layer `view -> local api -> #[server]`, and keep GraphQL as a parallel selected-path transport.
- For optional-wave module-owned Leptos admin surfaces, the current baseline is: `rustok-media-admin` works on the build-profile-selected native `#[server]` model with GraphQL selected path for `list/detail/translations/delete/usage` and with a preserved REST primary upload path; `rustok-comments-admin` works on the native-only `#[server]` model because no owner GraphQL/REST transport surface exists for `comments` admin.
- For backend modules, follow `docs/backend/module-backend-implementation.md`: domain logic stays in the owner module, host wiring stays in `apps/server`, reusable runtime/web/FBA/CLI contracts go into the dedicated foundation crates.
- Keep module-local `cli/` as an external adapter package: it may depend on the module
  domain crate and `rustok-cli-core`, but the domain crate and production server runtime
  must not depend on CLI parsing, stdout or process exit behavior.
- `rustok-content` remains a shared helper/orchestration boundary without its own operator-facing UI.
- Commerce split crates (`cart`, `customer`, `product`, `profiles`, `region`, `pricing`, `inventory`, `order`, `payment`, `fulfillment`) do not receive separate admin UIs in this wave and continue to live under the aggregate surface `rustok-commerce-admin`.

### Don't

- Do not invent a third module type besides `Core` and `Optional`.
- Do not substitute the architectural status of a module with the runtime wiring method.
- Do not bypass outbox in production event-flow.
- Do not delete GraphQL resolver/path just because a native `#[server]` path appeared nearby.
- Do not add new backend code around `loco_rs::app::AppContext`, `loco_rs::controller::format`, `cargo loco task` or host-local helper copies when a `rustok-*` foundation crate owns the concern.
