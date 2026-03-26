# RusToK Server — Loco.rs Feature Support & Anti-Duplication Matrix

**Date:** 2026-02-18  
**Loco.rs Version:** `0.16` (workspace dependency)  
**Purpose:** сохранить полный обзор реализованного server-функционала (включая auth и доменные API), при этом явно зафиксировать границы: где используем Loco, где сознательно используем самопис.

---

## 1) Полная матрица: Loco capability vs реализация RusToK

| Capability area | Loco support | Реализовано сейчас | Source of truth (целевое) | Риск дублей | Решение |
|---|---|---|---|---|---|
| Application hooks (`Hooks`) | ✅ | `boot`, `routes`, `after_routes`, `truncate`, `register_tasks`, `initializers`, `connect_workers`, `seed` | **Loco hooks** | Низкий | Оставить на Loco |
| Конфигурация приложения | ✅ | `development.yaml`/`test.yaml`, `auth.jwt`, custom `settings.rustok.*` | **Loco config + typed project settings** | Низкий | Оставить как есть |
| REST/GraphQL роутинг | ✅ | `AppRoutes` + Axum layers, GraphQL endpoint | **Loco + project controllers** | Низкий | Оставить как есть |
| ORM/migrations/entities | ✅ (SeaORM stack) | migration crate + entities + модели | **Loco/SeaORM stack** | Низкий | Оставить как есть |
| Auth framework primitives | ✅ (patterns/hooks) | JWT, refresh sessions, password reset tokens, RBAC domain wiring | **Project domain logic atop Loco runtime** | Средний | Не дублировать infra-слой Loco, но доменную auth-логику оставить своей |
| Tasks (`cargo loco task`) | ✅ | `CleanupTask` зарегистрирован | **Loco Tasks** | Низкий | Оставить на Loco |
| Initializers | ✅ | `TelemetryInitializer` через Loco API | **Loco Initializers** | Низкий | Оставить на Loco |
| Mailer subsystem | ✅ | Provider-based server email service (`smtp|loco|none`) + templated auth mail | **Server email service + Loco Mailer adapter** | Низкий | Оставить provider switch в server infra, без отдельного platform module |
| Workers/queue subsystem | ✅ | Сейчас собственный event-driven outbox relay worker | **RusToK custom (осознанно)** | Средний | Очереди/воркеры оставить самописными (не дублировать Loco queue runtime) |
| Storage abstraction (uploads/assets) | ✅ | Shared storage contract через `rustok-storage` + `rustok-media`, инициализируется в server runtime | **`rustok-storage` service + server runtime wiring** | Низкий | Использовать общий storage contract и не размножать adhoc upload paths |
| Кэширование tenancy | N/A (project concern) | custom tenant cache + negative cache + invalidation + metrics | **RusToK custom** | Низкий | Оставить самопис (platform-specific) |
| Event bus / outbox transport | N/A (project architecture) | memory/outbox/iggy transport + relay worker | **RusToK custom** | Низкий | Оставить самопис |

## Governance register

Реестр ниже — обязательная входная точка для архитектурных решений по Loco-capabilities в `apps/server`.

