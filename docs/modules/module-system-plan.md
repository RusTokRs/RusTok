# RusTok вЂ” РЎРёСЃС‚РµРјР° РјРѕРґСѓР»РµР№: РїРѕР»РЅР°СЏ РєР°СЂС‚Р° Рё РїР»Р°РЅ

> **Р”Р°С‚Р°**: 2026-03-19
> **Актуализировано**: 2026-04-03
> **РќР°Р·РЅР°С‡РµРЅРёРµ**: РїРѕР»РЅР°СЏ РєР°СЂС‚Р° СЂРµР°Р»РёР·Р°С†РёРё вЂ” С‡С‚Рѕ СЃРґРµР»Р°РЅРѕ, РіРґРµ РґРѕРєСѓРјРµРЅС‚РёСЂРѕРІР°РЅРѕ,
> С‡С‚Рѕ РѕСЃС‚Р°Р»РѕСЃСЊ. РЎР»СѓР¶РёС‚ РѕСЃРЅРѕРІРѕР№ РґР»СЏ РїРµСЂРёРѕРґРёС‡РµСЃРєРѕР№ РІРµСЂРёС„РёРєР°С†РёРё РєРѕСЂСЂРµРєС‚РЅРѕСЃС‚Рё.
>
> Р›РµРіРµРЅРґР°: вњ… СЂРµР°Р»РёР·РѕРІР°РЅРѕ В· вљ пёЏ С‡Р°СЃС‚РёС‡РЅРѕ В· в¬њ РЅРµ РЅР°С‡Р°С‚Рѕ

---

## РЎРѕРґРµСЂР¶Р°РЅРёРµ

