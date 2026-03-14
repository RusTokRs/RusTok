# Ревью плана интеграции Loco RS + Core

**Дата ревью:** 2026-03-14
**Ревьюируемый документ:** `loco-core-integration-plan.md` (2026-03-12, RFC)
**Цель:** Перепроверка полноты плана, выявление пропусков, рекомендации по границам «модуль vs server».

---

## 1. Общая оценка

План хорошо структурирован: фазы логичны, граф зависимостей корректен, архитектурный инвариант (core agnosticism) чётко зафиксирован. Ниже — обнаруженные пробелы и уточнения.

---

## 2. Что упущено или недостаточно раскрыто

### 2.1 Фаза 0 — i18n: нет плана миграции существующих строк

**Проблема:** В `rustok-core::i18n` уже есть ~40 ключей (валидационные ошибки). Но в серверных контроллерах, GraphQL resolvers и сервисах строки захардкожены на английском. План говорит «все API ответы через i18n», но не описывает:

- **Инвентаризацию** существующих строк в `controllers/`, `graphql/`, `services/` — сколько их, как собирать.
- **Формат translation bundles** — сейчас `i18n.rs` использует статический `HashMap`. Для масштабирования нужен формат файлов (`.ftl` Fluent? `.json`? `.toml`?). План не фиксирует.
- **Fallback-цепочку на уровне модулей** — `RusToKModule::translations()` упомянут в Фазе 4, но i18n активируется в Фазе 0. Кто поставляет переводы модулей между Фазой 0 и Фазой 4?

**Рекомендация:**
1. Добавить подзадачу «аудит hard-coded строк» в Фазу 0.
2. Зафиксировать формат файлов переводов (рекомендуется Fluent `.ftl` — стандарт Mozilla, есть Rust-крейт `fluent-rs`).
3. До Фазы 4 модули могут поставлять переводы через convention: `{module_slug}/translations/{locale}.ftl`, а `translations()` в трейте — формализация уже работающего паттерна.

---

### 2.2 Фаза 1 — Settings API: нет версионирования и миграции настроек

**Проблема:** `platform_settings.settings` — JSONB. При обновлении платформы схема настроек может измениться (новые поля, переименования, удаления). План не описывает:

- **Schema versioning** — как определить, что настройки в БД соответствуют текущей версии кода.
- **Миграцию значений** — что происходит при апгрейде: кто обновляет `settings` JSONB при появлении новых полей? Fallback на defaults работает для чтения, но «грязные» записи с устаревшими полями остаются.
- **Аудит изменений** — `updated_by` есть, но нет `updated_at` diff / history. Для compliance и отладки нужна хотя бы аудитная таблица или event.

**Рекомендация:**
1. Добавить поле `schema_version INTEGER NOT NULL DEFAULT 1` в `platform_settings`.
2. В `SettingsService` — lazy migration: при чтении проверить `schema_version`, при несоответствии — обновить до текущей через зарегистрированные миграторы.
3. Эмитить event `PlatformSettingsChanged { category, diff, changed_by }` через outbox — это даёт аудит и возможность реагировать (перезагрузка кэша, уведомления).

---

### 2.3 Фаза 1 — Settings API: нет валидации на уровне ядра

**Проблема:** План говорит «ядро хранит как opaque JSONB; валидацию делает модуль». Но `platform_settings` — это **платформенные** настройки (rate_limit, email, search), а не модульные. Кто валидирует платформенные категории?

**Рекомендация:**
- Ввести `SettingsValidator` trait в server. Для каждой категории регистрируется валидатор. Платформенные категории (`email`, `rate_limit`, `events`) валидируются серверным кодом. Модульные — через `module.validate_settings()`.

---

### 2.4 Фаза 2 — Mailer: нет шаблонизации для модулей

**Проблема:** План описывает `templates/email/` с Tera-шаблонами, но только для server-level emails (password reset). Модули (commerce: подтверждение заказа, forum: уведомление об ответе) тоже генерируют email. Как модули регистрируют свои email-шаблоны?

**Рекомендация:**
- Определить контракт `EmailTemplateProvider` в server, чтобы модули поставляли свои шаблоны.
- Convention: `templates/email/{module_slug}/` для шаблонов модулей.
- Шаблоны модулей должны поддерживать i18n (locale-aware rendering).

---

### 2.5 Фаза 3 — Storage: нет стратегии удаления и GC

**Проблема:** `StorageAdapter` описывает upload/download/thumbnail, но нет:

- **Удаления ассетов** — `delete()` метод отсутствует в контракте.
- **Orphan cleanup** — что делать с файлами, на которые никто не ссылается (удалённый контент, неудачный upload).
- **Soft delete vs hard delete** — для compliance может потребоваться retention period.

**Рекомендация:**
Расширить контракт:
```rust
async fn delete(&self, tenant_id: Uuid, asset_id: Uuid) -> Result<()>;
async fn soft_delete(&self, tenant_id: Uuid, asset_id: Uuid) -> Result<()>;
async fn gc_orphans(&self, tenant_id: Uuid, older_than: Duration) -> Result<u64>;
```
Добавить поле `deleted_at` в `media_assets`. GC как scheduled task (Фаза 6).

---

### 2.6 Фаза 3 — Storage: нет стратегии для существующих файлов

**Проблема:** Если модули уже сохраняют файлы ad-hoc, при переходе на `StorageAdapter` нужна миграция существующих файлов. План не описывает backward compatibility.

**Рекомендация:**
- Добавить подзадачу «инвентаризация существующих upload-путей в модулях».
- Предусмотреть migration task для переноса файлов в новую структуру `{tenant_id}/YYYY/MM/{uuid}.{ext}`.

---

### 2.7 Фаза 4 — Dynamic GraphQL: техническая сложность не раскрыта

**Проблема:** `async-graphql` использует макрос `#[derive(MergedObject)]` — это compile-time merge. Динамическая регистрация в runtime требует принципиально другого подхода. Варианты:

1. **Dynamic schema via `async_graphql::dynamic::*`** — полностью runtime, но теряются derive-макросы, типобезопасность и autocomplete.
2. **Feature-gated compilation** — `#[cfg(feature = "blog")]` и т.д. Compile-time, но нет runtime toggle.
3. **Code generation** — `build.rs` генерирует `schema.rs` из манифеста модулей.
4. **Гибридный подход** — compile-time merge всех _зарегистрированных_ модулей, runtime toggle через `@skip`/guard на уровне resolver.

