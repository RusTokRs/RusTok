# Единый план реализации Fluid Backend Architecture (FBA) для RusTok

Этот документ — **единственный актуальный план внедрения FBA** в RusTok.
Он заменяет разрозненные плановые материалы и задаёт обязательную последовательность этапов.

Связанный концептуальный документ: [Fluid Backend Architecture для RusTok](./fluid-backend-architecture.md).


Связка [Fluid Frontend Architecture (FFA)](./fluid-frontend-architecture.md) и [Fluid Backend Architecture (FBA)](./fluid-backend-architecture.md) даёт RusTok переносимость модулей между embedded и headless/remote профилями без переписывания core-логики.


## 0) Визуальный контекст админок (UI parity)

Ниже — иллюстрации двух runtime-вариантов админки, которые должны сохранять одинаковую
бизнес-семантику и навигационный контракт в рамках FFA+FBA.

### Leptos-вариант (SSR-first)

![Leptos Admin Dashboard](https://github.com/user-attachments/assets/leptos-admin-dashboard)

*Описание:* тёмная SSR-first админка с блоками `Total users / Content nodes / Orders / Revenue snapshot`,
блоком `Recent activity` и виджетом `Enabled modules`. Это референс для Leptos-host профиля
(`apps/admin`) и module-owned UI route contract.

### Next-вариант (headless/runtime parity)

![Next Admin Dashboard](https://github.com/user-attachments/assets/next-admin-dashboard)

*Описание:* Next-host вариант с тем же смысловым набором: дашборд метрик, активность,
операторские действия и модульная навигация. Это референс для `apps/next-admin`, где должен
сохраняться parity с Leptos-вариантом по данным, ролям и сценариям.

> Примечание: визуальный стиль может отличаться, но контракт FFA+FBA требует стабильности
> доменных сценариев, прав доступа, route/query semantics и backend orchestration behavior.

---

## 1) Цель и границы

## 1.1 Цель

Переводить отдельные module boundaries в remote execution profile (например, gRPC/async worker) **без переписывания domain/application-логики**.

## 1.2 Архитектурный инвариант FBA

Во всех этапах сохраняется:

- `module identity` (slug/ownership/область ответственности);
- `service contract` (команды, query, события);
- `domain rules` и policy semantics.

Меняется только `runtime topology`: embedded / remote / hybrid.

## 1.3 Что запрещено

- «Каждый crate = микросервис».
- Дублирование бизнес-логики по transport handlers.
- Прямой доступ к чужим таблицам после формализации портов.
- Ранний переход на service-owned DB до зрелости портов/событий/наблюдаемости.

---

## 2) Структура этапов (обязательный порядок)

1. **Этап A — Аудит и готовность модулей**
2. **Этап B — Базовые контракты FBA (до транспорта)**
3. **Этап C — Событийная дисциплина и contract testing**
4. **Этап D — Пилот 1 (async/read-boundary)**
5. **Этап E — Пилот 2 (Inventory Reservation)**
6. **Этап F — Пилот 3 (Payment/Fulfillment/Product read/Pricing)**
7. **Этап G — Выборочная storage-decomposition и write extraction**

Переход к следующему этапу допускается только после выполнения Exit Criteria текущего.

## 2.1 Текущие FBA-треки и единый шаблон

На 2026-06-14 в репозитории уже есть несколько FBA-треков. Они не должны переводиться
разными способами: новые и существующие инкременты обязаны сходиться к одному шаблону
`provider/consumer metadata + нейтральные ports + typed errors + fallback/rollout evidence`.

| Модуль | Текущая роль | Статус | Единообразный следующий шаг | Источник evidence |
|---|---|---|---|---|
| `page_builder` | reference provider для `preview/tree/properties/publish` | `in_progress` | Довести capability handlers и contract tests до `boundary_ready`, не меняя provider/consumer metadata format | `crates/rustok-page-builder/contracts/page-builder-fba-registry.json`, `crates/rustok-page-builder/docs/implementation-plan.md` |
| `pages` | первый consumer reference provider-а `page_builder` | `in_progress` | Заменить synthetic Wave 0 evidence фактическими tenant before/after snapshots и smoke/trace packet | `crates/rustok-pages/docs/implementation-plan.md`, registry page-builder |
| `commerce` | umbrella orchestration/readiness-hardening для ecommerce slices | `in_progress` | Выравнивать checkout/post-order boundaries под тот же шаблон: owner-module ports, typed errors/context, events и отсутствие rules в transport/UI | `crates/rustok-commerce/docs/implementation-plan.md` |
| `forum` | deferred consumer candidate для `page_builder` | `not_started` | Не повышать статус до появления local consumer evidence; держать запись как deferred в provider registry | `crates/rustok-page-builder/contracts/page-builder-fba-registry.json` |

Правила единообразия:

1. **FBA остаётся названием rollout-а, а не обязательным префиксом типов.** Code-facing контракты используют нейтральные имена (`PortContext`, `PortError`, `*Port`, `provider`, `consumer`).
2. **Источник статуса — local `docs/implementation-plan.md`, центральный board синхронизируется в том же изменении.** Нельзя оставлять `not_started`, если есть активный FBA provider/consumer evidence.
3. **Machine-readable metadata обязательна для provider/consumer tracks.** Для `page_builder -> pages` источником является `page-builder-fba-registry.json`; следующие tracks должны переиспользовать тот же формат или явно расширять его в этом плане, а не создавать параллельный формат.
4. **Нейтральные port primitives применяются только к новым/обновляемым портам.** Уже существующие FBA slices не переписываются механически без feature work; при следующем изменении они приводятся к тем же `context/error/idempotency/deadline` требованиям.
5. **Повышение до `boundary_ready` или `transport_verified` требует evidence.** Наличие metadata или FFA split само по себе не считается remote/runtime verification.

---

## 3) Этап A — Аудит и readiness matrix

## 3.1 Обязательные артефакты

- `Module Inventory Table` (по каждому целевому модулю):
  - slug, owner, owned storage, публичные use-cases;
  - входящие/исходящие события;
  - зависимости (Cargo + modules graph);
  - роль: orchestrator/facade, write-model owner, read-model provider, support service.
- `Coupling Debt Register`:
  - прямые вызовы соседних доменов;
  - прямой SQL к чужим таблицам;
  - отсутствие idempotency/deadline;
  - event gaps (нет outbox/versioning/replay policy).
- `Readiness Matrix`: High / Medium / Low.

## 3.2 Критерии готовности этапа A

- Все модули в целевом скоупе имеют заполненную inventory-строку.
- Для каждого Medium/Low модуля зафиксирован remediation backlog.
- Для каждого кандидата на remote есть ADR-черновик с рисками и rollback-подходом.

---

## 4) Этап B — Базовые FBA-контракты (Ports before transports)

## 4.1 Единый `PortContext`

Стартовая shared-реализация находится в `rustok-api::ports` и намеренно остаётся transport-agnostic: это контрактный примитив для портов/адаптеров, а не доменный сервис.

Обязательные поля:

- tenant;
- actor/service identity;
- claims/role;
- channel + locale;
- correlation/causation + trace context;
- idempotency key (write);
- deadline/timeout/cancellation.

Правило: передаётся явным параметром каждого порта.

## 4.2 Unified error model

Единый набор доменных ошибок (validation/not_found/conflict/forbidden/unavailable/timeout/invariant violation) + предсказуемый mapping в REST/GraphQL/gRPC.

## 4.3 Портовый слой

Минимальный целевой набор портов:

- `ProductPort`, `PricingPort`, `InventoryPort`, `CartPort`,
- `OrderPort`, `PaymentPort`, `FulfillmentPort`, `TaxPort`.

Требование: сначала in-process impl, потом remote adapters.

## 4.4 Data ownership policy

- Модуль пишет/читает только свой storage.
- Межмодульный data access — только через порт/snapshot DTO/read model.

## 4.5 Критерии готовности этапа B

- Все целевые порты определены в transport-agnostic виде.
- `PortContext` и error model используются во всех новых/обновлённых портовых вызовах.
- Новые прямые foreign-table доступы не допускаются.

---

## 5) Этап C — События, outbox и контрактное тестирование

## 5.1 Event vocabulary

Для критичных доменов задать versioned vocabulary (например: `ProductPublished`, `PriceChanged`, `InventoryReserved`, `OrderPlaced`, `PaymentAuthorized`).

Каждое событие обязано иметь: tenant, aggregate id, schema version, correlation/causation, idempotency semantics.

## 5.2 Outbox discipline

- Запись domain state + outbox в одной транзакции.
- Публикация через worker/dispatcher.
- Consumer-ы idempotent + replay-safe + tolerant к out-of-order.

## 5.3 Contract tests

Для каждого порта один и тот же набор тестов запускается:

- против in-process impl;
- против remote adapter.

Бизнес-результат должен совпадать, отличия допустимы только по latency/failure envelope.

## 5.4 Критерии готовности этапа C

- Outbox включён для всех write owners в пилотном скоупе.
- Contract tests есть для всех портов пилотного скоупа.
- Есть сценарии replay/idempotency/out-of-order в тестах.

---

## 6) Этап D — Пилот 1 (async/read-boundary)

## 6.1 Кандидаты

- search/indexing;
- AI enrichment/recommendations.

## 6.2 Шаги

1. Вынести boundary в порт и адаптер (gRPC либо async worker — по характеру use-case).
2. Подключить переключение embedded/remote через runtime config.
3. Перевести вызовы host/facade на порт.
4. Проверить SLO: latency, error rate, throughput, retry behavior.

## 6.3 Exit Criteria

- Функциональный паритет с embedded профилем подтверждён.
- Метрики и трассировка стабильны минимум на согласованном окне наблюдения.

---

## 7) Этап E — Пилот 2 (Inventory Reservation)

## 7.1 Шаги

1. Ввести `reservation` модель: idempotency key, TTL/expiration, статусный lifecycle.
2. Закрепить события: `InventoryReserved`, `InventoryReservationReleased`, `InventoryAdjusted`.
3. Реализовать `InventoryPort` remote server/client.
4. Встроить компенсации в checkout saga (`release_reservation`).
5. Провести нагрузочные тесты на пиковых checkout-сценариях.

## 7.2 Exit Criteria

- Reservation команды retry-safe.
- Компенсации корректно отрабатывают на контролируемых сбоях.
- Нагрузочный профиль не деградирует ниже согласованных порогов.

---

## 8) Этап F — Пилот 3 (Payment/Fulfillment/Product read/Pricing)

Порядок обязателен:

1. `PaymentPort` и `FulfillmentPort` как remote adapters (внешние провайдеры).
2. `ProductPort` read-side snapshots (`get_product_snapshot`, `list_publishable_catalog_page`).
3. `PricingPort` после стабилизации product read contracts.
4. `TaxPort` как explicit support boundary (embedded/stateless remote/provider adapter — решается ADR).

## 8.1 Exit Criteria

- Нет прямого чтения product internals из pricing.
- Checkout orchestration работает через порты с теми же бизнес-результатами.
- Synchronous path и async post-processing разделены архитектурно.

---

## 9) Этап G — Поздние стадии (storage и write extraction)

Разрешённые режимы хранения:

1. shared DB + in-process;
2. shared DB + remote process;
3. service-owned DB;
4. read-model replica/projection.

Правило: переход к `service-owned DB` только после стабильной remote работы модуля, зрелой saga/outbox модели и утверждённого ADR.

---

## 10) Единый Definition of Ready для перевода модуля в remote

Модуль можно переводить в remote profile только при выполнении **всех** условий:

1. Stable transport-agnostic port + contract tests (in-process/remote).
2. Полный `PortContext` на всех командах/запросах.
3. Outbox + versioned events + replay/idempotency policy.
4. Отсутствие foreign-table доступа вне owner boundary.
5. Write методы имеют idempotency key и deadline semantics.
6. Health/readiness/metrics/tracing parity между профилями.
7. Отдельный ADR с причинами, рисками, rollback и ownership impact.

---

## 11) Минимальный квартальный rollout (шаблон)

- **Q1:** Этапы A+B.
- **Q2:** Этап C + Пилот 1.
- **Q3:** Пилот 2.
- **Q4:** Пилот 3 + решения по selective storage evolution.

Если Exit Criteria этапа не выполнены, следующий квартальный шаг не стартует.

---

## 12) Управление изменениями документа

- Этот документ — каноничный implementation plan по FBA.
- Изменения в sequence/criteria вносятся только вместе с обновлением связанных ADR.
- Новые «параллельные планы FBA» не создаются; расширения добавляются сюда.
