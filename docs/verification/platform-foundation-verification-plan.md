# РџР»Р°РЅ РІРµСЂРёС„РёРєР°С†РёРё РїР»Р°С‚С„РѕСЂРјС‹: foundation

- **РЎС‚Р°С‚СѓСЃ:** РђРєС‚СѓР°Р»РёР·РёСЂРѕРІР°РЅРЅС‹Р№ РґРµС‚Р°Р»СЊРЅС‹Р№ С‡РµРєР»РёСЃС‚
- **РљРѕРЅС‚СѓСЂ:** РЎР±РѕСЂРєР°, workspace, Р°СЂС…РёС‚РµРєС‚СѓСЂРЅР°СЏ РєРѕРЅСЃРёСЃС‚РµРЅС‚РЅРѕСЃС‚СЊ, СЏРґСЂРѕ РїР»Р°С‚С„РѕСЂРјС‹, auth, RBAC, tenancy
- **Companion-РїР»Р°РЅ:** [Rolling-РїР»Р°РЅ RBAC РґР»СЏ server Рё runtime-РјРѕРґСѓР»РµР№](./rbac-server-modules-verification-plan.md)

---

## Р¤Р°Р·Р° 0: РљРѕРјРїРёР»СЏС†РёСЏ Рё СЃР±РѕСЂРєР°

### 0.1 Workspace Рё toolchain

- [ ] `cargo check --workspace` РїСЂРѕС…РѕРґРёС‚ РІ Р°РєС‚СѓР°Р»СЊРЅРѕР№ СЃСЂРµРґРµ.
- [ ] `cargo check --workspace --all-features` РїСЂРѕС…РѕРґРёС‚ Р±РµР· feature-drift.
- [ ] `cargo clippy --workspace --all-features -- -D warnings` РїСЂРѕС…РѕРґРёС‚ РёР»Рё known-failures Р·Р°РґРѕРєСѓРјРµРЅС‚РёСЂРѕРІР°РЅС‹ РѕС‚РґРµР»СЊРЅРѕ.
- [ ] `cargo fmt --all -- --check` РїСЂРѕС…РѕРґРёС‚.
- [ ] Р”Р»СЏ Leptos-С‡Р°СЃС‚Рё РїРѕРґС‚РІРµСЂР¶РґРµРЅС‹ Rust/WASM/Trunk prerequisites.
- [ ] Р”Р»СЏ Next.js-С‡Р°СЃС‚Рё РїРѕРґС‚РІРµСЂР¶РґРµРЅС‹ Node/npm prerequisites.

### 0.2 РћСЃРЅРѕРІРЅС‹Рµ РїР°РєРµС‚С‹ СЃР±РѕСЂРєРё

- [ ] `cargo build -p rustok-server`
- [ ] `cargo build -p rustok-admin`
- [ ] `cargo build -p rustok-storefront`
- [ ] `cargo build -p xtask`
- [ ] `cargo build -p benches`

### 0.3 РљР»СЋС‡РµРІС‹Рµ crate-С‹ РїР»Р°С‚С„РѕСЂРјС‹

- [ ] `rustok-core`
- [ ] `rustok-auth`
- [ ] `rustok-cache`
- [ ] `rustok-email`
- [ ] `rustok-events`
- [ ] `rustok-outbox`
- [ ] `rustok-index`
- [ ] `rustok-tenant`
- [ ] `rustok-rbac`
- [ ] `rustok-content`
- [ ] `rustok-commerce`
- [ ] `rustok-blog`
- [ ] `rustok-forum`
- [ ] `rustok-pages`
- [ ] `rustok-media`
- [ ] `rustok-workflow`
- [ ] `rustok-api`
- [ ] `rustok-storage`
- [ ] `rustok-telemetry`
- [ ] `rustok-iggy`
- [ ] `rustok-iggy-connector`
- [ ] `rustok-mcp`
- [ ] `rustok-test-utils`
- [ ] `alloy`
- [ ] `alloy`
- [ ] `flex`

### 0.4 Frontend Рё UI workspace

- [ ] `apps/next-admin`: `npm run build`
- [ ] `apps/next-admin`: `npm run lint`
- [ ] `apps/next-frontend`: `npm run build`
- [ ] `apps/next-frontend`: `npm run lint`
- [ ] `apps/next-frontend`: `npm run typecheck`
- [ ] `UI/leptos`: cargo build РїСЂРѕС…РѕРґРёС‚