**Рекомендация:**
Зафиксировать архитектурное решение по подходу ДО начала Фазы 4. Самый прагматичный вариант: **compile-time регистрация** через feature flags + Cargo features. Модуль компилируется в бинарь, но его resolvers проверяют `module.is_enabled(tenant_id)` в runtime. Это сохраняет типобезопасность `MergedObject`, убирает hard-coded импорты (заменяя на conditional compilation), и позволяет runtime toggle per-tenant.

```rust
// Пример: schema.rs с feature-gated модулями
#[derive(MergedObject, Default)]
pub struct Query(
    RootQuery,
    AuthQuery,
    #[cfg(feature = "mod-commerce")] CommerceQuery,
    #[cfg(feature = "mod-content")] ContentQuery,
    // ...
);
```

Это значительно проще полностью динамической schema и не ломает типизацию async-graphql.

---

### 2.8 Фаза 5 — Observability: нет алертинга и retention

**Проблема:** План описывает dashboard, но не упоминает:

- **Alerting rules** — при каких порогах отправлять уведомления (через Channels из Фазы 6? email? webhook?).
- **Retention** — `recentErrors` подразумевает хранение ошибок. Где? Сколько хранить?
- **Pagination** — `eventQueueStats`, `recentErrors` могут вернуть огромные объёмы данных.

**Рекомендация:**
- Добавить `AlertRule` сущность с порогами и каналами уведомлений.
- Ошибки хранить в ring-buffer (in-memory, ограниченный размер) или в таблице с автоочисткой.
- Все observability GraphQL-queries должны поддерживать pagination.

---

### 2.9 Фаза 6 — Scheduler: нет управления concurrency

**Проблема:** Scheduler для cron-задач не описывает:

- **Leader election** — в multi-instance deployment кто запускает задачу? Все инстансы или один?
- **Overlap protection** — что если задача не завершилась до следующего запуска?
- **Retry policy** — что если задача упала?

**Рекомендация:**
- Использовать advisory locks (PostgreSQL `pg_advisory_lock`) для leader election.
- Добавить `skip_if_running` флаг для задач.
- Retry policy наследовать от `RelayRetryPolicy` (уже реализован в event settings).

---

### 2.10 Пропущено: Graceful shutdown и drain

**Проблема:** Нигде в плане не упоминается graceful shutdown. При переходе на Channels (WebSocket), Scheduler и расширенный Storage — корректное завершение становится критичным:

- WebSocket-соединения нужно корректно закрывать.
- Outbox relay worker должен завершить текущий batch.
- Storage upload-in-progress не должен оставлять сирот.

**Рекомендация:**
Добавить cross-cutting concern «Graceful Shutdown Protocol» как подзадачу Фазы 6 или выделить отдельно.

---

### 2.11 Пропущено: Rate limiting settings в DB

**Проблема:** `RateLimitSettings` сейчас в YAML. После Фазы 1 (Settings API) rate limits должны быть per-tenant и управляемы из админки. Но план не упоминает rate limiting как категорию настроек явно (только в схеме таблицы `platform_settings`).

**Рекомендация:**
Явно добавить миграцию `rate_limit` в Settings API. Это важно для SaaS: разные тарифы → разные лимиты.

---

### 2.12 Пропущено: OAuth provider management из админки

**Проблема:** OAuth-провайдеры (Google, GitHub и т.д.) сейчас настраиваются через YAML/код. После Фазы 1 логично управлять ими из админки (включить/выключить провайдера, обновить client_id/secret). План это не упоминает.

**Рекомендация:**
Добавить `oauth` как категорию в `platform_settings` или отдельную таблицу `oauth_providers` с per-tenant настройками.

---

## 3. API Parity: REST и GraphQL

### 3.0 Принцип

> **Фронтенды и админки работают только через GraphQL.**
> REST существует для внешних интеграций и мобильных приложений.
> Бэкенд обязан одинаково покрывать оба протокола — каждая доменная операция доступна и через REST, и через GraphQL.

### 3.1 Текущее состояние по доменам

#### Полный паритет ✅

| Домен | REST | GraphQL |
|-------|------|---------|
| Content (nodes) | CRUD | CRUD |
| Blog (posts) | CRUD + publish/unpublish | CRUD + publish/unpublish |
| Forum (categories, topics, replies) | CRUD | CRUD |

#### Частичный паритет ⚠️

| Домен | Есть в REST, нет в GraphQL | Есть в GraphQL, нет в REST |
|-------|---------------------------|---------------------------|
| **Pages** | `POST /admin/pages/{id}/blocks/reorder` | — |
| **Commerce** | `PUT /variants/{id}/prices`, `GET/POST /variants/{id}/inventory/*`, `POST /inventory/check` | — |
| **Auth** | `POST /auth/logout`, `GET /auth/me`, `POST /auth/invite/accept` | — |
| **Admin/Users** | — | `createUser`, `updateUser`, `deleteUser`, `users`, `user`, `activityLog` |
| **Admin/Modules** | — | `toggleTenantModule`, `installModule`, `uninstallModule`, `tenantModules`, `marketplaceModules` |
| **Admin/Dashboard** | — | `dashboardStats`, `builds`, `buildHistory`, `release`, `releaseHistory` |
| **DLQ** | `GET /admin/events/dlq`, `POST /admin/events/dlq/{id}/replay` | — |
| **Sessions** | — | — (нет нигде) |

#### Осознанная асимметрия (паритет НЕ нужен)

| Компонент | Только REST | Причина |
|-----------|-------------|---------|
| OAuth flow (token, authorize, revoke, userinfo) | ✅ | RFC 6749 — обязательно REST |
| `.well-known/*` discovery | ✅ | Стандарт — только REST |
| Health / Metrics / OpenAPI | ✅ | Инфраструктурное, не для клиентов |
| Alloy scripting API | Только GraphQL | Внутренний инструмент |

### 3.2 Пробелы, которые нужно закрыть

#### 3.2.1 GraphQL — отсутствующие операции (высокий приоритет для админок)

| Операция | Приоритет | Обоснование |
|----------|-----------|-------------|
| `logout` mutation | 🔴 Критичный | Админка не может разлогинить пользователя |
| `me` / `currentUser` query | 🔴 Критичный | Админка не может получить текущего пользователя |
| `sessions` query + `revokeSession` mutation | 🔴 Высокий | «Активные сессии» в профиле пользователя |
| `acceptInvite` mutation | 🟡 Средний | Invite flow для команды |
| `reorderBlocks` mutation (Pages) | 🟡 Средний | Page builder в админке |
| `updateVariantPrices` mutation | 🟡 Средний | Commerce управляется из админки |
| `variantInventory` query + `adjustInventory`, `setInventory` mutations | 🟡 Средний | Управление складом из админки |
| `checkInventoryAvailability` query | 🟢 Низкий | Storefront может вызывать через REST |
| `dlqEvents` query + `replayDlqEvent` mutation | 🟢 Низкий | Можно добавить в Фазу 5 (Observability) |

