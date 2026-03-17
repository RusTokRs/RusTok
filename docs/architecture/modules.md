# Module Architecture

RusToK реализован как **Modular Monolith**: все модули компилируются в единый бинарник и поднимаются через `ModuleRegistry`.

## Ключевой принцип

Не каждый критичный компонент платформы реализует `RusToKModule`.
Есть три категории компонентов — подробно описаны ниже. Core-модули трактуются как обязательный слой платформы.

Отдельное правило platform boundaries: инфраструктурные возможности framework-уровня (например, Loco Mailer и Loco Storage в `apps/server`) не выносятся в отдельные `rustok-*` модули; они остаются server-infra слоем и потребляются доменными модулями через адаптеры.

## Категория A — Mandatory Core Crates (не `RusToKModule`)

Всегда линкуются в бинарник, не участвуют в lifecycle модулей:

| Crate | Роль |
|-------|------|
| `rustok-core` | Контракты, EventBus, кэш, Circuit Breaker — само ядро платформы |
| `rustok-outbox` | `TransactionalEventBus` для надёжной доставки событий |
| `rustok-iggy` + `rustok-iggy-connector` | L2 streaming transport (опционально) |
| `rustok-telemetry` | OpenTelemetry, tracing, Prometheus — сквозная зависимость |
| `rustok-storage` | Leaf crate: `StorageBackend` async trait + `LocalStorage`, `StorageService` ([docs](../../crates/rustok-storage/docs/README.md)) |
| `alloy-scripting` | Скриптовый движок, инициализируется напрямую в `app.rs` |
| `rustok-mcp` | MCP адаптер, отдельный сервер |
| `tailwind-rs/css/ast` | Build-time CSS инструментарий |
| `rustok-test-utils` | **Только `[dev-dependencies]`**, никогда не попадает в production |

## Категория B — Core Platform Modules (`ModuleKind::Core`)

Реализуют `RusToKModule`, обязательны для работы платформы, нельзя отключить.
В документации и в server-контексте они считаются критичным core-модульным baseline:

| Crate | Slug | Назначение |
|-------|------|-----------|
| `rustok-index` | `index` | **Core (critical)**: CQRS read-model, индексатор для storefront |
| `rustok-tenant` | `tenant` | **Core (critical)**: Tenant metadata, lifecycle hooks |
| `rustok-rbac` | `rbac` | **Core (critical)**: RBAC helpers, lifecycle hooks |
| `rustok-media` | `media` | **Core (feature `mod-media`)**: Upload, storage, translations. `MediaService` + SeaORM entities. ([docs](../../crates/rustok-media/docs/README.md)) |

Итоговые обязательные core-модули платформы (`ModuleKind::Core`): `rustok-index`, `rustok-tenant`, `rustok-rbac`.

`rustok-core`, `rustok-outbox`, `rustok-telemetry` — обязательные **core (critical)** модули платформы (инициализируются напрямую в runtime).

## Категория C — Optional Domain Modules (`ModuleKind::Optional`)

Реализуют `RusToKModule`, управляются per-tenant через `tenant_modules`:

| Crate | Slug | Тип | Depends on |
|-------|------|-----|-----------|
| `rustok-content` | `content` | Domain (фактически required) | `rustok-core` |
| `rustok-commerce` | `commerce` | Domain | `rustok-core` |
| `rustok-blog` | `blog` | Wrapper | `rustok-content` |
| `rustok-forum` | `forum` | Wrapper | `rustok-content` |
| `rustok-pages` | `pages` | Domain | `rustok-core` |

**Wrapper-паттерн:** `rustok-blog` и `rustok-forum` не имеют собственных таблиц контента — они используют nodes из `rustok-content` с разными значениями поля `kind`.

## Где смотреть в коде

| Что | Где |
|-----|-----|
| Runtime-регистрация модулей | `apps/server/src/modules/mod.rs` |
| Синхронизация манифеста и registry | `apps/server/src/modules/manifest.rs` |
| Контракт модуля `RusToKModule` | `crates/rustok-core/src/module.rs` |
| Реестр Core/Optional | `crates/rustok-core/src/registry.rs` |
| Конфигурация состава модулей | `modules.toml` |

## Жизненный цикл модуля

```text
modules.toml → cargo build → ModuleRegistry::register() → on_enable() → runtime
```

Изменение состава модулей = изменение `modules.toml` + пересборка бинарника.
Runtime hot-plug отсутствует намеренно (compile-time safety).

