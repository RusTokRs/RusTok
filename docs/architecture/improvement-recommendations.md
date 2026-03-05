# RusToK — Architecture Improvement Recommendations

- Date: 2026-02-19
- Status: Living document (updated)
- Last updated: 2026-03-05 (backlog cleanup pass)
- Author: Platform Architecture Review

---

## 1. Контекст: что мы видим в коде сегодня

Прежде чем давать рекомендации, зафиксируем реальное состояние системы.

### 1.1 Граница между ядром и опциональными модулями

После анализа кода можно выделить **три категории** компонентов:

#### Категория A — Compile-time Infrastructure (не `RusToKModule`, не регистрируются)

Это «невидимые» для реестра crate'ы. Они линкуются в бинарник всегда, но не участвуют в lifecycle модулей:

| Crate | Роль | Почему не `RusToKModule` |
|---|---|---|
| `rustok-core` | Контракты, EventBus, RBAC, кэш, Circuit Breaker, метрики | Это само ядро, определяет trait |
| `rustok-iggy` + `rustok-iggy-connector` | L2 streaming transport (опциональный транспорт) | Технический адаптер, не бизнес-логика |
| `rustok-telemetry` | OpenTelemetry, tracing, Prometheus | Сквозная зависимость |
| `rustok-test-utils` | Фикстуры, моки, хелперы для тестов | **Только `dev-dependencies`**, в production binary не входит |
| `utoipa-swagger-ui-vendored` | Vendored Swagger UI assets | Статический ресурс, не модуль платформы |
| `alloy-scripting` | Скриптовый движок Rhai | Сейчас инициализируется напрямую в `app.rs` |
| `tailwind-rs/css/ast` | CSS tooling | Build-time инструментарий |
| `rustok-mcp` | MCP адаптер с binary target | Отдельный сервер, не часть основного runtime |

#### Категория B — Core Platform Modules (регистрируются как `ModuleKind::Core`, нельзя отключить)

Это модули, реализующие `RusToKModule` и **обязательные для работы платформы**:

| Crate | Роль | Текущий статус |
|---|---|---|
| `rustok-index` | CQRS read-model, индексатор для storefront | ✅ Зарегистрирован как Core (`ModuleKind::Core`) |
| `rustok-tenant` | Tenant metadata, lifecycle хуки | ✅ Зарегистрирован как Core (`ModuleKind::Core`) |
| `rustok-rbac` | RBAC helpers, lifecycle хуки | ✅ Зарегистрирован как Core (`ModuleKind::Core`) |

> **`rustok-outbox` — core-компонент платформы.** Он не реализует `RusToKModule` и не входит в registry, но относится к категории Core Infrastructure: `TransactionalEventBus` используется при каждой write-операции во всех domain-модулях. Инициализируется через `build_event_runtime()` в `app.rs`, а не через `ModuleRegistry`. Остановка outbox = потеря гарантий доставки событий для всей платформы.

#### Категория C — Optional Domain Modules (регистрируются как `ModuleKind::Optional`, per-tenant toggle)

| Crate | Тип | Depends on |
|---|---|---|
| `rustok-content` | Domain (фактически required) | `rustok-core` |
| `rustok-commerce` | Domain | `rustok-core` |
| `rustok-blog` | Wrapper | `rustok-content` |
| `rustok-forum` | Wrapper | `rustok-content` |
| `rustok-pages` | Domain | `rustok-core` |

**Ключевые наблюдения:**
- `rustok-index`, `rustok-tenant`, `rustok-rbac` — Категория B: зарегистрированы в `build_registry()` как Core-модули и проходят health/lifecycle через единый реестр.
- `rustok-outbox` — ядро платформы, но **не через registry**: это `EventTransport`-слой, инициализируемый отдельно.
- `rustok-test-utils` — **исключительно `[dev-dependencies]`**, в production binary не входит никогда.
- `utoipa-swagger-ui-vendored` — vendored статика Swagger UI, не `RusToKModule`.

### 1.2 Реальное состояние событийной системы (три уровня)

Три транспортных уровня — это **не иерархия**, а **три независимых режима**, выбираемых через `settings.rustok.events.transport`:

```
build_event_runtime()  ←  вызывается в app.rs::after_routes()
         │
   ┌─────┴─────────────────────────────────────────┐
   │                    │                           │
   ▼                    ▼                           ▼
L0: MemoryTransport   L1: OutboxTransport      L2: IggyTransport
(tokio::broadcast)    (PostgreSQL sys_events)  (внешний Iggy-сервер)
  dev/MVP only          production default       highload / replay
                              │
                         OutboxRelay
                         (tokio::spawn loop)
                         batch=100, retry×5
                         backoff 1s → 60s
                         relay target: MemoryTransport  ← !
```

**Критические факты по коду:**

1. **L1 relay пишет обратно в `MemoryTransport`**, а не в Iggy. L1→L2 pipeline как связная цепочка **не реализован** — это три независимых режима.