### 0.5 Р’СЃРїРѕРјРѕРіР°С‚РµР»СЊРЅС‹Рµ РёРЅСЃС‚СЂСѓРјРµРЅС‚С‹ Рё РѕРєСЂСѓР¶РµРЅРёРµ

- [ ] `make help` РѕС‚СЂР°Р¶Р°РµС‚ Р°РєС‚СѓР°Р»СЊРЅС‹Рµ targets.
- [ ] `docker compose config` РґР»СЏ `docker-compose.yml` РїСЂРѕС…РѕРґРёС‚.
- [ ] `docker compose -f docker-compose.yml -f docker-compose.full-dev.yml config` РїСЂРѕС…РѕРґРёС‚.
- [ ] `docker compose -f docker-compose.observability.yml config` РїСЂРѕС…РѕРґРёС‚.

---

## Р¤Р°Р·Р° 1: РђСЂС…РёС‚РµРєС‚СѓСЂРЅР°СЏ РєРѕРЅСЃРёСЃС‚РµРЅС‚РЅРѕСЃС‚СЊ

### 1.1 `modules.toml` Рё runtime registry

**Р¤Р°Р№Р»С‹:**
- `modules.toml`
- `apps/server/src/modules/mod.rs`
- `apps/server/src/modules/manifest.rs`

- [ ] РџРѕРґС‚РІРµСЂР¶РґРµРЅРѕ, С‡С‚Рѕ `validate_registry_vs_manifest()` РІС‹Р·С‹РІР°РµС‚СЃСЏ РїСЂРё СЃС‚Р°СЂС‚Рµ СЃРµСЂРІРµСЂР°.
- [ ] Runtime-РјРѕРґСѓР»Рё СЃРѕРІРїР°РґР°СЋС‚ РјРµР¶РґСѓ manifest Рё `build_registry()`:
  - Core: `auth`, `cache`, `email`, `index`, `outbox`, `tenant`, `rbac`
  - Optional: `content`, `commerce`, `blog`, `forum`, `pages`, `media`, `workflow`
- [ ] РџРѕРґС‚РІРµСЂР¶РґРµРЅРѕ, С‡С‚Рѕ `rustok-outbox` Р·Р°СЂРµРіРёСЃС‚СЂРёСЂРѕРІР°РЅ РєР°Рє РѕР±С‹С‡РЅС‹Р№ `Core` module Рё РѕРґРЅРѕРІСЂРµРјРµРЅРЅРѕ РёСЃРїРѕР»СЊР·СѓРµС‚СЃСЏ СЃРµСЂРІРµСЂРѕРј РІ event runtime bootstrap.
- [ ] `required = true` СЃРѕРІРїР°РґР°РµС‚ СЃ `ModuleKind::Core` РґР»СЏ platform modules.
- [ ] `depends_on` РІ manifest СЃРѕРІРїР°РґР°РµС‚ СЃ `dependencies()` РІ `RusToKModule` impl.

### 1.2 Workspace members

- [ ] `Cargo.toml` workspace members РїРѕРєСЂС‹РІР°СЋС‚ `apps/server`, `apps/admin`, `apps/storefront`, `crates/*`, `UI/leptos`, `benches`, `xtask`.
- [ ] Р’ workspace РѕС‚СЂР°Р¶РµРЅС‹ crate-owned UI packages, РєРѕС‚РѕСЂС‹Рµ РґРµР№СЃС‚РІРёС‚РµР»СЊРЅРѕ РґРѕР»Р¶РЅС‹ СЃРѕР±РёСЂР°С‚СЊСЃСЏ РІРјРµСЃС‚Рµ СЃ workspace.
- [ ] РќРµС‚ orphan-crate РґРёСЂРµРєС‚РѕСЂРёР№ СЃ `Cargo.toml`, РІС‹РїР°РІС€РёС… РёР· workspace.

### 1.3 РўРµРєСѓС‰Р°СЏ taxonomy РєРѕРјРїРѕРЅРµРЅС‚РѕРІ

- [ ] РџСЂРѕРІРµСЂРµРЅРѕ, С‡С‚Рѕ shared library / support crate layer РІРЅРµ module taxonomy РІРєР»СЋС‡Р°РµС‚:
  - `rustok-core`, `rustok-events`, `rustok-telemetry`, `rustok-api`, `rustok-storage`, `rustok-test-utils`, `rustok-iggy`, `rustok-iggy-connector`, `rustok-mcp`, `alloy`, `alloy`, `flex`