#### 3.2.2 REST — отсутствующие эндпоинты (для интеграций и мобильных)

| Эндпоинт | Приоритет | Обоснование |
|----------|-----------|-------------|
| `GET /api/users`, `GET /api/users/{id}` | 🟡 Средний | Нужно для admin-интеграций |
| `POST/PUT/DELETE /api/users/*` | 🟡 Средний | CRUD пользователей для внешних систем |
| `GET /api/modules`, `POST /api/modules/{slug}/toggle` | 🟢 Низкий | Module management через API |
| `GET /api/dashboard/stats` | 🟢 Низкий | Dashboard для внешних интеграций |
| `GET /api/builds`, `GET /api/releases` | 🟢 Низкий | CI/CD интеграции |
| `POST /api/alloy/scripts/run` | 🟢 Низкий | Webhook-триггеры для скриптов |

### 3.3 Рекомендуемая фаза для закрытия пробелов

**Фаза 1.5 (параллельно с Фазой 1, ~1 нед):**
- Все 🔴 GraphQL операции (logout, me, sessions) — нужны фронтенду сейчас
- REST user management — нужно до появления интеграций

**Фаза 3 (вместе со Storage):**
- Commerce inventory/pricing GraphQL mutations
- Pages block reorder GraphQL mutation

**Фаза 5 (вместе с Observability):**
- DLQ GraphQL queries/mutations
- REST module/dashboard/builds endpoints

### 3.4 Детализированные шаги — Фаза 1.5: API Parity (~1 нед)

| # | Шаг | Файлы | Тип | Детали |
|---|-----|-------|-----|--------|
| 1.5.1 | `logout` mutation | `graphql/auth/mutation.rs` | Модификация | Revoke текущую сессию. Возвращает `success: Boolean`. |
| 1.5.2 | `me` / `currentUser` query | `graphql/auth/query.rs` | Модификация | Возвращает `User` с `permissions`, `roles`, `tenant`. Использует `AuthContext` из request. |
| 1.5.3 | `sessions` query | `graphql/auth/query.rs` | Новое | Список активных сессий текущего пользователя: `id`, `ip`, `user_agent`, `created_at`, `last_active_at`. |
| 1.5.4 | `revokeSession` mutation | `graphql/auth/mutation.rs` | Новое | Soft-revoke конкретной сессии (кроме текущей без подтверждения). |
| 1.5.5 | `revokeAllSessions` mutation | `graphql/auth/mutation.rs` | Новое | Revoke всех сессий кроме текущей. |
| 1.5.6 | `acceptInvite` mutation | `graphql/auth/mutation.rs` | Новое | Принятие invite-токена. |
| 1.5.7 | REST: User management | `controllers/users.rs` | Новое | `GET /api/users` (paginated), `GET /api/users/{id}`, `POST /api/users`, `PUT /api/users/{id}`, `DELETE /api/users/{id}`. RBAC: `users:read`, `users:manage`. |
| 1.5.8 | REST: Session list | `controllers/auth.rs` | Модификация | `GET /api/auth/sessions` — список активных сессий. |
| 1.5.9 | REST: Session revoke | `controllers/auth.rs` | Модификация | `DELETE /api/auth/sessions/{id}`, `DELETE /api/auth/sessions` (all). |
| 1.5.10 | Тесты | `tests/` | Новое | Все новые endpoints: auth flow, sessions CRUD, user management RBAC. |

---

## 4. Что вынести как модуль, а что оставить в server

### 4.1 Текущая архитектура и паттерн leaf crates

Проект уже модульный: 16 rustok-crates + server как оркестратор. Ключевой паттерн — **leaf crates**:

```
Leaf crates (БЕЗ зависимости от core):
  rustok-events (2K)      ── события, DomainEvent
  rustok-telemetry (1.5K) ── метрики, трейсинг

rustok-core (12.7K) ── агрегатор + платформенные контракты
  ├── зависит от rustok-events    (re-export)
  ├── зависит от rustok-telemetry (re-export)
  └── предоставляет: CacheBackend, ModuleRegistry, RBAC, i18n, ...

Core-модули (ModuleKind::Core — нельзя выключить):
  rustok-tenant (400)  ── мультитенантность
  rustok-rbac (4K)     ── RBAC
  rustok-index (2.1K)  ── CQRS read models

Доменные модули (ModuleKind::Optional — toggle per-tenant):
  rustok-content (4K)  ── CMS ядро
    ├── rustok-blog (3.3K)
    ├── rustok-forum (2.5K)
    └── rustok-pages (2.4K)
  rustok-commerce (3K) ── каталог, инвентарь, цены

apps/server ── Loco + Axum, оркестрирует всё
```

### 4.2 Что уже есть для медиа и storage

При ревью обнаружено, что инфраструктура для медиа **уже частично заложена**:

| Что | Где | Статус |
|-----|-----|--------|
| Миграция `media` + `media_translations` | `apps/server/migration/m20250130_000009_create_media.rs` | ✅ Готово |
| Колонки: `tenant_id`, `filename`, `original_name`, `mime_type`, `size`, `storage_path`, `storage_driver`, `width`, `height`, `metadata` | Миграция | ✅ Готово |
| Таблица `media_translations` (locale, title, alt_text, caption) | Миграция | ✅ Готово |
| Events: `MediaUploaded`, `MediaDeleted` | `rustok-events` | ✅ Готово |
| Permissions: `Resource::Media` (Create, Read, Update, Delete, List) | `rustok-content` | ✅ Готово |
| `ProductImage.media_id` (FK на media) | `rustok-commerce` | ✅ Готово |
| Файловое хранилище (storage backend) | Нигде | ❌ Отсутствует |
| `MediaService` (upload, download, CRUD) | Нигде | ❌ Отсутствует |
| REST/GraphQL endpoints для media | Нигде | ❌ Отсутствует |
| Thumbnail generation | Нигде | ❌ Отсутствует |

**Важно:** Loco RS 0.16 **не имеет встроенного Storage** — нечего расширять. Файловая реализация полностью наша, с нуля.

### 4.3 Архитектурное решение: `rustok-storage` как leaf crate

> **Принцип:** Storage и Media — core-модули платформы. Они являются частью ядра, всегда активны, не могут быть отключены.

**`rustok-storage` — leaf crate**, по аналогии с `rustok-events` и `rustok-telemetry`.