| Capability | Runtime owner (current) | Source of truth (target) | ADR / reference (required) | Decision status | Next review date | Кодовые точки |
|---|---|---|---|---|---|---|
| Application hooks (`Hooks`) | `apps/server` + `loco_rs` runtime | Loco hooks contract + `apps/server/src/app.rs` as integration layer | `apps/server/docs/loco/README.md`; `DECISIONS/2026-02-19-core-server-module-bundles-routing.md` | Accepted | 2026-06-01 | `apps/server/src/app.rs` |
| Конфигурация приложения (`Config` + `settings.rustok.*`) | `apps/server` | Loco config (`config/*.yaml`) + typed settings in server | `apps/server/docs/loco/README.md`; `docs/architecture/overview.md` | Accepted | 2026-06-01 | `apps/server/src/common/settings.rs`; `apps/server/config/development.yaml`; `apps/server/config/test.yaml` |
| REST/GraphQL routing | `apps/server` | Loco `AppRoutes` + server controllers/graphql modules | `DECISIONS/2026-02-19-core-server-module-bundles-routing.md`; `docs/architecture/api.md` | Accepted | 2026-06-01 | `apps/server/src/app.rs`; `apps/server/src/controllers/mod.rs`; `apps/server/src/graphql/mod.rs` |
| ORM/migrations/entities | `apps/server` migration + SeaORM entities | SeaORM stack in server app + migration crate | `docs/architecture/database.md`; `apps/server/docs/README.md` | Accepted | 2026-06-01 | `apps/server/migration/src/lib.rs`; `apps/server/src/models/mod.rs` |
| Auth framework primitives (JWT/sessions/reset/RBAC wiring) | `apps/server` + `rustok-core` + `rustok-rbac` | Domain auth logic поверх Loco runtime | `DECISIONS/2026-02-26-auth-lifecycle-unification-session-invalidation.md`; `DECISIONS/2026-03-05-rbac-relation-only-final-cutover-gate.md` | Accepted | 2026-05-20 | `apps/server/src/services/rbac_service.rs`; `apps/server/src/graphql/auth/mutation.rs`; `apps/server/src/controllers/auth.rs` |
| Tasks (`cargo loco task`) | `apps/server` via Loco task runtime | Loco tasks API with server task registration | `apps/server/docs/README.md`; `docs/guides/quickstart.md` | Accepted | 2026-06-01 | `apps/server/src/tasks/mod.rs`; `apps/server/src/tasks/cleanup.rs`; `apps/server/src/app.rs` |
| Initializers | `apps/server` via Loco initializer runtime | Loco initializer API + project initializers | `apps/server/docs/README.md`; `docs/guides/observability-quickstart.md` | Accepted | 2026-06-01 | `apps/server/src/initializers/mod.rs`; `apps/server/src/initializers/telemetry.rs`; `apps/server/src/app.rs` |
| Mailer subsystem | `apps/server` (`services/email.rs` + typed settings) | Server email service with `EmailProvider::{Smtp,Loco,None}` and Loco Mailer adapter | `apps/server/docs/loco/README.md`; `docs/architecture/api.md`; `apps/server/docs/README.md` | Accepted | 2026-06-01 | `apps/server/src/services/email.rs`; `apps/server/src/graphql/auth/mutation.rs`; `apps/server/src/common/settings.rs` |
| Workers/queue subsystem | `apps/server` + `rustok-outbox` | RusToK event-driven worker runtime (без Loco queue duplication) | `DECISIONS/2026-03-11-queue-runtime-source-of-truth-outbox.md`; `docs/architecture/event-flow-contract.md`; `docs/standards/transactional-outbox.md` | Accepted | 2026-05-01 | `apps/server/src/app.rs`; `apps/server/src/services/event_transport_factory.rs`; `crates/rustok-outbox/src/relay.rs` |
| Storage abstraction (uploads/assets) | `apps/server` + `rustok-storage` + `rustok-media` | Shared `StorageService` runtime + media module APIs | `apps/server/docs/README.md`; `docs/architecture/modules.md`; `docs/modules/registry.md` | Accepted | 2026-06-01 | `apps/server/src/services/app_runtime.rs`; `apps/server/src/services/graphql_schema.rs`; `apps/server/src/tasks/media_cleanup.rs` |
| Tenancy caching | `apps/server` + `rustok-core` cache backends | RusToK custom tenancy cache (`tenant.rs`) + shared cache backend contract | `crates/rustok-tenant/docs/README.md`; `docs/guides/observability-quickstart.md` | Accepted | 2026-05-01 | `apps/server/src/middleware/tenant.rs`; `apps/server/src/middleware/tenant_cache_v3.rs`; `crates/rustok-core/src/cache.rs` |
| Event bus / transport (`memory|outbox|iggy`) | `apps/server` + `rustok-events` + `rustok-outbox` | RusToK event transport contract + transactional outbox flow | `DECISIONS/2026-02-19-rustok-events-canonical-contract.md`; `crates/rustok-outbox/docs/README.md`; `apps/server/docs/event-transport.md` | Accepted | 2026-05-01 | `apps/server/src/services/event_transport_factory.rs`; `apps/server/src/services/build_request_events.rs`; `apps/server/src/workers/outbox_relay.rs` |

---

## 2) Что реализовано в сервере (полный функциональный срез)

### 2.1 Core Loco lifecycle & app bootstrap

Реализовано в `impl Hooks for App`:
- `app_name`, `app_version`;
- `boot` на `create_app::<Self, Migrator>`;
- `routes` с регистрацией health/metrics/auth/graphql и domain controllers;
- `after_routes` с tenant middleware + runtime extensions;
- `truncate` (не stub, а реальная очистка таблиц в dependency order);
- `register_tasks`;
- `initializers`;
- `connect_workers`;
- `seed`.

### 2.2 Configuration system

- Environment yaml-конфиги (`development.yaml`, `test.yaml`).
- Loco `auth.jwt` конфигурация.
- Typed settings-расширение через `settings.rustok.*` (`tenant`, `search`, `features`, `rate_limit`, `events`, `email`).

### 2.3 Controllers & API surface