2. **L2 (Iggy)** при выборе `transport = "iggy"` делает `connector.connect()` синхронно при старте сервера. Если Iggy-сервер недоступен → сервер **падает при старте** с ошибкой `BadRequest`.

3. **`replay()` в `IggyTransport`** — заглушка: проверяет `is_initialized()` и возвращает `Ok(())`. Replay событий не реализован.

4. **Текущий production-путь:** `outbox` (L1). `memory` — только dev. `iggy` — инфраструктурно готов, но функционально incomplete.

5. **`rustok-outbox` — самый критичный компонент платформы**: `TransactionalEventBus` вызывается при каждой write-операции во всех domain-модулях. Его остановка = полная остановка write-path.

### 1.3 Реализация кэша

Кэш **двухслойный** с автоматическим fallback и используется **только для tenant resolution**:

```
RUSTOK_REDIS_URL / REDIS_URL задан?
    ├── ДА  → RedisCacheBackend  (feature = "redis-cache")
    │            ├── CircuitBreaker на каждый GET/SET/DEL
    │            ├── При open circuit → Error::Cache, warn в лог
    │            └── Ключ: prefix + ":" + key
    └── НЕТ → InMemoryCacheBackend (moka)
                 set() использует default_ttl инстанса
                 set_with_ttl() поддерживает per-entry TTL
                 Capacity = 1000 записей
```

**Два отдельных кэша для каждого запроса:**

| Кэш | Ключ | TTL | Назначение |
|---|---|---|---|
| `tenant_cache` | `tenant:v1:{uuid\|slug\|host}:value` | 5 мин | Найденные tenants |
| `tenant_negative_cache` | `tenant_negative:v1:{uuid\|slug\|host}:value` | 60 сек | Несуществующие tenants (flood protection) |

**Stampede protection** через `in_flight: Arc<Mutex<HashMap<String, Arc<Notify>>>>`:
- 100 параллельных запросов по одному tenant → только **1** идёт в БД, остальные ждут `Notify::notified()`.

**Cross-instance invalidation** (только при Redis):
- При обновлении tenant → `PUBLISH tenant.cache.invalidate <key>|<neg_key>` в Redis pub/sub.
- Все инстансы подписаны и локально инвалидируют оба ключа.
- Метрики (hits/misses) тоже пишутся в Redis через `INCR` → `/metrics` показывает агрегат кластера.

**Обновление статуса:** проблема с `InMemoryCacheBackend::set_with_ttl()` закрыта (см. пункт 2.8): per-entry TTL поддерживается корректно через `moka::Expiry`.

---

## 2. Рекомендации (актуальный backlog)

> Раздел очищен от закрытых пунктов и задач, потерявших актуальность. Ниже оставлены только действия, которые ещё требуют реализации.

### 2.1 Вынести `DomainEvent` из `rustok-core` в `rustok-events` (стратегический трек)

**Почему это ещё в backlog:** контракт событий всё ещё частично завязан на core-слой, что усложняет эволюцию schema/versioning и переиспользование событийных контрактов вне core.

**Что осталось сделать:**
- завершить migration phases 2/3 из ADR `2026-02-19-rustok-events-canonical-contract.md`;
- убрать остаточные direct dependencies доменных модулей на legacy event API в `rustok-core`;
- зафиксировать deprecation-политику и финальную точку удаления совместимого слоя.

### 2.2 Typed per-tenant module settings

**Почему это ещё в backlog:** контракт настроек модулей формализован на уровне дизайна, но runtime и миграционный путь для всех модулей ещё не доведены до единообразного typed-формата.

**Что осталось сделать:**
- определить единый typed контракт настроек в runtime (валидация + дефолты + версионирование);
- подготовить безопасную миграцию с legacy `tenant_modules.settings`;
- добавить интеграционные тесты на backward compatibility.

### 2.3 `rustok-notifications` как опциональный модуль

**Почему это ещё в backlog:** capability нужна платформе, но полноценного optional-модуля с lifecycle/health/dependency контрактом пока нет.

**Что осталось сделать:**
- оформить `rustok-notifications` как `RusToKModule` (`ModuleKind::Optional`);
- подключить единый lifecycle (init/health/stop) и пер-tenant toggle;
- формализовать зависимости и эксплуатационные метрики.

### 2.4 Разделение `apps/server` на `core-server` + `module-bundles` (после ADR)

**Почему это ещё в backlog:** архитектурный трек зафиксирован в ADR, но требует отдельной фазы внедрения с контролем регрессий.

**Что осталось сделать:**
- подготовить минимальный план нарезки на bundles без breaking для runtime;
- внедрить поэтапно (сначала internal bundles, затем external/plugin-ready слой);
- синхронизировать route registration policy и module ownership boundaries.

## 3. Приоритизированный план действий (только открытые пункты)