- [ ] РџСЂРѕРІРµСЂРµРЅРѕ, С‡С‚Рѕ runtime core-РјРѕРґСѓР»Рё СЃ `ModuleKind::Core` СЃРµР№С‡Р°СЃ: `auth`, `cache`, `email`, `index`, `outbox`, `tenant`, `rbac`.
- [ ] РџСЂРѕРІРµСЂРµРЅРѕ, С‡С‚Рѕ runtime optional-РјРѕРґСѓР»Рё СЃРµР№С‡Р°СЃ: `content`, `commerce`, `blog`, `forum`, `pages`, `media`, `workflow`.
- [ ] РџСЂРѕРІРµСЂРµРЅРѕ, С‡С‚Рѕ Alloy РѕСЃС‚Р°С‘С‚СЃСЏ capability layer, Р° РЅРµ tenant-toggle runtime module.

### 1.4 Р—Р°РІРёСЃРёРјРѕСЃС‚Рё РјРµР¶РґСѓ runtime-РјРѕРґСѓР»СЏРјРё

- [ ] `blog -> content`
- [ ] `forum -> content`
- [ ] `pages -> content`
- [ ] `media` РЅРµ РІРІРѕРґРёС‚ СЃРєСЂС‹С‚С‹С… runtime dependencies
- [ ] `workflow` РЅРµ РІРІРѕРґРёС‚ СЃРєСЂС‹С‚С‹С… runtime dependencies
- [ ] Р’ registry-managed РіСЂР°С„Рµ РЅРµС‚ С†РёРєР»РёС‡РµСЃРєРёС… Р·Р°РІРёСЃРёРјРѕСЃС‚РµР№.

---

## Р¤Р°Р·Р° 2: РЇРґСЂРѕ РїР»Р°С‚С„РѕСЂРјС‹

### 2.1 `rustok-core`

- [ ] `RusToKModule`, `ModuleKind`, `MigrationSource`, `ModuleRegistry` РѕС‚СЂР°Р¶Р°СЋС‚ С‚РµРєСѓС‰РёР№ runtime contract.
- [ ] `Permission`, `Resource`, `Action` РїРѕРєСЂС‹РІР°СЋС‚ Р°РєС‚СѓР°Р»СЊРЅС‹Рµ РїРѕРІРµСЂС…РЅРѕСЃС‚Рё: content, commerce, blog, forum, pages, media, scripts, MCP, workflow, flex, users/modules/settings.
- [ ] `SecurityContext` Рё scope-РјРѕРґРµР»СЊ СЃРѕРѕС‚РІРµС‚СЃС‚РІСѓСЋС‚ С‚РµРєСѓС‰РёРј server-side usage patterns.
- [ ] Cache abstractions (`InMemory`, `Redis`, fallback) СЃРѕРІРїР°РґР°СЋС‚ СЃ СЂРµР°Р»СЊРЅС‹Рј РёСЃРїРѕР»СЊР·РѕРІР°РЅРёРµРј РІ server runtime.
- [ ] Error taxonomy Рё `ErrorResponse` СЃРѕРІРїР°РґР°СЋС‚ СЃ transport-layer mapping.

### 2.2 `rustok-events` Рё `rustok-outbox`

- [ ] `DomainEvent` / `EventEnvelope` РѕСЃС‚Р°СЋС‚СЃСЏ РєР°РЅРѕРЅРёС‡РµСЃРєРёРј event contract.
- [ ] `TransactionalEventBus` Рё transport abstraction СЃРѕРѕС‚РІРµС‚СЃС‚РІСѓСЋС‚ С‚РµРєСѓС‰РµРјСѓ write-path.
- [ ] `OutboxRelay` Рё `sys_events` flow СЃРѕРѕС‚РІРµС‚СЃС‚РІСѓСЋС‚ С‚РµРєСѓС‰РµРјСѓ server runtime.
- [ ] `rustok-outbox` РѕСЃС‚Р°С‘С‚СЃСЏ `Core` module Рё РЅРµ СѓС‡Р°СЃС‚РІСѓРµС‚ РІ tenant toggle flow РєР°Рє `Optional` module.

### 2.3 `rustok-telemetry`

- [ ] Telemetry config, tracing subscriber Рё OTEL wiring СЃРѕРѕС‚РІРµС‚СЃС‚РІСѓСЋС‚ С‚РµРєСѓС‰РµРјСѓ server bootstrap.
- [ ] Metrics API РѕС‚СЂР°Р¶Р°РµС‚ СЂРµР°Р»СЊРЅС‹Рµ РјРµС‚СЂРёРєРё, РїСѓР±Р»РёРєСѓРµРјС‹Рµ СЃРµСЂРІРµСЂРѕРј.
- [ ] Shutdown semantics Р°РєС‚СѓР°Р»СЊРЅС‹ РґР»СЏ С‚РµРєСѓС‰РµРіРѕ `apps/server/src/main.rs`.

