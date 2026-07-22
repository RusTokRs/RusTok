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

## Runtime and Operations Boundaries

`apps/server` is the Axum composition root. Server-owned handlers receive
`ServerRuntimeContext` or narrow typed runtime values; module-owned Leptos
functions receive `rustok_api::HostRuntimeContext`. HTTP response mapping uses
`rustok-web`, while executable runtime helpers belong in `rustok-runtime`.

Maintenance work runs through the separate `rustok-cli` and module-local
`cli/` adapters. Domain code must not depend on terminal parsing, process exit
policy, or the production HTTP server.

Background work uses explicit Tokio lifecycle handles and the transactional
outbox relay. For atomic domain-event publishing, use `publish_in_tx`.

---

## Important Crates

### `crates/rustok-core`

Platform contracts: `RusToKModule`, `ModuleRegistry`, permissions, events, health, metrics.

### `crates/rustok-events`

Canonical event contract layer on top of the platform event model.

### `crates/rustok-auth`

`Core` authentication module: JWT (HS256 and RS256), Argon2 password hashing, refresh tokens, password reset, invite, email verification tokens. It is composed through `apps/server/src/auth.rs` with RusToK-owned runtime/settings contracts.

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

Infrastructure support crate for direct `object_store` use. `StorageRuntime` exposes `Arc<dyn ObjectStore>`, an optional signer, runtime diagnostics, and canonical chronological/digest key constructors. It is initialized in `bootstrap_app_runtime`; domain owners call `ObjectStore` directly and own lifecycle metadata. Local storage is the development default and S3-compatible storage is optional through the `s3` feature.

### `crates/rustok-outbox`

`Core` module transactional outbox: `TransactionalEventBus`, `OutboxTransport`, `OutboxRelay`, `SysEventsMigration`. ADR: `DECISIONS/2026-03-11-queue-runtime-source-of-truth-outbox.md`.

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
- Do not add new backend code around host-wide service locators, duplicate controller formatters, server-binary maintenance commands or host-local helper copies when a `rustok-*` foundation crate owns the concern.