- REST controllers: health, metrics, auth, swagger, pages.
- Domain controllers: commerce, content, blog, forum.
- GraphQL endpoint + domain GraphQL modules (`auth`, `commerce`, `content`, `blog`, `forum`, loaders, persisted queries).

### 2.4 Models / ORM / persistence

- SeaORM integration активна.
- Migration crate подключён.
- Основные сущности и модели используются в auth/tenancy/domain flows.

### 2.5 Authentication & authorization (важно: не удалено)

Реализовано и используется:
- JWT access token + refresh token flow.
- Session management в БД (`sessions`).
- Password hashing (`argon2`) и verify.
- Password reset flow (forgot/reset mutations, reset token encoding/decoding, revoke sessions after reset).
- RBAC permissions/roles assignment через `RbacService` + `rustok-rbac`/domain entities.

### 2.6 Middleware / tenancy / rate-limit context

- Tenant resolution middleware (header/domain modes).
- Validation tenant identifiers.
- Cache + negative cache для tenant resolution.
- Middleware layering через `after_routes`.
- Rate-limit настройки есть в `settings`; реальное поведение завязано на серверные middleware/services.

### 2.7 Background processing / events

- Outbox relay worker запускается из `connect_workers`.
- Event runtime создаётся из конфигурации транспорта (`memory` / `outbox` / `iggy`).
- Event-driven подход остаётся приоритетным для очередей и интеграций.

### 2.8 Tasks & Initializers

- `cleanup` task зарегистрирован, поддерживает `sessions`, `cache`, full cleanup.
- `TelemetryInitializer` подключён через Loco initializer API.

### 2.9 Testing support

- Loco testing feature включён в server dev-dependencies.
- Набор unit/integration тестов в серверном модуле присутствует (см. `apps/server/tests` и inline tests в модулях).

---

## 3) Что в Loco есть, но у нас должно/решено быть иначе

### 3.1 Mailer (server infra с provider switch)

**Сейчас:** server email path централизован в `apps/server/src/services/email.rs` и поддерживает `EmailProvider::{Smtp,Loco,None}`. Built-in auth emails рендерятся через файловые шаблоны, а `provider=loco` использует `ctx.mailer`.
**Решение:** Loco Mailer уже является частью живого server-infra контракта, но SMTP сохранён как compatibility/provider option, а не как отдельный параллельный архитектурный слой.

**Правило границ:** Mailer не выносится в отдельный платформенный модуль. Это инфраструктурная ответственность `apps/server` на базе Loco API.

### 3.2 Workers/Queue (осознанно самопис — Loco Queue не подключён)

**Сейчас:** `loco-rs` подключён без queue-фич (нет `sidekiq` / `bg-redis` в features). `connect_workers` hook пустой — все фоновые процессы запускаются как tokio-таски в `connect_runtime_workers`:

| Воркер | Реализация | Тип |
|--------|-----------|-----|
| Outbox relay | `OutboxRelayWorkerHandle` | tokio task, polling outbox |
| Build worker | `BuildWorkerHandle` | tokio task, polling DB |
| Index dispatcher | `spawn_index_dispatcher` | tokio task |
| Search dispatcher | `spawn_search_dispatcher` | tokio task |
| Workflow cron | `WorkflowCronScheduler` | tokio task (feature `mod-workflow`) |

Loco Tasks (CLI): `cleanup`, `rebuild`, `db_baseline`, `media_cleanup`, `create_oauth_app` — запускаются вручную через `cargo loco task`.

Для медленных операций на request path (например, SMTP в `forgot_password`) используется `tokio::spawn` — без Sidekiq.

**Решение:** Loco Queue (Sidekiq/Redis) не нужен:
- Outbox паттерн архитектурно лучше для доменных событий — гарантирует атомарность write + publish.
- Build worker — редкая внутренняя операция, polling достаточен.
- Если понадобится push-based очередь с retry — расширять outbox relay, не подключать Sidekiq.

Policy anchor: `DECISIONS/2026-03-11-queue-runtime-source-of-truth-outbox.md`.

### 3.3 Storage abstraction (shared library contract + server wiring)

**Сейчас:** общий storage contract уже вынесен в `rustok-storage`, а server runtime инициализирует единый `StorageService`; доменный media path оформлен через `rustok-media`.
**Решение:** source of truth для storage теперь не adhoc controller-код и не отдельные per-module upload flows, а shared storage service + media module APIs.

**Правило границ:** Storage не превращается в `ModuleKind::Core` ради самого факта хранения файлов. Shared storage-contract живёт в library/runtime слое, а server отвечает за bootstrap и integration wiring.

---

## 4) Кэширование: текущее состояние (детально)

### 4.1 Tenant cache (основной путь)