### 2.4 `rustok-api` Рё shared host contracts

- [ ] `rustok-api` РѕСЃС‚Р°С‘С‚СЃСЏ С‚РѕРЅРєРёРј shared host/API layer РґР»СЏ tenant/auth/request contracts.
- [ ] `TenantContext`, `AuthContext`, extension traits Рё error model РёСЃРїРѕР»СЊР·СѓСЋС‚СЃСЏ СЃРµСЂРІРµСЂРѕРј Рё host-РїСЂРёР»РѕР¶РµРЅРёСЏРјРё РєРѕРЅСЃРёСЃС‚РµРЅС‚РЅРѕ.

---

## Р¤Р°Р·Р° 3: РђРІС‚РѕСЂРёР·Р°С†РёСЏ Рё Р°СѓС‚РµРЅС‚РёС„РёРєР°С†РёСЏ

### 3.1 Auth surfaces

**Р¤Р°Р№Р»С‹:**
- `crates/rustok-auth/`
- `apps/server/src/controllers/auth.rs`
- `apps/server/src/graphql/auth/`
- `apps/server/src/services/auth_lifecycle.rs`

- [ ] REST Рё GraphQL auth surfaces РїСЂРѕС…РѕРґСЏС‚ С‡РµСЂРµР· РµРґРёРЅС‹Р№ lifecycle/service layer.
- [ ] Р’ РєРѕРґРµ РїРѕРєСЂС‹С‚С‹: sign up, sign in, refresh, logout, password reset, change password, profile update, sessions.
- [ ] Session invalidation Рё password-change invalidation СЃРѕРѕС‚РІРµС‚СЃС‚РІСѓСЋС‚ С‚РµРєСѓС‰РµР№ СЃС…РµРјРµ С‚Р°Р±Р»РёС†/claims.
- [ ] Email verification Рё invite acceptance РѕС‚СЂР°Р¶РµРЅС‹ РєР°Рє РІ РєРѕРґРµ, С‚Р°Рє Рё РІ РїР»Р°РЅРµ.

### 3.2 JWT Рё session contract

- [ ] Claims СЃРѕРґРµСЂР¶Р°С‚ С‚РѕР»СЊРєРѕ СЂРµР°Р»СЊРЅРѕ РёСЃРїРѕР»СЊР·СѓРµРјС‹Рµ РїРѕР»СЏ.
- [ ] Bearer extraction Рё session revocation СЂР°Р±РѕС‚Р°СЋС‚ С‡РµСЂРµР· С‚РµРєСѓС‰РёРµ extractors/services.
- [ ] Browser/API auth flows РЅРµ СЂР°СЃС…РѕРґСЏС‚СЃСЏ РїРѕ С„РѕСЂРјР°С‚Сѓ С‚РѕРєРµРЅРѕРІ.

### 3.3 Password and identity guarantees

- [ ] Password hashing Рё verification СЃРѕРѕС‚РІРµС‚СЃС‚РІСѓСЋС‚ С‚РµРєСѓС‰РµРјСѓ `rustok-core`/`rustok-auth` contract.
- [ ] User/tenant binding РІ auth flows СЃРѕРѕС‚РІРµС‚СЃС‚РІСѓРµС‚ С‚РµРєСѓС‰РµР№ multi-tenant РјРѕРґРµР»Рё.

---

## Р¤Р°Р·Р° 4: RBAC

### 4.1 Typed permission surface

**Р¤Р°Р№Р»С‹:**
- `crates/rustok-core/src/permissions.rs`
- `crates/rustok-core/src/rbac.rs`
- `apps/server/src/extractors/rbac.rs`

- [ ] Typed permissions РїРѕРєСЂС‹РІР°СЋС‚ С‚РµРєСѓС‰РёРµ СЂРµСЃСѓСЂСЃС‹: users, tenants, modules, settings, flex, products, orders, pages, nodes, media, blog/forum, scripts, MCP, workflows.
- [ ] Р’ `extractors/rbac.rs` РµСЃС‚СЊ extractors РґР»СЏ СЂРµР°Р»СЊРЅРѕ РёСЃРїРѕР»СЊР·СѓРµРјС‹С… REST surfaces.
- [ ] Flex, MCP, scripts/alloy Рё workflow РёСЃРїРѕР»СЊР·СѓСЋС‚ typed permissions, Р° РЅРµ Р»РѕРєР°Р»СЊРЅС‹Рµ string aliases.

