# Loco Integration Assessment

> **Status:** Assessment only — no code changes. Branch: `claude/review-loco-integration-CCJIx`
> **Scope:** `apps/server` Loco 0.16.4 integration

---

## 1. What We Actually Use vs. What We've Replaced

### Used (genuinely load-bearing)

| Loco symbol | Where used | Why it matters |
|---|---|---|
| `cli::main` | `main.rs:11` | CLI entry point — `start`, `task`, `seed` subcommands |
| `Hooks` trait | `app.rs:30` | Lifecycle contract: boot, routes, after_routes, connect_workers, initializers, seed, truncate |
| `create_app::<Self, Migrator>` | `app.rs:57` | DB connection pool creation + SeaORM migration runner |
| `AppContext` | ~40 files | Shared state container: `.db`, `.config`, `.shared_store` |
| `Config` + `config.auth.jwt` | `auth.rs:17-23` | JWT secret and expiration parsed from Loco config YAML |
| `AppRoutes` / `Routes` | `app.rs:60`, all controllers | Route builder wrapping Axum router |
| `Task` / `TaskInfo` / `Vars` / `Tasks` | `tasks/*.rs` | CLI task system (cleanup, rebuild, seed, etc.) |
| `Initializer` | `initializers/*.rs` | Startup hooks (telemetry, superadmin seeding) |
| `loco_rs::Error` + `loco_rs::Result` | ~30 files | Universal return type for all controllers and services |
| `loco_rs::prelude::*` | ~20 files | Pulls in `format::json`, `State`, `Response`, `Error`, `Result` |
| `tests_cfg::app::get_app_context` | `app.rs:143`, `app_lifecycle.rs:164` | Test DB context setup |

### Replaced (Loco has a version; we don't use it)

| Loco subsystem | Our replacement | Evidence |
|---|---|---|
| `loco_rs::auth` (JWT, password hashing) | `rustok-auth` crate | `crate::auth` is a bridge module: `// Thin wrappers that convert rustok_auth::AuthError → loco_rs::Error` |
| `loco_rs::mailer` (Lettre-based) | `rustok-email` crate | `services/email.rs`: bridge wrapping `rustok_email::EmailService` |
| `loco_rs::cache` | `rustok-cache` crate (moka + Redis) | Tenant middleware uses `CacheService`, `CacheBackend` from `rustok-cache` |
| `loco_rs::bgworker` (Sidekiq) | `tokio::spawn` directly | Outbox relay + build worker use `JoinHandle`; `_queue: &Queue` param is ignored |
| Loco's scheduler | `alloy_scripting::Scheduler` | `app_runtime.rs:128-134` |
| Loco's user model | Custom `models/users.rs` | No Loco model macros used; pure SeaORM entities |
| Loco's auth controller | `controllers/auth.rs` | Full custom auth with sessions, OAuth2, invite tokens |
| Loco's seeder helpers | Custom `seeds/mod.rs` | |

### Never used at all

- `loco_rs::storage` (object store)
- Loco's view/template system
- Loco's CRUD model macros (`ModelCrud`, `find_all`, etc.)
- Loco's built-in pagination
- Loco's default health/ping routes (we add our own via `with_default_routes()` + our `/health`)

---

## 2. Where Loco Conventions Conflict with Our Architecture

### 2.1 `loco_rs::Error` as the universal error type

Every custom error domain must convert to `loco_rs::Error`. The bridge modules in `crate::auth` and `services/email.rs` exist solely for this conversion. Domain modules (`rustok-auth`, `rustok-email`, SeaORM `DbErr`) have their own error types; Loco forces a lossy conversion at the controller boundary.

**Conflict:** Our domain errors have fine-grained variants (`AuthError::InvalidCredentials`, `AuthError::TokenEncodingFailed`). After conversion to `loco_rs::Error` the variant information is partially lost. Adding new auth error cases requires updating both `rustok-auth` and the bridge.

### 2.2 `AppContext` as a God object passed everywhere

`loco_rs::AppContext` is passed to every controller, service, initializer, and task. Our runtime state (GraphQL schema, rate limiters, event runtime, module registry, tenant cache, marketplace catalog) all hang off `ctx.shared_store` as a typed map.

**Conflict with module system:** The `ModuleRegistry` in `rustok-core` is our own abstraction, but it's stored as `Extension(runtime.registry)` on the Axum router (inserted in `compose_application_router`), not via `ctx.shared_store`. This creates two parallel state distribution mechanisms: Loco's `shared_store` and Axum's `Extension`.

**Conflict with RBAC:** `RbacService`, `rbac_service`, permissions — all take `&AppContext` or `&DatabaseConnection`. Custom RBAC is entirely independent of Loco but must accept Loco's context type as the transport.

### 2.3 Config format coupling

