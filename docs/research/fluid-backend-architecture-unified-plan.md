# Консолидированный план реализации Fluid Backend Architecture (FBA)

Этот документ объединяет:

- внутренние материалы RusTok по FBA;
- существующий implementation plan;
- практический migration path «монолит → модульный монолит → выборочные сервисы».

Цель: дать единый исполнимый план, по которому команда может последовательно переводить отдельные module boundaries в remote profile **без переписывания domain/application-логики**.

---

## 0. Стратегические рамки (что считаем успехом)

### 0.1. Целевой принцип

FBA в RusTok — это **не** microservices-first. Это transport/topology-fluid подход:

- module identity и ownership стабильны;
- service contract стабилен;
- runtime topology может меняться (embedded/remote/hybrid);
- transport (gRPC/events) — адаптер, а не владелец бизнес-правил.

### 0.2. Что не делаем

- Не извлекаем каждый crate в отдельный сервис.
- Не начинаем с service-owned DB как первого шага.
- Не дублируем доменную логику в разных transport handlers.
- Не превращаем `rustok-commerce` в domain microservice (это orchestration/facade слой).

### 0.3. KPI миграции

Успешная фаза считается завершённой, если:

1. Контракты портов одинаково проходят contract tests в in-process и remote профилях.
2. Для write-path соблюдается idempotency/retry safety.
3. Метрики и трассировка сопоставимы между topology-профилями.
4. Нет прямого доступа к чужим таблицам вне owner boundary.

---

## 1. Аудит текущего состояния (Stage A)

### 1.1. Инвентаризация модулей

Собрать таблицу по всем целевым модулям (минимум ecommerce family):

- slug;
- владелец;
- owned storage;
- публичные команды/запросы;
- входящие/исходящие события;
- зависимости (Cargo + module graph);
- текущая роль: facade/orchestrator, write-model owner, read-model provider, supporting service.

### 1.2. Карта связности и «долгов»

Для каждого модуля отметить:

- прямые вызовы чужой доменной логики;
- прямой SQL к чужим таблицам;
- отсутствие idempotency keys на мутациях;
- отсутствие deadline/timeout semantics;
- event gaps (нет outbox, неversioned payload, неявная replay policy).

### 1.3. FBA readiness score

Оценивать модуль по 3 уровням:

- **High**: явный порт + слабая связность + события/контракты готовы;
- **Medium**: есть порт/контракт, но остались прямые зависимости;
- **Low**: фасад/плотный монолитный узел, не кандидат на ранний remote.

### 1.4. Формализация решений

Каждый перевод модуля в remote profile фиксировать отдельным ADR:

- почему выносим;
- почему сейчас;
- какие риски консистентности/latency;
- rollback strategy.

---

## 2. Stage 0–3: общие контракты до транспорта

## 2.1. Единый FBA context contract

Ввести канонический `FbaContext`, который обязательно проходит через каждый порт:

- tenant;
- actor/service identity;
- claims/role;
- channel;
- locale;
- correlation/causation;
- trace/span context;
- idempotency key (для write);
- deadline/timeout/cancellation.

Правило: context передаётся явно параметром порта, а не ad-hoc заголовками.

### 2.2. Unified error model

Согласовать общий доменный error taxonomy:

- validation;
- not_found;
- conflict;
- unauthorized/forbidden;
- unavailable/timeout;
- invariant_violation.

И закрепить маппинг в REST/GraphQL/gRPC, чтобы UI и orchestration слой получали предсказуемое поведение.

### 2.3. Ports before transports

Для модулей определить transport-agnostic порты (trait/contract):

- `ProductPort`;
- `PricingPort`;
- `InventoryPort`;
- `CartPort`;
- `OrderPort`;
- `PaymentPort`;
- `FulfillmentPort`;
- `TaxPort`.

Сначала сделать **in-process implementation**, затем remote adapters.

### 2.4. Policy: no foreign-table access

Жёстко закрепить:

- модуль читает/пишет только свои таблицы;
- межмодульное чтение — только через порт или snapshot DTO/read model.

### 2.5. Outbox и словарь событий

Для всех write owners:

- outbox write в одной транзакции с domain state;
- versioned event contracts;
- обязательные поля: tenant, aggregate_id, schema_version, correlation/causation, idempotency semantics.

Consumer side:

- idempotent handlers;
- replay safety;
- out-of-order tolerance policy.

### 2.6. Contract testing baseline

На каждый порт:

- общий test suite;
- запуск против in-process impl;
- запуск против remote adapter.

Цель: отличаться должны только latency/failure mode, а не бизнес-результат.

---

## 3. Пилотная дорожная карта

## 3.1. Пилот 1 (низкий риск): async/read-oriented сервис

