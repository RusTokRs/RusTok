# RusToK Core Periodic Verification Plan

**Creation date:** 2026-03-12
**Verification frequency:** with each significant PR / on schedule

> This document is a checklist for periodic verification of the platform core.
> Goal: ensure that the core maintains architectural integrity, AI agents
> have not introduced duplicate custom code, and all contracts work correctly.

---

## 1. Core Agnosticism — core doesn't know about domain modules

> [!CAUTION]
> The most common agent error is adding domain-specific code directly to server.

### 1.1 Hard-coded imports in core

```bash
# In apps/server/src/ there SHOULD NOT be direct use from domain modules
# (except ModuleRegistry / trait imports)
grep -rn "use rustok_content" apps/server/src/ --include="*.rs"
grep -rn "use rustok_commerce" apps/server/src/ --include="*.rs"
grep -rn "use rustok_blog" apps/server/src/ --include="*.rs"
grep -rn "use rustok_forum" apps/server/src/ --include="*.rs"
grep -rn "use rustok_pages" apps/server/src/ --include="*.rs"
```

**Expected result:** No results, except:
- `graphql/schema.rs` — KNOWN ISSUE until Phase 4 (dynamic registration).
- `graphql/{blog,commerce,forum,pages}/` — acceptable ONLY if module-scope GraphQL.
- `app.rs` — module registration via `ModuleRegistry::register()`.

### 1.2 `schema.rs` — check for domain coupling

```bash
grep -n "Query\|Mutation" apps/server/src/graphql/schema.rs
```

**Expected result (current/known):** product `ContentQuery`/`ContentMutation` already extracted from runtime; only domain query/mutation roots remain in schema.

### 1.3 `rustok-core` — does not contain domain logic

```bash
# core SHOULD NOT know about specific modules
grep -rn "content\|commerce\|blog\|forum\|pages" crates/rustok-core/src/ --include="*.rs"
```

**Expected result:** No matches (names in doc comments / module examples are acceptable).

---

## 2. Caching — custom implementation, not Loco Cache

### 2.1 Nobody added `loco_rs::cache`

```bash
grep -rn "loco_rs::cache\|loco_rs::prelude::cache\|CacheDriver" apps/server/src/ crates/ --include="*.rs"
```

**Expected result:** No matches. ONLY `rustok_core::CacheBackend` is used.

### 2.2 CacheBackend trait not changed without ADR

Check file `crates/rustok-core/src/context.rs` — trait `CacheBackend` must contain:
- `health()`, `get()`, `set()`, `set_with_ttl()`, `invalidate()`, `stats()`

### 2.3 FallbackCacheBackend is alive

```bash
grep -rn "FallbackCacheBackend" crates/rustok-core/src/cache.rs
```

**Expected result:** Struct + impl block exist.

### 2.4 Circuit breaker on Redis backend

```bash
grep -rn "CircuitBreaker" crates/rustok-core/src/cache.rs
```

**Expected result:** Used in `RedisCacheBackend`.

### 2.5 Anti-stampede coalescing works

```bash
grep -rn "in_flight\|get_or_load_with_coalescing" apps/server/src/middleware/tenant.rs
```

**Expected result:** Both present in `TenantCacheInfrastructure`.

---

## 3. Event Bus — Outbox, not Loco Queue

### 3.1 Nobody added Loco Queue for events

```bash
grep -rn "loco_rs::bgworker\|loco_rs::queue\|QueueProvider" apps/server/src/ --include="*.rs"
```

**Expected result:** No matches (except Hooks::connect_workers signature).

### 3.2 Outbox relay is alive and configured

```bash
grep -rn "spawn_outbox_relay_worker\|OutboxRelay" apps/server/src/ --include="*.rs"
```

**Expected result:** Called in `app.rs` / `connect_workers`.

### 3.3 EventTransport trait not changed

File `crates/rustok-core/src/events.rs` — trait `EventTransport`.

### 3.4 Transactional event bus works via outbox

```bash
grep -rn "TransactionalEventBus" apps/server/src/ --include="*.rs"
```

**Expected result:** Used in GraphQL schema and service layers.

---

## 4. Email — provider-based server infra

### 4.1 Email service exists

```bash
ls apps/server/src/services/email.rs
```

### 4.2 Provider switch and Loco Mailer adapter connected centrally

```bash
grep -rn "EmailProvider\|loco_rs::mailer\|LocoMailerAdapter\|email.provider" apps/server/src/services/email.rs apps/server/src/common/settings.rs
```

**Expected result:** Email runtime remains server-infra responsibility and supports `smtp|loco|none`; when `provider=loco` uses `ctx.mailer`, when `provider=smtp` remains compatibility path via SMTP.

### 4.3 Templates not hardcoded in send-path

```bash
grep -rn "include_str!\|mailers/auth/password_reset" apps/server/src/services/email.rs
```

