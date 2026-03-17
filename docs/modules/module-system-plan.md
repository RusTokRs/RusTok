# RusTok — Система модулей: полная карта и план

> **Дата**: 2026-03-17
> **Назначение**: полная карта реализации — что сделано, где документировано,
> что осталось. Служит основой для периодической верификации корректности.
>
> Легенда: ✅ реализовано · ⚠️ частично · ⬜ не начато

---

## Содержание

1. [Стандарт модуля](#1-стандарт-модуля)
2. [Tenant-level toggle](#2-tenant-level-toggle)
3. [Platform-level install/uninstall](#3-platform-level-installuninstall)
4. [Build pipeline](#4-build-pipeline)
5. [Marketplace каталог](#5-marketplace-каталог)
6. [Admin UI](#6-admin-ui)
7. [Внешний реестр и публикация](#7-внешний-реестр-и-публикация)
8. [Архитектурный долг](#8-архитектурный-долг)
9. [Приоритет незавершённого](#9-приоритет-незавершённого)

---

## 1. Стандарт модуля

### ✅ `rustok-module.toml` — манифест модуля

Каждый path-модуль обязан иметь `rustok-module.toml` в корне crate.
Парсится в `ManifestManager::catalog_modules()` и `apply_module_package_manifest()`.

| Секция | Что содержит | Статус |
|---|---|---|
| `[module]` | slug, name, version, description, authors, license | ✅ парсится |
| `[marketplace]` | icon, banner, screenshots, category, tags | ✅ парсится |
| `[compatibility]` | rustok_min, rustok_max | ✅ парсится |
| `[dependencies]` | depends_on с version_req | ⚠️ slug проверяется, version_req игнорируется |
| `[conflicts]` | несовместимые модули | ⚠️ парсится, но не проверяется |
| `[crate]` | name, entry_type | ✅ парсится |
| `[provides]` | migrations, permissions, events, admin_nav, storefront_slots, graphql | ✅ парсится |
| `[settings]` | схема настроек модуля (type, default, min, max) | ⚠️ парсится, но нет API для записи |
| `[locales]` | supported, default | ✅ парсится |

**Файлы**:
- `apps/server/src/modules/manifest.rs` — `apply_module_package_manifest()`

**Документация**:
- `docs/modules/manifest.md`

---

### ✅ Структура файлов модуля

```
crates/rustok-{slug}/
├── rustok-module.toml       # обязательно для path-модулей
├── Cargo.toml
├── src/
│   ├── lib.rs               # impl RusToKModule
│   └── migrations/          # impl MigrationSource (после migration distribution)
│       ├── mod.rs
│       └── m20250101_*.rs
├── admin/                   # Leptos-компоненты (опционально)
└── storefront/              # Slot-виджеты (опционально)
```

**Документация**:
- `docs/architecture/modules.md`

---

### ✅ Контракт `RusToKModule`

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

**Файлы**:
- `crates/rustok-core/src/module.rs`
- `crates/rustok-core/src/registry.rs`

**Документация**:
- `docs/architecture/modules.md`

---

### ✅ Migration distribution

Каждый модуль несёт миграции внутри своего crate (`src/migrations/`).
При старте бинарника `registry.migrations()` собирает миграции всех модулей
и прогоняет их автоматически. Вручную добавлять файлы в `apps/server/migration/` не нужно.

**Файлы**:
- `crates/rustok-*/src/migrations/` — миграции каждого модуля
- `crates/rustok-core/src/registry.rs` — `ModuleRegistry::migrations()`

---

## 2. Tenant-level toggle

### ✅ Схема `tenant_modules`

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

**Файлы**:
- `apps/server/migration/src/m20250101_000003_create_tenant_modules.rs`
- `crates/rustok-tenant/src/entities/tenant_module.rs`
- `apps/server/src/models/tenant_modules.rs`

---

### ✅ `ModuleLifecycleService::toggle_module`

Flow:
1. slug ∈ `ModuleRegistry` → иначе `UnknownModule`
2. не `ModuleKind::Core` → иначе `CoreModuleCannotBeDisabled`
3. `enabled=true`: все `depends_on` включены → иначе `MissingDependencies`
4. `enabled=false`: нет зависящих от него → иначе `HasDependents`
5. `BEGIN TRANSACTION` → UPDATE `tenant_modules` → `on_enable()` / `on_disable()`
6. При `HookFailed` — откат состояния в транзакции

**Файлы**:
- `apps/server/src/services/module_lifecycle.rs`

**Тесты**:
- `apps/server/tests/module_lifecycle.rs`

---

### ✅ GraphQL `toggleModule`

```graphql
mutation {
  toggleModule(moduleSlug: "blog", enabled: true) {
    moduleSlug enabled settings
  }
}
```

**Файлы**:
- `apps/server/src/graphql/mutations.rs` — `async fn toggle_module`

---

### ✅ `EnabledModulesProvider` + `<ModuleGuard>` (Leptos)

`EnabledModulesProvider` загружает включённые модули при старте и предоставляет
контекст всему приложению. `<ModuleGuard slug="blog">` рендерит children только
если модуль включён.

**Файлы**:
- `apps/admin/src/shared/context/enabled_modules.rs`

---

### ✅ Фильтрация slot-компонентов

`components_for_slot(slot_id, enabled_modules)` фильтрует виджеты витрины
по включённым модулям тенанта перед рендером.

**Файлы**:
- `apps/storefront/src/modules/registry.rs`

---

### ⚠️ Настройки модуля — нет API записи

Колонка `settings JSON` в `tenant_modules` есть. `on_enable()` может записать
дефолты. Но нет GraphQL мутации для обновления настроек через UI.

**Что нужно**:
```graphql
mutation {
  updateModuleSettings(moduleSlug: "blog", settings: { postsPerPage: 20 }): TenantModule!
}
```

Серверная сторона (`apps/server/src/graphql/mutations.rs`):
```rust
async fn update_module_settings(
    &self, ctx: &Context<'_>,
    module_slug: String,
    settings: serde_json::Value,
) -> Result<TenantModule> {
    // 1. Проверить что модуль включён для тенанта
    // 2. Валидировать по JSON Schema из [settings] rustok-module.toml
    // 3. UPDATE tenant_modules SET settings = ?
}
```

UI: форма из `[settings]` секции `rustok-module.toml`, в детальной панели `/modules`.

---

## 3. Platform-level install/uninstall

### ✅ `ManifestManager`

```rust
ManifestManager::load()                     // парсить modules.toml
ManifestManager::save(manifest)             // сохранить modules.toml
ManifestManager::validate(manifest)         // проверить граф зависимостей
ManifestManager::validate_with_registry()   // сверить с ModuleRegistry
ManifestManager::install_builtin_module()   // добавить в modules.toml
ManifestManager::uninstall_module()         // удалить из modules.toml
ManifestManager::upgrade_module()           // обновить версию
ManifestManager::catalog_modules()          // для MarketplaceCatalogService
ManifestManager::build_modules()            // для BuildService
ManifestManager::build_execution_plan()     // для BuildExecutor
```

**Файлы**:
- `apps/server/src/modules/manifest.rs`

**Документация**:
- `docs/modules/manifest.md`
- `docs/architecture/modules.md`

---

### ✅ GraphQL мутации install/uninstall/upgrade/rollback

```graphql
installModule(slug: String!, version: String): BuildJob!
uninstallModule(slug: String!): BuildJob!
upgradeModule(slug: String!, version: String!): BuildJob!
rollbackBuild(buildId: ID!): BuildJob!
```

**Файлы**:
- `apps/server/src/graphql/mutations.rs`

---

### ⚠️ Semver-валидация зависимостей и конфликтов

`ManifestManager::validate()` проверяет только факт наличия slug в манифесте.
Диапазоны версий в `[dependencies]` (`>= 1.0.0`, `~2.0`) не проверяются.
Секция `[conflicts]` парсится, но нигде не проверяется.

**Что нужно** в `apps/server/src/modules/manifest.rs`:
```rust
// Для каждой зависимости:
let req = semver::VersionReq::parse(&dep.version_req)?;
let installed = semver::Version::parse(&installed_spec.version)?;
if !req.matches(&installed) {
    return Err(IncompatibleDependencyVersion { ... });
}

// Для конфликтов:
if manifest.modules.contains_key(&conflict_slug) {
    return Err(ConflictingModule { slug, conflicts_with: conflict_slug });
}
```

Добавить `semver = "1"` в `apps/server/Cargo.toml`.

---

## 4. Build pipeline

### ✅ `BuildService`

```rust
BuildService::request_build(request)   // создать Build, хешировать, дедублировать
BuildService::get_build(build_id)
BuildService::active_build()           // следующий queued/running
BuildService::running_build()
```

Дедупликация: если в очереди уже есть build с таким же SHA-256 `modules_delta` —
возвращает существующий вместо создания нового.

**Файлы**:
- `apps/server/src/services/build_service.rs`
- `apps/server/src/models/build.rs` — `BuildStatus`, `BuildStage`, `DeploymentProfile`
- `apps/server/migration/src/m20250212_000001_create_builds_and_releases.rs`

---

### ✅ `BuildExecutor` — cargo build

Выполняет `cargo build -p rustok-server` с feature flags из установленных модулей.
Обновляет `builds.stage` и `builds.progress` по ходу выполнения.
Создаёт запись в `releases` при успехе.

**Файлы**:
- `apps/server/src/services/build_executor.rs`

**Env vars**:
- `RUSTOK_BUILD_CARGO_BIN` — путь к cargo (default: `cargo`)

---

### ✅ `buildProgress` GraphQL subscription

Истинный push через `tokio::sync::broadcast` канал.
`BuildEventHub` рассылает события по мере выполнения build executor'а.

```graphql
subscription {
  buildProgress(buildId: "...") { status stage progress logsUrl }
}
```

**Файлы**:
- `apps/server/src/graphql/subscriptions.rs`

---

### ✅ GraphQL queries для builds

```graphql
activeBuild: BuildJob
buildHistory(limit: Int, offset: Int): [BuildJob!]!
```

**Файлы**:
- `apps/server/src/graphql/queries.rs`

---

### ✅ `rollback_build`

Проверяет цепочку релизов через `releases.previous_release_id`.
Повторно активирует предыдущий `Release`. Полноценный откат, не просто смена статуса.

**Файлы**:
- `apps/server/src/graphql/mutations.rs` — `async fn rollback_build`
- `apps/server/src/models/release.rs`

---

### ⚠️ Docker deploy — не реализован

`BuildExecutor` выполняет только `cargo build`. После компиляции создаётся
`Release` запись, но `container_image` и `server_artifact_url` остаются пустыми.
`ReleaseStatus::Deploying` / `Active` не используются по назначению.

**Что нужно** в `apps/server/src/services/build_executor.rs`:
```
Stage: Deploy (progress 85–99%)
  1. docker build -t {registry}/rustok-server:{release_id} .
  2. docker push
  3. Заполнить releases.container_image
  4. Rolling restart (monolith) | kubectl rollout (K8s)
```

**Новые env vars**:
- `RUSTOK_BUILD_DOCKER_BIN` — путь к docker (default: `docker`)
- `RUSTOK_BUILD_REGISTRY` — registry URL
- `RUSTOK_DEPLOY_MODE` — `monolith` | `docker` | `k8s`

---

### ⚠️ Build progress UI — polling вместо subscription

Прогресс-бар в `/modules` обновляется опросом каждые 5 секунд
(`use_interval_fn(refresh_live_state, 5000)`).
Бэкенд-subscription (`buildProgress`) реализована, но UI к ней не подключён.

**Что нужно** в `apps/admin/src/features/modules/components/modules_list.rs`:
```rust
// Заменить:
use_interval_fn(refresh_live_state, 5000);
// На:
let _sub = use_graphql_subscription::<BuildProgressSubscription>(
    BuildProgressSubscriptionVariables { build_id: active_build_id },
    move |event| set_build.set(Some(event.build_progress)),
);
```

`leptos-graphql` уже поддерживает subscriptions.

---

## 5. Marketplace каталог

### ✅ `MarketplaceCatalogService` — provider chain

```
MarketplaceCatalogService
  ├─ LocalManifestMarketplaceProvider   → встроенные path-модули из modules.toml
  └─ RegistryMarketplaceProvider        → внешний реестр (RUSTOK_MARKETPLACE_REGISTRY_URL)
       └─ moka cache (TTL: RUSTOK_MARKETPLACE_REGISTRY_CACHE_TTL_SECS, default 60s)
```

При недоступности реестра — graceful fallback на local-manifest.
Дедупликация: побеждает первый провайдер.

**Файлы**:
- `apps/server/src/services/marketplace_catalog.rs`

**Env vars**:
- `RUSTOK_MARKETPLACE_REGISTRY_URL`
- `RUSTOK_MARKETPLACE_REGISTRY_TIMEOUT_MS` (default: 3000)
- `RUSTOK_MARKETPLACE_REGISTRY_CACHE_TTL_SECS` (default: 60)

---

### ✅ GraphQL `marketplace` + `marketplaceModule`

```graphql
marketplace(
  search: String
  category: String
  source: String          # "local" | "registry"
  installed: Boolean
  trust_level: String     # "first_party" | "third_party" | "community"
  compatible_only: Boolean
): [MarketplaceModule!]!

marketplaceModule(slug: String!): MarketplaceModule
```

**Файлы**:
- `apps/server/src/graphql/queries.rs`

---

### ✅ Deep-link `?module=slug`

Выбранный модуль в каталоге отражается в URL (`/modules?module=blog`).
Прямая ссылка открывает детальную панель без перехода.

**Файлы**:
- `apps/admin/src/features/modules/components/modules_list.rs`

---

### ⬜ Внешний реестр `modules.rustok.dev`

`RegistryMarketplaceProvider` делает HTTP-запросы, но сам сервис не существует.

**Scope V1** (read-only, first-party модули):
```
modules.rustok.dev
└── GET /v1/catalog → [{ slug, name, version, ... }]
```
Позволяет проверить весь `RegistryMarketplaceProvider` → AdminUI flow.

**Scope V2** (полный):
```
modules.rustok.dev
├── GraphQL API (каталог, версии, поиск, publish, yank)
├── Crate Storage (S3: .crate архивы + checksums)
└── Validation Pipeline (static → audit → compile → test → metadata)
```

**Аутентификация**: `docs/concepts/plan-oauth2-app-connections.md` (Приложение A).

---

## 6. Admin UI

### ✅ Страница `/modules`

| Элемент | Статус |
|---|---|
| Список установленных модулей (`modules` query) | ✅ |
| Каталог маркетплейса (`marketplace` query) | ✅ |
| Фильтры: поиск, категория, trust level, compatibility | ✅ |
| Детальная панель `marketplaceModule(slug)` | ✅ |
| Deep-link `?module=slug` | ✅ |
| Install / Uninstall кнопки → `installModule` / `uninstallModule` | ✅ |
| Toggle switch → `toggleModule` | ✅ |
| Секции: Installed / Marketplace / Updates | ✅ |
| Прогресс-бар build (polling 5 сек) | ✅ (но не real-time) |
| Прогресс-бар build (WebSocket subscription) | ⚠️ не подключён |
| "Update available" badge + `upgradeModule` кнопка | ⬜ нет |
| Форма настроек модуля (`updateModuleSettings`) | ⬜ нет |

---

## 7. Внешний реестр и публикация

### ⬜ `rustok mod publish` CLI

```bash
rustok mod init          # Шаблон модуля с rustok-module.toml
rustok mod validate      # Локальная проверка манифеста
rustok mod test          # Validation pipeline локально
rustok mod publish       # Опубликовать в реестр
rustok mod yank 1.2.0    # Отозвать версию
```

Зависит от п. [внешний реестр](#-внешний-реестр-modulesrustokdev).

---

### ⬜ Validation pipeline для публикации

| Стадия | Проверки |
|---|---|
| 1. Static | манифест валиден, slug уникален, semver, license, locales/en.json |
| 2. Security | cargo-audit, отсутствие unsafe без обоснования, нет std::process::Command |
| 3. Compilation | компилируется с rustok_min..rustok_max |
| 4. Runtime | cargo test, миграции up/down идемпотентны, on_enable/on_disable |
| 5. Metadata | icon.svg валиден, description >= 20 символов, screenshots |

---

## 8. Архитектурный долг

### ⬜ GraphQL и REST модулей живут в сервере, а не в модульных крейтах

**Текущее состояние**:

GraphQL и REST адаптеры для каждого модуля сейчас в `apps/server/`:

```
apps/server/src/
├── graphql/
│   ├── blog/        (~535 строк)   ← знает о rustok_blog::PostService
│   ├── content/     (~723 строк)   ← знает о rustok_content::NodeService
│   ├── commerce/    (~682 строк)   ← знает о rustok_commerce::CatalogService
│   ├── forum/       (~740 строк)   ← знает о rustok_forum::TopicService
│   ├── pages/       (~823 строк)   ← знает о rustok_pages::PageService
│   ├── workflow/    (~1071 строк)  ← знает о rustok_workflow::WorkflowService
│   ├── alloy/       (~799 строк)   ← знает о alloy_scripting::ScriptRegistry
│   └── media/       (~233 строк)   ← знает о rustok_media::MediaService
└── controllers/
    ├── blog/        (~271 строк)   ← то же самое для REST
    ├── content/     (~199 строк)
    ├── commerce/    (~1149 строк)
    ├── forum/       (~638 строк)
    ├── pages/       (~297 строк)
    ├── workflow/    (~272 строк)
    └── media/       (~191 строк)
```

**Почему это проблема**:
Сторонний модуль из маркетплейса не может добавить свой GraphQL/REST без правки
`apps/server/`. Это нарушает концепцию самодостаточного модуля.

**Почему не в `rustok-core`**:
`async-graphql`, `axum`, `loco_rs` — тяжёлые web-зависимости. Они не должны
попадать в доменное ядро. Модульный крейт должен оставаться framework-agnostic.

**Правильное решение — новый крейт `rustok-api`**:

```
crates/rustok-api/
  └── src/
      ├── context.rs       ← TenantContext, AuthContext (из apps/server/src/context/)
      ├── graphql/
      │   ├── common.rs    ← require_module_enabled, resolve_graphql_locale
      │   └── errors.rs    ← GraphQLError
      └── extractors/
          └── rbac.rs      ← базовые RBAC extractor трейты
  # зависит от: async-graphql, axum, loco_rs, rustok-core
```

После этого каждый модуль держит GraphQL + REST у себя:

```
crates/rustok-blog/src/
├── graphql/      ← переехало из apps/server/src/graphql/blog/
│   ├── mod.rs    ← pub struct BlogQuery; pub struct BlogMutation;
│   ├── query.rs
│   ├── mutation.rs
│   └── types.rs
└── controllers/  ← переехало из apps/server/src/controllers/blog/
    ├── mod.rs    ← pub fn routes() -> Routes
    └── posts.rs
```

Сервер — только composition root:
```rust
// apps/server/src/graphql/schema.rs
#[cfg(feature = "mod-blog")]
use rustok_blog::graphql::{BlogMutation, BlogQuery};
```

**Что нужно сделать**:
1. Создать `crates/rustok-api/` с общими типами из сервера
2. Перенести `graphql/{blog,content,commerce,forum,pages,workflow,alloy,media}/`
   в соответствующие модульные крейты
3. Перенести `controllers/{blog,content,commerce,forum,pages,workflow,media}/`
   в соответствующие модульные крейты
4. Обновить `apps/server/src/graphql/schema.rs` и `apps/server/src/app.rs`

**Открытый вопрос**: точный состав `rustok-api` — что именно туда идёт,
какие трейты нужны для абстракции `AppContext`.

---

### ⬜ Кодогенерация регистрации модулей (`build.rs`)

**Текущее состояние** — три места прошиты вручную:

```rust
// 1. apps/server/src/modules/mod.rs — build_registry()
registry.register(BlogModule);     // ← каждый модуль явно
registry.register(CommerceModule); // ← сторонний сюда не попадёт

// 2. apps/server/src/graphql/schema.rs — статический MergedObject
#[derive(MergedObject, Default)]
pub struct Query(
    #[cfg(feature = "mod-blog")] BlogQuery,   // ← compile-time типы
    // PodcastQuery стороннего модуля сюда не попадёт
);

// 3. apps/server/src/app.rs — маршруты
.add_route(controllers::blog::routes()) // ← сторонний не добавится
```

Для установки стороннего модуля нужно вручную менять все три файла.
Это не маркетплейс — это ручная интеграция.

**Решение — `apps/server/build.rs`**:

`build.rs` читает `modules.toml` и генерирует три файла:

```
apps/server/src/generated/
├── registry.rs   ← вызовы register() для всех установленных модулей
├── schema.rs     ← MergedObject с Query/Mutation всех модулей
└── routes.rs     ← add_route() для всех модулей
```

Каждый модуль экспортирует стандартные точки входа (после переноса в крейт):
```rust
// crates/rustok-podcast/src/lib.rs
pub mod graphql { pub struct PodcastQuery; pub struct PodcastMutation; }
pub mod controllers { pub fn routes() -> Routes { ... } }
```

`build.rs` знает что генерировать из `rustok-module.toml`:
```toml
[provides.graphql]
query_type = "PodcastQuery"
mutation_type = "PodcastMutation"

[provides.http]
routes_fn = "controllers::routes"
```

**Итог**: сервер больше никогда не трогается вручную при установке модуля.
`modules.toml` → кодогенерация → `cargo build` → бинарник с новым модулем.

**Зависит от**: `rustok-api` + перенос GraphQL/REST в модульные крейты.

---

### ⬜ UI тоже должен пересобираться — admin WASM и storefront WASM

**Ключевой факт**: Leptos компилируется в WASM. Как сервер → бинарник,
так admin и storefront → `.wasm`. Динамически подгрузить новый Rust-код
в runtime невозможно. Любой новый модуль = пересборка WASM.

**Что прошито вручную в admin**:

```
apps/admin/src/
├── pages/mod.rs         ← mod workflows; mod workflow_detail;  (явные объявления)
├── pages/workflows.rs   ← страница Workflows
├── pages/workflow_detail.rs
├── features/workflow/   ← компоненты workflow (400+ строк)
└── app/router.rs        ← Route path="/workflows" view=Workflows
```

Для стороннего `rustok-podcast`: нет ни `/podcasts` маршрута,
ни `PodcastsPage`, ни `features/podcast/`.

**Что динамично** (слот-система):
- `AdminSlot::NavItem` — nav items регистрируются через `register_component()` ✅
- `AdminSlot::DashboardSection` — виджеты дашборда ✅
- `StorefrontSlot::*` — слоты витрины ✅

Но даже для слотов: функция `render: fn() -> AnyView` должна быть
**скомпилирована в WASM заранее**. Слот-система управляет видимостью,
а не загрузкой кода.

**Что нужно сделать**:

1. **Перенести UI в модульные крейты** (аналогично GraphQL/REST):
```
crates/rustok-workflow/
└── ui/
    ├── admin/
    │   ├── pages/          ← WorkflowsPage, WorkflowDetailPage
    │   ├── features/       ← компоненты
    │   └── mod.rs          ← pub fn register_routes() + pub fn register_components()
    └── storefront/
        └── mod.rs          ← pub fn register_slot_components()
```

2. **`apps/admin/build.rs`** генерирует из `modules.toml`:
```rust
// generated/routes.rs
<Route path=path!("/workflows") view=rustok_workflow::ui::admin::WorkflowsPage />
<Route path=path!("/workflows/:id") view=rustok_workflow::ui::admin::WorkflowDetailPage />
```

3. **`apps/storefront/build.rs`** генерирует вызовы `register_component()`:
```rust
// generated/registrations.rs
rustok_workflow::ui::storefront::register_slot_components();
```

4. **`BuildExecutor`** собирает три артефакта:
```
cargo build -p rustok-server          // бинарник сервера (сейчас ✅)
wasm-pack build apps/admin            // admin WASM         (⬜ не реализовано)
cargo build -p rustok-storefront      // storefront         (⬜ не реализовано)
```

**`rustok-module.toml`** объявляет UI точки входа:
```toml
[provides.admin_ui]
routes_fn    = "ui::admin::register_routes"
components_fn = "ui::admin::register_components"

[provides.storefront_ui]
components_fn = "ui::storefront::register_slot_components"
```

**Зависит от**: кодогенерации `build.rs` (п.6 выше).

---

## 9. Приоритет незавершённого

| # | Задача | Сложность | Ценность |
|---|---|---|---|
| 1 | Semver + conflict валидация | Малая | Высокая — защита от broken installs |
| 2 | `updateModuleSettings` мутация | Малая | Высокая — `[settings]` уже везде есть |
| 3 | Build progress → subscription | Малая | Средняя — UX, инфраструктура уже есть |
| 4 | Docker deploy в BuildExecutor | Средняя | Высокая — без этого install не prod-ready |
| 5 | `rustok-api` + перенос GraphQL/REST в крейты | Большая | Критическая — блокирует 3rd party |
| 6 | Перенос UI (admin/storefront) в модульные крейты | Большая | Критическая — блокирует 3rd party |
| 7 | `build.rs` кодогенерация (сервер + admin + storefront) | Большая | Критическая — блокирует 3rd party |
| 8 | `BuildExecutor`: сборка admin WASM + storefront | Средняя | Критическая — без этого install неполный |
| 9 | Внешний реестр V1 (read-only) | Большая | Высокая — основа marketplace |
| 10 | Внешний реестр V2 + publish | Очень большая | Средняя — нужен только для 3rd party |

> Пп. 5, 6, 7, 8 — единый блок. Все четыре нужны вместе, чтобы
> сторонний модуль полноценно заработал (сервер + UI).
