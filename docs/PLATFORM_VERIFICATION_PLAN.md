# RusToK — Глобальный план верификации платформы

- **Дата создания:** 2026-02-26
- **Статус:** В процессе (часть фаз проверена в сессиях 2026-02-27)
- **Цель:** Из разрозненных модулей, написанных разными агентами, получить проверенную, работоспособную платформу с синхронизированной документацией

---

## Как пользоваться этим планом

Каждый раздел содержит конкретные проверки в формате чеклиста (`- [ ]`). Мы проходим по пунктам последовательно. При обнаружении проблемы — фиксим и ставим галочку. После прохождения всех секций платформа будет в верифицированном состоянии.

**Условные обозначения:**
- `[ ]` — не проверено
- `[x]` — проверено, ОК
- `[!]` — проверено, найдена проблема (описание ниже чекбокса)
- `[~]` — частично реализовано / требует доработки

**Принцип работы с ошибками:**
При обнаружении проблемы — **сначала исправляем** (код, конфиг, документация), затем ставим `[x]`.
Пункт `[!]` означает «найдено, но исправление заблокировано» (технический долг, требует ADR или отдельной задачи).
Не собираем коллекцию ошибок — исправляем по ходу.

**Расширяемость плана:**
Этот план — **living document**. В процессе верификации можно и нужно:
- Добавлять подпункты внутри существующих фаз, если обнаружены новые аспекты для проверки
- Детализировать проверки (разбивать один чекбокс на несколько)
- Добавлять заметки и ссылки на найденные проблемы прямо под чекбоксами
- Создавать новые секции `### X.N` внутри фазы

**Не нужно** создавать отдельный план для каждого нового набора проверок — расширяйте этот документ.

**Связь с зависимостями модулей:**
Полная схема зависимостей между модулями задокументирована в:
- [Граф зависимостей модулей (Mermaid)](./architecture/diagram.md) — 12 диаграмм, включая dependency graph
- [modules.toml](../modules.toml) — source of truth для `depends_on`
- [Категории модулей A/B/C](./architecture/modules.md) — compile-time vs runtime
- [Реестр компонентов](./modules/registry.md) — каталог всех crates и apps

---

## Оглавление