**Expected result:** Auth email is rendered via file templates (`mailers/...`) and unified server email service, not via inline HTML literals in business logic.

---

## 5. Settings — YAML vs DB

### 5.1 `RustokSettings` and DB overrides exist simultaneously

```bash
grep -rn "SettingsService\|platform_settings" apps/server/src/ --include="*.rs"
```

**Expected result:** typed settings live in `settings.rustok.*`, and `SettingsService`/`platform_settings` provide per-tenant DB overrides and validation layer.

### 5.2 YAML does not duplicate DB

Check `config/development.yaml` — should contain only bootstrap defaults.

### 5.3 Module settings should not pretend to be universally implemented

```bash
# GraphQL query tenantModules should return settings != {}
curl -s http://localhost:5150/graphql -H 'Content-Type: application/json' \
  -d '{"query":"{ tenantModules { moduleslug settings } }"}' | jq '.data.tenantModules[] | select(.settings == "{}")'
```

**Expected result:** For modules where settings UI/contract are already declared as active, there should be no meaningless empty `{}`. If module settings are not yet formalized, this should be clearly reflected in their docs, not masked as completed runtime.

---

## 6. i18n — current request locale contract

### 6.1 Locale resolution lives in request context and middleware

```bash
grep -rn "RequestContext\|Accept-Language\|rustok-admin-locale\|extract_requested_locale\|resolve_locale" apps/server/src/ --include="*.rs"
```

**Expected result:** The canonical locale resolution chain (`query -> x-medusa-locale -> cookie -> Accept-Language(q-values) -> tenant.default_locale -> en`) is assembled in request context, and middleware/GraphQL use the same effective locale.

### 6.2 API errors are localized

```bash
# Check that FieldError messages are not hardcoded
grep -rn "FieldError::new(\"" apps/server/src/graphql/ --include="*.rs" | head -20
```

**Expected result:** New transport errors should not hardcode user strings directly if there is already an i18n path/translation key for them.

### 6.3 Module-owned translation bundles are established via manifest-level contract

```bash
grep -rn "leptos_locales_path\|next_messages_path\|supported_locales\|default_locale" crates/rustok-*/rustok-module.toml
```

**Expected result:** If a module declares `[provides.*_ui.i18n]`, the manifest must describe `supported_locales`, `default_locale` and bundle paths. `ManifestManager` validates the existence/shape of this contract, and docs should not describe the layer again as an undefined trait/WIP.

### 6.4 UI locale bundles parity is verified mechanically

```bash
npm run verify:i18n:ui
```

**Expected result:** `locales/*.json` and `messages/*.json` in host apps and module-owned UI packages pass key parity check. Manifest-level existence contract and parity contract should live together: the first is caught by `ManifestManager`, the second by verifier script.

### 6.5 Password reset email uses effective locale regardless of transport/provider

```bash
grep -rn "email_service_from_ctx\|request_context.locale\|locale_from_ctx" apps/server/src/controllers apps/server/src/graphql apps/server/src/services/email.rs --include="*.rs"
```

**Expected result:** GraphQL and REST password-reset paths pass effective locale to `email_service_from_ctx(...)`, and `smtp` and `loco` providers render the same localized built-in template path instead of hardcoded English-only SMTP body.

---

## 7. RBAC — unified tenant policy runtime

### 7.1 Server uses modular `rustok-rbac` runtime

```bash
grep -rn "RbacService::has_permission\|RbacService::has_any_permission" apps/server/src/ --include="*.rs"
```

**Expected result:** Checks go through `RbacService`/common `rustok-rbac` helpers, not through local policy engine in `apps/server`.

Check that server wiring has not returned local authorization semantics:

```bash
grep -rn "RuntimePermissionResolver\|authorize_permission\|authorize_any_permission\|authorize_all_permissions" apps/server/src/services/ --include="*.rs"
```

**Expected result:** `apps/server` uses resolver/adapters from `rustok-rbac`, and decision path relies on modular tenant policy runtime.

### 7.2 No legacy rollout branches and duplicate auth middleware

```bash
grep -rn "fn check_permission\|fn verify_role\|relation_only\|policy_shadow\|mismatch\|RUSTOK_RBAC_AUTHZ_MODE" apps/server/src/ --include="*.rs"
```

**Expected result:** No adhoc checks bypassing `RbacService` and no return to old rollout/shadow branches in live server runtime.

---

## 8. Tenant Resolution — infrastructure

### 8.1 Tenant middleware works

```bash
grep -rn "TenantCacheInfrastructure\|init_tenant_cache_infrastructure" apps/server/src/ --include="*.rs"
```

### 8.2 Redis pub/sub invalidation listener

```bash
grep -rn "TENANT_INVALIDATION_CHANNEL\|spawn_invalidation_listener" apps/server/src/ --include="*.rs"
```

### 8.3 Negative cache exists

```bash
grep -rn "negative_cache\|set_negative\|check_negative" apps/server/src/middleware/tenant.rs
```