Кандидаты:

- search/indexing;
- AI-enrichment/recommendations.

Шаги:

1. Оформить порт + remote-capable adapter (gRPC или async worker).
2. Переключить вызовы фасада/host на порт.
3. Включить feature/config toggle between embedded/remote.
4. Сравнить SLO: latency, error rate, throughput.
5. Проверить event pipeline (обновления каталога/цен доходят до read-side).

## 3.2. Пилот 2 (core domain): Inventory Reservation Service

Шаги:

1. Ввести reservation model (`reservation`, TTL/expiry, idempotency key).
2. Зафиксировать события: `InventoryReserved`, `InventoryReservationReleased`, `InventoryAdjusted`.
3. Реализовать `InventoryPort` server/client для remote profile.
4. Добавить компенсации в checkout saga (`release_reservation`).
5. Провести нагрузочный профиль (cart peak/checkout spike).

## 3.3. Commerce orchestrator и checkout saga

`rustok-commerce` остаётся orchestration/facade слоем:

- нормализует `FbaContext`;
- выбирает in-process vs remote adapters;
- координирует checkout steps;
- управляет компенсациями.

Checkout saga должна иметь формализованные:

- шаги success-path;
- таблицу компенсаций по каждому шагу;
- ограничения retry/idempotency;
- разделение sync RPC и async post-processing (email/analytics/search updates).

## 3.4. Пилот 3: Payment/Fulfillment/Pricing/Product read-side

Порядок:

1. `PaymentPort` и `FulfillmentPort` как remote adapters к внешним провайдерам.
2. `ProductPort` read-side snapshots (`get_product_snapshot`, `list_publishable_catalog_page`).
3. `PricingPort` после стабилизации product snapshots.
4. `TaxPort` как explicit support boundary (дальше выбрать embedded/stateless remote/provider adapter).

---

## 4. Поздние стадии: storage decoupling и write-model extraction

### 4.1. Поддерживаемые режимы хранения

Для каждого remote-capable модуля явно выбрать профиль:

1. shared DB + in-process;
2. shared DB + remote process;
3. service-owned DB;
4. read-model replica/projection.

### 4.2. Правило очередности

Сначала контракты/контекст/события/observability, затем topology switch.

Service-owned DB — только после того, как модуль стабильно работает как remote boundary и имеет зрелую saga/outbox модель.

### 4.3. Где лучше read-model replica

Для high-read сценариев (поиск/аналитика/витринные read APIs) приоритетнее событийные проекции, чем ранний разрез transactional write model.

---

## 5. Definition of Ready для перевода модуля в remote

Модуль может перейти в remote profile только если выполнены все пункты:

1. **Stable port contract**: transport-agnostic интерфейс, контракт-тесты на 2 профилях.
2. **Context completeness**: все обязательные поля `FbaContext` валидируются на входе.
3. **Outbox/event maturity**: транзакционная публикация, versioning, replay-safe consumers.
4. **Data ownership purity**: отсутствует direct foreign-table access.
5. **Idempotency/deadlines**: все write команды retry-safe, с явными timeout/deadline.
6. **Observability parity**: health/readiness, latency/error metrics, trace propagation.
7. **ADR approved**: решение и риски зафиксированы архитектурно.

---

## 6. Исполнимый roadmap по кварталам (пример)

### Q1

- Закрыть аудит и readiness matrix.
- Ввести `FbaContext` + unified errors.
- Добить портовый слой и базовые contract tests.
- Зафиксировать event vocabulary + outbox baseline.

### Q2

- Провести Пилот 1 (async/read).
- Включить observability parity dashboards/alerts.
- Доработать компенсации checkout.

### Q3

- Провести Пилот 2 (Inventory Reservation).
- Нагрузочные испытания и стабилизация retry semantics.
- Подготовить Product read-side snapshots.

### Q4

- Пилот 3: Payment/Fulfillment adapters + Pricing read/compute boundary.
- Решения по selective storage decoupling (только там, где доказан выигрыш).

---

## 7. Короткие практические правила для команды

1. Сначала стабилизируем границы, потом переносим процессы.
2. Каждое remote-решение должно иметь измеримую operational причину.
3. Orchestration и domain ownership не смешиваем.
4. Для write-path не допускаем «best-effort без idempotency».
5. Любой cross-module read оформляем через порт/snapshot, а не прямой SQL.

---

## 8. Результат консолидации

Этот план объединяет архитектурные принципы FBA и пошаговую delivery-дорожку:

- от текущего modular monolith;
- через портовую и событийную дисциплину;
- к выборочному remote execution там, где это оправдано.

Главный инвариант: **service contract стабилен, topology изменяема**.