| ID | Рекомендация | Приоритет | Статус | Риск | Ценность | Owner area |
|---|---|---|---|---|---|---|
| 2.1 | Вынести `DomainEvent` из core | 🔵 Стратегически | In Progress | Высокий | Extensibility | Platform foundation |
| 2.2 | Typed per-tenant module config | 🟢 Улучшение | Backlog | Средний | Consistency / safety | Platform foundation + domain modules |
| 2.3 | `rustok-notifications` как optional module | 🟢 Улучшение | Backlog | Высокий | New capability | Domain modules |
| 2.4 | `core-server` + `module-bundles` | 🔵 Стратегически | ADR ready / Backlog | Высокий | DX / scalability | Platform foundation |

## 4. Итоговая картина

- Базовые архитектурные проблемы из первого ревью (границы Core/Optional, регистрация core-модулей, валидация `modules.toml`, outbox reliability baseline, Alloy lifecycle baseline) закрыты и больше не ведутся как отдельные action items в этом документе.
- Текущий фокус смещён на стратегические и расширяемые треки: event contract decoupling, typed module settings, notifications module и server bundling.
- Все новые изменения по этим направлениям должны синхронно отражаться в `docs/architecture/*`, `docs/modules/*` и соответствующих ADR.

## 5. Roadmap (следующие итерации, только незакрытые работы)

### 5.1 Итерация 1 — event contract decoupling (2.1)

**Scope**
- продолжить phases 2/3 по migration `DomainEvent` в `rustok-events`;
- подготовить и запустить deprecation-path для legacy API.

**DoD**
- доменные модули используют канонический контракт из `rustok-events`;
- legacy-слой помечен как deprecated с фиксированной датой удаления;
- миграция покрыта integration/regression тестами.

### 5.2 Итерация 2 — typed settings + notifications (2.2, 2.3)

**Scope**
- довести typed module settings до production-ready состояния;
- включить `rustok-notifications` в единый lifecycle optional-модулей.

**DoD**
- настроечный контракт модулей типизирован и валидируется в runtime;
- миграция legacy settings выполняется без потери совместимости;
- notifications-модуль наблюдаем через health/metrics и поддерживает per-tenant toggle.

### 5.3 Итерация 3 — server bundling prep (2.4)

**Scope**
- подготовить техническую фазу декомпозиции `apps/server` на `core-server` и `module-bundles`;
- согласовать sequence внедрения без breaking-runtime.

**DoD**
- зафиксирован пошаговый implementation plan с контрольными точками;
- route/module boundaries формализованы и проверяемы тестами;
- риски миграции описаны и закрыты mitigation-планом.

### 5.4 Отдельный backlog: замена самописного кода библиотеками

Ниже — точки, где самописная логика может быть заменена battle-tested библиотеками (отдельно от стратегического roadmap выше).

| Область | Что сейчас | Чем заменить | Почему это лучше |
|---|---|---|---|
| Security validation (`rustok-core`) | Ручной blacklist-regex для SQL/XSS/command/path, ручная HTML-экранизация, email/UUID regex | `ammonia` (sanitization), `validator` + `email_address`/`uuid::Uuid::parse_str`, `garde`/`nutype` | Меньше ложных срабатываний/пропусков, проще сопровождение |
| SSRF/URL validation (`rustok-core`) | Ручная проверка `localhost`, private IP и scheme | `url` + `ipnet`/`cidr` deny/allow policy | Меньше edge-case багов, централизуемая policy |
| Rate limiting (`rustok-core` + `apps/server`) | Собственный token bucket + cleanup | `governor` и/или `tower-governor` | Надёжнее и дешевле в поддержке |
| Retry/circuit-breaker (`rustok-core`) | Самописные `RetryPolicy` и `CircuitBreaker` | `tower` + `backoff`/`tokio-retry` (или `failsafe`) | Меньше custom state-machine кода |
| Frontend debounce hooks (`next-admin`) | Локальные `useDebounce`/`useDebouncedCallback` | `use-debounce` или `lodash.debounce` | Меньше edge cases и дублирования |

Приоритизация:
1. P0 — security validation + SSRF policy.
2. P1 — rate limiting + resilience stack.
3. P2 — frontend debounce.

Базовый rollout:
- adapter/feature-flag (`legacy` vs `library`);
- постепенное включение по сегментам трафика;
- regression corpus для security payloads в CI.

---

## 6. Связанные документы

- [`docs/architecture/overview.md`](./overview.md) — архитектурный обзор
- [`docs/architecture/principles.md`](./principles.md) — принципы архитектуры
- [`docs/architecture/events.md`](./events.md) — транзакционная публикация событий
- [`docs/modules/registry.md`](../modules/registry.md) — реестр компонентов
- [`docs/modules/overview.md`](../modules/overview.md) — состояние модулей
- [`DECISIONS/`](../../DECISIONS/) — архитектурные решения (ADR)