**Почему leaf (без зависимости от core):**

1. **Паттерн проекта.** `rustok-events` не зависит от core → core зависит от events и ре-экспортирует. Storage — такой же фундаментальный контракт.

2. **Минимальные зависимости.** Storage на своём уровне — «положи байты по пути, отдай байты по пути». Ему нужны: `async-trait`, `uuid`, `serde`, `tokio`, `bytes`. Не нужны: ORM, RBAC, events, i18n, cache (12.7K строк и 30+ dependencies от core).

3. **Переиспользуемость.** Leaf crate можно использовать в CLI-утилитах, migration tools, standalone backup скриптах — без подтягивания всего core.

4. **Core агрегирует, а не владеет.** Core = агрегатор контрактов. Storage trait определяется в leaf crate, core зависит от него и ре-экспортирует. Все модули получают storage через core.

**`rustok-media` — Core модуль** (`ModuleKind::Core`), по аналогии с `rustok-tenant`, `rustok-rbac`, `rustok-index`.

### 4.4 Итоговая архитектура: два новых crate

```
Leaf crates (БЕЗ зависимости от core):
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│rustok-events │  │rustok-storage│  │rustok-       │
│  (leaf)      │  │  (leaf, NEW) │  │telemetry     │
│              │  │              │  │  (leaf)       │
│ DomainEvent  │  │StorageBackend│  │              │
│ EventEnvelope│  │ LocalBackend │  │ metrics,     │
│ 40+ events   │  │ S3Backend    │  │ tracing      │
│              │  │ StoragePolicy│  │              │
└──────┬───────┘  └──────┬───────┘  └──────┬───────┘
       │                 │                 │
       ▼                 ▼                 ▼
┌──────────────────────────────────────────────────────┐
│  rustok-core (агрегатор + платформенные контракты)    │
│                                                       │
│  pub use rustok_events::*;                            │
│  pub use rustok_storage::*;    ← НОВОЕ               │
│  pub use rustok_telemetry::*;                         │
│                                                       │
│  + CacheBackend, ModuleRegistry, RusToKModule,        │
│    RBAC, i18n, SecurityContext, ...                   │
└──────────────────────┬───────────────────────────────┘
                       │
       ┌───────────────┼───────────────┐
       ▼               ▼               ▼
┌─────────────┐ ┌─────────────┐ ┌─────────────┐
│rustok-media │ │rustok-tenant│ │rustok-rbac  │
│ (Core, NEW) │ │ (Core)      │ │ (Core)      │
│             │ │             │ │             │
│MediaService │ │TenantModule │ │RbacModule   │
│Thumbnails   │ │             │ │             │
│Quota        │ │             │ │             │
│             │ │             │ │             │
│зависит от:  │ └─────────────┘ └─────────────┘
│ core (→     │
│  storage)   │
│ events      │
│ outbox      │
└──────┬──────┘
       │
┌──────┼──────────┬───────────┬───────────┐
▼      ▼          ▼           ▼           ▼
content blog   commerce   forum       pages
```

### 4.5 Разделение ответственности

#### `rustok-storage` (leaf crate)

**Содержит:** инфраструктуру файлового хранения — КАК хранить.

```rust
// Контракт
#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn put(&self, path: &StoragePath, bytes: Bytes) -> Result<()>;
    async fn get(&self, path: &StoragePath) -> Result<Bytes>;
    async fn delete(&self, path: &StoragePath) -> Result<()>;
    async fn exists(&self, path: &StoragePath) -> Result<bool>;
    async fn list(&self, prefix: &StoragePath) -> Result<Vec<StorageEntry>>;
    fn public_url(&self, path: &StoragePath) -> Option<String>;
}

// Политика
pub struct StoragePolicy { ... }
impl StoragePolicy {
    pub fn resolve_path(&self, tenant_id: Uuid, filename: &str) -> StoragePath;
    pub fn validate_mime(&self, mime: &str, whitelist: &[String]) -> Result<()>;
}

// Backends
pub struct LocalStorageBackend { ... }
#[cfg(feature = "s3")]
pub struct S3StorageBackend { ... }
#[cfg(feature = "gcp")]
pub struct GcpStorageBackend { ... }

// Для тестов
pub struct InMemoryStorageBackend { ... }
```

| Зависимости | Версия |
|-------------|--------|
| `async-trait` | workspace |
| `uuid` | workspace |
| `serde` | workspace |
| `tokio` | workspace |
| `bytes` | 1.x |
| `thiserror` | workspace |
| `aws-sdk-s3` | optional, feature `s3` |
| `google-cloud-storage` | optional, feature `gcp` |

**НЕ содержит:** ORM, events, RBAC, i18n, thumbnails, quota — ничего доменного.

#### `rustok-media` (Core модуль)

**Содержит:** доменную логику медиа — ЧТО хранить и зачем.

| Компонент | Ответственность |
|-----------|-----------------|
| `MediaModule` | `impl RusToKModule`, `ModuleKind::Core`, миграции, permissions, event listeners |
| `MediaService` | CRUD метаданных, вызов `StorageBackend.put/get/delete`, emit events |
| `ThumbnailService` | Ресайз изображений (через `image` crate), lazy generation |
| `QuotaService` | `SUM(size_bytes)` per tenant, лимиты из settings |
| `MediaRepository` | SeaORM queries: `media`, `media_translations` |
| Entities | `media.rs`, `media_translations.rs` (SeaORM) |
| Миграции | Перенос из `apps/server/migration/m20250130_000009_create_media.rs` |

| Зависимости | Источник |
|-------------|----------|
| `rustok-core` → `rustok-storage` | Получает `StorageBackend` через core |
| `rustok-events` | `MediaUploaded`, `MediaDeleted` |
| `rustok-outbox` | Transactional event publishing |
| `sea-orm` | Persistence |
| `image` | Thumbnail generation |

#### `apps/server` (оркестратор)

| Компонент | Ответственность |
|-----------|-----------------|
| Выбор backend | Конфигурация: `LocalStorageBackend` / `S3StorageBackend` по `platform_settings` |
| Инжекция в `AppContext` | `StorageBackend` как shared resource (аналогично `CacheBackend`) |
| Multipart upload handler | HTTP-специфичная обработка `multipart/form-data` |
| REST endpoints | `POST /api/media/upload`, `GET/PUT/DELETE /api/media/{id}`, `GET /api/media/{id}/thumbnail/{size}` |
| GraphQL resolvers | `uploadMedia`, `updateMedia`, `deleteMedia`, `mediaAssets`, `mediaAsset` |
| Регистрация `MediaModule` | `.register(MediaModule)` в `build_registry()` |