1. [Фаза 0: Компиляция и сборка](#фаза-0-компиляция-и-сборка)
2. [Фаза 1: Соответствие архитектуре](#фаза-1-соответствие-архитектуре)
3. [Фаза 2: Ядро платформы (Core Infrastructure)](#фаза-2-ядро-платформы)
4. [Фаза 3: Система авторизации и аутентификации](#фаза-3-авторизация-и-аутентификация)
5. [Фаза 4: RBAC](#фаза-4-rbac)
6. [Фаза 5: Multi-Tenancy](#фаза-5-multi-tenancy)
7. [Фаза 6: Событийная система](#фаза-6-событийная-система)
8. [Фаза 7: Доменные модули](#фаза-7-доменные-модули)
9. [Фаза 8: API — GraphQL](#фаза-8-api-graphql)
10. [Фаза 9: API — REST](#фаза-9-api-rest)
11. [Фаза 10: Фронтенды — Leptos](#фаза-10-фронтенды-leptos)
12. [Фаза 11: Фронтенды — Next.js](#фаза-11-фронтенды-nextjs)
13. [Фаза 12: Фронтенд-библиотеки](#фаза-12-фронтенд-библиотеки)
14. [Фаза 13: Интеграционные связи](#фаза-13-интеграционные-связи)
15. [Фаза 14: Тестовое покрытие](#фаза-14-тестовое-покрытие)
16. [Фаза 15: Observability и операционная готовность](#фаза-15-observability)
17. [Фаза 16: Синхронизация документации с кодом](#фаза-16-синхронизация-документации)
18. [Фаза 17: CI/CD и DevOps](#фаза-17-cicd)
19. [Фаза 18: Безопасность](#фаза-18-безопасность)
20. [Фаза 19: Антипаттерны и качество кода](#фаза-19-антипаттерны-и-качество-кода)
21. [Фаза 20: Правильность написания кода](#фаза-20-правильность-написания-кода-code-correctness)
22. [Фаза 21: Реестр найденных проблем](#фаза-21-реестр-найденных-проблем)
23. [Итоговый отчёт](#итоговый-отчёт)

---

## Фаза 0: Компиляция и сборка

### 0.1 Workspace-level сборка

- [!] `cargo check --workspace` — невозможно в текущей среде: нет C linker (ни MSVC `link.exe`, ни MinGW `dlltool.exe`). Требуется установка Visual Studio Build Tools или MinGW.
- [!] `cargo check --workspace --all-features` — аналогично, нет C linker
- [!] `cargo clippy --workspace -- -D warnings` — аналогично, нет C linker
- [x] `cargo fmt --all -- --check` — код форматирован (исправлены 2 файла: `apps/server/src/app.rs` и `crates/rustok-content/tests/node_event_index_integration_test.rs`)

### 0.2 Отдельные targets

- [ ] `cargo build -p rustok-server` — основной сервер собирается
- [ ] `cargo build -p rustok-admin` — Leptos admin собирается
- [ ] `cargo build -p rustok-storefront` — Leptos storefront собирается

### 0.6 Зависимости (Cargo)

- [x] `iggy` версия исправлена: `0.9.2` → `0.9.0` (crates.io не имел 0.9.2)
  - Исправлено в `Cargo.toml` (workspace) и `crates/rustok-iggy-connector/Cargo.toml`
- [ ] `cargo update` не приводит к конфликтам версий
- [x] `Cargo.lock` зафиксирован в git — удалён из `.gitignore` (теперь tracked для бинарных приложений как рекомендовано Cargo docs). Junction `node_modules` → `apps/next-admin/node_modules` создан в корне для Turbopack.

### 0.3 Каждый crate компилируется независимо

- [ ] `rustok-core`
- [ ] `rustok-events`
- [ ] `rustok-outbox`
- [ ] `rustok-tenant`
- [ ] `rustok-rbac`
- [ ] `rustok-index`
- [ ] `rustok-content`
- [ ] `rustok-commerce`
- [ ] `rustok-blog`
- [ ] `rustok-forum`
- [ ] `rustok-pages`
- [ ] `alloy-scripting`
- [ ] `rustok-telemetry`
- [ ] `rustok-iggy`
- [ ] `rustok-iggy-connector`
- [ ] `rustok-mcp`
- [ ] `rustok-test-utils`

### 0.4 Frontend builds

- [ ] `apps/next-admin`: `npm install && npm run build` проходит
- [ ] `apps/next-frontend`: `npm install && npm run build` проходит
- [ ] `UI/leptos`: cargo build собирается
- [ ] `UI/next`: TypeScript компилируется

### 0.5 Вспомогательные инструменты

- [ ] `cargo build -p xtask` — xtask собирается
- [x] `make help` — Makefile содержит актуальные targets
- [x] Docker: `docker-compose.yml` валиден (`docker compose config`)
- [x] Docker: `docker-compose.full-dev.yml` валиден
  - Проверено через `docker compose config` и `docker compose -f docker-compose.yml -f docker-compose.full-dev.yml config`
  - Дополнительно устранены предупреждения Compose V2: удалён устаревший ключ `version` из `docker-compose.full-dev.yml` и `docker-compose.observability.yml`

---

## Фаза 1: Соответствие архитектуре

### 1.1 Module Registry vs modules.toml

**Файлы:** `apps/server/src/modules/mod.rs`, `modules.toml`

- [x] Все модули из `modules.toml` зарегистрированы в `build_registry()`
- [x] `validate_registry_vs_manifest()` вызывается при старте сервера в `app.rs`
- [x] Slug'и в registry совпадают со slug'ами в `modules.toml`
- [x] `required = true` в `modules.toml` совпадает с `ModuleKind::Core` в коде
  - Исправлено: `content` убран из раздела required в `modules.toml` (был `required = true`, но `ContentModule::kind()` возвращает `ModuleKind::Optional` по умолчанию)
- [x] `depends_on` в `modules.toml` совпадают с `dependencies()` в `RusToKModule` impl

### 1.2 Cargo.toml workspace members

- [x] Все директории из `crates/*` присутствуют в `Cargo.toml` workspace members — через glob `crates/*`
- [x] `apps/server`, `apps/admin`, `apps/storefront` — в workspace members
- [x] `UI/leptos` — в workspace members
- [x] `benches`, `xtask` — в workspace members
- [x] Нет orphan-директорий в `crates/` без Cargo.toml (проверено: все 27 crates входят в glob)

### 1.3 Категоризация компонентов

**Проверяем соответствие Категориям A/B/C из `docs/architecture/improvement-recommendations.md`:**

- [x] **Категория A (Compile-time, не модули):** `rustok-core`, `rustok-outbox`, `rustok-events`, `rustok-telemetry`, `rustok-test-utils`, `rustok-iggy`, `rustok-iggy-connector`, `rustok-mcp`, `utoipa-swagger-ui-vendored`, `tailwind-*` — НЕ имеют `impl RusToKModule`
- [x] **Категория B (Core modules):** `rustok-index`, `rustok-tenant`, `rustok-rbac` — имеют `impl RusToKModule` с `kind() -> ModuleKind::Core`
- [x] **Категория C (Optional modules):** `rustok-content`, `rustok-commerce`, `rustok-blog`, `rustok-forum`, `rustok-pages`, `alloy-scripting` — имеют `impl RusToKModule` с `kind() -> ModuleKind::Optional`
  - `rustok-content` использует default `kind()` из trait (возвращает `ModuleKind::Optional`), что корректно

### 1.4 Зависимости между crate'ами

- [x] `rustok-blog` зависит от `rustok-content` (в Cargo.toml)
- [x] `rustok-forum` зависит от `rustok-content` (в Cargo.toml)
- [x] `rustok-index` зависит от `rustok-core` (в Cargo.toml)
- [x] Все domain crates зависят от `rustok-core`
- [x] Нет циклических зависимостей
- [x] `rustok-events` → `rustok-core` dependency chain корректен

---

## Фаза 2: Ядро платформы

### 2.1 rustok-core

**Путь:** `crates/rustok-core/src/`

#### Trait'ы и контракты
- [x] `RusToKModule` trait определён в `module.rs` с методами: `slug()`, `kind()`, `health()`, `dependencies()`, `migrations()`
- [x] `ModuleKind` enum имеет варианты `Core` и `Optional`
- [x] `EventBus` struct определён в `events/bus.rs` (broadcast-channel based, со статистикой и backpressure)
- [x] `DomainEvent` enum содержит 51 вариант, покрывающий все домены (Node*, Product*, Variant*, Inventory*, Order*, BlogPost*, ForumTopic*, ForumReply*, Tenant*, User*, Media*, Tag*, Category*, Index*, Build*)

#### Permissions
- [x] `Permission` struct определён в `permissions.rs`
- [x] `Resource` enum содержит: users, tenants, modules, settings, products, categories, orders, customers, inventory, discounts, posts, pages, nodes, media, comments, analytics, logs, webhooks, scripts
- [x] `Action` enum содержит: create, read, update, delete, list, export, import, manage
- [x] Роли определены: SuperAdmin, Admin, Manager, Customer
- [x] Каждая роль имеет корректный набор permissions

#### Registry
- [x] `ModuleRegistry` в `registry.rs` разделён на `core_modules` и `optional_modules`
- [x] `register()` корректно проверяет `ModuleKind`
- [x] `health_all()` возвращает статус всех модулей
  - Реализовано через `registry.list()` и `HealthRegistry` в health controller
- [x] `toggle_module()` запрещает отключение Core-модулей
  - `ModuleRegistry::register()` корректно разделяет Core и Optional

#### Security
- [~] `SecurityContext` struct содержит `user_id`, `role` (без `tenant_id` — tenant передаётся явно через параметры сервисов)
- [x] `PermissionScope` enum: `All`, `Own`, `None`

#### Cache
- [x] `CacheBackend` trait определён в `context.rs` (re-export через `lib.rs`)
- [x] `InMemoryCacheBackend` поддерживает per-entry TTL через moka (в `cache.rs`)
- [x] `RedisCacheBackend` работает с CircuitBreaker (в `cache.rs`)
- [x] Fallback: Redis → InMemory — реализован `FallbackCacheBackend` в `crates/rustok-core/src/cache.rs`: при Cache-ошибке от primary (Redis circuit breaker open) читает из InMemory, пишет в оба бэкенда (write-through для прогрева). Экспортирован в `lib.rs` и `prelude`.

#### Error handling
- [x] `Error` enum определён в `error/mod.rs` с вариантами: InvalidIdFormat, Database, Serialization, Auth, NotFound, Forbidden, Cache, Scripting, Validation, External
- [x] Ошибки конвертируются корректно через `ErrorResponse` в `error/response.rs`

### 2.2 rustok-outbox

**Путь:** `crates/rustok-outbox/src/`

- [x] `TransactionalEventBus` struct определён в `transactional.rs`
- [x] Атомарная запись событий в рамках DB-транзакции
- [x] `OutboxRelay` в `relay.rs` — корректный batch processing (batch=100)
- [x] Retry policy: max retries, exponential backoff
- [x] Transport trait: `EventTransport` определён в `crates/rustok-core/src/events/transport.rs`
- [x] Миграция для таблицы `sys_events` существует в `crates/rustok-outbox/src/migration.rs` и добавлена в `apps/server/migration/src/lib.rs` как `m20260211_000002_create_sys_events`

### 2.3 rustok-events

**Путь:** `crates/rustok-events/src/`

- [x] Re-export `DomainEvent` из `rustok-core` (в `crates/rustok-events/src/lib.rs`)
- [x] Re-export `EventEnvelope` из `rustok-core`
- [x] Нет дублирования определений — только re-exports

### 2.4 rustok-telemetry

**Путь:** `crates/rustok-telemetry/src/`

- [x] OpenTelemetry setup работает — `init()` настраивает tracing subscriber с `otel::init_otel_layer()` при наличии `OtelConfig`
- [x] Tracing subscriber конфигурируется — `init()` принимает `TelemetryConfig` с `log_format: Json|Pretty` и `EnvFilter::try_from_default_env()`
- [x] Prometheus metrics экспортируются — `MetricsHandle::render()` возвращает текст; `/metrics` endpoint вызывает `handle.render()`
- [x] Graceful shutdown для telemetry — `rustok_telemetry::otel::shutdown()` вызывается в `apps/server/src/main.rs` после завершения CLI (при наличии OTEL конфигурации); внутри вызывает `provider.shutdown()` через `TRACER_PROVIDER`

---

## Фаза 3: Авторизация и аутентификация

### 3.1 Auth модель

**Файлы:**
- `apps/server/src/services/auth_lifecycle.rs` — единый AuthLifecycleService
- `apps/server/src/services/auth.rs` — вспомогательные auth-функции
- `apps/server/src/controllers/auth.rs` — REST auth endpoints
- `apps/server/src/graphql/auth/` — GraphQL auth resolvers
- `apps/server/src/extractors/auth.rs` — CurrentUser extractor

#### Auth lifecycle (REST ↔ GraphQL паритет)
- [x] `register` — доступен через REST (`POST /api/auth/register`) и GraphQL (`mutation register`)
- [x] `login/sign_in` — доступен через REST и GraphQL
- [x] `refresh` — доступен через REST и GraphQL
- [x] `change_password` — доступен через REST и GraphQL
- [x] `reset_password` — доступен через REST и GraphQL (реализован через `/api/auth/reset/request` + `/api/auth/reset/confirm`)
- [x] Оба transport-слоя используют единый `AuthLifecycleService` через `AuthLifecycleService::login/register/refresh`

#### JWT
- [x] JWT generation корректен: `Claims` содержит `sub`, `tenant_id`, `role`, `exp`, `session_id`
- [x] JWT validation работает через `CurrentUser` extractor в `apps/server/src/extractors/auth.rs`
- [x] Refresh token flow реализован через `Sessions` table + `claims.session_id`
- [x] Token invalidation при смене пароля — через `Sessions::revoke()` в `AuthLifecycleService`

#### Password hashing
- [x] Используется Argon2 (crate `argon2`) — в `crates/rustok-core/src/auth/password.rs`
- [x] Параметры хэширования: `Argon2::default()` — безопасные defaults
- [x] Salt генерируется рандомно через `SaltString::generate(&mut OsRng)`

### 3.2 CurrentUser Extractor

**Файл:** `apps/server/src/extractors/auth.rs`

- [x] Извлекает JWT из `Authorization: Bearer <token>` header через `TypedHeader<Authorization<Bearer>>`
- [x] Декодирует claims и создаёт `CurrentUser` (проверяет `claims.tenant_id == tenant_id` из `TenantContext`)
- [x] Возвращает 401 при отсутствии/невалидности токена
- [x] `CurrentUser` содержит: `user` (users::Model с id, email), `session_id`, `permissions: Vec<Permission>`

---

## Фаза 4: RBAC

### 4.1 Permission Enforcement

**Файлы:**
- `apps/server/src/extractors/rbac.rs`
- `crates/rustok-core/src/permissions.rs`
- `crates/rustok-core/src/rbac.rs`

#### Extractors
- [x] `RequireNodesCreate`, `RequireNodesRead`, `RequireNodesUpdate`, `RequireNodesDelete`, `RequireNodesList` — определены и работают
- [x] `RequirePostsCreate`, `RequirePostsRead`, `RequirePostsUpdate`, `RequirePostsDelete`, `RequirePostsList` — определены
- [x] `RequireProductsCreate`, `RequireProductsRead`, `RequireProductsUpdate`, `RequireProductsDelete`, `RequireProductsList` — определены
- [x] `RequireOrdersCreate`, `RequireOrdersRead`, `RequireOrdersUpdate`, `RequireOrdersDelete`, `RequireOrdersList` — определены
- [x] `RequireUsersCreate`, `RequireUsersRead`, `RequireUsersUpdate`, `RequireUsersDelete`, `RequireUsersList` — определены
- [x] `RequireSettingsRead`, `RequireSettingsUpdate` — определены
- [x] `RequireAnalyticsRead`, `RequireAnalyticsExport` — определены
- [x] `RequireBlogPostsCreate`, `RequireBlogPostsRead`, `RequireBlogPostsUpdate`, `RequireBlogPostsDelete`, `RequireBlogPostsList`, `RequireBlogPostsPublish` — определены
- [x] `RequireForumTopicsCreate`, `RequireForumTopicsRead`, `RequireForumTopicsUpdate`, `RequireForumTopicsDelete`, `RequireForumTopicsList`, `RequireForumTopicsModerate` — определены
- [x] `RequireForumRepliesCreate`, `RequireForumRepliesRead`, `RequireForumRepliesModerate` — определены
- [x] `RequireForumCategoriesCreate`, `RequireForumCategoriesList`, `RequireForumCategoriesUpdate`, `RequireForumCategoriesDelete` — определены
- [x] `RequirePagesCreate`, `RequirePagesRead`, `RequirePagesUpdate`, `RequirePagesDelete` — определены
- [x] `RequireScriptsCreate`, `RequireScriptsRead`, `RequireScriptsList`, `RequireScriptsManage` — определены
- [x] `RequireLogsRead` — определён (для DLQ admin endpoints)
- [x] Макрос `define_permission_extractor!` работает

#### Inline checks
- [x] `check_permission()` — проверяет одну permission
- [x] `check_any_permission()` — проверяет OR-набор
- [x] `check_all_permissions()` — проверяет AND-набор

#### Role-Permission матрица
- [x] **SuperAdmin** — полный доступ ко всем ресурсам
- [x] **Admin** — полный доступ к tenant-ресурсам, нет доступа к tenant management
- [x] **Manager** — commerce + content CRUD, нет user management
- [x] **Customer** — только read + own orders/comments

### 4.2 RBAC на уровне сервисов

- [x] Все service-методы принимают `SecurityContext`
- [x] `get_scope()` возвращает `PermissionScope::All/Own/None`
- [x] Фильтрация по scope применяется в list-запросах — `NodeService::list_nodes()` корректно применяет `PermissionScope::Own` (фильтрует по `author_id = user_id` для Customer); Orders не реализован (OrderService отсутствует — известный `[!]`)

### 4.3 RBAC на GraphQL

- [x] GraphQL resolvers проверяют permissions перед выполнением
  - `mutations.rs`: `create_user`, `update_user`, `delete_user`, `disable_user` — через `AuthService::has_permission()`
  - `graphql/blog/mutation.rs`: все mutations — через `AuthService::has_any_permission()`
  - `graphql/content/mutation.rs`: `create_node`, `update_node`, `delete_node` — через `AuthService::has_any_permission()` (NODES_CREATE/UPDATE/DELETE)
  - `graphql/commerce/mutation.rs`: `create_product`, `update_product`, `publish_product`, `delete_product` — через `AuthService::has_any_permission()` (PRODUCTS_CREATE/UPDATE/DELETE)
  - `graphql/pages/mutation.rs`: все 5 mutations — через `AuthService::has_any_permission()` (PAGES_CREATE/UPDATE/DELETE)
  - `graphql/forum/mutation.rs`: все mutations — через `AuthService::has_any_permission()` (FORUM_TOPICS/REPLIES/CATEGORIES permissions)
  - `graphql/alloy/mutation.rs`: через `require_admin()` (SCRIPTS_MANAGE)
- [x] Механизм проверки permissions в GraphQL context — `AuthService::has_any_permission(db, tenant_id, user_id, permissions)`
- [x] Ошибка 403 корректно преобразуется в GraphQL error extension — через `GraphQLError::permission_denied()`

### 4.4 RBAC consistency

- [x] REST endpoints `content/nodes.rs`, `blog/posts.rs`, `forum/topics.rs`, `forum/replies.rs`, `forum/categories.rs`, `pages.rs`, `admin_events.rs` — RBAC extractors применены
- [x] REST `commerce/products.rs`, `commerce/variants.rs`, `commerce/inventory.rs` — RBAC extractors применены
- [x] GraphQL mutations Blog — RBAC через `AuthService::has_any_permission()` добавлен
- [x] GraphQL mutations Forum — реализованы с полноценным RBAC (topics/replies/categories)
- [x] GraphQL mutations Content — RBAC через `AuthService::has_any_permission()` добавлен
- [x] GraphQL mutations Commerce — RBAC через `AuthService::has_any_permission()` добавлен
- [x] GraphQL mutations Pages — RBAC через `AuthService::has_any_permission()` добавлен
- [~] Нет endpoints без auth/RBAC (кроме public: health, login, register, public storefront queries)
  - Blog/Pages queries — публичные (для storefront), не требуют auth
  - Forum queries — требуют auth через `AuthContext`

---

## Фаза 5: Multi-Tenancy

### 5.1 Tenant Resolution

**Файлы:**
- `apps/server/src/middleware/tenant.rs`
- `crates/rustok-tenant/src/`

- [x] Middleware извлекает tenant из: `X-Tenant-Slug` header, `X-Tenant-ID` header, hostname/domain
- [x] При отсутствии tenant → 400/404 (через `TenantError.status_code()`)
- [x] `TenantContext` реализует `FromRequestParts` в `apps/server/src/context/tenant.rs`

### 5.2 Tenant Cache

- [x] `TenantCacheInfrastructure` хранится в `AppContext.shared_store` — через `ctx.shared_store.insert(infra.clone())` в `init_tenant_cache_infrastructure()`
- [x] Positive cache: TTL 5 мин (`TENANT_CACHE_TTL = 300s`), capacity 1000 (`TENANT_CACHE_MAX_CAPACITY = 1_000`)
- [x] Negative cache: TTL 60 сек (`TENANT_NEGATIVE_CACHE_TTL = 60s`), capacity 1000
- [x] Versioned keys: `v1:<type>:<value>` — через `TenantCacheKeyBuilder::new(TENANT_CACHE_VERSION)` где `TENANT_CACHE_VERSION = "v1"`
- [x] Redis backend выбирается при наличии `RUSTOK_REDIS_URL` или `REDIS_URL` (feature = "redis-cache")
- [x] Fallback на InMemory при отсутствии Redis — `build_tenant_cache_backend()` возвращает `InMemoryCacheBackend` по умолчанию
- [x] Stampede protection: singleflight pattern реализован через `get_or_load_with_coalescing()` с `Arc<Notify>`

### 5.3 Tenant Isolation в данных

- [x] **Все** domain-таблицы имеют поле `tenant_id` (проверено по миграциям):
  - `nodes`, `products`, `product_variants` — `tenant_id` NOT NULL + FK + индексы
  - `categories`, `category_translations`, `tags`, `tag_translations` — `tenant_id` NOT NULL
  - `stock_locations`, `price_lists`, `regions`, `collections`, `commerce_categories` — `tenant_id` NOT NULL
  - `media`, `metas`, `index_content`, `index_products` — `tenant_id` NOT NULL
  - `product_options`, `variant_option_values` — изолируются через parent FK
- [x] **Все** SELECT-запросы в сервисах фильтруют по `tenant_id` — NodeService.find_node добавляет .filter(TenantId.eq(tenant_id)), tenant_id передаётся во все методы content/blog/forum/pages
- [x] **Все** INSERT-запросы проставляют `tenant_id` — create_node, create_post, create_topic принимают tenant_id первым параметром
- [x] Нет cross-tenant data leaks — исправлено в NodeService, PostService, TopicService, ReplyService, CategoryService, PageService, BlockService, MenuService, ModerationService

### 5.4 Tenant Modules

**Таблица:** `tenant_modules`

- [x] Таблица `tenant_modules` имеет schema: `id, tenant_id, module_slug, enabled, settings, created_at, updated_at`
- [x] UNIQUE constraint на `(tenant_id, module_slug)`
- [x] `toggle_module()` проверяет зависимости перед отключением — при enable: MissingDependencies; при disable: HasDependents
- [x] Core-модули нельзя отключить — использует `registry.is_core()` вместо hardcoded slug-списка (Issue #19)

### 5.5 Cross-instance Cache Invalidation (Redis mode)

- [x] При обновлении tenant → PUBLISH в `tenant.cache.invalidate` — через `TenantInvalidationPublisher::publish()` с Redis PUBLISH команд
- [x] Все инстансы подписаны и инвалидируют matching ключи — через `spawn_invalidation_listener()` с Redis PubSub
- [x] Метрики cache hit/miss экспортируются через Redis INCR — через `TenantCacheMetricsStore::incr()`

---

## Фаза 6: Событийная система

### 6.1 Event Transport Architecture

**Файлы:**
- `apps/server/src/services/event_transport_factory.rs`
- `apps/server/src/app.rs` — `build_event_runtime()`
- `crates/rustok-outbox/src/`
- `crates/rustok-iggy/src/`

- [x] `build_event_runtime()` вызывается в `app.rs::after_routes()` — реализовано в `event_transport_factory.rs`
- [x] Transport selection: `settings.rustok.events.transport` = `memory|outbox|iggy` — через `EventTransportKind` enum
- [x] L0 (Memory): `MemoryTransport::new()` — для dev режима
- [x] L1 (Outbox): `TransactionalEventBus` + `OutboxTransport` — пишет в outbox таблицу
- [x] L1 (Outbox): `OutboxRelay` читает pending events и публикует с retry
- [x] L2 (Iggy): `IggyTransport` — соединение с Iggy-сервером
- [x] Default production transport = `outbox` (relay_config задаётся через settings)

### 6.2 Event Flow (Write Path)

- [x] Domain service создаёт сущность + публикует DomainEvent в одной транзакции
  - [x] `rustok-content` (NodeService): корректно использует `publish_in_tx()`
  - [x] `rustok-commerce` (CatalogService, InventoryService, PricingService): корректно использует `publish_in_tx()`
  - [x] `rustok-blog` (PostService): исправлено — все вызовы используют `publish_in_tx()` через открытую транзакцию
  - [x] `rustok-forum` (TopicService, ReplyService, ModerationService): исправлено — все вызовы используют `publish_in_tx()`
  - [x] REST `variants.rs` controller: исправлено — `create_variant`, `update_variant`, `delete_variant` переведены на `publish_in_tx()` внутри транзакции
- [x] `TransactionalEventBus::publish_in_tx()` атомарно записывает через `OutboxTransport::write_to_outbox()`
- [x] EventEnvelope содержит: id, event_type, schema_version, tenant_id, actor_id, timestamp, retry_count
- [x] `tenant_id` передаётся в EventEnvelope через `publish_in_tx(txn, tenant_id, actor_id, event)`

### 6.3 Event Flow (Read Path — Index)

- [x] `rustok-index` подписывается на DomainEvents
- [x] При `NodeCreated/NodeUpdated` — обновляет `index_content` (ContentIndexer реализован: JOIN nodes+translations+bodies+category_translations+users, UPSERT в index_content)
- [x] При `ProductCreated/ProductUpdated` — обновляет `index_products` (ProductIndexer реализован: JOIN products+product_translations+variants+prices, UPSERT в index_products)
- [x] Indexer корректно обрабатывает ошибки (warn!() при ошибке reindex_all, не прерывает пакет)

### 6.4 DomainEvent Coverage

**Проверяем, что каждый модуль публикует нужные события:**

#### Content Events
- [x] `NodeCreated` — при создании node (`node_service.rs:239`)
- [x] `NodeUpdated` — при обновлении node (`node_service.rs:411,514,541,567,588,614`)
- [~] `NodeDeleted` — при удалении node (soft delete через статус, NodeUpdated публикуется)
- [x] `NodePublished` / `NodeUnpublished` — при смене статуса (`node_service.rs:518,545`)

#### Commerce Events
- [x] `ProductCreated`, `ProductUpdated`, `ProductDeleted` (в `CatalogService`)
- [x] `VariantCreated`, `VariantUpdated`, `VariantDeleted` (в REST `variants.rs` — исправлено, теперь через `publish_in_tx()`)
- [x] `PriceUpdated` (в `PricingService`)
- [x] `InventoryUpdated` (в `InventoryService`)
- [!] `OrderPlaced`, `OrderStatusChanged` — OrderService не реализован

#### Blog Events
- [x] `BlogPostCreated`, `BlogPostUpdated`, `BlogPostDeleted` (в `PostService`)
- [x] `BlogPostPublished`, `BlogPostUnpublished` (в `PostService`)

#### Forum Events
- [x] `ForumTopicCreated` (в `TopicService`)
- [x] `ForumTopicStatusChanged`, `ForumTopicPinned` (в `ModerationService`)
- [x] `ForumTopicReplied` — в `ReplyService` (`reply.rs:104`)

#### Pages Events
- [x] Pages публикуют события через NodeService (NodeCreated/NodePublished/NodeUnpublished)
  - Нет отдельных PageCreated/PagePublished событий — используются Node* события

### 6.5 Outbox Relay

- [x] Relay обрабатывает pending events корректно
- [x] Retry с exponential backoff (1s → 60s)
- [x] Max retries = 5 (или конфигурируемо)
- [x] DLQ: после max retries → status = `failed`
- [x] Метрики: `outbox_backlog_size`, `outbox_retries_total`, `outbox_dlq_total`

### 6.6 Event Versioning

- [x] Каждый DomainEvent имеет версию — `schema_version: u16` в `EventEnvelope`; `DomainEvent::schema_version()` возвращает версию для каждого типа события (все текущие события = v1)
- [x] Обратная совместимость при добавлении новых полей — `#[serde(tag = "type", content = "data")]` + `EventEnvelope` с `schema_version` позволяет добавлять новые варианты без поломки десериализации

---

## Фаза 7: Доменные модули

### 7.1 rustok-content

**Путь:** `crates/rustok-content/`

#### Entities
- [x] `nodes` entity: id, tenant_id, slug, node_type, status, author_id, created_at, updated_at
- [x] `node_translations` entity: id, node_id, locale, title, body
- [x] `bodies` entity — реализована в `crates/rustok-content/src/entities/body.rs` (id, node_id, locale, body, format, updated_at); миграция в `m20250130_000005_create_nodes.rs`
- [x] Все entities имеют tenant_id

#### Services
- [x] `NodeService` — CRUD для nodes
- [x] `create_node()` — валидация, сохранение, публикация `NodeCreated`
- [x] `update_node()` — валидация, сохранение, публикация `NodeUpdated`
- [x] `delete_node()` — soft delete через deleted_at, публикация `NodeDeleted` через publish_in_tx()
- [x] `list_nodes()` — пагинация, фильтрация, tenant_id scope
- [x] `publish_node()` / `unpublish_node()` — state machine transition

#### DTOs
- [x] `CreateNodeInput` — валидация полей
- [x] `UpdateNodeInput` — partial update
- [x] `NodeResponse` — response DTO
- [x] `NodeListItem` — list response DTO

#### State Machine
- [x] Node status transitions: Draft → Published → Archived
- [x] Невалидные transitions возвращают ошибку
- [x] Property tests для state machine (`state_machine_proptest.rs`)

#### Migrations
- [x] Миграция создаёт таблицу `nodes` — в `m20250130_000005_create_nodes.rs`
- [x] Миграция создаёт таблицу `node_translations` — в `m20250130_000005_create_nodes.rs`
- [x] Миграции управляются через apps/server/migration (не через ContentModule::migrations())
  - Добавлена миграция m20260301_000002_alter_nodes_add_soft_delete для deleted_at и version полей

### 7.2 rustok-commerce

**Путь:** `crates/rustok-commerce/`

#### Entities
- [x] `products` entity: id, tenant_id, status (enum: Draft/Active/Archived), vendor, product_type, metadata, created_at, updated_at, published_at
- [x] `product_translations` entity: id, product_id, locale, title, handle, description, meta_title, meta_description
- [x] `product_variants` entity: id, product_id, tenant_id, sku, barcode, inventory_policy, inventory_quantity, weight, option1-3, position, created_at, updated_at
- [x] `variant_translations` entity (в entities/variant_translation.rs)
- [x] `prices` entity: id, variant_id, currency_code, amount, compare_at_amount
- [x] `product_images` entity (в entities/product_image.rs)
- [x] `product_options` entity (в entities/product_option.rs)
- [x] Tenant isolation: `products` и `product_variants` имеют `tenant_id`; дочерние записи (translations, prices, images, options) изолируются через parent ID

#### Services
- [x] `CatalogService` — CRUD для products
- [x] `InventoryService` — управление стоками
- [x] `PricingService` — управление ценами
- [x] Все сервисы используют `TransactionalEventBus` (корректно через `publish_in_tx()`)
- [x] Все сервисы принимают `SecurityContext`

#### DTOs
- [x] `CreateProductInput` / `UpdateProductInput` (в `dto/product.rs`)
- [x] `ProductResponse` / `ProductListItem` (в `dto/product.rs`)
- [x] `CreateVariantInput` / `VariantResponse` (в `dto/variant.rs`)

#### State Machine
- [x] Product status: Draft → Active → Archived (в `state_machine.rs`)
- [x] Property tests для state machine (`state_machine_proptest.rs`)

#### Migrations
- [x] Все commerce-таблицы имеют миграции — `m20250130_000012..000018` покрывают products/options/variants/prices/inventory/collections/categories
- [~] Миграции управляются через `apps/server/migration` (не через `RusToKModule::migrations()`)

### 7.3 rustok-blog

**Путь:** `crates/rustok-blog/`

- [x] Зависит от `rustok-content` (uses nodes internally)
- [x] `BlogModule::dependencies()` возвращает `&["content"]`
- [x] `PostService` — CRUD для постов (обёртка над NodeService)
- [x] State machine: Draft → Published → Archived
- [x] Events: `PostCreated`, `PostPublished`, etc. — исправлено, все события публикуются через `publish_in_tx()` в рамках транзакции
- [x] DTOs: `CreatePostInput`, `PostResponse`, `PostListItem`
- [x] Поддержка i18n (locale.rs)
- [~] Миграции: не нужны — blog использует таблицы rustok-content (nodes + node_translations + bodies) через NodeService

### 7.4 rustok-forum

**Путь:** `crates/rustok-forum/`

- [x] Зависит от `rustok-content`
- [x] `ForumModule::dependencies()` возвращает `&["content"]`
- [x] `TopicService` — CRUD для тем
- [x] `ReplyService` — CRUD для ответов
- [x] `CategoryService` — категории форума
- [x] Events: `TopicCreated`, `ReplyCreated`, etc. — исправлено, все события публикуются через `publish_in_tx()`
- [x] DTOs: `CreateTopicInput`, `TopicResponse`, etc.
- [x] Поддержка i18n (locale.rs)
- [~] Миграции: не нужны — forum использует таблицы rustok-content (nodes + node_translations + bodies) через NodeService
- [x] Constants (`constants.rs`)

### 7.5 rustok-pages

**Путь:** `crates/rustok-pages/`

- [x] `PageService` — CRUD для страниц (create, update, publish, unpublish, delete, get, get_by_slug, list)
- [x] State machine: Draft → Published → Archived (использует ContentStatus из rustok-content)
- [x] Events: публикуются через NodeService (NodeCreated/NodePublished/etc.)
- [x] DTOs: `CreatePageInput`, `UpdatePageInput`, `PageResponse`, `PageListItem`, `ListPagesFilter`
- [~] Миграции: не нужны — использует таблицы rustok-content (nodes + translations + bodies)

### 7.6 alloy-scripting

**Путь:** `crates/alloy-scripting/`

- [x] `AlloyModule` зарегистрирован как `ModuleKind::Optional` (в `apps/server/src/modules/alloy.rs`)
- [x] Rhai scripting engine инициализируется — `create_default_engine()` в `apps/server/src/app.rs::after_routes()` с `EngineConfig` (max_operations, max_call_depth, max_string_size, max_array_size)
- [x] `scripts` таблица — CRUD реализован в `SeaOrmStorage` (`storage/sea_orm.rs`); GraphQL API в `graphql/alloy/`; REST API через `controllers::alloy::router()`
- [x] RBAC permissions: `Scripts` resource (create/read/update/delete/list/manage) — объявлены в `AlloyModule::permissions()`, проверяются в `require_admin()`
- [x] Безопасность: Rhai `set_max_operations()`, `set_strict_variables(true)` — I/O только через `bridge/http.rs` в OnCommit/Manual/Scheduled фазах
- [x] Миграции — `ScriptsMigration` и `ScriptExecutionsMigration` добавлены в главный Migrator через wrapper-файлы `m20260302_000001_create_scripts.rs` и `m20260302_000002_create_script_executions.rs`

### 7.7 rustok-index (CQRS Read Models)

**Путь:** `crates/rustok-index/`

- [x] `IndexModule` зарегистрирован как `ModuleKind::Core` (в `crates/rustok-index/src/lib.rs`)
- [x] `IndexListener` определён в `listener.rs` — слушает NodeCreated/NodeUpdated/NodePublished/NodeDeleted, вызывает `reindex_node()`/`engine.delete()`
- [x] Engine trait определён в `engine.rs` — trait `SearchEngine` с методами `index()`, `delete()`, `search()`
- [x] `PgSearchEngine` реализован в `pg_engine.rs` — PostgreSQL full-text search
- [x] Denormalized models в `models.rs`
- [x] Search services в `services/`
- [x] Product indexer: полностью реализован в `product/indexer.rs` — JOIN products+product_translations+variants+prices, UPSERT в index_products, все commerce events
- [x] Content indexer: полностью реализован в `content/indexer.rs` — JOIN nodes+translations+bodies+category_translations+users, UPSERT в index_content, все content events

### 7.8 rustok-rbac

**Путь:** `crates/rustok-rbac/`

- [x] `RbacModule` зарегистрирован как `ModuleKind::Core` (в `crates/rustok-rbac/src/lib.rs`)
- [x] Entities в `entities/` — role, permission, user_role сущности
- [x] DTOs в `dto/`
- [x] Services: `permission_resolver`, `permission_authorizer`, `permission_evaluator`, `permission_policy`, `relation_permission_resolver`, `runtime_permission_resolver`, `shadow_decision`, `shadow_dual_read`, `authz_mode`
- [x] Health check работает — `health()` возвращает `HealthStatus::Healthy`
- [x] Integration events: `RbacRoleAssignmentEvent` для cross-module RBAC notifications
- [~] Миграции: `migrations()` возвращает `Vec::new()` — таблицы управляются через главный migration сервер

### 7.9 rustok-tenant

**Путь:** `crates/rustok-tenant/`

- [x] `TenantModule` зарегистрирован как `ModuleKind::Core` (в `crates/rustok-tenant/src/lib.rs`)
- [x] Entities: `tenants`, `tenant_modules` — реализованы в `crates/rustok-tenant/src/entities/` (tenant.rs, tenant_module.rs) с полными SeaORM entity definitions
- [x] Services: CRUD для tenants, module toggle — реализован `TenantService` в `crates/rustok-tenant/src/services/tenant_service.rs` с методами create/get/update/list/toggle_module/list_tenant_modules
- [x] Health check работает — `health()` возвращает `HealthStatus::Healthy`
- [~] Миграции: `migrations()` возвращает `Vec::new()` — таблицы управляются через apps/server/migration

---

## Фаза 8: API — GraphQL

### 8.1 Schema Assembly

**Файлы:**
- `apps/server/src/graphql/schema.rs`
- `apps/server/src/graphql/queries.rs`
- `apps/server/src/graphql/mutations.rs`

- [x] Schema собирается через `MergedObject` в `apps/server/src/graphql/schema.rs`
- [x] Merged Query: `RootQuery`, `AuthQuery`, `CommerceQuery`, `ContentQuery`, `BlogQuery`, `ForumQuery`, `AlloyQuery`, `PagesQuery`
- [x] Merged Mutation: `RootMutation`, `AuthMutation`, `CommerceMutation`, `ContentMutation`, `BlogMutation`, `ForumMutation`, `AlloyMutation`, `PagesMutation`
- [x] Schema endpoint: `POST /api/graphql` (через Loco routing)
- [~] GraphQL Playground — определяется конфигурацией async-graphql

### 8.2 Content GraphQL

**Файлы:** `apps/server/src/graphql/content/`

- [x] Query: `node(id)`, `nodes(filter)` — реализованы в `graphql/content/query.rs`
- [x] Mutation: `createNode`, `updateNode`, `deleteNode`, `publishNode` — реализованы с RBAC
- [x] Auth/RBAC проверяются через `AuthService::has_any_permission()`
- [x] Tenant isolation соблюдается (tenant_id передаётся в сервисы)

### 8.3 Commerce GraphQL

**Файлы:** `apps/server/src/graphql/commerce/`

- [x] Query: `product(id)`, `products(filter)` — реализованы
- [~] `order(id)`, `orders(filter)` — OrderService не реализован
- [x] Mutation: `createProduct`, `updateProduct`, `deleteProduct` — с RBAC
- [~] `createOrder` — не реализован (OrderService отсутствует)
- [~] Variants, Inventory, Prices mutations — управляются через REST, не GraphQL (только products в GraphQL)

### 8.4 Blog GraphQL

**Файлы:** `apps/server/src/graphql/blog/`

- [x] Query: `post(id)`, `posts(filter)` — реализованы в `graphql/blog/query.rs`
- [x] Mutation: `createPost`, `updatePost`, `deletePost`, `publishPost` — с полным RBAC

### 8.5 Forum GraphQL

**Файлы:** `apps/server/src/graphql/forum/`

- [x] Query: `forum_topics(filter)`, `forum_replies(topicId)`, `forum_categories()` — реализованы
- [x] Mutation: `createTopic`, `createReply`, `updateTopic`, `deleteTopic`, `createCategory`, `deleteCategory` — с RBAC
- [x] Categories: query + mutation реализованы с полным RBAC

### 8.6 Alloy GraphQL

**Файлы:** `apps/server/src/graphql/alloy/`

- [~] Query: `script(id)`, `scripts(filter)` — зависит от состояния `alloy-scripting` crate
- [~] Mutation: `createScript`, `updateScript`, `executeScript`, `deleteScript` — требует audit `graphql/alloy/`

### 8.7 Pages GraphQL

**Файлы:** `apps/server/src/graphql/pages/`

- [x] Query: `page(id)`, `page_by_slug(slug)`, `pages(filter)` — реализованы
- [x] Mutation: `createPage`, `updatePage`, `deletePage`, `publishPage`, `unpublishPage` — реализованы с RBAC
- [x] Pages добавлены в `schema.rs` (Query и Mutation merged objects)
- [x] `pages` добавлен в `graphql/mod.rs`

### 8.8 DataLoader

**Файл:** `apps/server/src/graphql/loaders.rs`

- [x] DataLoaders определены для N+1 prevention — реализованы `TenantNameLoader`, `NodeLoader`, `NodeTranslationLoader`, `NodeBodyLoader` с батчингом через `async_graphql::dataloader::Loader`
- [x] Зарегистрированы в `schema.rs`: `DataLoader::new(TenantNameLoader, tokio::spawn)` и аналогично для Node, NodeTranslation, NodeBody

### 8.9 Auth GraphQL

**Файлы:** `apps/server/src/graphql/auth/`

- [x] `register` mutation — реализована в `graphql/auth/mutation.rs`
- [x] `login` / `signIn` mutation — реализована
- [x] `refreshToken` mutation — реализована
- [x] `changePassword` mutation — реализована
- [x] `me` query — реализована в `graphql/auth/query.rs`

### 8.10 Observability GraphQL

**Файл:** `apps/server/src/graphql/observability.rs`

- [~] Query для module health — реализован через `RootQuery::health()` и `module_registry()`/`enabled_modules()` в `queries.rs`
- [~] Query для system status — реализован через `RootQuery::dashboard_stats()` (nodes count, events, active users) в `queries.rs`
- [x] GraphQL Observability Extension — `GraphqlObservability` в `observability.rs` логирует latency/status каждого resolver через tracing; подключён в `schema.rs`

---

## Фаза 9: API — REST

### 9.1 Auth REST

**Файл:** `apps/server/src/controllers/auth.rs`

- [x] `POST /api/auth/register` — регистрация
- [x] `POST /api/auth/login` — вход
- [x] `POST /api/auth/refresh` — обновление токена
- [x] `POST /api/auth/change-password` — смена пароля
- [x] `POST /api/auth/reset/request` + `POST /api/auth/reset/confirm` — сброс пароля

### 9.2 Health REST

**Файл:** `apps/server/src/controllers/health.rs`

- [x] `GET /health`, `GET /health/live`, `GET /health/ready` — реализованы в `controllers/health.rs`
- [x] Включает статус модулей из `ModuleRegistry`
- [x] Включает статус DB-соединения (через `ready` endpoint)
- [x] Возвращает корректные статусы через health checks

### 9.3 Commerce REST

**Файлы:** `apps/server/src/controllers/commerce/`

- [x] `products.rs` — CRUD + publish/unpublish (8 endpoints: list, create, get, update, delete, publish, unpublish)
- [x] `variants.rs` — управление вариантами (6 endpoints: list, create, get, update, delete, update_prices)
- [x] `inventory.rs` — управление стоками (4 endpoints: get, adjust, set, check_availability)
- [x] Все endpoints имеют RBAC (RequireProducts* / RequireInventory*)
- [x] Все endpoints имеют tenant isolation (TenantContext передаётся во все сервисы)
- [x] OpenAPI annotations (`#[utoipa::path(...)]`) на всех handlers
- [x] События публикуются через `publish_in_tx()` в транзакции (исправлено в variants.rs)

### 9.4 Content REST

**Файлы:** `apps/server/src/controllers/content/`

- [x] `nodes.rs` — CRUD для nodes
- [x] RBAC: все 5 endpoints используют RBAC extractors (`RequireNodesList`, `RequireNodesRead`, `RequireNodesCreate`, `RequireNodesUpdate`, `RequireNodesDelete`)
- [x] Tenant isolation: `TenantContext` передаётся в сервис

### 9.5 Blog REST

**Файлы:** `apps/server/src/controllers/blog/`

- [x] `posts.rs` — CRUD + publish/unpublish для posts (7 endpoints)
- [x] RBAC: все endpoints используют специализированные Blog RBAC extractors
- [x] Tenant isolation: `TenantContext` передаётся в сервис

### 9.6 Forum REST

**Файлы:** `apps/server/src/controllers/forum/`

- [x] `topics.rs` — CRUD для topics (6 endpoints с RBAC)
- [x] `replies.rs` — CRUD для replies (5 endpoints с RBAC)
- [x] `categories.rs` — CRUD для categories (5 endpoints с RBAC)
- [x] RBAC: Forum-специфичные extractors (`RequireForumTopicsCreate`, etc.)
- [x] Tenant isolation: `TenantContext` передаётся в сервис

### 9.7 Pages REST

**Файл:** `apps/server/src/controllers/pages.rs`

- [x] GET `/api/pages` — получение страницы по slug
- [x] POST `/api/admin/pages` — создание страницы
- [x] RBAC: `RequirePagesRead` и `RequirePagesCreate` применены
- [x] Tenant isolation: `TenantContext` передаётся в сервис

### 9.8 Admin Events REST

**Файл:** `apps/server/src/controllers/admin_events.rs`

- [x] `GET /api/admin/events/dlq` — просмотр DLQ
- [x] `POST /api/admin/events/dlq/{id}/replay` — replay
- [x] RBAC: `RequireLogsRead` применён — доступен только SuperAdmin и Admin

### 9.9 Metrics & Swagger

- [x] `GET /metrics` — Prometheus metrics реализован в `controllers/metrics.rs`, маршрут `/metrics` зарегистрирован в `app.rs`
- [x] `GET /api/openapi.json` и `GET /api/openapi.yaml` — OpenAPI spec endpoints добавлены в `controllers/swagger.rs`, маршруты зарегистрированы в `app.rs`
- [x] OpenAPI spec генерируется корректно через `utoipa` — `ApiDoc::openapi().to_json()/to_yaml()` используется в handlers

### 9.10 Rate Limiting

**Файл:** `apps/server/src/middleware/rate_limit.rs`

- [x] Rate limiting middleware подключён — в `app.rs::after_routes()` через `axum_middleware::from_fn` с per-IP sliding window limiter
- [x] Корректные лимиты для auth endpoints (login/register/reset): 20 req/60 сек per IP для `/api/auth/login`, `/api/auth/register`, `/api/auth/reset/*`
  - Cleanup task запускается через `tokio::spawn` каждые 5 мин
  - Response headers: `x-ratelimit-limit`, `x-ratelimit-remaining`, `x-ratelimit-reset`; при 429 — `retry-after`
- [x] Корректные лимиты для API endpoints: глобальный rate limit 300 req/60 сек per IP для всех `/api/*` через `rate_limit_for_paths` middleware, подключён в `app.rs::after_routes()`

---

## Фаза 10: Фронтенды — Leptos

### 10.1 apps/admin (Leptos CSR)

**Путь:** `apps/admin/`

- [x] Cargo.toml: зависимости корректны — leptos (csr), leptos_router, leptos-auth, leptos-graphql, leptos-ui, leptos-forms, leptos-hook-form, leptos-zod, leptos-zustand, leptos-shadcn-pagination, leptos_i18n, etc.
- [ ] Собирается: `cargo build -p rustok-admin` (требует Trunk/WASM toolchain)
- [x] Entry point: `main.rs` монтирует `<App />` через `mount_to_body`; `lib.rs` экспортирует модули app/entities/features/pages/shared/widgets
- [x] Routing: `apps/admin/src/app/router.rs` — страницы зарегистрированы (dashboard, login, register, profile, users, user_details, security, reset, not_found)
- [x] Auth: `leptos-auth` crate используется (`use_auth`, `use_current_user`, `use_token` hooks + GraphQL mutations SignIn/SignUp)
- [x] GraphQL client: `leptos-graphql` crate (hooks: `use_query`, `use_mutation`, `use_lazy_query`; endpoint через `GRAPHQL_ENDPOINT` const)
- [x] Используется `leptos-auth` для auth state
- [x] Используется `leptos-zustand` для state management
- [x] Используется `leptos-graphql` для GraphQL queries/mutations
- [x] Используется `leptos-ui` (shadcn-port) для UI компонентов

#### Страницы admin panel
- [x] Dashboard (`pages/dashboard.rs`)
- [~] Products list / create / edit — зависит от modules wiring
- [ ] Orders list / view — OrderService не реализован
- [~] Content / Nodes list / create / edit — зависит от modules wiring
- [~] Blog posts list / create / edit — зависит от modules wiring
- [~] Pages list / create / edit — зависит от modules wiring
- [x] Users management (`pages/users.rs`, `pages/user_details.rs`)
- [x] Settings / Security (`pages/security.rs`)
- [~] Module management (toggle per-tenant) — UI в `app/modules/`

### 10.2 apps/storefront (Leptos SSR)

**Путь:** `apps/storefront/`

- [ ] Собирается (требует Trunk/WASM toolchain)
- [x] SSR работает — Axum-based SSR в `main.rs`: `render_shell()` + `StorefrontShell` компонент, роутер с поддержкой `?lang=en|ru`
- [x] SEO: structured HTML с meta-данными через SSR rendering
- [x] Product catalog page — `ProductCard` компонент + `ProductCardData` struct реализованы
- [~] Product detail page — не реализована отдельная страница
- [~] Blog posts page — не реализована отдельная страница
- [x] Static pages — i18n (ru/en) через `LocaleStrings` struct
- [ ] Cart / Checkout flow — не реализован

---

## Фаза 11: Фронтенды — Next.js

### 11.1 apps/next-admin

**Путь:** `apps/next-admin/`

- [x] `package.json`: зависимости корректны — Next.js 16, Clerk, shadcn/ui, recharts, react-day-picker, @tanstack/react-table, etc.
- [x] `npm install` проходит (с предупреждениями о peerDeps из-за bun.lock)
- [x] `npm run build` — повторно проверено после фиксов `calendar.tsx`/`chart.tsx`/`next.config.ts`: `npm install && npm run build` проходит в `apps/next-admin`.
  - Исправлено ранее и перепроверено: `calendar.tsx` — `Chevron` вместо `IconLeft`/`IconRight`, новые `classNames` (month_caption, button_previous, button_next, etc.)
  - Исправлено ранее и перепроверено: `chart.tsx` — `TooltipProps<ValueType, NameType>` вместо `React.ComponentProps<typeof Tooltip>`
  - Исправлено ранее и перепроверено: `next.config.ts` — `turbopack.root` для разрешения local crate UI packages; webpack `resolve.alias`
  - Исправлено ранее и перепроверено: `@rustok/blog-admin` `package.json` — добавлены реальные `dependencies` вместо `peerDependencies`
- [x] `npm run lint` проходит
  - Для рабочей локальной верификации `apps/next-admin/package.json` переведён с `next lint` на прямой вызов `eslint src`: в Next.js 16 CLI-команда `next lint` в этом приложении падала с `Invalid project directory .../lint`, а ESLint 10 требовал flat-config миграцию. Добавлены `eslint.config.mjs` и `.eslintignore`, а `eslint` зафиксирован на `8.57.1`, чтобы `npm run lint` реально проверял исходники `src/`.
- [x] Clerk auth setup — `docs/clerk_setup.md` есть, `src/auth.ts` настроен
- [x] RBAC навигация — `docs/nav-rbac.md` есть, `src/config/nav-config.ts` использует `getAdminNavItems()` с `access: { role }`
- [x] Темизация — `docs/themes.md` есть, `src/styles/` + CSS variables
- [x] GraphQL клиент подключён — `src/lib/graphql/` с Apollo/fetch клиентом
- [x] Используются packages из `packages/` — `leptos-auth`, `leptos-graphql` через `packages/*/next/index.ts`
- [x] Routing: admin-страницы через Next.js App Router в `src/app/dashboard/`
- [x] TypeScript компилируется — полная проверка подтверждена успешным `npm run build`

### 11.2 apps/next-frontend

**Путь:** `apps/next-frontend/`

- [x] `package.json`: зависимости корректны — Next.js, next-intl, Tailwind
- [x] `npm install && npm run build` проходит — `SUCCESS` (проверено локально)
- [x] SSR / SSG для SEO — Next.js App Router с `generateStaticParams` / Server Components
- [~] Product catalog — структура есть (`src/app/`), наполнение зависит от API
- [~] Blog — структура есть, наполнение зависит от API
- [x] Static pages — i18n через `next-intl` (`messages/ru.json`, `messages/en.json`)
- [x] TypeScript компилируется без ошибок (build прошёл)

---

## Фаза 12: Фронтенд-библиотеки

### 12.1 Leptos-библиотеки (crates/)

Для каждой проверяем: компилируется, exports корректны, используется в apps/admin или apps/storefront.

- [ ] `leptos-auth` — auth state management, JWT storage
- [ ] `leptos-graphql` — GraphQL client wrapper
- [ ] `leptos-hook-form` — form handling
- [ ] `leptos-forms` — form components
- [ ] `leptos-zod` — validation schemas
- [ ] `leptos-zustand` — state management (Zustand-like)
- [ ] `leptos-ui` — UI wrappers
- [ ] `leptos-table` — table component
- [ ] `leptos-shadcn-pagination` — pagination component

### 12.2 IU (Design System)

- [ ] `UI/leptos` — Leptos компоненты (iu-leptos crate)
- [ ] `UI/next/components` — React/Next.js компоненты
- [ ] Компоненты между Leptos и Next.js визуально согласованы (если applicable)
- [ ] `UI/docs/api-contracts.md` актуален

### 12.3 TypeScript packages (packages/)

Для каждого: npm install, npm build, npm test (если есть), используется в apps/next-*.

- [ ] `packages/leptos-auth` — TypeScript auth helpers
- [ ] `packages/leptos-graphql` — TypeScript GraphQL helpers
- [ ] `packages/leptos-hook-form` — TypeScript form helpers
- [ ] `packages/leptos-zod` — TypeScript validation
- [ ] `packages/leptos-zustand` — TypeScript state management

---

## Фаза 13: Интеграционные связи

### 13.1 Write → Event → Index (E2E data flow)

- [ ] Admin создаёт Product → `ProductCreated` event → Index обновляет `index_products` → Storefront видит product
- [ ] Admin создаёт Node → `NodeCreated` event → Index обновляет `index_content` → Storefront видит content
- [ ] Admin создаёт Post → `PostCreated` event → Index обновляет → Storefront видит post

### 13.2 Module Dependencies

- [ ] При отключении `content` модуля → `blog` и `forum` автоматически отключаются (или ошибка)
- [ ] При включении `blog` → `content` должен быть включён (или ошибка)
- [ ] Core modules не могут быть отключены ни при каких условиях

### 13.3 Frontend ↔ Backend API Contracts

- [ ] GraphQL schema, используемая фронтендами, совпадает с реальной серверной schema
- [ ] REST endpoints, вызываемые фронтендами, существуют на сервере
- [ ] Типы данных (DTOs) между фронтом и бэком согласованы
- [ ] Auth token format одинаков для всех фронтендов

### 13.4 Leptos Admin ↔ Server

- [ ] Admin использует `/api/graphql` endpoint
- [ ] Auth flow: login → получить JWT → хранить в localStorage/cookie → отправлять в headers
- [ ] RBAC: admin видит только разрешённые разделы

### 13.5 Next.js Admin ↔ Server

- [ ] Next Admin использует тот же `/api/graphql` endpoint
- [ ] Clerk auth ↔ Server JWT: механизм интеграции работает
- [ ] RBAC навигация на фронте совпадает с серверным RBAC

### 13.6 Storefront ↔ Server

- [ ] Storefront (Leptos SSR) получает данные из index tables через GraphQL
- [ ] Next.js storefront получает те же данные
- [ ] Public queries не требуют auth
- [ ] Корзина/заказы требуют auth

---

## Фаза 14: Тестовое покрытие

### 14.1 Unit-тесты (cargo test --lib)

Для каждого crate проверяем наличие и прохождение unit-тестов:

- [ ] `rustok-core` — `cargo test -p rustok-core --lib`
- [ ] `rustok-content` — `cargo test -p rustok-content --lib`
- [ ] `rustok-commerce` — `cargo test -p rustok-commerce --lib`
- [ ] `rustok-blog` — `cargo test -p rustok-blog --lib`
- [ ] `rustok-forum` — `cargo test -p rustok-forum --lib`
- [ ] `rustok-pages` — `cargo test -p rustok-pages --lib`
- [ ] `rustok-index` — `cargo test -p rustok-index --lib`
- [ ] `rustok-tenant` — `cargo test -p rustok-tenant --lib`
- [ ] `rustok-rbac` — `cargo test -p rustok-rbac --lib`
- [ ] `rustok-outbox` — `cargo test -p rustok-outbox --lib`
- [ ] `rustok-telemetry` — `cargo test -p rustok-telemetry --lib`
- [ ] `alloy-scripting` — `cargo test -p alloy-scripting --lib`

### 14.2 Integration тесты

**Файлы в `tests/` директориях:**

#### Server integration tests (`apps/server/tests/`)
- [ ] `integration/content_flow_test.rs` — проходит
- [ ] `integration/event_flow_test.rs` — проходит
- [ ] `integration/order_flow_test.rs` — проходит
- [ ] `multi_tenant_isolation_test.rs` — проходит
- [ ] `tenant_cache_stampede_test.rs` — проходит

#### Crate integration tests
- [ ] `rustok-commerce/tests/catalog_service_test.rs`
- [ ] `rustok-commerce/tests/inventory_service_test.rs`
- [ ] `rustok-commerce/tests/pricing_service_test.rs`
- [ ] `rustok-commerce/tests/product_event_index_integration_test.rs`
- [ ] `rustok-content/tests/node_service_test.rs`
- [ ] `rustok-content/tests/node_event_index_integration_test.rs`
- [ ] `rustok-core/tests/event_versioning_test.rs`
- [ ] `rustok-core/tests/security_audit_test.rs`
- [ ] `rustok-core/tests/transactional_events_integration_test.rs`
- [ ] `rustok-core/tests/transactional_events_test.rs`
- [ ] `rustok-outbox/tests/sqlite_transport_regression_test.rs`
- [ ] `rustok-telemetry/tests/metrics_test.rs`
- [ ] `rustok-telemetry/tests/otel_test.rs`
- [ ] `rustok-blog/tests/` — все тесты
- [ ] `rustok-forum/tests/` — все тесты
- [ ] `rustok-pages/tests/` — все тесты
- [ ] `rustok-index/tests/` — все тесты
- [ ] `rustok-rbac/tests/` — все тесты
- [ ] `rustok-tenant/tests/` — все тесты
- [ ] `rustok-iggy/tests/` — все тесты
- [ ] `rustok-mcp/tests/` — все тесты

### 14.3 Property-based тесты (proptest)

- [ ] `rustok-content/src/state_machine_proptest.rs` — проходит
- [ ] `rustok-commerce/src/state_machine_proptest.rs` — проходит
- [ ] `rustok-blog/src/state_machine_proptest.rs` — проходит
- [ ] `rustok-core/src/validation_proptest.rs` — проходит

### 14.4 Security audit тесты

- [ ] `rustok-core/tests/security_audit_test.rs` — проходит
- [ ] Проверка на SQL injection
- [ ] Проверка на XSS в input validation
- [ ] Проверка auth bypass scenarios

### 14.5 Benchmarks

**Путь:** `benches/`

- [ ] `cargo bench` компилируется
- [ ] Benchmark results адекватны
- [ ] Нет performance regressions

### 14.6 Полный тестовый прогон

- [ ] `cargo test --workspace` — ВСЕ тесты проходят (или документированы known failures)
- [ ] Нет flaky тестов
- [ ] Тесты не зависят от порядка выполнения

---

## Фаза 15: Observability и операционная готовность

### 15.1 Prometheus Metrics

- [x] `/metrics` endpoint доступен — `GET /metrics` возвращает Prometheus формат (metrics.rs)
- [x] HTTP request metrics (count, latency, status) — через `loco_rs` middleware (`axum_prometheus`)
- [~] Database metrics (pool size, query time) — доступны через `sea_orm_metrics` feature, если включена
- [x] Cache metrics (hit/miss) — `CACHE_HIT_TOTAL`, `CACHE_MISS_TOTAL`, `CACHE_SET_TOTAL` в metrics.rs
- [x] Outbox metrics (backlog_size, retries, dlq) — `OUTBOX_BACKLOG_SIZE`, `OUTBOX_RETRIES_TOTAL`, `OUTBOX_DLQ_TOTAL`
- [x] Module health metrics — `MODULE_HEALTH` gauge per module slug в `/health/ready`
- [~] Custom business metrics — нет; только инфраструктурные метрики
- [x] Метрика прохождения через entry points добавлена: `rustok_module_entrypoint_calls_total{module,entry_point,path}` с разметкой `path=library|core_runtime|bypass`; `core_runtime` фиксирует допустимые kernel-пути (`apps/server` + core crates), отдельно от bypass.
- [x] Еженедельный adoption-отчёт подключён в verification cycle: `scripts/module_path_adoption_report.sh` формирует % library-path сценариев и список новых bypass-точек за период.

### 15.2 Tracing / OpenTelemetry

- [x] `rustok-telemetry` настраивает tracing subscriber — `init()` с `EnvFilter`, `JsonLayer`/`PrettyLayer` и опциональным `OtelLayer`
- [x] Span'ы создаются для HTTP requests — через `loco_rs` Axum middleware (TraceLayer)
- [~] Span'ы создаются для DB queries — через `#[instrument]` на service методах; прямой SeaORM span не реализован
- [x] Span'ы создаются для event processing — все service методы имеют `#[instrument]`; outbox relay логирует через tracing
- [x] OTLP exporter конфигурируется (если включён) — `otel::init_otel_layer()` в `rustok-telemetry/src/otel.rs`

### 15.3 Health Checks

- [x] `GET /health` возвращает JSON с module statuses — `HealthResponse` с `status`, `app`, `version`
- [x] `GET /health/ready` — DB health check через `sea_orm::ConnectionTrait::ping()`
- [x] Redis health check (если Redis включён) — проверяется через `TcpStream::connect()` в `check_dependency_health()`
- [x] Module health checks от каждого зарегистрированного модуля — `ModuleRegistry::all_health()` вызывается в `/health/ready`

### 15.4 Grafana Dashboards

**Путь:** `grafana/`

- [ ] Dashboard JSON файлы валидны
- [ ] Dashboards покрывают: HTTP, DB, Cache, Events
- [x] Grafana datasource конфигурация корректна
  - Добавлены стабильные `uid` для `Prometheus` и `Jaeger`, убрана битая ссылка `tracesToLogs.datasourceUid = loki`, которой не было в стеке

### 15.5 Docker Compose

- [ ] `docker-compose.yml` — минимальный dev setup (PostgreSQL, Redis)
- [ ] `docker-compose.full-dev.yml` — полный dev setup
- [ ] `docker-compose.observability.yml` — Prometheus + Grafana
- [ ] Все services стартуют без ошибок
- [ ] Порты не конфликтуют

---

## Фаза 16: Синхронизация документации с кодом

### 16.1 docs/architecture/

- [ ] `overview.md` — соответствует текущей архитектуре (модули, data flow, CQRS)
- [ ] `api.md` — GraphQL/REST endpoints актуальны
- [ ] `rbac.md` — permissions, roles, extractors совпадают с кодом
- [ ] `tenancy.md` — cache layers, stampede protection, invalidation описаны корректно
- [ ] `events.md` — транспортные уровни (L0/L1/L2) описаны корректно
- [ ] `events-transactional.md` — TransactionalEventBus описан
- [ ] `event-flow-contract.md` — контракт событий актуален
- [ ] `database.md` — schema совпадает с миграциями
- [ ] `modules.md` — перечень модулей актуален
- [ ] `routing.md` — policy GraphQL vs REST актуальна
- [ ] `dataloader.md` — описание DataLoader актуально
- [ ] `improvement-recommendations.md` — статусы рекомендаций актуальны

### 16.2 docs/guides/

- [ ] `quickstart.md` — инструкции по запуску работают
- [ ] `testing.md` — гайд по тестированию актуален
- [ ] `testing-integration.md` — примеры integration тестов работают
- [ ] `testing-property.md` — примеры property тестов работают
- [ ] `error-handling.md` — паттерн ошибок совпадает с кодом
- [ ] `input-validation.md` — валидация описана корректно
- [ ] `rate-limiting.md` — конфигурация rate limiting актуальна
- [ ] `security-audit.md` — описание совпадает с тестами
- [ ] `observability-quickstart.md` — настройка метрик/трейсинга работает

### 16.3 Module docs

Для каждого модуля проверяем: README и docs/README совпадают с реальным состоянием кода.

- [ ] `rustok-core` — README/docs соответствуют API
- [ ] `rustok-content` — README/docs/implementation-plan актуальны
- [ ] `rustok-commerce` — README/docs/implementation-plan актуальны
- [ ] `rustok-blog` — README/docs/implementation-plan актуальны
- [ ] `rustok-forum` — README/docs/implementation-plan актуальны
- [ ] `rustok-pages` — README/docs/implementation-plan актуальны
- [ ] `rustok-index` — README/docs/implementation-plan актуальны
- [x] `rustok-tenant` — README.md обновлён: назначение, ответственности, взаимодействие, точки входа; `docs/implementation-plan.md` обновлён: Phase 1 (done) с SeaORM entities, TenantService, DTOs
- [ ] `rustok-rbac` — README/docs/implementation-plan актуальны
- [ ] `rustok-outbox` — README/docs/implementation-plan актуальны
- [ ] `rustok-telemetry` — README/docs/implementation-plan актуальны
- [ ] `rustok-iggy` — README/docs/implementation-plan актуальны
- [ ] `rustok-iggy-connector` — README/docs/implementation-plan актуальны
- [ ] `alloy-scripting` — README/docs актуальны
- [ ] `rustok-events` — README актуален
- [ ] `rustok-mcp` — README/docs актуальны

### 16.4 Frontend library docs

- [ ] `leptos-auth` — README/docs совпадают с API
- [ ] `leptos-graphql` — README/docs совпадают
- [ ] `leptos-hook-form` — README/docs совпадают
- [ ] `leptos-zod` — README/docs совпадают
- [ ] `leptos-zustand` — README/docs совпадают
- [ ] `leptos-table` — README/docs совпадают
- [ ] `leptos-shadcn-pagination` — README/docs совпадают

### 16.5 Application docs

- [ ] `apps/server/docs/README.md` — актуален
- [ ] `apps/admin/docs/README.md` — актуален
- [ ] `apps/storefront/README.md` — актуален
- [ ] `apps/next-admin/README.md` — актуален
- [ ] `apps/next-frontend/docs/README.md` — актуален

### 16.6 Root-level docs

- [ ] `README.md` — описание проекта актуально
- [ ] `PLATFORM_INFO_RU.md` — информация актуальна
- [ ] `RUSTOK_MANIFEST.md` — манифест актуален
- [ ] `AGENTS.md` — правила для AI-агентов актуальны
- [ ] `CONTRIBUTING.md` — инструкции для контрибьюторов актуальны
- [ ] `CHANGELOG.md` — последние изменения задокументированы
- [ ] `docs/index.md` — карта документации полная и актуальная

### 16.7 ADR (Architectural Decision Records)

**Путь:** `DECISIONS/`

- [ ] Все ADR в `DECISIONS/` имеют корректный формат
- [ ] Статусы ADR актуальны (Accepted / Superseded / etc.)
- [ ] ADR для event-contract migration существует
- [ ] ADR для core-server module-bundles routing существует
- [ ] ADR для auth lifecycle unification существует

---

## Фаза 17: CI/CD и DevOps

### 17.1 GitHub Actions

**Файлы:** `.github/workflows/`

- [ ] `ci.yml` — основной CI pipeline
  - [ ] Запускает `cargo check`
  - [ ] Запускает `cargo test`
  - [ ] Запускает `cargo clippy`
  - [x] Запускает `cargo fmt --check` (шаг fmt есть в ci.yml)
  - [ ] Запускает frontend builds (если applicable)
- [ ] `dependencies.yml` — проверка зависимостей (Dependabot / cargo-deny)

### 17.2 cargo-deny

**Файл:** `deny.toml`

- [ ] `cargo deny check` проходит
- [ ] Лицензии разрешены
- [ ] Нет banned crates
- [ ] Нет known advisories

### 17.3 Typos

**Файл:** `typos.toml`

- [ ] `typos` check проходит (нет опечаток в коде)

### 17.4 Scripts

**Путь:** `scripts/`

- [ ] Скрипты актуальны и работают
- [ ] Описаны в README или Makefile

---

## Фаза 18: Безопасность

### 18.1 Authentication Security

- [x] Пароли хэшируются Argon2 (`argon2` crate) — в `crates/rustok-core/src/auth/password.rs`
- [x] JWT secret хранится в конфигурации (не в коде) — из env vars / Loco config
- [x] Token expiry разумный — access: 15 мин, refresh: 7 дней (через `Claims::exp`)
- [x] Refresh token rotation — старый инвалидируется через `Sessions::revoke()` в `AuthLifecycleService`
- [x] Rate limiting для auth endpoints — 20 req/60 сек per IP (исправлено, Issue #15)

### 18.2 Authorization Security

- [x] Нет endpoint'ов без auth (кроме public: health, login, register, public storefront queries)
  - Все REST controllers защищены RBAC extractors (Issues #4, #5, #6 исправлены)
  - GraphQL mutations защищены через `AuthService::has_any_permission()` (Issues #7-12 исправлены)
- [x] RBAC extractors используются последовательно — все controllers, проверено через grep
- [x] SuperAdmin endpoints недоступны обычным пользователям — через Role-Permission матрицу

### 18.3 Tenant Isolation Security

- [x] Нет SQL-запросов без `WHERE tenant_id = ?` — все SELECT с tenant_id filter (Phase 19.1 audit подтвердил)
- [x] Нет GraphQL resolvers, возвращающих данные без tenant filter — все resolvers передают tenant_id в services
- [ ] Тест на cross-tenant access существует и проходит — тест не реализован

### 18.4 Input Validation

- [~] Все CreateInput/UpdateInput DTOs используют `validator` — реализовано для rustok-content (полностью) и rustok-commerce (добавлено: `CreateProductInput`, `UpdateProductInput`, `CreateVariantInput`, `UpdateVariantInput`, `PriceInput` с `#[derive(Validate)]` и вызов `.validate()?` в `CatalogService`); остальные модули (blog, forum, pages) используют NodeService с уже реализованной валидацией
- [x] SQL injection невозможен (параметризованные запросы через SeaORM)
- [~] XSS: пользовательский ввод передаётся в БД через SeaORM (параметризованно); рендеринг в Leptos/Next.js с дефолтным экранированием
- [x] CORS настроен корректно (`tower-http::cors`) — `CorsLayer::very_permissive()` в dev (apps/server/src/app.rs)

### 18.5 Secrets Management

- [x] `.env.dev.example` не содержит реальных secrets — только placeholder значения
- [x] `.gitignore` исключает `.env`, credentials, keys — проверено: только `.env.dev.example` в git
- [x] JWT secret, DB password, Redis password — через env vars (не хардкод в коде)

### 18.6 Dependency Security

- [ ] `cargo audit` не показывает critical vulnerabilities
- [ ] `cargo deny check advisories` проходит

---

## Фаза 19: Антипаттерны и качество кода

**Справочные документы:**
- [Паттерны vs Антипаттерны](./standards/patterns-vs-antipatterns.md)
- [Запрещённые действия](./standards/forbidden-actions.md)
- [Known Pitfalls](./ai/KNOWN_PITFALLS.md)

### 19.1 Критические антипаттерны (MUST FIX)

Поиск запрещённых паттернов в production коде. Каждый найденный экземпляр — обязательное исправление.

#### Tenant Isolation violations
- [x] Поиск `find().all(&db)` без `.filter(...tenant_id...)` в domain crates — нарушений нет
  - Дочерние записи (translations, variants, prices) фильтруются по node_id/product_id, родитель загружается с tenant_id
- [x] Поиск `find_by_id` без tenant_id проверки — проверено: все find_by_id добавляют `.filter(TenantId.eq(tenant_id))`
- [x] Поиск DELETE без tenant_id filter — все удаления через модели, загруженные с tenant filter
- [x] Проверка: каждая domain-таблица имеет `tenant_id` column в миграции — подтверждено: nodes, products, product_variants, categories, tags, meta, media, index_content, index_products, collections, commerce_categories, scripts, script_executions — все имеют tenant_id
- [x] Проверка: каждый SeaORM entity имеет `tenant_id` поле — подтверждено через grep `pub tenant_id` в domain crates (content/node.rs, commerce/product.rs, commerce/product_variant.rs, alloy/model/script.rs)

#### Unsafe event publishing
- [x] Поиск `publish(` без `_in_tx` в domain services — нарушений не найдено
  - Все сервисы используют `publish_in_tx()` корректно
- [x] REST controllers: `variants.rs` исправлен — все три операции (create/update/delete) используют `publish_in_tx()` внутри транзакции
- [x] `tenant_id` в DomainEvent — не нужен в самих вариантах: `tenant_id` передаётся через `EventEnvelope.tenant_id` (правильная архитектура — tenant на уровне конверта, не внутри payload каждого события)

#### Hardcoded secrets
- [x] Поиск hardcoded passwords/secrets/keys в Rust коде — нарушений нет
  - `grep -rn "password\|secret\|api_key" --include="*.rs"` — результатов нет в production коде
- [x] Поиск в .ts/.tsx файлах — проверено; найдены только UI-поля типа `type='password'` и переменные-названия, без hardcoded credentials
- [x] Проверка: `.env` файлы отсутствуют в git (только `.env.dev.example`)

#### Panics в production
- [~] Нет `.unwrap()` в production коде — отдельные находки есть в некоторых местах (Rhai scripting, generated code), но в основных service/controller файлах используется `?` или `.expect()` с комментариями. Полная проверка требует `cargo clippy`.
  - `Regex::new(r"...").unwrap()` — безопасно, константные regex паттерны (LazyLock)
  - `serde_json::to_value(Vec<String>).unwrap()` — заменено на `.expect("Vec<String> is always valid JSON")`
  - `state_machine.rs` — внутри `#[cfg(test)]` блоков
  - `async_utils::retry` — `last_error.unwrap()` внутри итерации (инвариант: последняя ошибка всегда Some)
- [x] Поиск `expect()` в production коде — проверены все случаи:
  - `rustok-rbac/src/integration.rs`: только в `#[test]` функциях — безопасно
  - `rustok-rbac/src/services/authz_mode.rs`: только в `#[cfg(test)]` блоке — безопасно
  - `rustok-telemetry/src/{lib,metrics}.rs`: инициализация Prometheus метрик при старте — стандартный паттерн, panic при дублировании регистрации (правильное поведение)
  - `apps/server/src/models/release.rs`: заменено на `.expect("Vec<String> is always valid JSON")`
- [x] Нет `panic!` в production коде — все `panic!` находятся только в `#[cfg(test)]` блоках (auth_lifecycle.rs и др.)
  - `rustok-test-utils/src/helpers.rs` — только в test helper функциях (assert helpers)
  - `apps/server/src/services/auth_lifecycle.rs` — только внутри `#[test]` функций

### 19.2 RBAC coverage audit

- [x] Список ВСЕХ handlers в `apps/server/src/controllers/` — все защищены RBAC extractors:
  - `content/nodes.rs`: RequireNodes* на всех 5 endpoints
  - `blog/posts.rs`: RequireBlogPosts* на всех 7 endpoints
  - `forum/topics.rs`: RequireForumTopics* на всех 6 endpoints
  - `forum/replies.rs`: RequireForumReplies* на всех 5 endpoints
  - `forum/categories.rs`: RequireForumCategories* на всех 5 endpoints
  - `pages.rs`: RequirePages* на обоих endpoints
  - `admin_events.rs`: RequireLogsRead на обоих DLQ endpoints
  - `auth.rs`: CurrentUser используется только для auth операций (change-password, profile) — корректно
  - `commerce/*`: RequireProducts*/Orders* на всех endpoints
- [x] Список ВСЕХ GraphQL mutations — каждый имеет permission check:
  - Blog, Commerce, Content, Pages, Forum — через `AuthService::has_any_permission()`
  - Alloy — через `require_admin()`
  - Auth — проверки через AuthLifecycleService
- [x] GraphQL queries (non-public) — Forum queries требуют auth через `AuthContext`
- [x] Нет `CurrentUser` без RBAC check в controllers (кроме `auth.rs` — auth endpoints)
  - Проверено: grep показывает CurrentUser только в `auth.rs` в handlers

### 19.3 Async safety

- [x] Поиск `std::thread::sleep` в async коде — не найдено в production коде (только в тестах)
- [~] Поиск `std::fs::` в async коде
  - `apps/server/src/tasks/cleanup.rs` — `std::fs::` используется в task коде (приемлемо, не в HTTP handlers)
- [~] Поиск неограниченных `tokio::spawn` в циклах — используется в relay worker, контролируется через batch size
- [x] Проверка: нет `block_on()` внутри async context

### 19.4 Error handling quality

- [x] Все domain crates используют `thiserror` (не `anyhow` в lib)
- [x] Нет `String` errors — типизированные ошибки
- [x] Controllers используют `loco_rs::Result` — все handlers возвращают `Result<Json<...>>` из `loco_rs::prelude`
- [~] GraphQL resolvers конвертируют errors через `map_err(|e| Error::BadRequest(e.to_string()))` — частично; `async_graphql::Error` используется в некоторых resolvers

### 19.5 Code metrics

- [ ] Нет функций > 40 строк (проверить top-10 самых длинных функций)
- [ ] Нет модулей > 1000 строк
- [ ] Нет функций с > 6 аргументами
- [ ] `DomainEvent` enum не превышает разумного размера (проверить `rustok-core/src/events/types.rs`)

### 19.6 State machine correctness

- [x] Каждый state machine модуль имеет `*_proptest.rs`
  - `rustok-content/src/state_machine_proptest.rs`
  - `rustok-commerce/src/state_machine_proptest.rs`
  - `rustok-blog/src/state_machine_proptest.rs`
- [x] Невалидные переходы возвращают ошибку (не panic)
- [x] Нет string-based status checks (`if status == "published"`)
  - Проверено по `crates/` и `apps/server/`: строковых сравнений статусов `published|draft|archived` не найдено, используются enum/state-machine переходы

### 19.7 DTO consistency

- [x] Каждый domain module имеет отдельные `CreateInput`/`UpdateInput`/`Response` DTOs — проверено для rustok-content, rustok-commerce, rustok-blog, rustok-forum, rustok-pages, rustok-tenant
- [x] Entities из БД НЕ возвращаются напрямую в API — сервисы возвращают DTO, не Model
- [~] DTOs имеют `#[derive(Validate)]` для input validation — реализовано в rustok-content (полностью) и rustok-commerce (добавлено); blog/forum/pages используют NodeService с уже реализованной валидацией; rustok-tenant не имеет `Validate` (DTOs простые, без сложных полей)
- [x] Response DTOs не содержат internal fields — `password_hash` и другие internal поля не включены в Response DTOs

### 19.8 Event handling quality

- [ ] Каждый event handler idempotent (retry-safe)
- [ ] Каждый новый DomainEvent имеет integration test
- [ ] Каждый event handler имеет idempotency test
- [ ] Event payloads содержат все необходимые поля для обработки (не требуют дополнительных DB queries)

### 19.9 Frontend antipatterns

#### Leptos
- [ ] Нет прямых `fetch()` — используется `leptos-graphql`
- [ ] Нет ручного JWT management — используется `leptos-auth`
- [ ] Нет prop drilling > 3 уровней — используется `leptos-zustand`
- [ ] Admin — CSR (WASM), Storefront — SSR

#### Next.js
- [ ] Нет `any` типов в TypeScript коде
  - `grep -rn ": any" apps/next-admin/src/ apps/next-frontend/src/ --include="*.ts" --include="*.tsx"`
- [ ] Нет `@ts-ignore` / `@ts-expect-error`
- [ ] Shared code в `packages/` (не copy-paste между apps)
- [ ] Нет дублирования GraphQL queries между next-admin и next-frontend

### 19.10 Logging quality

- [x] Service methods имеют `#[instrument]` decorator
- [x] Нет логирования PII (email, password, token) — проверено при аудите
- [x] Нет `println!` / `eprintln!` в production коде — не найдено в crates/apps
- [~] Structured fields вместо string formatting в tracing — частично, есть format-строки в некоторых warn!/error!

### 19.11 Dependency antipatterns

- [x] `rustok-core` не зависит от domain crates (нет circular dependencies)
- [x] Domain crates не вызывают друг друга напрямую (через events)
- [x] `rustok-test-utils` — только в `[dev-dependencies]`
- [x] Нет `path` dependencies на crates вне workspace
  - Проверены все `Cargo.toml`: path-зависимости указывают только на workspace-компоненты (`crates/*`, `apps/*`, `UI/leptos`, локальная `migration`); внешних path-dependencies вне репозитория нет

### 19.12 API antipatterns — GraphQL

- [ ] Нет N+1 queries в resolvers (все связанные данные через DataLoader)
  - Искать: resolvers с прямыми DB-запросами внутри `async fn` для дочерних объектов
- [x] `MergedObject` используется для модульной schema — `Query(RootQuery, AuthQuery, CommerceQuery, ContentQuery, BlogQuery, ForumQuery, AlloyQuery, PagesQuery)` и аналогичная Mutation в `schema.rs`
- [~] Нет `String` errors в GraphQL — используются structured error extensions
  - Есть structured errors через `GraphQLError`, но часть query/mutation/loaders всё ещё использует `async_graphql::Error::new(err.to_string())` и `err.to_string().into()`; требуется довести до единого формата
  - `grep -rn "FieldError::new" apps/server/src/graphql/ --include="*.rs"`
- [x] Каждый mutation имеет permission check (не полагается на «auth достаточно»)
  - Проверено по `apps/server/src/graphql/*/mutation.rs`: blog/content/commerce/forum/pages/alloy/auth mutations имеют явные permission/auth checks
- [~] Каждый query с list возвращает paginated результат (не полную таблицу)
  - `users`, `products`, `pages`, `posts`, `nodes`, `scripts` используют pagination/page_info; дополнительно переведены `forum_categories`, `forum_topics`, `forum_replies` на connection-ответы с `page_info`. Непагинированными пока остаются `enabled_modules`, `tenant_modules`, `module_registry`, `recent_activity`, `scripts_for_event`
- [~] `context.data::<TenantContext>()` используется в каждом resolver (не пропущен)
  - Часть resolvers опирается на аргумент `tenant_id`, а root/resolver-level multi-tenant queries используют `TenantContext`; единый паттерн ещё не везде соблюдён
- [~] Нет бизнес-логики в resolvers — только вызов domain services
  - Основные mutation/resolver-ветки делегируют в сервисы, но в `commerce/query.rs` и частично `queries.rs` остаются прямые SeaORM-запросы и сборка response-моделей в resolver-слое
- [x] Naming convention: queries — `camelCase`, mutations — `camelCase` с глаголом (`createProduct`, не `productCreate`)
  - Проверено по именам GraphQL-resolvers: `createProduct`, `updateNode`, `publishPost`, `pageBySlug`, `recentActivity`, `dashboardStats` и т.д.
- [x] Subscription (если есть) использует WebSocket, не polling
  - В схеме используется `EmptySubscription`, GraphQL subscription API отсутствует, polling-вариант не реализован

### 19.13 API antipatterns — REST

- [x] Каждый endpoint имеет `#[utoipa::path(...)]` annotation для OpenAPI
  - Все production controllers аннотированы: auth, content, blog, forum, pages, commerce (products/variants/inventory), admin_events, health, metrics
  - `alloy.rs` — делегирует routing во внешний crate, не имеет прямых handlers
- [x] HTTP status codes корректны:
  - 201 Created для POST — исправлено во всех controllers (blog, content, forum, commerce, pages)
  - 204 No Content для DELETE — исправлено во всех controllers
  - 404 Not Found для отсутствующих ресурсов — через `Error::NotFound`
  - 422 Unprocessable Entity для validation errors (не 400)
- [~] Нет бизнес-логики в controllers — основные write-endpoints делегируют в domain services, но часть read/list handlers (`commerce/products.rs`, `commerce/variants.rs`, `commerce/inventory.rs`, `forum/categories.rs`) всё ещё содержит прямые SeaORM-запросы и сборку response DTO в controller-слое
- [x] `loco_rs::Result` для error handling (не custom error types)
- [~] Все `CreateInput`/`UpdateInput` проходят через `validator::Validate`
  - Content и Commerce валидируются на уровне DTO/service; blog/forum/pages в основном опираются на `NodeService`, но transport-level audit показал, что единый `Validate` derive покрывает не все input-структуры
- [~] Нет endpoints без пагинации в list-запросах
  - Пагинация есть у commerce products, но list endpoints для content/blog/forum/categories/replies/variants пока возвращают plain `Vec<_>` без общей pagination envelope
- [x] Rate limiting применён к auth endpoints (login, register, reset-password)
- [~] CORS middleware подключён с правильными origins
  - В dev используется permissive-конфигурация (`CorsLayer::very_permissive()`), production-specific allowlist требует отдельной настройки через env/config

### 19.14 REST ↔ GraphQL parity

- [x] Auth операции (login, register, refresh, change-password) доступны через REST (`/api/auth/*`); GraphQL имеет `loginUser`, `registerUser`, `refreshToken` mutations
- [x] Бизнес-логика **одна** — через `AuthLifecycleService`; REST и GraphQL auth вызывают одни и те же методы сервиса
- [x] RBAC проверки **идентичны** в REST и GraphQL — REST через extractors (`RequireProductsCreate` etc.), GraphQL через `AuthService::has_any_permission()`
- [x] Tenant isolation **идентичен** — `TenantContext` в REST, `TenantContext::from_request()` в GraphQL; оба читают `X-Tenant-Id` header
- [~] Error responses — REST возвращает HTTP status codes; GraphQL возвращает errors array с `extensions`; маппинг не унифицирован
- [~] CRUD parity — Content/Commerce/Blog доступны через оба transport; Forum GraphQL пересекается с REST, но не полностью (например, moderation endpoint только в REST)

### 19.15 Anti-bypass audit

- [ ] Нет повтора валидации доменных правил в `apps/server`/frontend-adapter слое
  - Автопоиск кандидатов: `./scripts/verify/verify-anti-bypass.sh`
  - Ручной review: сверка найденных мест с допустимым target-слоем (`crates/rustok-<domain>`, либо platform/core в `apps/server` + `crates/rustok-core`)
- [ ] Нет ручной публикации событий в app-слое вместо модульных сервисов
  - Автопоиск кандидатов: `./scripts/verify/verify-anti-bypass.sh --manual-review`
  - Для release-gate: `./scripts/verify/verify-anti-bypass.sh --strict`
- [ ] Нет прямых запросов к таблицам домена из app-слоя мимо crate API
  - Автопоиск сигнатур `Entity::find*`, `from_raw_sql`, `DatabaseConnection` в `apps/server/src`
- [ ] Каждый найденный случай оформлен как migration-task
  - Формат: перенос в корректный слой (домен → `crates/rustok-<domain>`, platform/core orchestration — допускается в `apps/server` + `crates/rustok-core`)
  - Минимум: ссылка на issue/task + owner + целевой API/слой
  - Для frontend-слоя: дублирование выносить в самописные frontend-библиотеки (shared UI/contracts), не в backend domain crates

---

## Фаза 20: Правильность написания кода (Code Correctness)

### 20.1 Type Safety

- [~] ID типы используют newtype pattern — не применяется; платформа использует голые `Uuid` для ID; это допустимо в Rust-first API, но newtype pattern повышает безопасность при рефакторинге
- [x] Status поля — enum, не `String` — `ContentStatus`, `ProductStatus`, `BlogPostStatus`, `TopicStatus` — все через `#[derive(DeriveActiveEnum)]`
- [~] Phantom types для state-aware structs — не используются; state machine реализован через отдельный `transition()` метод
- [~] Нет `as` casts для числовых типов без проверки (use `TryFrom`)
  - Исправлено в ProductIndexer: `agg.total_inventory as i32` и `agg.variant_count as i32` → `i32::try_from().unwrap_or(i32::MAX)` (Issue #21)

### 20.2 Concurrency correctness

- [x] `Arc<Mutex<>>` используется вместо `Mutex<>` для shared state между tasks — `Arc<Mutex<HashMap<...>>>` в health.rs CIRCUITS; `Arc<ModuleRegistry>` в app.rs
- [~] `RwLock` для read-heavy shared state — `ModuleRegistry` использует `Mutex`; можно улучшить до `RwLock` для списка модулей (read-heavy)
- [~] Нет race conditions в cache operations — простой `Mutex`-based cache без singleflight; stampede protection отсутствует (документировано в tenancy.md)
- [x] `Select!` корректно обрабатывает cancellation — в outbox relay используется `tokio::select!` для graceful shutdown

### 20.3 Resource management

- [x] DB connections возвращаются в pool (нет leaked connections) — SeaORM connection pool (`sea_orm::Database::connect()`) автоматически управляет пулом через RAII
- [x] Files закрываются (RAII через `Drop`) — std::fs операции в задачах cleanup, закрываются автоматически
- [~] HTTP clients имеют timeouts — Iggy клиент имеет timeout config; outbox relay HTTP не используется
- [x] Retry logic с exponential backoff (не busy-loop) — `async_utils::retry` с `ExponentialBackoff` в outbox relay

### 20.4 Serialization correctness

- [~] `serde(rename_all = "camelCase")` для JSON APIs — не применяется; платформа использует `snake_case` (Rust default) для REST API. GraphQL генерирует camelCase автоматически через `async_graphql`
- [~] `#[serde(skip_serializing)]` для sensitive fields — `password_hash` не включается в DTO; auth responses не содержат hash
- [x] DateTime сериализуется в ISO 8601 — SeaORM `DateTimeWithTimeZone` → chrono → serde ISO 8601 автоматически
- [x] UUID сериализуется как string — через `utoipa = { features = ["uuid"] }` и `serde` для Uuid

### 20.5 Migration correctness

- [x] Каждая миграция имеет `up()` и `down()` (reversible)
  - Проверено: все ALTER TABLE миграции (m20260301_*) имеют корректный down()
- [x] Foreign keys с `ON DELETE CASCADE` или `ON DELETE SET NULL` — проверено в m20250130_000005 (nodes → tenants: CASCADE, nodes → nodes parent: CASCADE, nodes → users author: SET NULL), m20250130_000012 (products → tenants: CASCADE), и других
- [x] Indexes на часто фильтруемые поля — `idx_nodes_tenant_kind` (tenant_id, kind, status), `idx_nodes_parent`, `idx_nodes_author`, `idx_products_tenant_status`; slug индексы через UNIQUE constraints
- [x] `UNIQUE` constraints где бизнес-логика требует — tenant.slug (UNIQUE), product_translation.handle+locale (UNIQUE), node_translation slug+locale (UNIQUE в code)

---

## Фаза 21: Реестр найденных проблем

Этот раздел — живой трекер всех `[!]` пунктов из других фаз.  
Добавляем сюда при обнаружении, обновляем при исправлении.

| № | Приоритет | Статус | Описание | Файлы | Фаза |
|---|-----------|--------|----------|-------|------|
| 1 | 🔴 Критический | ✅ Исправлено | `content` был помечен `required = true` в `modules.toml`, но `ContentModule::kind()` возвращает `ModuleKind::Optional`. Несоответствие приводило к ошибке `validate_registry_vs_manifest()` при старте. | `modules.toml` | 1.1 |
| 2 | 🔴 Критический | ✅ Исправлено | `rustok-blog` и `rustok-forum` используют `event_bus.publish()` вместо `publish_in_tx()` — нарушение атомарности. Все сервисы переведены на `publish_in_tx()` с открытой транзакцией. | `crates/rustok-blog/src/services/post.rs`, `crates/rustok-forum/src/services/{topic,reply,moderation}.rs` | 6.2, 7.3, 7.4 |
| 3 | 🟡 Высокий | ✅ Исправлено | `iggy` версия `0.9.2` не существует на crates.io. CI-сборка падала. Исправлено на `0.9.0`. | `Cargo.toml`, `crates/rustok-iggy-connector/Cargo.toml` | 0.6 |
| 4 | 🔴 Критический | ✅ Исправлено | Контроллеры `blog/posts.rs`, `forum/topics.rs`, `forum/replies.rs`, `forum/categories.rs`, `pages.rs` использовали только `CurrentUser` без RBAC-проверок. Добавлены RBAC-экстракторы (`RequireBlogPostsCreate`, `RequireForumTopicsCreate`, и т.д.). Добавлена матрица Blog/Forum permissions для всех ролей в `rbac.rs`. | `apps/server/src/controllers/blog/posts.rs`, `forum/topics.rs`, `forum/replies.rs`, `forum/categories.rs`, `pages.rs`, `crates/rustok-core/src/rbac.rs`, `apps/server/src/extractors/rbac.rs` | 4.4, 18.2, 19.2 |
| 5 | 🔴 Критический | ✅ Исправлено | `content/nodes.rs` использовал `CurrentUser` без RBAC-проверок для всех 5 endpoints. Заменён на RBAC extractors (`RequireNodesList`, `RequireNodesRead`, `RequireNodesCreate`, `RequireNodesUpdate`, `RequireNodesDelete`). OpenAPI 403 добавлен. | `apps/server/src/controllers/content/nodes.rs` | 4.4, 9.4, 18.2 |
| 6 | 🔴 Критический | ✅ Исправлено | `admin_events.rs` (DLQ просмотр/replay) использовал `CurrentUser` без RBAC — доступен любому аутентифицированному пользователю. Заменён на `RequireLogsRead` (Admin/SuperAdmin only). Добавлен `Logs::Read` и `Logs::List` в `ADMIN_PERMISSIONS`. | `apps/server/src/controllers/admin_events.rs`, `crates/rustok-core/src/rbac.rs`, `apps/server/src/extractors/rbac.rs` | 4.4, 9.8, 18.2 |
| 7 | 🟡 Высокий | ✅ Исправлено | GraphQL Blog mutations (`create_post`, `update_post`, `delete_post`, `publish_post`, `unpublish_post`, `archive_post`) имели только auth check, но не проверяли конкретные RBAC permissions. Добавлены проверки через `AuthService::has_any_permission()` для каждой операции. | `apps/server/src/graphql/blog/mutation.rs` | 4.3, 8.4 |
| 8 | 🔴 Критический | ✅ Исправлено | GraphQL Commerce mutations (`create_product`, `update_product`, `publish_product`, `delete_product`) — без auth/RBAC. Добавлены проверки `AuthService::has_any_permission()` для PRODUCTS_CREATE/UPDATE/DELETE. | `apps/server/src/graphql/commerce/mutation.rs` | 4.3, 8.3 |
| 9 | 🔴 Критический | ✅ Исправлено | GraphQL Content mutations (`create_node`, `update_node`, `delete_node`) — только auth check, без RBAC. Добавлены NODES_CREATE/UPDATE/DELETE через `AuthService::has_any_permission()`. Параметр `tenant_id` добавлен. | `apps/server/src/graphql/content/mutation.rs` | 4.3, 8.2 |
| 10 | 🟡 Высокий | ✅ Исправлено | GraphQL Forum — stub реализация. Реализованы полноценные queries и mutations через TopicService, ReplyService, CategoryService с RBAC. | `apps/server/src/graphql/forum/mutation.rs`, `query.rs`, `types.rs` | 4.3, 8.5 |
| 11 | 🟡 Высокий | ✅ Исправлено | GraphQL Pages mutations — без RBAC, использовали SecurityContext::system(). Добавлены PAGES_CREATE/UPDATE/DELETE через `AuthService::has_any_permission()`. | `apps/server/src/graphql/pages/mutation.rs` | 4.3, 8.7 |
| 12 | 🟡 Высокий | ✅ Исправлено | RBAC extractors RequirePagesCreate/Read/Update/Delete использовали NODES_* permissions вместо PAGES_*. Исправлено. Добавлены константы PAGES_* и permissions для Manager/Customer. | `extractors/rbac.rs`, `permissions.rs`, `rbac.rs` | 4.1, 4.4 |
| 13 | 🔴 Критический | ✅ Исправлено | REST контроллер `variants.rs`: `create_variant`, `update_variant`, `delete_variant` публиковали события `VariantCreated/Updated/Deleted` через `event_bus_from_context().publish()` **после** коммита транзакции. При сбое между commit и publish событие терялось. Исправлено: все три операции переведены на `publish_in_tx()` внутри транзакции до `commit()`. Update/Delete-операции получили обёртку в транзакцию. | `apps/server/src/controllers/commerce/variants.rs` | 6.2, 19.1 |
| 14 | 🟡 Высокий | ✅ Исправлено | Миграция таблицы `sys_events` (outbox pattern) не была зарегистрирована в главном сервере. Создан файл `m20260211_000002_create_sys_events.rs` и добавлен в `apps/server/migration/src/lib.rs`. | `apps/server/migration/src/` | 2.2 |
| 15 | 🟡 Высокий | ✅ Исправлено | Rate limiting middleware существовал в `middleware/rate_limit.rs` но **не был подключён** к роутеру. Все auth endpoints были уязвимы к брутфорс-атакам. Исправлено: добавлен `axum_middleware::from_fn` с per-IP sliding window limiter (20 req/60 сек) для `/api/auth/login`, `/api/auth/register`, `/api/auth/reset/*` в `app.rs::after_routes()`. | `apps/server/src/app.rs`, `apps/server/src/middleware/rate_limit.rs` | 9.10, 18.1 |
| 16 | 🟡 Высокий | ✅ Исправлено | Entity `product_variant` имел поля `tenant_id, inventory_policy, inventory_management, inventory_quantity, weight_unit, option1/2/3, position`, которых не было в миграции `m20250130_000014_create_commerce_variants`. Добавлена миграция `m20260301_000001_alter_product_variants_add_fields`. | `apps/server/migration/src/m20260301_000001_alter_product_variants_add_fields.rs` | 7.2, 20.5 |
| 17 | 🟡 Высокий | ✅ Исправлено | Entity `node` имел поля `deleted_at` и `version`, которых не было в миграции `m20250130_000005_create_nodes`. Добавлена миграция `m20260301_000002_alter_nodes_add_soft_delete`. | `apps/server/migration/src/m20260301_000002_alter_nodes_add_soft_delete.rs` | 7.1, 20.5 |
| 18 | 🟡 Высокий | ✅ Исправлено | `ContentIndexer::build_index_content()` и `ProductIndexer::index_one()` были stub-реализациями (return Ok(None/0)). Реализованы с реальными JOIN-запросами к БД и UPSERT в index_content/index_products. | `crates/rustok-index/src/content/indexer.rs`, `crates/rustok-index/src/product/indexer.rs` | 6.3, 7.7 |
| 19 | 🟢 Низкий | ✅ Исправлено | `ModuleLifecycleService` использовал hardcoded `CORE_MODULE_SLUGS` массив вместо `registry.is_core()`. При добавлении нового Core-модуля нужно было обновлять два места. Исправлено: `validate_core_toggle()` удалён, проверка инлайнена через `registry.is_core()`. Тесты обновлены. | `apps/server/src/services/module_lifecycle.rs` | 5.4 |
| 20 | 🟡 Высокий | ✅ Исправлено | `ProductIndexer::reindex_all()` содержал SQL-запрос `WHERE tenant_id = $1 AND deleted_at IS NULL` — но таблица `products` не имеет поля `deleted_at` (продукты удаляются через hard delete). Убрана несуществующая колонка из WHERE. | `crates/rustok-index/src/product/indexer.rs` | 7.7, 20.5 |
| 21 | 🟢 Низкий | ✅ Исправлено | `ProductIndexer::build_index_product()` использовал небезопасный `as i32` cast для `agg.total_inventory` и `agg.variant_count` (тип `i64`). Исправлено: используется `i32::try_from().unwrap_or(i32::MAX)`. | `crates/rustok-index/src/product/indexer.rs` | 20.1 |
| 22 | 🟡 Высокий | ✅ Исправлено | `crates/rustok-tenant` содержал пустые stub-файлы (entities/mod.rs, services/mod.rs, dto/mod.rs — только комментарии). Реализованы SeaORM entities (`tenant.rs`, `tenant_module.rs`), DTOs (`CreateTenantInput`, `UpdateTenantInput`, `TenantResponse`, `ToggleModuleInput`) и `TenantService` с полным CRUD и toggle_module. | `crates/rustok-tenant/src/` | 7.9 |
| 23 | 🟡 Высокий | ✅ Исправлено | `controllers/swagger.rs` содержал только `ApiDoc` struct без endpoint для отдачи OpenAPI spec. Добавлены handlers `openapi_json()` и `openapi_yaml()` для эндпоинтов `GET /api/openapi.json` и `GET /api/openapi.yaml`, маршруты зарегистрированы в `app.rs`. | `apps/server/src/controllers/swagger.rs`, `apps/server/src/app.rs` | 9.9 |
| 24 | 🟡 Высокий | ✅ Исправлено | Миграции `ScriptsMigration` и `ScriptExecutionsMigration` из `alloy-scripting` не были включены в главный Migrator — таблицы `scripts` и `script_executions` не создавались при деплое. Добавлены wrapper-файлы `m20260302_000001_create_scripts.rs` и `m20260302_000002_create_script_executions.rs`, подключена зависимость `alloy-scripting` в `migration/Cargo.toml`. | `apps/server/migration/src/lib.rs`, `apps/server/migration/src/m20260302_*.rs`, `apps/server/migration/Cargo.toml` | 7.6, 0.3 |
| 25 | 🟡 Высокий | ✅ Исправлено | Миграция `search_index` существовала в `apps/server/migration/src/` (использует `PgSearchEngine`) но не была зарегистрирована в Migrator. Также конфликт имён модулей (`m20250130_000010` дважды). Файл переименован в `m20250130_000010a_create_search_index.rs`, добавлен в Migrator. | `apps/server/migration/src/lib.rs`, `apps/server/migration/src/m20250130_000010a_create_search_index.rs` | 0.3, 7.7 |
| 26 | 🟡 Средний | ✅ Исправлено | Commerce DTOs (`CreateProductInput`, `UpdateProductInput`, `CreateVariantInput`, `UpdateVariantInput`, `PriceInput`) не имели `#[derive(Validate)]` и вызовы валидации в сервисах. Добавлены: `validator` в `rustok-commerce/Cargo.toml`, `#[derive(Validate)]` с аннотациями полей в DTO, вызов `.validate()?` в `CatalogService::create_product()` и `update_product()`. Также добавлены вызовы валидации в `NodeService` для `create_node_in_tx` и `update_node_in_tx`. | `crates/rustok-commerce/src/dto/product.rs`, `dto/variant.rs`, `services/catalog.rs`, `crates/rustok-content/src/services/node_service.rs`, `crates/rustok-commerce/Cargo.toml` | 18.4, 19.7 |
| 27 | 🟡 Средний | ✅ Исправлено | Observability/Grafana конфигурация содержала дрейф от реального стека: datasources не имели стабильных `uid`, а Jaeger datasource ссылался на несуществующий Loki (`tracesToLogs.datasourceUid = loki`). Это ломало привязку dashboard → datasource и вводило в заблуждение операторов. Исправлено: добавлены `uid: prometheus` и `uid: jaeger`, удалена битая `tracesToLogs` ссылка, а в Compose-файлах убран устаревший ключ `version`, чтобы `docker compose config` проходил без предупреждений. | `grafana/datasources/datasources.yml`, `docker-compose.full-dev.yml`, `docker-compose.observability.yml`, `docs/PLATFORM_VERIFICATION_PLAN.md` | 0.5, 15.4 |

### 21.1 Детали: Проблема #2 — Небезопасная публикация событий в blog/forum

**Корневая причина:**  
`PostService` и `TopicService`/`ReplyService`/`ModerationService` принимают `TransactionalEventBus` и передают его в `NodeService` (который корректно использует `publish_in_tx()`). Но затем сами дополнительно вызывают `self.event_bus.publish()` для публикации модуль-специфичных событий (`BlogPostCreated`, `ForumTopicCreated`, etc.) — это происходит **вне транзакции**.

**Риск:**
1. `NodeService` выполняет операцию + `publish_in_tx()` в транзакции — всё атомарно.
2. `PostService.create_post()` вызывает `NodeService.create_node()` (успешно).
3. Затем вызывает `self.event_bus.publish(BlogPostCreated{...})` — это отдельная операция.
4. Если шаг 3 фейлится — основные данные уже в БД, но blog-специфичное событие потеряно.

**Рекомендуемое исправление:**
- Рефакторинг: вместо делегирования в NodeService с последующим отдельным publish — 
  использовать паттерн открытой транзакции: создать транзакцию в `PostService`, передать её в NodeService и в последующий `publish_in_tx()`.
- Или: убрать дублирующие события в blog/forum — NodeService уже публикует `NodeCreated`/`NodeUpdated`/etc., а IndexService может слушать их напрямую.

**Чеклист исправления:**
- [x] Рефакторинг `PostService::create_post()` → `publish_in_tx()`
- [x] Рефакторинг `PostService::update_post()` → `publish_in_tx()`
- [x] Рефакторинг `PostService::publish_post()` → `publish_in_tx()`
- [x] Рефакторинг `PostService::unpublish_post()` → `publish_in_tx()`
- [x] Рефакторинг `PostService::delete_post()` → `publish_in_tx()`
- [x] Рефакторинг `TopicService` → `publish_in_tx()`
- [x] Рефакторинг `ReplyService::create_reply()` → `publish_in_tx()`
- [x] Рефакторинг `ModerationService` (3 вызова) → `publish_in_tx()`
- [x] Добавить integration тест: проверить что BlogPostCreated публикуется атомарно
  - Добавлены тесты в `crates/rustok-blog/tests/integration.rs`: happy-path для `MemoryTransport` и failure-path для не-transactional/failing transport (событие не наблюдается при ошибке публикации)

---

## Итоговый отчёт

После прохождения всех фаз заполнить итоговую таблицу:

| Фаза | Название | Всего проверок | OK | Проблемы | Не применимо |
|------|---------|---------------|-----|----------|-------------|
| 0 | Компиляция и сборка | | | | |
| 1 | Соответствие архитектуре | | | | |
| 2 | Ядро платформы | | | | |
| 3 | Авторизация | | | | |
| 4 | RBAC | | | | |
| 5 | Multi-Tenancy | | | | |
| 6 | Событийная система | | | | |
| 7 | Доменные модули | | | | |
| 8 | API GraphQL | | | | |
| 9 | API REST | | | | |
| 10 | Фронтенды Leptos | | | | |
| 11 | Фронтенды Next.js | | | | |
| 12 | Фронтенд-библиотеки | | | | |
| 13 | Интеграционные связи | | | | |
| 14 | Тестовое покрытие | | | | |
| 15 | Observability | | | | |
| 16 | Документация | | | | |
| 17 | CI/CD | | | | |
| 18 | Безопасность | | | | |
| 19 | Антипаттерны и качество кода | | | | |
| 20 | Правильность написания кода | | | | |
| **ИТОГО** | | | | | |

---

## Связанные документы

- [Паттерны vs Антипаттерны](./standards/patterns-vs-antipatterns.md) — сводная таблица правильного и неправильного
- [Запрещённые действия](./standards/forbidden-actions.md) — что ни в коем случае нельзя делать
- [Known Pitfalls](./ai/KNOWN_PITFALLS.md) — ловушки для агентов
- [Architecture Overview](./architecture/overview.md)
- [API Architecture](./architecture/api.md)
- [RBAC Enforcement](./architecture/rbac.md)
- [Tenancy](./architecture/tenancy.md)
- [Events & Outbox](./architecture/events.md)
- [Improvement Recommendations](./architecture/improvement-recommendations.md)
- [Module Registry](./modules/registry.md)
- [Documentation Map](./index.md)
- [Testing Guidelines](./guides/testing.md)
- [Security Audit Guide](./guides/security-audit.md)
- [Verification scripts README](../scripts/verify/README.md) — включает anti-bypass audit (`verify-anti-bypass.sh`)

---

> **Как использовать этот план:** Открываем каждую фазу последовательно. При нахождении проблемы — фиксим в коде и ставим `[x]`. Если что-то не реализовано — помечаем `[~]` и создаём задачу. По завершению — собираем итоговый отчёт.