`middleware/tenant.rs` реализует:
- versioned cache keys,
- positive cache + negative cache,
- anti-stampede request coalescing (`in_flight` + `Notify`),
- Redis pub/sub invalidation channel (`tenant.cache.invalidate`) при включённом `redis-cache`,
- метрики (`hits/misses/negative/coalesced`).

### 4.2 Cache backends (shared infra)

`rustok-core` предоставляет:
- `InMemoryCacheBackend` (Moka),
- `RedisCacheBackend` (feature-gated), включая circuit breaker.

В сервере используется общий CacheBackend-контракт с выбором backend по feature/runtime.

### 4.3 Cache observability

`/metrics` отдаёт tenant cache метрики `rustok_tenant_cache_*` (hits, misses, entries, negative indicators).

### 4.4 Tenant cache v3

`tenant_cache_v3.rs` присутствует как альтернативная реализация с circuit breaker + Moka моделью, но основной production path сейчас проходит через инфраструктуру `tenant.rs`.

---

## 5) Практические anti-duplication правила

1. Перед добавлением infra-функционала проверять, есть ли его зрелая реализация в Loco.
2. Для осознанных отклонений фиксировать rationale (как для queue/workers) в этом документе.
3. Не держать параллельные production-реализации одного слоя (Mailer/Storage/Queue) без миграционного плана.
4. Любое изменение в кэше должно сопровождаться требованиями к invalidation + метрикам.
5. Для новых модулей: использовать зафиксированный source of truth из матрицы раздела 1.

---

## 6) Current operating contract for Mailer and Storage

### Mailer

- Runtime config lives in `settings.rustok.email.*`.
- Provider selection is explicit: `smtp | loco | none`.
- Built-in auth email rendering is template-based and stays in server infra.
- `provider=loco` uses `ctx.mailer`; `provider=smtp` remains a compatibility/provider path, not a second architecture owner.

### Storage

- Runtime config lives in `settings.rustok.storage.*` and is parsed into `rustok_storage::StorageConfig`.
- `StorageService` is initialized once during app bootstrap and stored in `AppContext.shared_store`.
- Media/domain workflows consume the shared storage contract through `rustok-media` and runtime wiring, not via controller-local backend setup.

### Anti-duplication invariant

- Не добавлять параллельный mailer runtime вне `apps/server/src/services/email.rs`.
- Не добавлять adhoc upload/download backends мимо `rustok-storage` / shared runtime wiring.
- Любой новый provider/driver должен расширять существующий typed config и observability path, а не вводить отдельный source of truth.

## 7) Operational runbook (incidents / rollback)

- Incident/rollback runbook для фаз 2–4: [`LOCO_FEATURE_SUPPORT.md#6-loco-mailer--storage-roadmap-release-phases`](./LOCO_FEATURE_SUPPORT.md#6-loco-mailer--storage-roadmap-release-phases).
- Обязательная процедура при тревогах по gate-метрикам:
  1. Зафиксировать инцидент с phase ID (`mailer-shadow`, `mailer-cutover`, `storage-cutover`).
  2. Включить rollback toggle (provider=`smtp` или legacy storage provider) в runtime config.
  3. Проверить восстановление SLA в 2 последовательных окнах наблюдения.
  4. Сохранить post-incident summary и обновить этот roadmap перед повторным rollout.

## 6.1 Зафиксированное архитектурное правило (2026-03-11)

- Mailer и Storage считаются частью Loco-backed infrastructure в `apps/server`.
- Для Mailer/Storage запрещено создавать отдельный модуль платформы в `crates/rustok-*`.
- Доменные модули используют эти возможности через server-level adapters/policies.
- Детали решения зафиксированы в ADR: `DECISIONS/2026-03-11-loco-mailer-storage-as-server-infra.md`.

---

## 8) Sources

- `apps/server/src/app.rs`
- `apps/server/src/controllers/mod.rs`
- `apps/server/src/controllers/metrics.rs`
- `apps/server/src/graphql/mod.rs`
- `apps/server/src/graphql/auth/mutation.rs`
- `apps/server/src/services/email.rs`
- `apps/server/src/services/event_transport_factory.rs`
- `apps/server/src/tasks/mod.rs`
- `apps/server/src/tasks/cleanup.rs`
- `apps/server/src/initializers/mod.rs`
- `apps/server/src/initializers/telemetry.rs`
- `apps/server/src/middleware/tenant.rs`
- `apps/server/src/middleware/tenant_cache_v3.rs`
- `apps/server/src/common/settings.rs`
- `apps/server/config/development.yaml`
- `apps/server/config/test.yaml`
- `crates/rustok-core/src/cache.rs`
- `crates/rustok-core/src/context.rs`
- `apps/server/Cargo.toml`
- `Cargo.toml`