### 4.6 Остальные компоненты: server vs crate

#### Оставить в server (инфраструктура)

| Компонент | Обоснование | Фаза |
|-----------|-------------|------|
| **Settings API** (`SettingsService`, `platform_settings`) | Платформенная infra, все модули зависят от неё. Тесно связан с Loco config. | 1 |
| **Loco Mailer adapter** | Инфраструктурная обёртка Loco API. Модули не должны напрямую зависеть от Loco Mailer. | 2 |
| **i18n middleware** (locale resolution) | Server-level concern: парсинг `Accept-Language`, injection locale в request context. | 0 |
| **Observability dashboard** (GraphQL queries) | Платформенные метрики, не доменная логика. | 5 |
| **Scheduler runtime** | Инфраструктура выполнения задач. Задачи регистрируют модули, runtime — server. | 6 |
| **Channels runtime** (WebSocket hub) | Transport-level, аналогично event bus. | 6 |
| **GraphQL schema builder** | Compile-time merge, server отвечает за сборку schema. | 4 |
| **Rate limiting middleware** | Cross-cutting server concern. | Есть |
| **Auth lifecycle** (JWT, sessions, password reset) | Loco-специфичная интеграция, используется всеми. | Есть |
| **OAuth controller + service** | Тесно связан с auth lifecycle. Настройки провайдеров — через Settings API. | Есть |
| **REST/GraphQL endpoints** для всех доменов | HTTP-слой, не бизнес-логика. | Есть |

#### Вынести в отдельный crate (модуль или core-контракт)

| Компонент | Куда | Обоснование | Фаза |
|-----------|------|-------------|------|
| **`StorageAdapter` trait** | `rustok-core` | Контракт, от которого зависят все модули. Аналогично `CacheBackend`, `EventBus`. Реализация (`LocoStorageAdapter`) — в server. | 3 |
| **`MediaService` + `ThumbnailService` + `QuotaService`** | **Новый `rustok-media`** | Доменная логика, используемая content/blog/commerce/pages. Миграция уже существует. | 3 |
| **`EmailTemplateProvider` trait** | `rustok-core` | Контракт для модулей. Модули поставляют шаблоны, server рендерит через Loco Mailer. | 2 |
| **`SettingsSchema` / `SettingsValidator` traits** | `rustok-core` | Уже запланировано: `settings_schema()` и `validate_settings()` в `RusToKModule`. Это часть модульного контракта. | 4 |
| **`TranslationBundle` trait/struct** | `rustok-core` | Уже запланировано: `translations()` в `RusToKModule`. Формат должен быть в core. | 0/4 |
| **`ScheduledTask` trait** | `rustok-core` | Контракт для модулей: «что запускать». Runtime (когда и как) — server. Аналогично `EventListener`. | 6 |
| **`AlertRule` + notification contracts** | `rustok-core` | Если модули могут определять свои alert rules (например, commerce: «заказ не обработан >1ч»), нужен контракт. | 5 |

#### Не выносить (осознанные решения, задокументированы)

| Компонент | Обоснование |
|-----------|-------------|
| **CacheBackend** (Moka + Redis + Fallback) | Уже в `rustok-core`. Значительно мощнее Loco Cache. Решение принято. |
| **Event bus / outbox** | Уже в `rustok-outbox` + `rustok-events`. Loco queue не подходит. ADR есть. |
| **RBAC engine** | Уже в `rustok-rbac`. Loco не имеет аналога. |

---

## 4. Детализированные шаги по фазам (расширенная версия)

### Фаза 0 — i18n по умолчанию (~1.5 нед, было ~1 нед)

| # | Шаг | Файлы | Тип | Детали |
|---|-----|-------|-----|--------|
| 0.1 | Аудит hard-coded строк | `controllers/`, `graphql/`, `services/` | Исследование | Grep по строковым литералам в ответах API. Составить список ключей. |
| 0.2 | Выбрать формат переводов | — | Решение | Рекомендуется `fluent-rs` (`.ftl`) или JSON. Зафиксировать в ADR. |
| 0.3 | Рефакторинг `i18n.rs` | `crates/rustok-core/src/i18n.rs` | Код | Заменить статический `HashMap` на загрузку из файлов. Добавить `TranslationBundle` struct. |
| 0.4 | Создать файлы переводов для core | `crates/rustok-core/translations/` | Новое | `en.ftl`, `ru.ftl` — перенести 40+ ключей из хардкода. |
| 0.5 | Locale resolution middleware | `apps/server/src/middleware/locale.rs` | Новое | Парсинг `Accept-Language`, fallback chain: header → tenant default → `ru`. Inject `Locale` в request extensions. |
| 0.6 | Интеграция в контроллеры | `controllers/*.rs`, `graphql/**/*.rs` | Модификация | Заменить строковые литералы на `t!(key, locale)` или аналог. |
| 0.7 | Language switcher в админках | Frontend | Модификация | UI для выбора языка, сохранение preference. |
| 0.8 | Тесты | `tests/` | Новое | Тест locale resolution chain, fallback, missing key handling. |

---

### Фаза 1 — Settings API (~2.5 нед, было ~2 нед)

| # | Шаг | Файлы | Тип | Детали |
|---|-----|-------|-----|--------|
| 1.1 | Миграция `platform_settings` | `migration/` | Новое | Таблица с `schema_version`, `tenant_id`, `category`, `settings` JSONB, `updated_by`, timestamps. Unique constraint `(tenant_id, category)`. |
| 1.2 | SeaORM entity | `models/platform_settings.rs` | Новое | Entity, ActiveModel, relation to tenants. |
| 1.3 | `SettingsService` | `services/settings.rs` | Новое | CRUD с fallback chain: DB → YAML → defaults. Кэширование в `CacheBackend` (invalidation через event). |
| 1.4 | `SettingsValidator` trait | `rustok-core` или server | Новое | Регистрация валидаторов по категориям. Платформенные категории валидируются в server. |
| 1.5 | Валидаторы платформенных категорий | `services/settings_validators/` | Новое | Валидаторы для `email`, `rate_limit`, `events`, `search`, `features`, `i18n`. |
| 1.6 | Event при изменении | `services/settings.rs` | Код | Emit `PlatformSettingsChanged` через outbox. Подписчики: cache invalidation, runtime reconfiguration. |
| 1.7 | Рефакторинг `RustokSettings` | `common/settings.rs` | Модификация | `from_settings()` → `from_settings_with_db()`. DB имеет приоритет, YAML — defaults. |
| 1.8 | GraphQL: queries и mutations | `graphql/settings/` | Новое | `platformSettings(category)`, `allPlatformSettings`, `updatePlatformSettings(category, settings)`. |
| 1.9 | RBAC permissions | `rustok-rbac` | Модификация | Добавить `settings:read`, `settings:manage`. |
| 1.10 | Runtime hot-reload | `services/settings.rs` | Код | При изменении критичных настроек (rate_limit, events) — применять без перезапуска. Через event subscription. |
| 1.11 | Страница Settings в админках | Frontend | Новое | Категоризированная форма, JSON Schema → UI form. |
| 1.12 | Тесты | `tests/` | Новое | Fallback chain, validation, RBAC, hot-reload, concurrent updates. |