### 4.2 Server-side enforcement

- [ ] REST handlers РЅРµ РёСЃРїРѕР»СЊР·СѓСЋС‚ `CurrentUser` РєР°Рє Р·Р°РјРµРЅСѓ permission checks.
- [ ] GraphQL mutations/queries РёСЃРїРѕР»СЊР·СѓСЋС‚ `RbacService` РёР»Рё permission-aware guards.
- [ ] `infer_user_role_from_permissions()` РЅРµ РёСЃРїРѕР»СЊР·СѓРµС‚СЃСЏ РєР°Рє Р·Р°РјРµРЅР° Р°РІС‚РѕСЂРёР·Р°С†РёРё.
- [ ] РќРµС‚ hardcoded `UserRole::Admin` / `UserRole::SuperAdmin` РІ РєСЂРёС‚РёС‡РЅС‹С… authorization path.

### 4.3 Runtime-module permission ownership

- [ ] `permissions()` Сѓ runtime-РјРѕРґСѓР»РµР№ СЃРѕРѕС‚РІРµС‚СЃС‚РІСѓРµС‚ СЂРµР°Р»СЊРЅС‹Рј server callsites.
- [ ] README runtime-РјРѕРґСѓР»РµР№ СЃРѕРґРµСЂР¶РёС‚ `## Purpose`, `## Responsibilities`, `## Entry points`, `## Interactions`, СЃСЃС‹Р»РєСѓ РЅР° `docs/README.md` Рё permission surface.
- [ ] `pages`, `media`, `workflow` РѕС‚СЂР°Р¶РµРЅС‹ РІ RBAC vocabulary Рё server enforcement.

---

## Р¤Р°Р·Р° 5: Multi-tenancy Рё module lifecycle

### 5.1 Tenant resolution Рё context propagation

- [ ] `TenantContext` РёРјРїРѕСЂС‚РёСЂСѓРµС‚СЃСЏ РёР· `rustok-api` Рё РёСЃРїРѕР»СЊР·СѓРµС‚СЃСЏ СЃРµСЂРІРµСЂРѕРј РєРѕРЅСЃРёСЃС‚РµРЅС‚РЅРѕ.
- [ ] Tenant resolution middleware/guards СЃРѕРѕС‚РІРµС‚СЃС‚РІСѓСЋС‚ С‚РµРєСѓС‰РµРјСѓ runtime.
- [ ] Hostname/header-based resolution РЅРµ СЂР°СЃС…РѕРґРёС‚СЃСЏ СЃ РґРѕРєСѓРјРµРЅС‚Р°С†РёРµР№.

### 5.2 Tenant cache

- [ ] Positive/negative cache, stampede protection Рё Redis invalidation СЃРѕРѕС‚РІРµС‚СЃС‚РІСѓСЋС‚ С‚РµРєСѓС‰РµР№ СЂРµР°Р»РёР·Р°С†РёРё.
- [ ] Cache metrics Рё invalidation channels РѕС‚СЂР°Р¶Р°СЋС‚ С‚РµРєСѓС‰РёР№ РєРѕРґ.

### 5.3 Tenant data isolation

- [ ] Domain tables Рё services РїРѕ-РїСЂРµР¶РЅРµРјСѓ tenant-scoped С‚Р°Рј, РіРґРµ СЌС‚Рѕ С‚СЂРµР±СѓРµС‚СЃСЏ.
- [ ] `tenant_modules` РѕС‚СЂР°Р¶Р°РµС‚ С‚РµРєСѓС‰СѓСЋ runtime toggle-РјРѕРґРµР»СЊ.
- [ ] РћС‚РєР»СЋС‡РµРЅРёРµ core-РјРѕРґСѓР»РµР№ РїРѕ-РїСЂРµР¶РЅРµРјСѓ Р·Р°РїСЂРµС‰РµРЅРѕ С‡РµСЂРµР· `registry.is_core()`.

### 5.4 Module lifecycle

- [ ] `ModuleLifecycleService` СЃРѕРіР»Р°СЃРѕРІР°РЅ СЃ `ModuleRegistry`, manifest Рё build pipeline.
- [ ] Enable/disable РїСЂРѕРІРµСЂСЏРµС‚ dependencies Рё dependents РґР»СЏ runtime optional-РјРѕРґСѓР»РµР№.
- [ ] Build/deployment manifest flow РЅРµ СЂР°СЃС…РѕРґРёС‚СЃСЏ СЃ tenant module lifecycle.