---

## 9. Loco Integration — proper Framework usage

### 9.1 All routes via Hooks::routes / after_routes

```bash
# There should be no standalone Router::new() without Loco integration
grep -rn "axum::Router::new()" apps/server/src/ --include="*.rs" | grep -v "test"
```

**Expected result:** None (or only in modular sub-routers connected via Loco).

### 9.2 AppContext via State, not global variables

```bash
grep -rn "lazy_static\|static.*OnceCell\|static.*Mutex" apps/server/src/ --include="*.rs"
```

**Expected result:** No runtime state bypassing `AppContext.shared_store`.

### 9.3 Errors via loco_rs::Result

```bash
# Handlers should return loco Result, not axum IntoResponse directly
grep -rn "impl IntoResponse" apps/server/src/controllers/ --include="*.rs"
```

**Expected result:** No custom IntoResponse in controllers.

---

## 10. Module system — integrity

### 10.1 ModuleRegistry contains all registered modules

```bash
grep -rn "ModuleRegistry::new()\|\.register(" apps/server/src/modules/ apps/server/src/services/module_lifecycle.rs
```

### 10.2 Module lifecycle hooks are called

```bash
grep -rn "on_enable\|on_disable" apps/server/src/services/module_lifecycle.rs
```

### 10.3 `modules.toml` manifest is valid

```bash
# Check that all modules from modules.toml have corresponding crates
cat modules.toml
```

---

## 11. Storage — shared `rustok-storage` / `rustok-media` contract

> `rustok-storage` = shared storage backend/service layer. `rustok-media` = core domain module on top of storage. Server only initializes shared runtime and passes it to consumers.

### 11.1 Shared storage service initialized in app runtime

```bash
grep -rn "StorageService\|init_storage" apps/server/src/ --include="*.rs"
```

**Expected result:** `StorageService` is created once in runtime bootstrap and used as shared dependency, not as adhoc storage client at call site.

### 11.2 Controllers don't pull storage backend directly

```bash
# Direct backend wiring in controllers is an anti-pattern
grep -rn "loco_rs::storage\|StorageService::from_config" apps/server/src/controllers/ --include="*.rs"
```

**Expected result:** Backend wiring remains in runtime/bootstrap layer; controllers/graphql/services use already-prepared shared storage contract.

### 11.3 Files organized by date and tenant

```bash
# Check that storage is used together with media/runtime cleanup flows
grep -rn "media cleanup\|storage_path\|MediaService" crates/rustok-media/src/ crates/rustok-media/cli/src/ --include="*.rs"
```

### 11.4 media_assets table exists

```bash
grep -rn "media_assets" apps/server/src/models/ migration/ --include="*.rs"
```

### 11.5 No ad-hoc upload bypassing StorageAdapter

```bash
grep -rn "multipart\|tokio::fs::write\|std::fs::write" apps/server/src/controllers/ --include="*.rs"
```

**Expected result:** None — all via `StorageAdapter`.

---

## 12. Observability — telemetry and health

### 12.1 Telemetry initializer

```bash
ls apps/server/src/initializers/telemetry.rs
```

### 12.2 Health endpoint works

```bash
curl -s http://localhost:5150/api/_health | jq .
```

### 12.3 Metrics endpoint

```bash
curl -s http://localhost:5150/api/_metrics | head -20
```

---

## 13. Anti-patterns — what SHOULD NOT appear

| Anti-pattern | How to detect | Severity |
|---|---|---|
| Loco Cache instead of CacheBackend | `grep "loco_rs::cache"` | 🔴 Critical |
| Loco Queue instead of Outbox | `grep "loco_rs::bgworker" \| grep "QueueProvider"` | 🔴 Critical |
| Domain imports in core crate | `grep "content\|commerce" crates/rustok-core/` | 🔴 Critical |
| Static globals bypassing AppContext | `grep "lazy_static\|OnceCell"` | 🟡 High |
| Inline SQL instead of SeaORM | `grep "raw_sql\|execute_unprepared"` in new code | 🟡 High |
| New HTTP client instead of Loco fetch | `grep "reqwest::Client::new()"` in core | 🟢 Medium |
| Custom auth middleware bypassing RbacService | Manual `fn check_role` | 🟡 High |
| Hard-coded tenant ID in business logic | `grep "00000000-0000-0000-0000"` in non-config files | 🟡 High |

---

## How to conduct verification

1. **Automated:** Add checks from §§1–11 to CI as lint step (grep-based). Fail on match.
2. **Manual (periodic):** Go through checklist manually once per sprint / on major PR.
3. **Agent pre-commit:** AI agents must check against this document before any changes in `apps/server/` or `crates/rustok-core/`.

> **Rule for agents:** If you are going to add a new dependency, middleware, or
> infrastructure service to `apps/server/` — **first** check this document
> to see if it duplicates an already existing solution.