---

### Фаза 2 — Loco Mailer (~1.5 нед, было ~1 нед)

| # | Шаг | Файлы | Тип | Детали |
|---|-----|-------|-----|--------|
| 2.1 | `EmailTemplateProvider` trait | `rustok-core` | Новое | Контракт: `fn templates(&self) -> Vec<EmailTemplate>`. Template = name + locale + body (Tera string). |
| 2.2 | Email templates directory | `templates/email/` | Новое | `password_reset.{locale}.html.tera`, `welcome.{locale}.html.tera`. Base layout с header/footer. |
| 2.3 | Loco Mailer adapter | `services/email.rs` | Модификация | Добавить `LocoMailerSender` рядом с `SmtpEmailSender`. Feature flag: `email.provider = loco | smtp`. |
| 2.4 | Расширить `EmailService` enum | `services/email.rs` | Модификация | `Disabled`, `Smtp(...)`, `Loco(...)`. Выбор по `platform_settings.email.provider`. |
| 2.5 | Shadow mode | `services/email.rs` | Код | Dual-send: primary path + shadow path. Логирование расхождений. |
| 2.6 | Метрики | `services/email.rs` | Код | `email_send_total`, `email_send_errors_total`, `email_send_latency_ms`. |
| 2.7 | Settings UI — Email | Frontend | Модификация | Provider selector, SMTP credentials, тестовая отправка, template preview. |
| 2.8 | Удаление legacy path | `services/email.rs` | Удаление | После стабилизации: удалить `SmtpEmailSender`, `lettre` dependency, legacy settings. |
| 2.9 | Тесты | `tests/` | Новое | Template rendering с i18n, provider switching, shadow mode parity. |

---

### Фаза 3 — Storage + Media (~3.5 нед, было ~2 нед)

> **Архитектурное решение:**
> - `rustok-storage` — **leaf crate** (без зависимости от core, аналогично `rustok-events`)
> - `rustok-core` зависит от `rustok-storage` и ре-экспортирует
> - `rustok-media` — **Core модуль** (`ModuleKind::Core`, нельзя отключить)
> - Loco RS 0.16 не имеет встроенного Storage — реализация полностью наша, с нуля
> - Миграция `media` + `media_translations` уже существует в server — перенести в `rustok-media`

#### Подфаза 3a — `rustok-storage` (leaf crate, ~1 нед)

| # | Шаг | Файлы | Тип | Детали |
|---|-----|-------|-----|--------|
| 3a.1 | Создать crate `rustok-storage` | `crates/rustok-storage/` | Новое | Leaf crate. `Cargo.toml`: `async-trait`, `uuid`, `serde`, `tokio`, `bytes`, `thiserror`. Без `rustok-core`. |
| 3a.2 | `StorageBackend` trait | `crates/rustok-storage/src/backend.rs` | Новое | `put`, `get`, `delete`, `exists`, `list`, `public_url`. Чистый контракт без доменной семантики. |
| 3a.3 | Типы: `StoragePath`, `StorageEntry`, `StorageConfig` | `crates/rustok-storage/src/types.rs` | Новое | Path = typed string. Entry = path + size + modified_at. Config = provider + bucket + region + CDN URL. |
| 3a.4 | `StoragePolicy` | `crates/rustok-storage/src/policy.rs` | Новое | Path generation: `{tenant_id}/YYYY/MM/{uuid}.{ext}`. MIME whitelist validation. Tenant isolation. |
| 3a.5 | `LocalStorageBackend` | `crates/rustok-storage/src/backends/local.rs` | Новое | Реализация через `tokio::fs`. Автосоздание директорий. |
| 3a.6 | `S3StorageBackend` (feature-gated) | `crates/rustok-storage/src/backends/s3.rs` | Новое | `#[cfg(feature = "s3")]`. Через `aws-sdk-s3` или `opendal`. |
| 3a.7 | `InMemoryStorageBackend` | `crates/rustok-storage/src/backends/memory.rs` | Новое | Для тестов. `HashMap<StoragePath, Bytes>`. |
| 3a.8 | CDN URL rewrite | `crates/rustok-storage/src/cdn.rs` | Новое | Если `cdn_base_url` в конфиге — подменять URL в `public_url()`. |
| 3a.9 | Подключить в `rustok-core` | `crates/rustok-core/Cargo.toml` + `lib.rs` | Модификация | Добавить `rustok-storage.workspace = true`. Ре-экспорт: `pub use rustok_storage;`. |
| 3a.10 | Тесты | `crates/rustok-storage/tests/` | Новое | Все backends: put/get/delete/exists/list, policy path generation, MIME validation, CDN rewrite. |

#### Подфаза 3b — `rustok-media` (Core модуль, ~1.5 нед)

| # | Шаг | Файлы | Тип | Детали |
|---|-----|-------|-----|--------|
| 3b.1 | Создать crate `rustok-media` | `crates/rustok-media/` | Новое | `Cargo.toml`: `rustok-core` (→ storage), `rustok-events`, `rustok-outbox`, `sea-orm`, `image`. |
| 3b.2 | Перенести миграцию из server | `crates/rustok-media/src/migration/` | Перенос | Перенести `m20250130_000009_create_media.rs`. Добавить `deleted_at` для soft delete. |
| 3b.3 | SeaORM entities | `crates/rustok-media/src/entities/` | Новое | `media.rs`, `media_translations.rs`. Scopes: `by_tenant`, `not_deleted`, `by_mime`. |
| 3b.4 | `MediaService` | `crates/rustok-media/src/service.rs` | Новое | CRUD metadata. Вызывает `StorageBackend.put/get/delete` через DI. Emit `MediaUploaded`/`MediaDeleted`. |
| 3b.5 | `ThumbnailService` | `crates/rustok-media/src/thumbnails.rs` | Новое | Lazy generation через `image` crate. Хранение thumbnails через `StorageBackend` рядом с оригиналом. |
| 3b.6 | `QuotaService` | `crates/rustok-media/src/quota.rs` | Новое | `SUM(size_bytes)` per tenant. Лимиты из module settings. Reject upload при превышении. |
| 3b.7 | `MediaModule` | `crates/rustok-media/src/module.rs` | Новое | `impl RusToKModule`: `slug: "media"`, `kind: Core`, миграции, permissions, event listeners. |
| 3b.8 | GC task | `crates/rustok-media/src/gc.rs` | Новое | Очистка soft-deleted файлов старше retention. Orphan detection. |
| 3b.9 | Подключить к доменным модулям | `rustok-content`, `rustok-commerce` | Модификация | Добавить `rustok-media` в dependencies. Commerce `ProductImage` → `MediaService`. |
| 3b.10 | Тесты | `crates/rustok-media/tests/` | Новое | Upload/download/delete через `InMemoryStorageBackend`, quota, thumbnails, soft delete, GC. |