---

## Два уровня операций

| Уровень | Действие | Время | Механизм |
|---|---|---|---|
| **Platform-level** | Установить / удалить модуль | 2–5 мин (сборка) | `ManifestManager` → `BuildService` → `BuildExecutor` |
| **Tenant-level** | Включить / отключить для тенанта | Мгновенно | `ModuleLifecycleService` → `tenant_modules` |

**Инвариант**: tenant-level toggle работает только над модулями, уже скомпилированными
в бинарник (присутствующими в `ModuleRegistry`).

### Platform-level: install/uninstall

```
GraphQL installModule(slug, version)
  └─ ManifestManager::install_builtin_module()  → apps/server/src/modules/manifest.rs
       ├─ Добавить в modules.toml
       ├─ validate(manifest)
       ├─ save(manifest)
       └─ BuildService::request_build()          → apps/server/src/services/build_service.rs
            ├─ Хешировать modules_delta (SHA-256, дедупликация)
            ├─ INSERT INTO builds (status=queued)
            └─ Publish BuildRequested event

BuildExecutor::execute_next_queued_build()       → apps/server/src/services/build_executor.rs
  ├─ cargo build -p rustok-server --features=[modules]
  ├─ UPDATE builds SET stage=..., progress=...
  └─ INSERT INTO releases (status=active)
       ← Миграции новых модулей прогоняются при старте бинарника
         через registry.migrations() → MigrationSource каждого модуля
```

Каждый модуль несёт миграции внутри своего crate (`src/migrations/`), реализуя
`MigrationSource`. Добавлять миграции в `apps/server/migration/` вручную не нужно.

### Tenant-level: toggle

```
GraphQL toggleModule(moduleSlug, enabled)
  └─ ModuleLifecycleService::toggle_module()     → apps/server/src/services/module_lifecycle.rs
       ├─ slug ∈ ModuleRegistry?                 (UnknownModule)
       ├─ не Core?                               (CoreModuleCannotBeDisabled)
       ├─ enabled=true:  все depends_on включены  (MissingDependencies)
       ├─ enabled=false: нет зависящих от него    (HasDependents)
       ├─ BEGIN TRANSACTION
       │    UPDATE tenant_modules SET enabled=?
       │    module.on_enable() / on_disable()
       └─ COMMIT  ─── или ─── ROLLBACK при HookFailed → откат состояния
```

---

## Marketplace каталог

`MarketplaceCatalogService` агрегирует модули из нескольких провайдеров:

```
MarketplaceCatalogService                        → apps/server/src/services/marketplace_catalog.rs
  ├─ LocalManifestMarketplaceProvider            — встроенные path-модули из modules.toml
  └─ RegistryMarketplaceProvider                 — внешний реестр (env: RUSTOK_MARKETPLACE_REGISTRY_URL)
       └─ moka cache (TTL: RUSTOK_MARKETPLACE_REGISTRY_CACHE_TTL_SECS, default 60s)
```

Дедупликация: если модуль есть у нескольких провайдеров — побеждает первый.
Fallback: при недоступности реестра — возвращает только local-manifest.

GraphQL:
- `marketplace(search, category, source, installed, trust_level, compatible_only)` — каталог
- `marketplaceModule(slug)` — детальная карточка с историей версий

---

## Runtime-гарантии для UI

**`EnabledModulesProvider`** (`apps/admin/src/shared/context/enabled_modules.rs`)
Загружает `enabledModules` при старте, предоставляет контекст всему приложению.

**`<ModuleGuard slug="blog">`**
Рендерит children только если модуль включён для текущего тенанта. Иначе —
placeholder "Module unavailable".

**`components_for_slot(slot_id, enabled_modules)`** (`apps/storefront/src/modules/registry.rs`)
Фильтрует slot-компоненты витрины по включённым модулям тенанта.

---

## Связанные документы

- [Module overview](../modules/overview.md) — что зарегистрировано в сервере
- [Module & application registry](../modules/registry.md) — полный реестр компонентов
- [Module manifest](../modules/manifest.md) — формат `modules.toml` и rebuild lifecycle
- [Module system plan](../modules/module-system-plan.md) — что ещё не реализовано
- [Architecture overview](./overview.md)
- [Events and outbox](./events.md)
- [WebSocket-каналы](./channels.md)
- [ADR: Loco Mailer/Storage as server infra](../../DECISIONS/2026-03-11-loco-mailer-storage-as-server-infra.md)