`auth_config_from_ctx` reads `ctx.config.auth.jwt.secret` and `ctx.config.auth.jwt.expiration` — these are Loco-specific config fields. Our own settings are stored under `ctx.config.settings` as a JSON blob (the `rustok` key), then deserialized into `RustokSettings`. This means auth configuration is split: JWT secret lives in Loco config, everything else in our settings. If the tenant changes the JWT secret it needs to know to use Loco's YAML format.

### 2.4 `AppRoutes::with_default_routes()` adds unknown routes

`app.rs:62`: `AppRoutes::with_default_routes()` is called before our routes are added. It is not clear from reading the code what Loco adds here (Loco 0.16 adds a default ping route and potentially others). We add our own `/health/*` routes separately, which may or may not conflict.

### 2.5 `Queue` parameter is vestigial

`Hooks::connect_workers` receives `_queue: &Queue` (Loco's Sidekiq queue handle) which we ignore entirely. Our background workers are spawned via `tokio::spawn` into `OutboxRelayWorkerHandle` and `BuildWorkerHandle`. The Loco worker infrastructure (Sidekiq, Redis queue) is compiled but never activated.

### 2.6 Event bus is completely our own

`rustok-events`, `rustok-outbox`, `rustok-iggy` are custom. Loco has no concept of an event bus, transactional outbox, or Iggy transport. These systems are entirely orthogonal to Loco and create zero conflict — they simply don't touch Loco. This is healthy.

---

## 3. Pain Points

### 3.1 Version pinning and transitive dependency conflict surface

- Current: `loco-rs = "0.16"` (resolves to 0.16.4)
- Loco 0.16.4 brings in: `axum 0.8`, `sea-orm 1.0`, `tokio 1.x`, `lettre` (email)
- We pin `axum = "0.8.8"` and `sea-orm = "1.0"` in the workspace — these are co-dependent with Loco's version
- `lettre` is compiled as a Loco transitive dependency even though we use `rustok-email` (our own SMTP implementation). This means email is compiled twice.
- Any Loco minor version that bumps Axum or SeaORM will require coordinated workspace updates

### 3.2 `loco_rs::prelude::*` wildcard coupling

~20 files use `use loco_rs::prelude::*`. This makes it impossible to know without reading Loco source which symbols come from Loco vs. our code. Common victims: `format::json`, `State`, `Response`. If Loco renames or removes any prelude item, compilation breaks with unhelpful errors.

### 3.3 `loco_rs::Error` variants are insufficiently typed for our use

We use: `Error::Unauthorized`, `Error::BadRequest`, `Error::InternalServerError`, `Error::Message`, `Error::string()`. The `Message` and `string()` variants carry raw strings — no structured error code, no machine-readable category. This makes consistent API error responses fragile. GraphQL error handling does its own separate error formatting, diverging from REST error format.

### 3.4 Telemetry initializer is a no-op

`initializers/telemetry.rs` does nothing: `// Telemetry is already initialized in main.rs via rustok_telemetry`. The `Initializer` abstraction adds indirection without value here. The actual init happens before `cli::main` is called.

### 3.5 Testing infra dependency on Loco internals

`tests_cfg::app::get_app_context` is an internal Loco test helper. It is used in 2 integration test modules. If Loco changes how test contexts are set up (e.g., uses a different in-memory DB), tests break invisibly.

---

## 4. What Would Break on a Loco Upgrade

| Risk area | Specific breakage | Severity |
|---|---|---|
| `Config` struct | `config.auth.jwt.secret`, `config.auth.jwt.expiration`, `config.database.uri`, `config.settings` field accesses | High — direct field access, no abstraction |
| `AppContext` struct | `ctx.db`, `ctx.config`, `ctx.shared_store` field access | High — used in ~40 files |
| `Hooks` trait | New required methods, changed signatures on `boot`/`after_routes`/`connect_workers` | High — `App` struct implements `Hooks` |
| `loco_rs::Error` variants | `Error::Unauthorized`, `Error::BadRequest`, `Error::Message`, `Error::string()` | High — used in ~30 files |
| `create_app` function | Signature change (`mode`, `environment`, `config` parameter types) | High — sole DB bootstrap path |
| `AppRoutes` / `Routes` | `.add_route()`, `.prefix()`, `.add()` API | Medium — all controllers return `Routes` |
| `loco_rs::prelude::*` | Any symbol removed/renamed | Medium — hard to predict |
| `tests_cfg::app::get_app_context` | Internal test helper change | Medium — only affects integration tests |
| `Task` / `TaskInfo` / `Vars` | Trait signature change | Low — isolated to tasks/ |
| `Initializer` trait | New required methods | Low — isolated to initializers/ |
| Transitive dep versions | Axum, SeaORM, tokio version bumps | High — cascades to entire workspace |

**Most dangerous path to Loco 0.17+:** Axum 0.8 → 0.9 would require changes to extractors, middleware signatures, and router composition across the entire server. This is the highest-impact upgrade path.

---

## 5. Recommendations by Subsystem

### Auth — KEEP OUR OWN, THIN THE BRIDGE

**Current:** `rustok-auth` does the work; `crate::auth` is a bridge converting errors.
**Recommendation:** Good separation. The bridge pattern is correct. Future work: define a `crate::server_error::ServerError` newtype instead of using `loco_rs::Error` directly, so error conversions have one place to change at upgrade time.

### Mailer — KEEP OUR OWN, REMOVE LOCO DEPENDENCY PULL

**Current:** `rustok-email` does the work; `services/email.rs` is a bridge.
**Recommendation:** The bridge is fine. Investigate disabling Loco's default `mailer` feature to stop compiling `lettre` as a transitive dep. Check `loco-rs` cargo features for `with-mailer` or similar.

### Storage — NOT USED, NO ACTION NEEDED

Loco's storage subsystem is not referenced. No action required.

### Cache — KEEP OUR OWN (`rustok-cache`)

**Current:** `rustok-cache` with moka + optional Redis is the cache layer. Loco's cache is not used.
**Recommendation:** This is clean. Continue as-is.

### Workers/Queue — REPLACE LOCO HOOK WITH DIRECT STARTUP

**Current:** `Hooks::connect_workers` is called by Loco with an unused `Queue`. Workers are spawned separately.
**Recommendation:** Keep using `tokio::spawn` for background workers. Consider moving `connect_runtime_workers` call from Loco's `connect_workers` hook into `after_routes` where the rest of the runtime is bootstrapped. The `Queue` dependency can then be dropped from the import.

### Event Bus / Outbox / Iggy — KEEP, FULLY INDEPENDENT

No Loco integration. Custom system. No action needed.

### Module Registry — KEEP, BUT CLARIFY STATE DISTRIBUTION

**Current:** `ModuleRegistry` goes through Axum's `Extension`, not `ctx.shared_store`.
**Recommendation:** This is fine but the two-track state distribution (Loco's `shared_store` vs. Axum's `Extension`) is confusing. Document the rule: Loco lifecycle state goes in `shared_store`; request-scoped middleware state goes in Axum `Extension`.

### RBAC — KEEP OUR OWN (`rustok-rbac`)

Entirely custom. Clean separation. No Loco conflict.

### Tenant isolation — KEEP OUR OWN

Custom middleware with two-layer cache and Redis pub/sub invalidation. Entirely independent of Loco. No conflict.

### Migration — KEEP VIA LOCO BOOTSTRAP

`create_app::<Self, Migrator>` runs SeaORM migrations. This is the correct integration point. The `apps/server/migration` crate is pure `sea-orm-migration` and is not Loco-specific — migrations would survive a Loco removal.

### CLI / Tasks — KEEP LOCO'S CLI

`loco_rs::cli::main` + `Task` trait is the lowest-cost Loco integration. Replacing it would require a custom CLI (clap/argh) and task runner. Not worth it unless dropping Loco entirely.

### Initializers — SIMPLIFY

The `TelemetryInitializer` is a no-op. The `SuperAdminInitializer` does real work (seed first tenant) but it uses `AppContext`. These could be merged into a single `StartupInitializer` or, better, moved into `after_routes` alongside the rest of bootstrap logic.

### Config format — ISOLATE LOCO CONFIG ACCESS

**Current:** `config.auth.jwt.secret` is read directly in `auth.rs:17-23`. This is the most fragile integration point.
**Recommendation:** Create a single `config_from_ctx(ctx: &AppContext) -> ServerConfig` function that reads all Loco config fields and returns our own type. No code outside this function should access `ctx.config.auth` or `ctx.config.database` directly. This reduces the upgrade blast radius to one file.

---

## Summary Table

| Loco subsystem | Verdict | Effort | Priority |
|---|---|---|---|
| CLI (`cli::main`) | **Keep** | — | — |
| `Hooks` lifecycle | **Keep** | — | — |
| `AppContext` / `create_app` | **Keep** | — | — |
| `loco_rs::Error` | **Wrap/Isolate** | Low | Medium |
| `loco_rs::prelude::*` | **Replace with explicit imports** | Medium | Medium |
| Config field access | **Wrap/Isolate** | Low | High |
| Auth | **Our own — keep bridge** | — | — |
| Mailer | **Our own — disable Loco feature** | Low | Low |
| Cache | **Our own — no action** | — | — |
| Workers/Queue | **Replace hook, use after_routes** | Low | Low |
| Storage | **Unused — no action** | — | — |
| Module Registry | **Keep + document** | — | — |
| Telemetry Initializer | **Remove (it's a no-op)** | Trivial | Low |
| Task system | **Keep** | — | — |
| Migrations | **Keep** | — | — |