#### Подфаза 3c — Server integration (~1 нед)

| # | Шаг | Файлы | Тип | Детали |
|---|-----|-------|-----|--------|
| 3c.1 | Выбор и инжекция backend | `apps/server/src/app.rs` | Модификация | Конфигурация: `LocalStorageBackend` / `S3StorageBackend` по `platform_settings.storage`. Инжекция в `AppContext` (аналогично `CacheBackend`). |
| 3c.2 | Зарегистрировать `MediaModule` | `apps/server/src/modules/mod.rs` | Модификация | `.register(MediaModule)` в `build_registry()`. |
| 3c.3 | REST API для media | `apps/server/src/controllers/media.rs` | Новое | `POST /api/media/upload` (multipart), `GET /api/media` (paginated), `GET /api/media/{id}`, `PUT /api/media/{id}`, `DELETE /api/media/{id}`, `GET /api/media/{id}/thumbnail/{size}`. |
| 3c.4 | GraphQL API для media | `apps/server/src/graphql/media/` | Новое | `uploadMedia`, `updateMedia`, `deleteMedia`, `mediaAssets(filter, pagination)`, `mediaAsset(id)`. |
| 3c.5 | Multipart upload handler | `apps/server/src/controllers/media.rs` | Новое | Парсинг `multipart/form-data`, streaming в `StorageBackend`. |
| 3c.6 | Страница Media в админке | Frontend | Новое | File manager: upload (drag-and-drop), browse grid/list, preview, alt-text, quota meter. |
| 3c.7 | Storage settings в Settings UI | Frontend | Модификация | Provider, bucket, CDN URL, MIME whitelist, quota, thumbnail sizes. |
| 3c.8 | Integration тесты | `apps/server/tests/` | Новое | E2E: upload через REST/GraphQL, quota enforcement, tenant isolation, thumbnail. |

---

### Фаза 4 — Module Settings + Dynamic Registration (~2 нед)

| # | Шаг | Файлы | Тип | Детали |
|---|-----|-------|-----|--------|
| 4.1 | Расширить `RusToKModule` trait | `crates/rustok-core/src/module.rs` | Модификация | Добавить: `settings_schema() -> Value`, `validate_settings(&Value) -> Result<(), Vec<String>>`, `translations() -> Option<TranslationBundle>`. |
| 4.2 | ADR по подходу к GraphQL registration | `DECISIONS/` | Новое | Зафиксировать решение: feature-gated compilation vs dynamic schema. Рекомендуется feature-gated. |
| 4.3 | Feature flags для модулей | `apps/server/Cargo.toml` | Модификация | `mod-commerce`, `mod-blog`, `mod-content`, `mod-forum`, `mod-pages`. Default = all enabled. |
| 4.4 | Рефакторинг `schema.rs` | `graphql/schema.rs` | Модификация | `#[cfg(feature = "mod-*")]` для каждого модульного Query/Mutation. Убрать unconditional imports. |
| 4.5 | Рефакторинг `modules/mod.rs` | `modules/mod.rs` | Модификация | Feature-gated `register()` calls. |
| 4.6 | Runtime module enable guard | `graphql/guards/` | Новое | Guard, проверяющий `is_module_enabled(tenant_id, module_slug)` перед выполнением resolver. |
| 4.7 | Module default settings при `on_enable()` | `services/module_lifecycle.rs` | Модификация | При включении модуля — записать `settings_schema()` defaults в `tenant_modules.settings`. |
| 4.8 | Module settings validation при toggle | `services/module_lifecycle.rs` | Модификация | При включении — `validate_settings()`. При невалидных settings — reject. |
| 4.9 | Убрать `AppContext.scripting` hard-wire | `context.rs` | Модификация | Alloy scripting — через `shared_store` или optional extension. |
| 4.10 | Страница Modules в админке | Frontend | Модификация | Settings panel per module (generated from JSON Schema), health badge, dependency tree. |
| 4.11 | Тесты | `tests/` | Новое | Feature-gated compilation, runtime guard, settings validation, module enable/disable with settings. |

---

### Фаза 5 — Observability Dashboard (~1.5 нед, было ~1 нед)

| # | Шаг | Файлы | Тип | Детали |
|---|-----|-------|-----|--------|
| 5.1 | Error ring-buffer | `services/error_tracker.rs` | Новое | In-memory ring buffer (bounded, configurable size). Struct: `TrackedError { timestamp, source, message, correlation_id, module }`. |
| 5.2 | GraphQL queries | `graphql/observability/` | Новое | `systemHealth`, `eventQueueStats`, `cacheStats`, `recentErrors(limit, offset)`, `moduleHealth(slug)`. Все с pagination. |
| 5.3 | Trigger actions | `graphql/observability/` | Новое | Mutations: `triggerReindex(module?)`, `flushCache(scope?)`, `retryDlq(event_ids?)`. RBAC: `system:manage`. |
| 5.4 | Alert rules (basic) | `services/alerts.rs` | Новое | Threshold-based alerts. Хранение в `platform_settings.alerts`. Каналы: in-app (через Channels), email (через Mailer). |
| 5.5 | Страница System в админке | Frontend | Новое | Tabs: Health, Events pipeline, Cache, Errors, Alerts. Auto-refresh. |
| 5.6 | RBAC permissions | `rustok-rbac` | Модификация | `system:read`, `system:manage` (для trigger actions). |
| 5.7 | Тесты | `tests/` | Новое | Ring buffer bounds, trigger actions RBAC, alert threshold evaluation. |

---

### Фаза 6 — Advanced Loco Features (~2.5 нед, было ~2 нед)