1. [РЎС‚Р°РЅРґР°СЂС‚ РјРѕРґСѓР»СЏ](#1-СЃС‚Р°РЅРґР°СЂС‚-РјРѕРґСѓР»СЏ)
2. [Tenant-level toggle](#2-tenant-level-toggle)
3. [Platform-level install/uninstall](#3-platform-level-installuninstall)
4. [Build pipeline](#4-build-pipeline)
5. [Marketplace РєР°С‚Р°Р»РѕРі](#5-marketplace-РєР°С‚Р°Р»РѕРі)
6. [Admin UI](#6-admin-ui)
7. [Р’РЅРµС€РЅРёР№ СЂРµРµСЃС‚СЂ Рё РїСѓР±Р»РёРєР°С†РёСЏ](#7-РІРЅРµС€РЅРёР№-СЂРµРµСЃС‚СЂ-Рё-РїСѓР±Р»РёРєР°С†РёСЏ)
8. [РђСЂС…РёС‚РµРєС‚СѓСЂРЅС‹Р№ РґРѕР»Рі](#8-Р°СЂС…РёС‚РµРєС‚СѓСЂРЅС‹Р№-РґРѕР»Рі)
9. [РџСЂРёРѕСЂРёС‚РµС‚ РЅРµР·Р°РІРµСЂС€С‘РЅРЅРѕРіРѕ](#9-РїСЂРёРѕСЂРёС‚РµС‚-РЅРµР·Р°РІРµСЂС€С‘РЅРЅРѕРіРѕ)

---

## Статус на 2026-04-03

- ✅ `rustok-api` уже существует как foundation-слой и подключён к модульным crate-ам; это больше не «будущий план».
- ✅ Основные GraphQL/REST адаптеры уже вынесены из `apps/server` в crate-ы модулей, а `apps/server` удерживает роль composition root.
- ✅ Build/release pipeline теперь исполняет полный manifest-derived план: `cargo build` для `apps/server`, `trunk build` для `apps/admin` и `cargo build -p rustok-storefront` для Leptos storefront; filesystem/container backend публикуют реальные `server`/`admin`/`storefront` артефакты и заполняют отдельные artifact URLs, а `container` дополнительно поддерживает generic rollout hook без знания о конкретном orchestrator.
- ✅ `ManifestManager` теперь валидирует не только metadata path-модулей и admin-surface конфликты, но и semver-диапазоны зависимостей, product/runtime-конфликты модулей и ошибки schema-driven module settings contract.
- ✅ `updateModuleSettings` теперь закрыт end-to-end: GraphQL mutation и `/modules` в Leptos Admin валидируют payload по `[settings]` из `rustok-module.toml`, применяют `default`, рендерят typed controls для scalar-полей, top-level structured editor и deep nested editor с create-actions / key rename / array reorder для `object` / `array`, а recursive `shape` (`properties` / `items`) уже направляет schema-driven add actions и schema-locked object keys не только наверху, но и внутри nested JSON tree; nested `object` / `array` children уже редактируются inline в structured editors, а nested scalar children начинают уважать child `type`, `options`, `min` и `max`. Per-field JSON editors с helper-actions (`Format JSON`, `Reset`, `Add property/item`) остаются для complex values и schema-less fallback.
- ✅ `buildProgress` теперь работает end-to-end: `apps/server` поднимает GraphQL WS transport на `/api/graphql/ws`, а `/modules` в `apps/admin` подписывается на live progress и держит polling только как fallback.
- ⚠️ Read-only registry V1 теперь имеет референсную server-side реализацию: `apps/server` публикует schema-versioned list/detail contract `GET /v1/catalog` и `GET /v1/catalog/{slug}` (с legacy aliases под `GET /catalog`), list endpoint уже поддерживает optional query filters `search`, `category`, `tag` и page-window `limit` / `offset`, а также `X-Total-Count`, а итоговая выдача стабильно сортируется по `slug`, payload уже несёт module `name` / `description`, manifest-driven `category` / `tags` и optional visual metadata (`icon`, `banner`, `screenshots`) для registry-only UX, endpoints включены в host OpenAPI как часть внешнего API-контракта, причём cache/paging headers теперь тоже задокументированы в спецификации, endpoints отдают `ETag` + `Cache-Control` и уважают `If-None-Match` / `304 Not Modified`, `RegistryMarketplaceProvider` ходит в V1 list-path и откатывается на legacy path только как backward-compatible fallback, single-module consumer path (`MarketplaceCatalogService::get_module` / GraphQL `marketplaceModule`) уже использует detail-aware lookup вместо обязательной полной загрузки каталога, а list consumer path теперь тоже умеет прокидывать shared catalog query в provider chain, чтобы registry-backed discovery не тянул полный каталог без необходимости. GraphQL `marketplace` и `/modules` filter bar теперь тоже экспонируют `tag`, так что provider-side narrowing доступен не только HTTP registry contract, но и операторскому UI. Publish metadata на read-only host тоже стала строже: normalization теперь подрезает пустой `publisher`, не пропускает невалидный `checksum_sha256` в GraphQL/Admin UI как ложный publish signal, canonicalizes valid `published_at` to UTC RFC3339 и стабильно сортирует version trail, а GraphQL `latest_version` теперь может падать обратно на этот trail, если top-level `version` в registry payload отсутствует. Поверх этого `apps/server` уже умеет работать в `settings.rustok.runtime.host_mode = "registry_only"` как выделенный read-only catalog host: остаются только `health` / `metrics` / `swagger` / registry routes, deployment surface принудительно становится `HeadlessApi`, background workers не поднимаются, optional module routes не добавляются, а GraphQL/auth/MCP/admin/storefront shell не экспонируются; `/health/ready` при этом уже выровнен под этот reduced surface и не требует event/search/module runtime, которых у dedicated catalog host нет по определению, generated OpenAPI в этом режиме теперь тоже фильтруется до реально доступного read-only host surface, `/health/runtime` и Prometheus runtime-guardrail metrics теперь явно несут `host_mode` / `runtime_dependencies_enabled`, reduced router теперь корректно прокидывает `ModuleRegistry`, так что `/health/ready` и `/health/modules` не падают на missing extension, а deployment path можно активировать не только через config, но и через env override `RUSTOK_RUNTIME_HOST_MODE=registry_only`. Локальный operator path для этого режима теперь зафиксирован в `apps/server/docs/health.md`, а profile smoke matrix дополнительно покрывает `registry_only` через `scripts/verify/verify-deployment-profiles.sh`. Отдельный `modules.rustok.dev` deployment ещё не выделен, но V2 write-path уже перестал быть чистым scaffold: full host теперь умеет создавать publish request, отдавать `request_id`/status, принимать artifact upload, переводить request через реальные `artifact_uploaded -> submitted -> validating -> approved` или `rejected` состояния, валидировать uploaded JSON bundle против publish request и TOML manifest-ов (`rustok-module.toml`, crate/UI `Cargo.toml`), а затем отдельным `approve`/`reject` lifecycle-решением либо проецировать release в V1 trail, либо фиксировать governance reason. Первый cut ownership/policy тоже уже включён: live routes уважают header-driven actor contract (`x-rustok-actor` / `x-rustok-publisher`) с governance actors (`system:*`, `xtask:*`, `registry:*`, `moderator:*`) и slug-scoped publishers вроде `publisher:blog` / `publisher:*`; live `yank` по-прежнему требует обязательную причину. Незакрытыми остаются richer async validation/governance и отдельный внешний deployment.
- ⚠️ `apps/server/build.rs` уже генерирует optional module registry, GraphQL schema fragments и HTTP routes из `modules.toml`; explicit server entry-point contract через `[crate]` / `[provides.graphql]` / `[provides.http]` уже поднят, `apps/admin/build.rs` уже доведён до generic module root pages/nav/dashboard wiring и nested route metadata из `[[provides.admin_ui.pages]]`, а `apps/storefront/build.rs` уже поддерживает multi-slot storefront sections и generic route `/modules/{route_segment}`. По факту кода dual-surface Leptos exemplar-ами теперь являются `blog`, `commerce`, `forum`, `pages` и `search`; admin-only manifest-wired slice уже есть у `workflow` и `channel`. Перенос остальных модулей и внешний registry всё ещё открыты.
- ✅ В implementation contract теперь явно зафиксированы две независимые оси host/runtime: `DeploymentProfile` (`monolith`, `server-with-admin`, `server-with-storefront`, `headless-api`) описывает build/deploy surface, а `RuntimeHostMode` (`full`, `registry_only`) описывает runtime-exposed API surface. `registry_only` остаётся read-only runtime mode поверх `HeadlessApi`, тогда как V2 publish/governance должен жить как API-first слой в `full` host и быть одинаково доступным и в monolith, и в headless profile.
- ✅ `Registry V1` теперь зафиксирован как постоянный read-model, а не как временная ступень перед V2: внешние discovery consumers и operator UI продолжают читать каталог через `GET /v1/catalog*`, а будущий V2 write/governance flow должен лишь кормить published projection в V1.
- ✅ Manifest-wired UI contract стал строже: `ManifestManager`, `cargo xtask module validate`, `apps/admin/build.rs` и `apps/storefront/build.rs` теперь трактуют наличие `admin/Cargo.toml` или `storefront/Cargo.toml` без соответствующего `[provides.admin_ui].leptos_crate` / `[provides.storefront_ui].leptos_crate` как явную wiring error, а не как «почти подключённый» UI slice.

> [!NOTE]
> Раздел 9 ниже и этот статус-блок являются канонической точкой отсчёта на 2026-04-03.

## 1. РЎС‚Р°РЅРґР°СЂС‚ РјРѕРґСѓР»СЏ

### вњ… `rustok-module.toml` вЂ” РјР°РЅРёС„РµСЃС‚ РјРѕРґСѓР»СЏ

РљР°Р¶РґС‹Р№ path-РјРѕРґСѓР»СЊ РѕР±СЏР·Р°РЅ РёРјРµС‚СЊ `rustok-module.toml` РІ РєРѕСЂРЅРµ crate.
РџР°СЂСЃРёС‚СЃСЏ РІ `ManifestManager::catalog_modules()` Рё `apply_module_package_manifest()`.

| РЎРµРєС†РёСЏ | Р§С‚Рѕ СЃРѕРґРµСЂР¶РёС‚ | РЎС‚Р°С‚СѓСЃ |
|---|---|---|
| `[module]` | slug, name, version, description, authors, license | вњ… РїР°СЂСЃРёС‚СЃСЏ |
| `[marketplace]` | icon, banner, screenshots, category, tags | ✅ `category`, `tags`, `icon`, `banner`, `screenshots` уже парсятся и доходят до catalog / GraphQL / registry; базовая local validation (`description >= 20`, `icon` = absolute `http(s)` `.svg`, `banner` / `screenshots` = absolute `http(s)` image URLs) уже включена, дальше остаётся publish/governance policy |
| `[compatibility]` | rustok_min, rustok_max | вњ… РїР°СЂСЃРёС‚СЃСЏ |
| `[dependencies]` | depends_on с version_req | ✅ парсится и валидируется по semver-диапазонам |
| `[conflicts]` | несовместимые модули | ✅ парсится и валидируется как product/runtime conflict |
| `[crate]` | name, entry_type | вњ… РїР°СЂСЃРёС‚СЃСЏ |
| `[provides]` | graphql/http entry points, `admin_ui`, `storefront_ui`, nested admin pages | ✅ server/admin/storefront codegen уже читает эти секции; richer UI contract остаётся эволюционной темой |
| `[settings]` | схема настроек модуля (`type`, `default`, `min`, `max`, `options`, `object_keys`, `item_type`, `properties`, `items`) | ✅ парсится, валидируется и рендерится в `/modules` как schema-driven form; scalar `options` уже дают typed `select`, recursive nested metadata (`properties` / `items`) уже доходит до host как `shape`, а object/array editors используют её и на top-level, и внутри nested JSON tree для schema-driven add actions, schema-locked keys, inline nested child editors и nested scalar controls по child `type` / `options` / `min` / `max`; raw JSON остаётся только для модулей без schema |
| `[locales]` | supported, default | вњ… РїР°СЂСЃРёС‚СЃСЏ |

**Р¤Р°Р№Р»С‹**:
- `apps/server/src/modules/manifest.rs` вЂ” `apply_module_package_manifest()`

**Р”РѕРєСѓРјРµРЅС‚Р°С†РёСЏ**:
- `docs/modules/manifest.md`

---

### вњ… РЎС‚СЂСѓРєС‚СѓСЂР° С„Р°Р№Р»РѕРІ РјРѕРґСѓР»СЏ

```text
crates/rustok-{slug}/
в”њв”Ђв”Ђ rustok-module.toml       # РѕР±СЏР·Р°С‚РµР»СЊРЅРѕ РґР»СЏ path-РјРѕРґСѓР»РµР№
в”њв”Ђв”Ђ Cargo.toml
в”њв”Ђв”Ђ src/
в”‚   в”њв”Ђв”Ђ lib.rs               # impl RusToKModule (backend)
в”‚   в””в”Ђв”Ђ migrations/          # impl MigrationSource
в”‚       в”њв”Ђв”Ђ mod.rs
в”‚       в””в”Ђв”Ђ m20250101_*.rs
в”њв”Ђв”Ђ admin/                   # [NEW] Leptos-admin sub-crate
в””в”Ђв”Ђ storefront/              # [NEW] Leptos-storefront sub-crate
```

**Р”РѕРєСѓРјРµРЅС‚Р°С†РёСЏ**:
- `docs/architecture/modules.md`

---

### вњ… РљРѕРЅС‚СЂР°РєС‚ `RusToKModule`

```rust
pub trait RusToKModule: Send + Sync {
    fn slug(&self) -> &'static str;
    fn kind(&self) -> ModuleKind;          // Core | Optional
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>>;
    fn on_enable(&self, ctx: &AppContext) -> Result<()>;
    fn on_disable(&self, ctx: &AppContext) -> Result<()>;
    fn health(&self) -> ModuleHealth;
    fn event_listeners(&self) -> Vec<Box<dyn EventListener>>;
}
```

**Р¤Р°Р№Р»С‹**:
- `crates/rustok-core/src/module.rs`
- `crates/rustok-core/src/registry.rs`

**Р”РѕРєСѓРјРµРЅС‚Р°С†РёСЏ**:
- `docs/architecture/modules.md`

---

### вњ… Migration distribution

РљР°Р¶РґС‹Р№ РјРѕРґСѓР»СЊ РЅРµСЃС‘С‚ РјРёРіСЂР°С†РёРё РІРЅСѓС‚СЂРё СЃРІРѕРµРіРѕ crate (`src/migrations/`).
РџСЂРё СЃС‚Р°СЂС‚Рµ Р±РёРЅР°СЂРЅРёРєР° `registry.migrations()` СЃРѕР±РёСЂР°РµС‚ РјРёРіСЂР°С†РёРё РІСЃРµС… РјРѕРґСѓР»РµР№
Рё РїСЂРѕРіРѕРЅСЏРµС‚ РёС… Р°РІС‚РѕРјР°С‚РёС‡РµСЃРєРё. Р’СЂСѓС‡РЅСѓСЋ РґРѕР±Р°РІР»СЏС‚СЊ С„Р°Р№Р»С‹ РІ `apps/server/migration/` РЅРµ РЅСѓР¶РЅРѕ.

**Р¤Р°Р№Р»С‹**:
- `crates/rustok-*/src/migrations/` вЂ” РјРёРіСЂР°С†РёРё РєР°Р¶РґРѕРіРѕ РјРѕРґСѓР»СЏ
- `crates/rustok-core/src/registry.rs` вЂ” `ModuleRegistry::migrations()`

---

## 2. Tenant-level toggle

### вњ… РЎС…РµРјР° `tenant_modules`

```sql
CREATE TABLE tenant_modules (
  id         UUID PRIMARY KEY,
  tenant_id  UUID NOT NULL REFERENCES tenants(id),
  module_slug VARCHAR(64) NOT NULL,
  enabled    BOOLEAN NOT NULL DEFAULT true,
  settings   JSON NOT NULL DEFAULT '{}',
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL,
  UNIQUE(tenant_id, module_slug)
)
```

**Р¤Р°Р№Р»С‹**:
- `apps/server/migration/src/m20250101_000003_create_tenant_modules.rs`
- `crates/rustok-tenant/src/entities/tenant_module.rs`
- `apps/server/src/models/tenant_modules.rs`

---

### вњ… `ModuleLifecycleService::toggle_module`

Flow:
1. slug в€€ `ModuleRegistry` в†’ РёРЅР°С‡Рµ `UnknownModule`
2. РЅРµ `ModuleKind::Core` в†’ РёРЅР°С‡Рµ `CoreModuleCannotBeDisabled`
3. `enabled=true`: РІСЃРµ `depends_on` РІРєР»СЋС‡РµРЅС‹ в†’ РёРЅР°С‡Рµ `MissingDependencies`
4. `enabled=false`: РЅРµС‚ Р·Р°РІРёСЃСЏС‰РёС… РѕС‚ РЅРµРіРѕ в†’ РёРЅР°С‡Рµ `HasDependents`
5. `BEGIN TRANSACTION` в†’ UPDATE `tenant_modules` в†’ `on_enable()` / `on_disable()`
6. РџСЂРё `HookFailed` вЂ” РѕС‚РєР°С‚ СЃРѕСЃС‚РѕСЏРЅРёСЏ РІ С‚СЂР°РЅР·Р°РєС†РёРё

**Р¤Р°Р№Р»С‹**:
- `apps/server/src/services/module_lifecycle.rs`

**РўРµСЃС‚С‹**:
- `apps/server/tests/module_lifecycle.rs`

---

### вњ… GraphQL `toggleModule`

```graphql
mutation {
  toggleModule(moduleSlug: "blog", enabled: true) {
    moduleSlug enabled settings
  }
}
```

**Р¤Р°Р№Р»С‹**:
- `apps/server/src/graphql/mutations.rs` вЂ” `async fn toggle_module`

---

### вњ… `EnabledModulesProvider` + `<ModuleGuard>` (Leptos)

`EnabledModulesProvider` Р·Р°РіСЂСѓР¶Р°РµС‚ РІРєР»СЋС‡С‘РЅРЅС‹Рµ РјРѕРґСѓР»Рё РїСЂРё СЃС‚Р°СЂС‚Рµ Рё РїСЂРµРґРѕСЃС‚Р°РІР»СЏРµС‚
РєРѕРЅС‚РµРєСЃС‚ РІСЃРµРјСѓ РїСЂРёР»РѕР¶РµРЅРёСЋ. `<ModuleGuard slug="blog">` СЂРµРЅРґРµСЂРёС‚ children С‚РѕР»СЊРєРѕ
РµСЃР»Рё РјРѕРґСѓР»СЊ РІРєР»СЋС‡С‘РЅ.

**Р¤Р°Р№Р»С‹**:
- `apps/admin/src/shared/context/enabled_modules.rs`

---

### вњ… Р¤РёР»СЊС‚СЂР°С†РёСЏ slot-РєРѕРјРїРѕРЅРµРЅС‚РѕРІ

`components_for_slot(slot_id, enabled_modules)` С„РёР»СЊС‚СЂСѓРµС‚ РІРёРґР¶РµС‚С‹ РІРёС‚СЂРёРЅС‹
РїРѕ РІРєР»СЋС‡С‘РЅРЅС‹Рј РјРѕРґСѓР»СЏРј С‚РµРЅР°РЅС‚Р° РїРµСЂРµРґ СЂРµРЅРґРµСЂРѕРј.

**Р¤Р°Р№Р»С‹**:
- `apps/storefront/src/modules/registry.rs`

---

### ✅ Настройки модуля — schema-driven UI работает end-to-end

Колонка `settings JSON` в `tenant_modules` есть, и операторский путь обновления
через UI работает end-to-end. `updateModuleSettings` сохраняет tenant-specific
payload, сервер валидирует его по `[settings]` из `rustok-module.toml`, а
`/modules` в Leptos Admin рендерит schema-driven controls для всех текущих
поддерживаемых типов.

**Что уже есть**:
```graphql
mutation {
  updateModuleSettings(moduleSlug: "blog", settings: { postsPerPage: 20 }): TenantModule!
}
```

Серверная сторона (`apps/server/src/graphql/mutations.rs` + `apps/server/src/services/module_lifecycle.rs`) уже:
```rust
async fn update_module_settings(
    &self, ctx: &Context<'_>,
    module_slug: String,
    settings: serde_json::Value,
) -> Result<TenantModule> {
    // 1. Проверяет, что модуль включён для tenant (или core/upsert case)
    // 2. Валидирует settings по `[settings]` из rustok-module.toml
    // 3. Применяет `default` и отклоняет unknown keys / type/range mismatch
    // 4. UPDATE tenant_modules SET settings = ?
}
```

**Что остаётся**:

- richer field metadata для nested object contracts, если понадобится не просто JSON editor, а полноценно вложенная форма; scalar `options`, `object_keys`, `item_type`, `properties` и `items` уже дают typed `select`, recursive shape contract, deep schema-driven add actions, schema-locked nested keys, inline nested child editors и nested scalar controls по child constraints, так что этот остаток теперь в основном про fully type-directed child controls beyond current generic JSON tree;
- richer nested schema metadata для object/array children, если понадобится уйти от generic JSON-tree UX к полностью type-directed subforms beyond current recursive `shape` propagation;
- при желании отдельный read/query contract, чтобы другие admin surfaces рендерили форму из того же server-side schema source of truth.

---

## 3. Platform-level install/uninstall

### вњ… `ManifestManager`

```rust
ManifestManager::load()                     // РїР°СЂСЃРёС‚СЊ modules.toml
ManifestManager::save(manifest)             // СЃРѕС…СЂР°РЅРёС‚СЊ modules.toml
ManifestManager::validate(manifest)         // РїСЂРѕРІРµСЂРёС‚СЊ РіСЂР°С„ Р·Р°РІРёСЃРёРјРѕСЃС‚РµР№
ManifestManager::validate_with_registry()   // СЃРІРµСЂРёС‚СЊ СЃ ModuleRegistry
ManifestManager::install_builtin_module()   // РґРѕР±Р°РІРёС‚СЊ РІ modules.toml
ManifestManager::uninstall_module()         // СѓРґР°Р»РёС‚СЊ РёР· modules.toml
ManifestManager::upgrade_module()           // РѕР±РЅРѕРІРёС‚СЊ РІРµСЂСЃРёСЋ
ManifestManager::catalog_modules()          // РґР»СЏ MarketplaceCatalogService
ManifestManager::build_modules()            // РґР»СЏ BuildService
ManifestManager::build_execution_plan()     // РґР»СЏ BuildExecutor
```

**Р¤Р°Р№Р»С‹**:
- `apps/server/src/modules/manifest.rs`

**Р”РѕРєСѓРјРµРЅС‚Р°С†РёСЏ**:
- `docs/modules/manifest.md`
- `docs/architecture/modules.md`

---

### вњ… GraphQL РјСѓС‚Р°С†РёРё install/uninstall/upgrade/rollback

```graphql
installModule(slug: String!, version: String): BuildJob!
uninstallModule(slug: String!): BuildJob!
upgradeModule(slug: String!, version: String!): BuildJob!
rollbackBuild(buildId: ID!): BuildJob!
```

**Р¤Р°Р№Р»С‹**:
- `apps/server/src/graphql/mutations.rs`

---

### ✅ Semver-валидация зависимостей и конфликтов

Секция закрыта в `apps/server/src/modules/manifest.rs`.

Что теперь реально проверяется:

- наличие `rustok-module.toml` для path-модулей;
- `ownership`, `trust_level` и admin-surface metadata;
- semver-диапазоны из `[dependencies]` с отдельной ошибкой `IncompatibleDependencyVersion`;
- продуктовые/runtime-конфликты из `[conflicts]` с отдельной ошибкой `ConflictingModule`;
- совместимость с текущей версией платформы через `rustok_min_version` / `rustok_max_version`.

Остаток этой темы больше не в backend-валидации, а в richer operator UX и внешнем registry/governance слое. `/modules` теперь уже показывает operator-facing metadata readiness для `description` / visuals / publisher / publish signals, а detail panel дополнительно видит V2 publish lifecycle (latest request status / actor / timestamps, latest persisted release state, surfaced validation/rejection details), но publish-time moderation и внешний registry workflow всё ещё остаются отдельным этапом.

---

## 4. Build pipeline

### вњ… `BuildService`

```rust
BuildService::request_build(request)   // СЃРѕР·РґР°С‚СЊ Build, С…РµС€РёСЂРѕРІР°С‚СЊ, РґРµРґСѓР±Р»РёСЂРѕРІР°С‚СЊ
BuildService::get_build(build_id)
BuildService::active_build()           // СЃР»РµРґСѓСЋС‰РёР№ queued/running
BuildService::running_build()
```

Р”РµРґСѓРїР»РёРєР°С†РёСЏ: РµСЃР»Рё РІ РѕС‡РµСЂРµРґРё СѓР¶Рµ РµСЃС‚СЊ build СЃ С‚Р°РєРёРј Р¶Рµ SHA-256 `modules_delta` вЂ”
РІРѕР·РІСЂР°С‰Р°РµС‚ СЃСѓС‰РµСЃС‚РІСѓСЋС‰РёР№ РІРјРµСЃС‚Рѕ СЃРѕР·РґР°РЅРёСЏ РЅРѕРІРѕРіРѕ.

**Р¤Р°Р№Р»С‹**:
- `apps/server/src/services/build_service.rs`
- `apps/server/src/models/build.rs` вЂ” `BuildStatus`, `BuildStage`, `DeploymentProfile`
- `apps/server/migration/src/m20250212_000001_create_builds_and_releases.rs`

---

### вњ… `BuildExecutor` вЂ” manifest-derived build plan

Р’С‹РїРѕР»РЅСЏРµС‚ РЅРµ С‚РѕР»СЊРєРѕ `cargo build -p rustok-server`, Р° РІРµСЃСЊ build plan,
РІС‹РІРµРґРµРЅРЅС‹Р№ РёР· `modules.toml`: server, optional `admin` Рё optional `storefront`.
РћР±РЅРѕРІР»СЏРµС‚ `builds.stage` Рё `builds.progress` РїРѕ С…РѕРґСѓ РІС‹РїРѕР»РЅРµРЅРёСЏ.
РЎРѕР·РґР°С‘С‚ Р·Р°РїРёСЃСЊ РІ `releases` РїСЂРё СѓСЃРїРµС…Рµ.

**Р¤Р°Р№Р»С‹**:
- `apps/server/src/services/build_executor.rs`

**Env vars**:
- `RUSTOK_BUILD_CARGO_BIN` вЂ” РїСѓС‚СЊ Рє cargo (default: `cargo`)
- `RUSTOK_BUILD_TRUNK_BIN` вЂ” РїСѓС‚СЊ Рє trunk (default: `trunk`)

Что уже делается в build plan:

- server СЃРѕР±РёСЂР°РµС‚СЃСЏ С‡РµСЂРµР· `cargo build -p <app>`;
- `admin` СЃРѕР±РёСЂР°РµС‚СЃСЏ С‡РµСЂРµР· `trunk build` Рё РґР°С‘С‚ Р°СЂС‚РµС„Р°РєС‚-РґРёСЂРµРєС‚РѕСЂРёСЋ `apps/admin/dist`;
- `storefront` СЃРѕР±РёСЂР°РµС‚СЃСЏ РєР°Рє РѕС‚РґРµР»СЊРЅС‹Р№ SSR-Р±РёРЅР°СЂРЅРёРє `cargo build -p rustok-storefront`;
- execution plan СЃРµСЂРёР°Р»РёР·СѓРµС‚СЃСЏ РІ metadata build-Р·Р°РїРёСЃРё Рё РїРµСЂРµРёСЃРїРѕР»СЊР·СѓРµС‚СЃСЏ release backend-РѕРј.

---

### вњ… `buildProgress` GraphQL subscription

РСЃС‚РёРЅРЅС‹Р№ push С‡РµСЂРµР· `tokio::sync::broadcast` РєР°РЅР°Р».
`BuildEventHub` СЂР°СЃСЃС‹Р»Р°РµС‚ СЃРѕР±С‹С‚РёСЏ РїРѕ РјРµСЂРµ РІС‹РїРѕР»РЅРµРЅРёСЏ build executor'Р°.

```graphql
subscription {
  buildProgress(buildId: "...") { status stage progress logsUrl }
}
```

**Р¤Р°Р№Р»С‹**:
- `apps/server/src/graphql/subscriptions.rs`

---

### вњ… GraphQL queries РґР»СЏ builds

```graphql
activeBuild: BuildJob
buildHistory(limit: Int, offset: Int): [BuildJob!]!
```

**Р¤Р°Р№Р»С‹**:
- `apps/server/src/graphql/queries.rs`

---

### вњ… `rollback_build`

РџСЂРѕРІРµСЂСЏРµС‚ С†РµРїРѕС‡РєСѓ СЂРµР»РёР·РѕРІ С‡РµСЂРµР· `releases.previous_release_id`.
РџРѕРІС‚РѕСЂРЅРѕ Р°РєС‚РёРІРёСЂСѓРµС‚ РїСЂРµРґС‹РґСѓС‰РёР№ `Release`. РџРѕР»РЅРѕС†РµРЅРЅС‹Р№ РѕС‚РєР°С‚, РЅРµ РїСЂРѕСЃС‚Рѕ СЃРјРµРЅР° СЃС‚Р°С‚СѓСЃР°.

**Р¤Р°Р№Р»С‹**:
- `apps/server/src/graphql/mutations.rs` вЂ” `async fn rollback_build`
- `apps/server/src/models/release.rs`

---

### ✅ Release deploy backend — server publish path закрыт

Старое описание «после `cargo build` ничего нет» больше не соответствует коду.

Что уже есть:

- `ReleaseDeploymentService` и `release_backend` в `apps/server/src/services/`;
- публикация серверного артефакта в filesystem и HTTP backend;
- публикация реальных frontend-артефактов (`apps/admin/dist` и `rustok-storefront`) в filesystem/container backend;
- отдельный `container` backend поверх того же release bundle;
- `docker build` / `docker push` через generic config (`docker_bin`, `image_repository`);
- заполнение `releases.container_image`;
- optional `rollout_command` hook без жёсткой привязки к Kubernetes/docker-compose;
- заполнение `server_artifact_url`, `admin_artifact_url`, `storefront_artifact_url`;
- связка build-worker → publish-release → attach artifacts.

Что остаётся вне этого блока:

- provider-specific deploy logic по-прежнему должна жить во внешнем orchestrator/hook, а не в `apps/server`;
- HTTP backend пока не грузит директорию `apps/admin/dist` сам по multipart и остаётся совместимым с внешним deployment endpoint, который может вернуть готовые `admin_artifact_url` / `storefront_artifact_url`.

---

### вњ… Build progress UI вЂ” live subscription + polling fallback

РџСЂРѕРіСЂРµСЃСЃ-Р±Р°СЂ РІ `/modules` С‚РµРїРµСЂСЊ РѕР±РЅРѕРІР»СЏРµС‚СЃСЏ С‡РµСЂРµР· `buildProgress` subscription, Р° polling РѕСЃС‚Р°С‘С‚СЃСЏ С‚РѕР»СЊРєРѕ РєР°Рє fallback РїСЂРё РѕР±СЂС‹РІРµ live-РєР°РЅР°Р»Р°.

РС‚РѕРіРѕРІС‹Р№ transport contract:
- GraphQL websocket endpoint: `/api/graphql/ws`;
- browser-side auth/tenant/context РёРґСѓС‚ С‡РµСЂРµР· `connection_init` payload (`token`, `tenantSlug`, `locale`);
- `apps/server` РґР»СЏ subscription route РЅРµ С‚СЂРµР±СѓРµС‚ `X-Tenant-Slug` РЅР° HTTP upgrade и СЂРµР·РѕР»РІРёС‚ tenant РІРЅСѓС‚СЂРё handshake;
- `apps/admin` Р»РѕРєР°Р»СЊРЅРѕ РѕР±РЅРѕРІР»СЏРµС‚ active build РїРѕ push-СЃРѕР±С‹С‚РёСЏРј Рё РґРµР»Р°РµС‚ РѕР±С‹С‡РЅС‹Р№ refresh С‚РѕР»СЊРєРѕ РґР»СЏ terminal state / resync.

---

## 5. Marketplace РєР°С‚Р°Р»РѕРі

### вњ… `MarketplaceCatalogService` вЂ” provider chain

```
MarketplaceCatalogService
  в”њв”Ђ LocalManifestMarketplaceProvider   в†’ РІСЃС‚СЂРѕРµРЅРЅС‹Рµ path-РјРѕРґСѓР»Рё РёР· modules.toml
  в””в”Ђ RegistryMarketplaceProvider        в†’ РІРЅРµС€РЅРёР№ СЂРµРµСЃС‚СЂ (RUSTOK_MARKETPLACE_REGISTRY_URL, `GET /v1/catalog`, detail `GET /v1/catalog/{slug}`, shared query forwarding)
       в””в”Ђ moka cache (TTL: RUSTOK_MARKETPLACE_REGISTRY_CACHE_TTL_SECS, default 60s)
```

РџСЂРё РЅРµРґРѕСЃС‚СѓРїРЅРѕСЃС‚Рё СЂРµРµСЃС‚СЂР° вЂ” graceful fallback РЅР° local-manifest.
Р”РµРґСѓРїР»РёРєР°С†РёСЏ: РїРѕР±РµР¶РґР°РµС‚ РїРµСЂРІС‹Р№ РїСЂРѕРІР°Р№РґРµСЂ.

**Р¤Р°Р№Р»С‹**:
- `apps/server/src/services/marketplace_catalog.rs`

**Env vars**:
- `RUSTOK_MARKETPLACE_REGISTRY_URL`
- `RUSTOK_MARKETPLACE_REGISTRY_TIMEOUT_MS` (default: 3000)
- `RUSTOK_MARKETPLACE_REGISTRY_CACHE_TTL_SECS` (default: 60)

---

### вњ… GraphQL `marketplace` + `marketplaceModule`

```graphql
marketplace(
  search: String
  category: String
  tag: String
  source: String          # "path" | "registry" | "git"
  installed: Boolean
  trust_level: String     # "core" | "verified" | "unverified" | "private"
  compatible_only: Boolean
): [MarketplaceModule!]!

marketplaceModule(slug: String!): MarketplaceModule
```

**Р¤Р°Р№Р»С‹**:
- `apps/server/src/graphql/queries.rs`

---

### вњ… Deep-link `?module=slug`

Р’С‹Р±СЂР°РЅРЅС‹Р№ РјРѕРґСѓР»СЊ РІ РєР°С‚Р°Р»РѕРіРµ РѕС‚СЂР°Р¶Р°РµС‚СЃСЏ РІ URL (`/modules?module=blog`).
РџСЂСЏРјР°СЏ СЃСЃС‹Р»РєР° РѕС‚РєСЂС‹РІР°РµС‚ РґРµС‚Р°Р»СЊРЅСѓСЋ РїР°РЅРµР»СЊ Р±РµР· РїРµСЂРµС…РѕРґР°.

**Р¤Р°Р№Р»С‹**:
- `apps/admin/src/features/modules/components/modules_list.rs`

---

### ⚠️ Внешний реестр `modules.rustok.dev`

`RegistryMarketplaceProvider` уже ходит в schema-versioned `GET /v1/catalog` и сохраняет
legacy fallback на `GET /catalog`, а `apps/server` теперь публикует этот list/detail read-only контракт как
референсную реализацию для first-party модулей. Отдельный внешний сервис и publish/governance
слой всё ещё не существуют.

**Scope V1** (read-only, first-party РјРѕРґСѓР»Рё):
```
modules.rustok.dev
в”њв”Ђ GET /v1/catalog в†’ [{ slug, name, version, ... }] + filters + `limit/offset` + `X-Total-Count`
в””в”Ђ GET /v1/catalog/{slug} в†’ { slug, name, version, ... }
```
Позволяет проверить весь `RegistryMarketplaceProvider` → AdminUI flow, включая registry-only `name` / `description` metadata. В текущем workspace этот
контракт уже можно локально поднять через `apps/server` в режиме `settings.rustok.runtime.host_mode = "registry_only"`, причём readiness в этом режиме уже не ждёт event/search/module runtime, но отдельный `modules.rustok.dev` deployment и операционный rollout для него ещё не выделены. HTTP-слой уже cache-friendly: `ETag`, `Cache-Control`, `If-None-Match`, `304 Not Modified`.
Локальный smoke-path для dedicated host теперь формализован: runbook живёт в `apps/server/docs/health.md`,
а `scripts/verify/verify-deployment-profiles.sh` проверяет env override, reduced runtime surface и reduced OpenAPI.

**Scope V2** (РїРѕР»РЅС‹Р№):
```
modules.rustok.dev
в”њв”Ђв”Ђ GraphQL API (РєР°С‚Р°Р»РѕРі, РІРµСЂСЃРёРё, РїРѕРёСЃРє, publish, yank)
в”њв”Ђв”Ђ Crate Storage (S3: .crate Р°СЂС…РёРІС‹ + checksums)
в””в”Ђв”Ђ Validation Pipeline (static в†’ audit в†’ compile в†’ test в†’ metadata)
```

РџРµСЂРІС‹Р№ live scaffold РїРѕ РјСѓС‚Р°С†РёСЏРј СѓР¶Рµ РїРѕРґРЅСЏС‚ РІ `apps/server`: full host now exposes
`POST /v2/catalog/publish`, `GET /v2/catalog/publish/{request_id}`,
`PUT /v2/catalog/publish/{request_id}/artifact` и `POST /v2/catalog/yank` как OpenAPI-backed
REST contract с local sync validation, persisted publish requests/releases и published projection
обратно в V1 catalog trail. Dry-run остаётся backward-compatible preview-режимом, а
`registry_only` host mode по-прежнему strictly read-only и не монтирует write routes.

**РђСѓС‚РµРЅС‚РёС„РёРєР°С†РёСЏ**: `docs/concepts/plan-oauth2-app-connections.md` (РџСЂРёР»РѕР¶РµРЅРёРµ A).

---

## 6. Admin UI

### вњ… РЎС‚СЂР°РЅРёС†Р° `/modules`

| Р­Р»РµРјРµРЅС‚ | РЎС‚Р°С‚СѓСЃ |
|---|---|
| РЎРїРёСЃРѕРє СѓСЃС‚Р°РЅРѕРІР»РµРЅРЅС‹С… РјРѕРґСѓР»РµР№ (`modules` query) | вњ… |
| РљР°С‚Р°Р»РѕРі РјР°СЂРєРµС‚РїР»РµР№СЃР° (`marketplace` query) | вњ… |
| Р¤РёР»СЊС‚СЂС‹: РїРѕРёСЃРє, РєР°С‚РµРіРѕСЂРёСЏ, tag, trust level, compatibility | вњ… |
| Р”РµС‚Р°Р»СЊРЅР°СЏ РїР°РЅРµР»СЊ `marketplaceModule(slug)` | ✅ включает operator-facing metadata readiness hints для registry/publish flow, latest non-yanked release summary и version trail вместо слепого publish signal, а также V2 lifecycle panel с latest request/release state и surfaced validation details |
| Deep-link `?module=slug` | вњ… |
| Install / Uninstall РєРЅРѕРїРєРё в†’ `installModule` / `uninstallModule` | вњ… |
| Toggle switch в†’ `toggleModule` | вњ… |
| РЎРµРєС†РёРё: Installed / Marketplace / Updates | вњ… |
| Прогресс-бар build (polling 5 сек) | ✅ fallback/resync path |
| Прогресс-бар build (WebSocket subscription) | ✅ подключён end-to-end |
| Updates tab + `upgradeModule` для version-pinned модулей | ✅ работает; path-модули без pinned version осознанно не попадают в update flow |
| Форма настроек модуля (`updateModuleSettings`) | ✅ schema-driven UI работает для scalar и complex settings, включая declarative `options` → `select`; raw fallback нужен только без schema |

---

## 7. Р’РЅРµС€РЅРёР№ СЂРµРµСЃС‚СЂ Рё РїСѓР±Р»РёРєР°С†РёСЏ

### в¬њ `rustok mod publish` CLI

```bash
rustok mod init          # РЁР°Р±Р»РѕРЅ РјРѕРґСѓР»СЏ СЃ rustok-module.toml
rustok mod validate      # Р›РѕРєР°Р»СЊРЅР°СЏ РїСЂРѕРІРµСЂРєР° РјР°РЅРёС„РµСЃС‚Р°
rustok mod test          # Validation pipeline Р»РѕРєР°Р»СЊРЅРѕ
rustok mod publish       # РћРїСѓР±Р»РёРєРѕРІР°С‚СЊ РІ СЂРµРµСЃС‚СЂ
rustok mod yank 1.2.0    # РћС‚РѕР·РІР°С‚СЊ РІРµСЂСЃРёСЋ
```

Р¤Р°РєС‚РёС‡РµСЃРєРё СѓР¶Рµ РµСЃС‚СЊ РїРµСЂРІС‹Р№ Р»РѕРєР°Р»СЊРЅС‹Р№ scaffold РЅР° `xtask`:

```bash
cargo xtask module validate [slug]
cargo xtask module test <slug> [--dry-run]
cargo xtask module publish <slug> [--dry-run] [--registry-url <url>]
cargo xtask module yank <slug> <version> [--dry-run] [--reason <text>] [--registry-url <url>]
```

РћРЅ СѓР¶Рµ РїСЂРѕРІРµСЂСЏРµС‚ publish-readiness contract РґР»СЏ path-РјРѕРґСѓР»РµР№:
- РєРѕРЅСЃРёСЃС‚РµРЅС‚РЅРѕСЃС‚СЊ `modules.toml` в†” `rustok-module.toml` в†” `Cargo.toml`;
- semver РІ `module.version`;
- РЅР°Р»РёС‡РёРµ `package.license`;
- СЃРѕРІРїР°РґРµРЅРёРµ РІРµСЂСЃРёР№ РѕСЃРЅРѕРІРЅРѕРіРѕ crate Рё publishable `admin/` / `storefront` UI crate-РѕРІ;
- РјРёРЅРёРјР°Р»СЊРЅС‹Р№ metadata baseline РґР»СЏ publish path (`module.name`, `module.description >= 20`).

`test` С‚РµРїРµСЂСЊ СѓР¶Рµ СѓРјРµРµС‚ СЃРѕР±РёСЂР°С‚СЊ / РІС‹РїРѕР»РЅСЏС‚СЊ Р»РѕРєР°Р»СЊРЅС‹Р№ smoke-plan
(`module validate` + `cargo check` РґР»СЏ РѕСЃРЅРѕРІРЅРѕРіРѕ crate Рё publishable UI package-РѕРІ), Р°
`publish` / `yank` больше не ограничены только preview-JSON: при наличии `--registry-url`
`xtask` создаёт live V2 publish request, загружает artifact bundle, опрашивает status endpoint
и умеет дернуть live `yank` с обязательной причиной. Dry-run при этом остаётся совместимым
preview-режимом для тех же CLI-команд.

`xtask` С‚РµРїРµСЂСЊ СѓР¶Рµ СѓРјРµРµС‚ Рё HTTP round-trip: `module publish/yank` can target
the new server mutation contract through `--registry-url <url>` (or env
`RUSTOK_MODULE_REGISTRY_URL`) and print the normalized V2 API response instead of only local JSON.
Payload assembly для `publish` / `yank` теперь вынесен в явные builder-ы с unit coverage, loopback
registry URLs (`localhost` / `127.0.0.1`) идут через direct client path без системного proxy, а live
`publish` дополнительно собирает локальный artifact bundle из `rustok-module.toml` / `Cargo.toml` /
UI manifest-ов, чтобы smoke против `apps/server` не зависел от внешнего artifact storage.

Р—Р°РІРёСЃРёС‚ РѕС‚ Рї. [РІРЅРµС€РЅРёР№ СЂРµРµСЃС‚СЂ](#-РІРЅРµС€РЅРёР№-СЂРµРµСЃС‚СЂ-modulesrustokdev).

---

### в¬њ Validation pipeline РґР»СЏ РїСѓР±Р»РёРєР°С†РёРё

| РЎС‚Р°РґРёСЏ | РџСЂРѕРІРµСЂРєРё |
|---|---|
| 1. Static | РјР°РЅРёС„РµСЃС‚ РІР°Р»РёРґРµРЅ, slug СѓРЅРёРєР°Р»РµРЅ, semver, license, locales/en.json |
| 2. Security | cargo-audit, РѕС‚СЃСѓС‚СЃС‚РІРёРµ unsafe Р±РµР· РѕР±РѕСЃРЅРѕРІР°РЅРёСЏ, РЅРµС‚ std::process::Command |
| 3. Compilation | РєРѕРјРїРёР»РёСЂСѓРµС‚СЃСЏ СЃ rustok_min..rustok_max |
| 4. Runtime | cargo test, РјРёРіСЂР°С†РёРё up/down РёРґРµРјРїРѕС‚РµРЅС‚РЅС‹, on_enable/on_disable |
| 5. Metadata | icon.svg валиден, description >= 20 символов, screenshots, publisher/checksum hygiene | ✅ Базовая local validation уже есть в `ManifestManager`, а registry normalization теперь ещё и режет пустой `publisher` и невалидный `checksum_sha256`; publish-time moderation и governance остаются отдельным этапом |

---

## 8. РђСЂС…РёС‚РµРєС‚СѓСЂРЅС‹Р№ РґРѕР»Рі

### вљ пёЏ GraphQL Рё REST РјРѕРґСѓР»РµР№ Р¶РёРІСѓС‚ РІ СЃРµСЂРІРµСЂРµ, Р° РЅРµ РІ РјРѕРґСѓР»СЊРЅС‹С… РєСЂРµР№С‚Р°С…

**РСЃС‚РѕСЂРёС‡РµСЃРєРѕРµ СЃРѕСЃС‚РѕСЏРЅРёРµ, СЃ РєРѕС‚РѕСЂРѕРіРѕ СЃС‚Р°СЂС‚РѕРІР°Р» РІС‹РЅРѕСЃ Р°РґР°РїС‚РµСЂРѕРІ**:

РР·РЅР°С‡Р°Р»СЊРЅРѕ GraphQL Рё REST Р°РґР°РїС‚РµСЂС‹ РґР»СЏ РєР°Р¶РґРѕРіРѕ РјРѕРґСѓР»СЏ Р¶РёР»Рё РІ `apps/server/`:

```
apps/server/src/
в”њв”Ђв”Ђ graphql/
в”‚   в”њв”Ђв”Ђ blog/        (~535 СЃС‚СЂРѕРє)   в†ђ Р·РЅР°Р» Рѕ rustok_blog::PostService
в”‚   в”њв”Ђв”Ђ content/     (removed)            в†ђ product GraphQL/REST transport removed; `rustok-content` remains shared helper/orchestration only
в”‚   в”њв”Ђв”Ђ commerce/    (~682 СЃС‚СЂРѕРє)   в†ђ Р·РЅР°Р» Рѕ rustok_commerce::CatalogService
в”‚   в”њв”Ђв”Ђ forum/       (~740 СЃС‚СЂРѕРє)   в†ђ Р·РЅР°Р» Рѕ rustok_forum::TopicService
в”‚   в”њв”Ђв”Ђ pages/       (~823 СЃС‚СЂРѕРє)   в†ђ Р·РЅР°Р» Рѕ rustok_pages::PageService
в”‚   в”њв”Ђв”Ђ workflow/    (~1071 СЃС‚СЂРѕРє)  в†ђ Р·РЅР°РµС‚ Рѕ rustok_workflow::WorkflowService
в”‚   в”њв”Ђв”Ђ alloy/       (~799 СЃС‚СЂРѕРє)   в†ђ Р·РЅР°РµС‚ Рѕ alloy::ScriptRegistry
в”‚   в””в”Ђв”Ђ media/       (~233 СЃС‚СЂРѕРє)   в†ђ Р·РЅР°РµС‚ Рѕ rustok_media::MediaService
в””в”Ђв”Ђ controllers/
    в”њв”Ђв”Ђ blog/        (~271 СЃС‚СЂРѕРє)   в†ђ С‚Рѕ Р¶Рµ СЃР°РјРѕРµ РґР»СЏ REST
    в”њв”Ђв”Ђ content/     (~199 СЃС‚СЂРѕРє)
    в”њв”Ђв”Ђ commerce/    (~1149 СЃС‚СЂРѕРє)
    в”њв”Ђв”Ђ forum/       (~638 СЃС‚СЂРѕРє)
    в”њв”Ђв”Ђ pages/       (~297 СЃС‚СЂРѕРє)
    в”њв”Ђв”Ђ workflow/    (~272 СЃС‚СЂРѕРє)
    в””в”Ђв”Ђ media/       (~191 СЃС‚СЂРѕРє)
```

**РџРѕС‡РµРјСѓ СЌС‚Рѕ РїСЂРѕР±Р»РµРјР°**:
РЎС‚РѕСЂРѕРЅРЅРёР№ РјРѕРґСѓР»СЊ РёР· РјР°СЂРєРµС‚РїР»РµР№СЃР° РЅРµ РјРѕР¶РµС‚ РґРѕР±Р°РІРёС‚СЊ СЃРІРѕР№ GraphQL/REST Р±РµР· РїСЂР°РІРєРё
`apps/server/`. Р­С‚Рѕ РЅР°СЂСѓС€Р°РµС‚ РєРѕРЅС†РµРїС†РёСЋ СЃР°РјРѕРґРѕСЃС‚Р°С‚РѕС‡РЅРѕРіРѕ РјРѕРґСѓР»СЏ.

**РџРѕС‡РµРјСѓ РЅРµ РІ `rustok-core`**:
`async-graphql`, `axum`, `loco_rs` вЂ” С‚СЏР¶С‘Р»С‹Рµ web-Р·Р°РІРёСЃРёРјРѕСЃС‚Рё. РћРЅРё РЅРµ РґРѕР»Р¶РЅС‹
РїРѕРїР°РґР°С‚СЊ РІ РґРѕРјРµРЅРЅРѕРµ СЏРґСЂРѕ. РњРѕРґСѓР»СЊРЅС‹Р№ РєСЂРµР№С‚ РґРѕР»Р¶РµРЅ РѕСЃС‚Р°РІР°С‚СЊСЃСЏ framework-agnostic.

**РЎС‚Р°С‚СѓСЃ РЅР° 2026-03-19**:

- вњ… РЎРѕР·РґР°РЅ РЅРѕРІС‹Р№ crate `crates/rustok-api/` РєР°Рє РѕР±С‰РёР№ API-СЃР»РѕР№ РјРµР¶РґСѓ `apps/server` Рё Р±СѓРґСѓС‰РёРјРё РјРѕРґСѓР»СЊРЅС‹РјРё web-Р°РґР°РїС‚РµСЂР°РјРё.
- вњ… Р’ `rustok-api` РІС‹РЅРµСЃРµРЅС‹ РѕР±С‰РёРµ РїСЂРёРјРёС‚РёРІС‹: `AuthContext`, `TenantContext`, `RequestContext`, `scope_matches`, `PageInfo`, `PaginationInput`, `GraphQLError`, `require_module_enabled`, `resolve_graphql_locale`.
- вњ… `apps/server` РїРµСЂРµРІРµРґС‘РЅ РЅР° СЃРѕРІРјРµСЃС‚РёРјС‹Рµ re-export/shim-С‚РѕС‡РєРё, С‡С‚РѕР±С‹ СЃСѓС‰РµСЃС‚РІСѓСЋС‰РёР№ РєРѕРґ РїСЂРѕРґРѕР»Р¶Р°Р» СЃРѕР±РёСЂР°С‚СЊСЃСЏ Р±РµР· РјР°СЃСЃРѕРІРѕР№ РїСЂР°РІРєРё РёРјРїРѕСЂС‚РѕРІ.
- вњ… РџРёР»РѕС‚РЅС‹Р№ РјРѕРґСѓР»СЊ `pages` РїРµСЂРµРЅРµСЃС‘РЅ: GraphQL Рё REST Р°РґР°РїС‚РµСЂС‹ С‚РµРїРµСЂСЊ Р¶РёРІСѓС‚ РІ `crates/rustok-pages`, Р° `apps/server` РґРµСЂР¶РёС‚ С‚РѕР»СЊРєРѕ С‚РѕРЅРєРёРµ re-export shim-С„Р°Р№Р»С‹.
- вњ… РЎР»РµРґРѕРј РїРµСЂРµРЅРµСЃС‘РЅ `blog`: РµРіРѕ GraphQL/REST Р°РґР°РїС‚РµСЂС‹ С‚РµРїРµСЂСЊ Р¶РёРІСѓС‚ РІ `crates/rustok-blog`, Р° СЃРµСЂРІРµСЂ РѕСЃС‚Р°РІР»СЏРµС‚ С‚РѕР»СЊРєРѕ shim/composition-root СЃР»РѕР№ Рё РјР°СЂС€СЂСѓС‚ health-check.
- вњ… Р—Р°С‚РµРј РїРµСЂРµРЅРµСЃС‘РЅ `forum`: GraphQL/REST Р°РґР°РїС‚РµСЂС‹ Рё connection helper С‚РµРїРµСЂСЊ Р¶РёРІСѓС‚ РІ `crates/rustok-forum`, Р° СЃРµСЂРІРµСЂ РѕСЃС‚Р°РІР»СЏРµС‚ С‚РѕР»СЊРєРѕ shim/composition-root СЃР»РѕР№ Рё РјР°СЂС€СЂСѓС‚ health-check.
- вњ… Р—Р°С‚РµРј РїРµСЂРµРЅРµСЃС‘РЅ `commerce`: GraphQL/REST Р°РґР°РїС‚РµСЂС‹ С‚РµРїРµСЂСЊ Р¶РёРІСѓС‚ РІ `crates/rustok-commerce`, permission-check РїРµСЂРµРІРµРґС‘РЅ РЅР° `AuthContext.permissions`, Р° `apps/server` РѕСЃС‚Р°РІР»СЏРµС‚ С‚РѕР»СЊРєРѕ shim/composition-root СЃР»РѕР№.
- вњ… Р—Р°С‚РµРј РїРµСЂРµРЅРµСЃС‘РЅ `content`: GraphQL/REST Р°РґР°РїС‚РµСЂС‹ С‚РµРїРµСЂСЊ Р¶РёРІСѓС‚ РІ `crates/rustok-content`, permission-check РІ REST/GraphQL mutation РїРµСЂРµРІРµРґС‘РЅ РЅР° `AuthContext.permissions`, Р° `apps/server` РѕСЃС‚Р°РІР»СЏРµС‚ С‚РѕР»СЊРєРѕ shim/composition-root СЃР»РѕР№ Рё РјР°СЂС€СЂСѓС‚ health-check.
- вњ… Р—Р°С‚РµРј РїРµСЂРµРЅРµСЃС‘РЅ `workflow`: GraphQL/REST Р°РґР°РїС‚РµСЂС‹ Рё webhook ingress С‚РµРїРµСЂСЊ Р¶РёРІСѓС‚ РІ `crates/rustok-workflow`, permission-check РїРµСЂРµРІРµРґС‘РЅ РЅР° `AuthContext.permissions`, Р° `apps/server` РѕСЃС‚Р°РІР»СЏРµС‚ С‚РѕР»СЊРєРѕ shim/composition-root СЃР»РѕР№.
- вњ… Р—Р°С‚РµРј РїРµСЂРµРЅРµСЃС‘РЅ `media`: GraphQL/REST Р°РґР°РїС‚РµСЂС‹ С‚РµРїРµСЂСЊ Р¶РёРІСѓС‚ РІ `crates/rustok-media`, REST РёСЃРїРѕР»СЊР·СѓРµС‚ РѕР±С‰РёР№ `AuthContext`, РјРµС‚СЂРёРєРё РѕСЃС‚Р°Р»РёСЃСЊ СЂСЏРґРѕРј СЃ transport-СЃР»РѕРµРј РјРѕРґСѓР»СЏ, Р° `apps/server` РѕСЃС‚Р°РІР»СЏРµС‚ С‚РѕР»СЊРєРѕ re-export shim.
- ? Затем исправлена позиция `alloy`: transport-слой вынесен в `crates/alloy`, а сам Alloy зафиксирован как module-agnostic capability вне runtime module registry; `apps/server` оставляет только composition-root shim.
- вњ… Server-only transport-С…РІРѕСЃС‚ Р°СЂС…РёС‚РµРєС‚СѓСЂРЅРѕРіРѕ РґРѕР»РіР° РґР»СЏ РјРѕРґСѓР»СЊРЅС‹С… GraphQL/REST Р°РґР°РїС‚РµСЂРѕРІ Р·Р°РєСЂС‹С‚.

**РџСЂР°РІРёР»СЊРЅРѕРµ СЂРµС€РµРЅРёРµ вЂ” РЅРѕРІС‹Р№ РєСЂРµР№С‚ `rustok-api`**:

```
crates/rustok-api/
  в””в”Ђв”Ђ src/
      в”њв”Ђв”Ђ context.rs       в†ђ TenantContext, AuthContext (РёР· apps/server/src/context/)
      в”њв”Ђв”Ђ graphql/
      в”‚   в”њв”Ђв”Ђ common.rs    в†ђ require_module_enabled, resolve_graphql_locale
      в”‚   в””в”Ђв”Ђ errors.rs    в†ђ GraphQLError
      в””в”Ђв”Ђ extractors/
          в””в”Ђв”Ђ rbac.rs      в†ђ Р±Р°Р·РѕРІС‹Рµ RBAC extractor С‚СЂРµР№С‚С‹
  # Р·Р°РІРёСЃРёС‚ РѕС‚: async-graphql, axum, loco_rs, rustok-core
```

РџРѕСЃР»Рµ СЌС‚РѕРіРѕ РєР°Р¶РґС‹Р№ РјРѕРґСѓР»СЊ РґРµСЂР¶РёС‚ GraphQL + REST Сѓ СЃРµР±СЏ:

```
crates/rustok-blog/src/
в”њв”Ђв”Ђ graphql/      в†ђ РїРµСЂРµРµС…Р°Р»Рѕ РёР· apps/server/src/graphql/blog/
в”‚   в”њв”Ђв”Ђ mod.rs    в†ђ pub struct BlogQuery; pub struct BlogMutation;
в”‚   в”њв”Ђв”Ђ query.rs
в”‚   в”њв”Ђв”Ђ mutation.rs
в”‚   в””в”Ђв”Ђ types.rs
в””в”Ђв”Ђ controllers/  в†ђ РїРµСЂРµРµС…Р°Р»Рѕ РёР· apps/server/src/controllers/blog/
    в”њв”Ђв”Ђ mod.rs    в†ђ pub fn routes() -> Routes
    в””в”Ђв”Ђ posts.rs
```

РЎРµСЂРІРµСЂ вЂ” С‚РѕР»СЊРєРѕ composition root:
```rust
// apps/server/src/graphql/schema.rs
#[cfg(feature = "mod-blog")]
use rustok_blog::graphql::{BlogMutation, BlogQuery};
```

**Р§С‚Рѕ РЅСѓР¶РЅРѕ СЃРґРµР»Р°С‚СЊ РґР°Р»СЊС€Рµ**:
1. Зафиксировать `rustok-api` как тонкий и единственный shared host/API layer для общих GraphQL/HTTP helper-ов, не допускать обратного дрейфа этих типов в `apps/server` и не создавать параллельную реализацию такого же слоя в других crate-ах.
2. Довести до одинакового стандарта оставшиеся composition-root shim-слои, чтобы новые модульные transport-адаптеры появлялись сразу в crate-ах модулей.
3. Зафиксировать layout, где module-agnostic capability crate сам содержит transport surfaces, а `apps/server` остаётся только composition root.
4. Перейти к следующему реальному блокеру extensibility: `build.rs`-кодогенерация entry points и системный вынос UI-слоя в publishable module packages.

**РћС‚РєСЂС‹С‚С‹Р№ РІРѕРїСЂРѕСЃ**: СЃР»РµРґСѓСЋС‰РёР№ СЃСЂРµР· РґР»СЏ `rustok-api` вЂ” abstraction around `CurrentUser`/RBAC extractor-С‹ Рё РјРёРЅРёРјР°Р»СЊРЅС‹Р№ runtime-contract, РЅСѓР¶РЅС‹Р№ РјРѕРґСѓР»СЊРЅС‹Рј HTTP-РєРѕРЅС‚СЂРѕР»Р»РµСЂР°Рј Р±РµР· РїСЂРѕС‚Р°СЃРєРёРІР°РЅРёСЏ РІСЃРµРіРѕ `AppContext`.

---

### ⚠️ Кодогенерация регистрации модулей (`build.rs`)

`apps/server/build.rs` уже существует и генерирует три include-файла в `OUT_DIR`:

```
modules_registry_codegen.rs
graphql_schema_codegen.rs
app_routes_codegen.rs
```

Что уже закрыто:

- `apps/server/src/modules/mod.rs` больше не регистрирует optional-модули вручную.
- `apps/server/src/graphql/schema.rs` больше не держит статический список optional `Query`/`Mutation`.
- `apps/server/src/app.rs` больше не добавляет optional HTTP routes вручную.
- `workflow` и `media` уже проходят через тот же manifest-managed composition-root path, что и остальные optional-модули.
- startup smoke теперь реально собирает полный runtime через codegen, а не через ручной список entry points.

Текущий контракт генерации:

- источник правды для server composition root — `modules.toml`;
- path-модули могут объявить server entry points явно в `rustok-module.toml` через `[crate].entry_type`, `[provides.graphql]` и `[provides.http]`;
- external/non-path crate-ы могут хранить уже нормализованные entry points прямо в `modules.toml` (`entry_type`, `graphql_query_type`, `graphql_mutation_type`, `http_routes_fn`, `http_webhook_routes_fn`);
- naming conventions (`<PascalSlug>Module`, `<PascalSlug>Query`, `<PascalSlug>Mutation`, `controllers::routes`, optional `webhook_routes`) остались только как backward-compatible fallback для path-модулей без явного контракта;
- если в `apps/server/src/controllers/<slug>` существует shim, generator сохраняет его как route entry point, чтобы не ломать server-specific health/webhook composition.

Что ещё остаётся:

- расширение manifest-driven nested admin metadata beyond current wildcard route + secondary nav, если модулям понадобятся более строгие host-level guarantees;
- richer storefront slot/page contract поверх уже существующего generated slot wiring;
- правило namespace-safety для GraphQL-visible типов модулей: после включения server codegen пришлось развести `BlogPostStatus` и `GqlContentStatus`, и новые модули не должны публиковать конфликтующие GraphQL names.

**Итог**: server composition root уже не требует ручной правки при подключении manifest-managed optional-модуля внутри текущего workspace, storefront host уже регистрирует module-owned Leptos sections через generated slot wiring, а admin host уже монтирует module-owned dashboard sections, nav items и module root pages через generated registry wiring. Полная extensibility всё ещё упирается в explicit entry-point contract и более богатые UI contracts поверх базового root-page path.

---

### в¬њ UI С‚РѕР¶Рµ РґРѕР»Р¶РµРЅ РїРµСЂРµСЃРѕР±РёСЂР°С‚СЊСЃСЏ вЂ” admin WASM Рё storefront WASM (Leptos)

**РљР»СЋС‡РµРІРѕР№ С„Р°РєС‚**: Leptos РєРѕРјРїРёР»РёСЂСѓРµС‚СЃСЏ РІ WASM. РљР°Рє СЃРµСЂРІРµСЂ в†’ Р±РёРЅР°СЂРЅРёРє,
С‚Р°Рє admin Рё storefront в†’ `.wasm`. Р”РёРЅР°РјРёС‡РµСЃРєРё РїРѕРґРіСЂСѓР·РёС‚СЊ РЅРѕРІС‹Р№ Rust-РєРѕРґ
РІ runtime РЅРµРІРѕР·РјРѕР¶РЅРѕ. Р›СЋР±РѕР№ РЅРѕРІС‹Р№ РјРѕРґСѓР»СЊ = РїРµСЂРµСЃР±РѕСЂРєР° WASM.

> [!IMPORTANT]
> **Next.js** (`apps/next-admin`, `apps/next-frontend`) **РЅРµ РІС…РѕРґРёС‚** РІ build pipeline
> РїСЂРё install/uninstall РјРѕРґСѓР»СЏ. РџРµСЂРµСЃР±РѕСЂРєР° Next.js вЂ” С‚РѕР»СЊРєРѕ РІСЂСѓС‡РЅСѓСЋ.
> РђРІС‚Рѕ-СѓСЃС‚Р°РЅРѕРІРєР° С‡РµСЂРµР· marketplace СЂР°Р±РѕС‚Р°РµС‚ РёСЃРєР»СЋС‡РёС‚РµР»СЊРЅРѕ РґР»СЏ **Leptos**-СЃС‚РµРєР°.

**Р§С‚Рѕ РїСЂРѕС€РёС‚Рѕ РІСЂСѓС‡РЅСѓСЋ РІ admin** (Leptos):

```
apps/admin/src/
в”њв”Ђв”Ђ pages/mod.rs         в†ђ mod workflows; mod workflow_detail;  (СЏРІРЅС‹Рµ РѕР±СЉСЏРІР»РµРЅРёСЏ)
в”њв”Ђв”Ђ pages/workflows.rs   в†ђ СЃС‚СЂР°РЅРёС†Р° Workflows
в”њв”Ђв”Ђ pages/workflow_detail.rs
в”њв”Ђв”Ђ features/workflow/   в†ђ РєРѕРјРїРѕРЅРµРЅС‚С‹ workflow (400+ СЃС‚СЂРѕРє)
в””в”Ђв”Ђ app/router.rs        в†ђ Route path="/workflows" view=Workflows
```

Р”Р»СЏ СЃС‚РѕСЂРѕРЅРЅРµРіРѕ `rustok-podcast`: РЅРµС‚ РЅРё `/podcasts` РјР°СЂС€СЂСѓС‚Р°,
РЅРё `PodcastsPage`, РЅРё `features/podcast/`.

**Р§С‚Рѕ РґРёРЅР°РјРёС‡РЅРѕ** (СЃР»РѕС‚-СЃРёСЃС‚РµРјР°):
- `AdminSlot::NavItem` вЂ” nav items СЂРµРіРёСЃС‚СЂРёСЂСѓСЋС‚СЃСЏ С‡РµСЂРµР· `register_component()` вњ…
- `AdminSlot::DashboardSection` вЂ” РІРёРґР¶РµС‚С‹ РґР°С€Р±РѕСЂРґР° вњ…
- `StorefrontSlot::*` вЂ” СЃР»РѕС‚С‹ РІРёС‚СЂРёРЅС‹ вњ…

РќРѕ РґР°Р¶Рµ РґР»СЏ СЃР»РѕС‚РѕРІ: С„СѓРЅРєС†РёСЏ `render: fn() -> AnyView` РґРѕР»Р¶РЅР° Р±С‹С‚СЊ
**СЃРєРѕРјРїРёР»РёСЂРѕРІР°РЅР° РІ WASM Р·Р°СЂР°РЅРµРµ**. РЎР»РѕС‚-СЃРёСЃС‚РµРјР° СѓРїСЂР°РІР»СЏРµС‚ РІРёРґРёРјРѕСЃС‚СЊСЋ,
Р° РЅРµ Р·Р°РіСЂСѓР·РєРѕР№ РєРѕРґР°.

**Р§С‚Рѕ РЅСѓР¶РЅРѕ СЃРґРµР»Р°С‚СЊ**:

1. **UI РІ РїРѕРґРїР°РїРєР°С… РјРѕРґСѓР»СЏ** (РЅР°РїСЂ. `crates/rustok-workflow/admin/`):

```text
crates/rustok-workflow/
в”њв”Ђв”Ђ Cargo.toml          # rustok-workflow (backend)
в”њв”Ђв”Ђ src/                # backend logic
в”њв”Ђв”Ђ admin/
в”‚   в”њв”Ђв”Ђ Cargo.toml      # rustok-workflow-admin (publishable)
в”‚   в””в”Ђв”Ђ src/            # Leptos components & register_routes()
в””в”Ђв”Ђ storefront/
    в”њв”Ђв”Ђ Cargo.toml      # rustok-workflow-storefront (publishable)
    в””в”Ђв”Ђ src/            # Leptos SSR components
```

2. **`apps/admin/build.rs`** уже доведён до рабочего manifest-driven wiring:
```rust
// generated/registry.rs
register_component(AdminComponentRegistration {
    module_slug: Some("blog"),
    slot: AdminSlot::DashboardSection,
    render: render_blog_dashboard_section,
});
```

Состояние на 2026-03-29:
- generated host wiring для admin уже монтирует module-owned `<PascalSlug>Admin`
  как `DashboardSection`, `NavItem` и `AdminPageRegistration`;
- host routes `/modules/:module_slug` и `/modules/:module_slug/*module_path` резолвят модульную страницу через generated registry и прокидывают `UiRouteContext`;
- `[provides.admin_ui]` дополнительно поддерживает optional `route_segment`, `nav_label` и `[[provides.admin_ui.pages]]` для generic secondary nav.
- `workflow`, `search` и `channel` уже используют nested/module root contracts без ручного router/sidebar special-case в `apps/admin`.
- `blog`, `commerce`, `forum`, `pages` и `search` уже являются живыми dual-surface proof point-ами: их admin/storefront пакеты объявлены в `rustok-module.toml` и реально подхватываются host-приложениями.

Что ещё остаётся открытым:
- более явный entry-point contract для внешних crate-ов вместо чистых naming conventions.
- явный operational status для UI-пакетов, которые уже существуют в workspace, но ещё не подключены через manifest-driven contract.

3. **`apps/storefront/build.rs`** уже генерирует `register_component()` и `register_page()`:
```rust
// generated/registrations.rs
register_component(StorefrontComponentRegistration {
    module_slug: Some("blog"),
    slot: StorefrontSlot::HomeAfterCatalog,
    render: render_blog_storefront_view,
});

register_page(StorefrontPageRegistration {
    module_slug: "blog",
    route_segment: "blog",
    title: "Blog",
    render: render_blog_storefront_view,
});
```

4. **`BuildExecutor`** СЃРѕР±РёСЂР°РµС‚ С‚СЂРё Р°СЂС‚РµС„Р°РєС‚Р°:
```
cargo build -p rustok-server          // Р±РёРЅР°СЂРЅРёРє СЃРµСЂРІРµСЂР° (СЃРµР№С‡Р°СЃ вњ…)
trunk build apps/admin                // admin artifact     (СЃРµР№С‡Р°СЃ вњ…)
cargo build -p rustok-storefront      // storefront binary  (СЃРµР№С‡Р°СЃ вњ…)
```

**`rustok-module.toml`** РѕР±СЉСЏРІР»СЏРµС‚ UI С‚РѕС‡РєРё РІС…РѕРґР°:
```toml
[provides.admin_ui]
leptos_crate  = "rustok-search-admin"
route_segment = "search"
nav_label     = "Search"

[[provides.admin_ui.pages]]
subpath       = "dictionaries"
title         = "Search Dictionaries"
nav_label     = "Dictionaries"

[provides.storefront_ui]
leptos_crate  = "rustok-search-storefront"
slot          = "home_after_hero"
route_segment = "search"
page_title    = "Search"
```

**Р—Р°РІРёСЃРёС‚ РѕС‚**: РєРѕРґРѕРіРµРЅРµСЂР°С†РёРё `build.rs` (Рї.6 РІС‹С€Рµ).

---

## 9. РџСЂРёРѕСЂРёС‚РµС‚ РЅРµР·Р°РІРµСЂС€С‘РЅРЅРѕРіРѕ

| # | Р—Р°РґР°С‡Р° | РЎР»РѕР¶РЅРѕСЃС‚СЊ | Р¦РµРЅРЅРѕСЃС‚СЊ |
|---|---|---|---|
| **1** | **Довести manifest-wired Leptos UI coverage до последовательного состояния** | Большая | Критическая — dual-surface exemplar-ы уже есть у `blog`, `commerce`, `forum`, `pages`, `search`; admin-only slice сейчас осознанно остаётся у `workflow` и `channel`, но у остальных модулей отсутствие manifest registration всё ещё не должно маскироваться наличием подпапки или отдельного crate |
| 2 | Внешний реестр V1 (read-only catalog) | Большая | Высокая — фундамент marketplace |
| 3 | Внешний реестр V2 + publish/governance | Очень большая | Средняя — следующий шаг после read-only каталога; первый live cut уже поднят (`publish request` + `artifact upload` + `status` + live `yank`, а `cargo xtask module publish/yank` уже умеет ходить в этот V2 API), но async validation, ownership/governance и отдельный внешний deployment ещё впереди |
| 4 | richer nested schema metadata для type-directed subforms beyond generic JSON tree | Средняя | Средняя — текущий editor уже рабочий, recursive `shape` уже доходит до deep JSON tree и даёт schema-driven add actions / key locking, nested child editors уже встроены inline, а nested scalar controls уже уважают child `type` / `options` / bounds; основной остаток теперь в fully type-directed child forms для complex descendants |

> П. 3 остаётся текущим UI/extensibility блоком: после закрытия nested admin contract главный остаток — перевести больше модулей в publishable пакеты без scaffold-only результата.

### Р§С‚Рѕ РёР·РјРµРЅРёР»РѕСЃСЊ (2026-03-18) вЂ” С„РёРЅР°Р»СЊРЅС‹Р№ РѕСЂРёРµРЅС‚РёСЂ

РџСЂРёРЅСЏС‚С‹Рµ СЂРµС€РµРЅРёСЏ РїРѕ СЃС‚СЂСѓРєС‚СѓСЂРµ UI:

1. **Leptos UI** вЂ” РІС‹РЅРµСЃРµРЅ РІ РѕС‚РґРµР»СЊРЅС‹Рµ publishable СЃСѓР±-РєСЂРµР№С‚С‹ `admin/` Рё `storefront/` РІРЅСѓС‚СЂРё РїР°РїРєРё РјРѕРґСѓР»СЏ. Р­С‚Рѕ РїРѕР·РІРѕР»СЏРµС‚ РїСѓР±Р»РёРєРѕРІР°С‚СЊ РёС… РІ crates.io Рё РјРёРЅРёРјРёР·РёСЂРѕРІР°С‚СЊ Р·Р°РІРёСЃРёРјРѕСЃС‚Рё РѕСЃРЅРѕРІРЅРѕРіРѕ Р±РµРєРµРЅРґ-РєСЂРµР№С‚Р°.
2. **Next.js UI** вЂ” РїРµСЂРµРЅС‘СЃС‘РЅ РІ `apps/*/packages/<module>/` РІ РІРёРґРµ Р»РѕРєР°Р»СЊРЅС‹С… npm-РїР°РєРµС‚РѕРІ. Р­С‚Рѕ РѕР±РµСЃРїРµС‡РёРІР°РµС‚ РёР·РѕР»СЏС†РёСЋ РєРѕРґР° РјРѕРґСѓР»РµР№ РѕС‚ РѕСЃРЅРѕРІРЅРѕРіРѕ РїСЂРёР»РѕР¶РµРЅРёСЏ РїСЂРё СЃРѕС…СЂР°РЅРµРЅРёРё РІРѕР·РјРѕР¶РЅРѕСЃС‚Рё РїСѓР±Р»РёРєР°С†РёРё РІ npm.
3. **РђРІС‚Рѕ-РґРµРїР»РѕР№** вЂ” СЂР°Р±РѕС‚Р°РµС‚ **С‚РѕР»СЊРєРѕ РґР»СЏ Leptos** С‡РµСЂРµР· BuildExecutor. Next.js РїСЂРёР»РѕР¶РµРЅРёСЏ С‚СЂРµР±СѓСЋС‚ СЂСѓС‡РЅРѕР№ СЃР±РѕСЂРєРё/РѕР±РЅРѕРІР»РµРЅРёСЏ `package.json`.
4. **РњР°РЅРёС„РµСЃС‚** вЂ” `rustok-module.toml` С‚РµРїРµСЂСЊ РґРѕР»Р¶РµРЅ СЏРІРЅРѕ СѓРєР°Р·С‹РІР°С‚СЊ РёРјРµРЅР° UI-РїР°РєРµС‚РѕРІ (`leptos_crate`, `next_package`).

РЎРІСЏР·Р°РЅРЅС‹Р№ ADR: `DECISIONS/2026-03-17-dual-ui-strategy-next-batteries-included.md` (РѕР±РЅРѕРІР»РµРЅ 2026-03-18).