| # | Шаг | Файлы | Тип | Детали |
|---|-----|-------|-----|--------|
| 6.1 | `ScheduledTask` trait в core | `crates/rustok-core/src/scheduler.rs` | Новое | `fn cron(&self) -> &str`, `fn name(&self) -> &str`, `async fn run(&self, ctx: TaskContext) -> Result<()>`, `fn skip_if_running(&self) -> bool`. |
| 6.2 | Scheduler runtime в server | `services/scheduler.rs` | Новое | Cron engine. Leader election через `pg_advisory_lock`. Overlap protection. |
| 6.3 | Регистрация scheduled tasks от модулей | `RusToKModule` trait | Модификация | `fn scheduled_tasks(&self) -> Vec<Box<dyn ScheduledTask>>` (default: empty). |
| 6.4 | Built-in tasks | `tasks/scheduled/` | Новое | Session cleanup, index consistency check, RBAC audit, stale outbox cleanup, storage GC. |
| 6.5 | Scheduler management в админке | Frontend | Новое | Список задач, расписание, последний запуск, статус, ручной trigger. |
| 6.6 | Loco Channels setup | `initializers/channels.rs` | Новое | WebSocket hub через Loco Channels API. |
| 6.7 | Real-time notification types | `services/notifications.rs` | Новое | Module state changes, settings changes, alert triggers, build status. |
| 6.8 | Graceful shutdown protocol | `app.rs` + services | Модификация | Drain WebSocket connections, finish outbox relay batch, complete in-progress uploads, stop scheduler. |
| 6.9 | Тесты | `tests/` | Новое | Scheduler concurrency, leader election, overlap protection, channel subscribe/unsubscribe, shutdown drain. |

---

## 6. Оценка объёма (обновлённая)

| Фаза | Исходная оценка | Новая оценка | Дельта | Причина |
|------|----------------|--------------|--------|---------|
| 0. i18n | ~1 нед | ~1.5 нед | +0.5 | Аудит строк, формат переводов |
| 1. Settings API | ~2 нед | ~2.5 нед | +0.5 | Versioning, validation, hot-reload |
| **1.5. API Parity** | — | **~1 нед** | **+1** | **Новая фаза: logout, me, sessions, REST users** |
| 2. Loco Mailer | ~1 нед | ~1.5 нед | +0.5 | `EmailTemplateProvider`, модульные шаблоны |
| 3. Storage + Media | ~2 нед | ~3.5 нед | +1.5 | `rustok-storage` leaf crate + `rustok-media` Core модуль + server integration |
| 4. Module Settings | ~2 нед | ~2 нед | — | — |
| 5. Observability | ~1 нед | ~1.5 нед | +0.5 | Alerting, retention |
| 6. Advanced Loco | ~2 нед | ~2.5 нед | +0.5 | Graceful shutdown, leader election |

**Итого:** ~15.5 нед последовательно; ~10 нед с параллелизацией (было 11/7).

---

## 7. Обновлённый граф зависимостей (с Фазой 1.5)

```mermaid
graph LR
    P0[Фаза 0: i18n<br/>~1.5 нед] --> P1[Фаза 1: Settings API<br/>~2.5 нед]
    P1 --> P15[Фаза 1.5: API Parity<br/>~1 нед]
    P1 --> P2[Фаза 2: Loco Mailer<br/>~1.5 нед]
    P1 --> P3[Фаза 3: Storage + Media<br/>~3 нед]
    P1 --> P4[Фаза 4: Module Settings<br/>~2 нед]
    P15 --> P3
    P2 --> P5[Фаза 5: Observability<br/>~1.5 нед]
    P3 --> P5
    P4 --> P5
    P5 --> P6[Фаза 6: Advanced Loco<br/>~2.5 нед]
```

**Итого:** ~16 нед последовательно; ~10.5 нед с параллелизацией.

Фаза 1.5 параллелится с Фазами 2 и 4, но должна завершиться до Фазы 3 (media GraphQL endpoints зависят от паритетного паттерна).
Подфазы 3a (storage) → 3b (media) → 3c (server) выполняются последовательно внутри Фазы 3.

---

## 8. Резюме

### Найденные пробелы (15):

**Архитектурные:**
1. Нет аудита hard-coded строк и формата переводов (Фаза 0)
2. Нет версионирования schema настроек (Фаза 1)
3. Нет валидации платформенных категорий настроек (Фаза 1)
4. Нет контракта email-шаблонов для модулей (Фаза 2)
5. Нет `delete()` / `soft_delete()` / `gc_orphans()` в StorageAdapter (Фаза 3)
6. Не выбран подход к dynamic GraphQL registration (Фаза 4)
7. Нет alerting и retention для observability (Фаза 5)
8. Нет leader election и overlap protection для scheduler (Фаза 6)
9. Нет graceful shutdown protocol (Фаза 6)
10. Нет per-tenant rate limiting через Settings API
11. Нет OAuth provider management из админки

**API Parity (НОВОЕ):**
12. GraphQL: нет `logout` mutation — фронтенд не может разлогинить
13. GraphQL: нет `me`/`currentUser` query — фронтенд не знает текущего пользователя
14. GraphQL: нет управления сессиями (list/revoke)
15. REST: нет user management, module management — нужно для интеграций и мобильных

### Рекомендации по модульности:

**Новый leaf crate `rustok-storage`** (аналогично `rustok-events`):
- Без зависимости от core — чистый контракт: `StorageBackend` trait + backends (Local, S3, InMemory)
- Core зависит от него и ре-экспортирует (`pub use rustok_storage;`)
- `StoragePolicy` (path generation, MIME validation, tenant isolation) — часть этого crate
- Loco RS 0.16 не имеет Storage — реализация полностью наша

**Новый Core модуль `rustok-media`** (`ModuleKind::Core`):
- Зависит от core (→ получает storage), events, outbox
- `MediaService`, `ThumbnailService`, `QuotaService`
- Миграция `media` + `media_translations` уже существует — перенести из server
- Commerce уже ссылается на `media_id`; content/blog/pages будут зависеть от media

**Добавить в `rustok-core` (контракты через ре-экспорт leaf crates):**
- `rustok-storage` — ре-экспорт (как `rustok-events`)
- `EmailTemplateProvider` trait
- `TranslationBundle` struct
- `ScheduledTask` trait

**Оставить в server (HTTP/оркестрация):**
- Выбор storage backend по конфигу, инжекция в AppContext
- Settings API, Mailer adapter, i18n middleware
- Observability dashboard, Scheduler runtime, Channels runtime
- GraphQL schema builder, все REST/GraphQL endpoints, multipart upload handler

**Не трогать:** CacheBackend, Event bus/outbox, RBAC engine — осознанные решения, задокументированы
