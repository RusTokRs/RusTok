# План реализации Fluid Backend Architecture для RusTok

Этот документ развивает концепцию [Fluid Backend Architecture](./fluid-backend-architecture.md)
и фиксирует исследование того, как переносить module-owned backend contracts между
modular monolith и remote `server-grpc` topology без превращения RusTok в преждевременный
набор микросервисов.

Документ является планом и исследовательской картой, а не финальным ADR. Любой фактический
переход конкретного модуля в remote/service-owned storage topology должен оформляться
отдельным ADR в `DECISIONS/` и обновлением local docs соответствующего модуля.

## Краткий вывод

Да, паттерн FFA можно симметрично применить к backend-у, но реализация должна идти через
**FBA-ready modular monolith**, а не через «сразу микросервисы».

Правильная формула для RusTok:

```text
один module-owned service contract
+ несколько runtime adapters
+ явная orchestration model
= fluid backend
```

Неправильная формула:

```text
каждый crate = отдельный микросервис
```

Причина: часть модулей уже достаточно изолирована для service boundary, но часть модулей
сейчас является orchestration/facade, shared foundation или strongly consistent write path.
Их нельзя выносить как remote service без предварительной работы над contract-ами, events,
context propagation, idempotency и consistency model.

## Пересмотр концепта FBA

### Что именно должно стать fluid

В FBA переносится не база данных, не HTTP endpoint и не конкретный crate. Переносимым
становится **module-owned application service boundary**:

```text
GraphQL / REST / #[server] / jobs / CLI
        ↓
module service port
        ↓
in-process implementation или remote gRPC client
        ↓
module-owned domain/runtime
```

Это означает:

- public API surface остаётся стабильным для UI и интеграций;
- module identity, ownership и business semantics не меняются;
- transport selection живёт в composition/runtime layer;
- `server-grpc` adapter не содержит business rules;
- remote mode не имеет права вводить второй policy/auth/tenant contract.

### Три уровня, которые нельзя смешивать

| Уровень | Вопрос | Кто владеет | FBA-правило |
|---|---|---|---|
| Module identity | Что это за модуль и чем он владеет? | `modules.toml`, `rustok-module.toml`, local docs | Не меняется между embedded и remote mode |
| Service contract | Какие команды/запросы и события поддерживает модуль? | module crate | Становится canonical boundary |
| Runtime topology | Где выполняется реализация? | host composition + deployment | Может быть in-process, remote gRPC или hybrid |

Если gRPC schema начинает определять domain model вместо module service contract-а, FBA
ломается: transport становится владельцем архитектуры.

### Что не является целью

FBA не должна:

- заставлять все модули становиться микросервисами;
- заменять GraphQL/REST/`#[server]` contracts для UI;
- переносить business logic в gRPC handlers;
- обещать distributed transactions без saga/outbox design;
- делать host app владельцем module-owned domain logic;
- разрешать прямой SQL-доступ к storage remote-owned модуля из других сервисов.

## Текущее состояние модульной изоляции

### Наблюдения по `modules.toml`

Сейчас ecommerce family описана как набор optional modules и umbrella-модуль
`commerce`, который зависит от `cart`, `customer`, `product`, `region`, `pricing`,
`inventory`, `order`, `payment` и `fulfillment`.

Ключевые dependency edges:

```text
commerce → cart
commerce → customer
commerce → product
commerce → region
commerce → pricing → product
commerce → inventory → product
commerce → order
commerce → payment
commerce → fulfillment
product  → taxonomy
```

Отдельно важно: `rustok-tax` существует как crate и используется `rustok-cart`, но не
объявлен как platform module в `modules.toml`. Для FBA это означает, что `tax` пока лучше
читать как support/capability service boundary, а не как полноценный tenant-toggled module,
пока не будет отдельного module contract-а.

### Наблюдения по Cargo-зависимостям ecommerce crate-ов

Фактические Cargo edges показывают несколько типов связности:

- `rustok-commerce` является плотным facade/orchestration crate: он зависит почти от всей
  ecommerce family и поэтому сам не должен становиться domain microservice.
- `rustok-pricing` напрямую зависит от `rustok-product`, что усложняет вынос pricing без
  product port/read-model.
- `rustok-cart` зависит от `rustok-fulfillment` и `rustok-tax`, но по docs уже старается
  хранить product/variant references как snapshots, а не как hard foreign ownership.
- `rustok-product`, `rustok-pricing`, `rustok-inventory` и `rustok-order` уже используют
  `rustok-events`/`rustok-outbox`, что является хорошей предпосылкой для async integration.
- `rustok-payment` и `rustok-fulfillment` выглядят более автономными по dependency graph,
  но им нужны connector/provider contracts, idempotency и event semantics до remote mode.

### Оценка FBA-готовности ecommerce modules

| Модуль / crate | Текущая роль | Текущая изоляция | Рекомендуемый FBA-профиль | Комментарий |
|---|---|---|---|---|
| `commerce` | umbrella, API facade, orchestration | Низкая как service, высокая как composition layer | Оставить in-process facade/orchestrator | Должен выбирать adapters и собирать checkout/order flows, но не владеть storage соседей |
| `product` | catalog, variants, translations, publication | Средняя | Сначала remote read-side, позже write service | Есть связи с `taxonomy`, `flex`, SEO targets и outbox; write path лучше не выносить первым |
| `pricing` | price lists, effective price, discounts | Средняя | Хороший кандидат для query/compute gRPC после product port | Нужно убрать прямую необходимость читать product internals без typed product catalog port |
| `inventory` | stock levels, adjustments, availability | Средняя/высокая | Кандидат для reservation service | Требуются idempotent reservations, deadlines и компенсации |
| `cart` | transient cart, totals, checkout input | Средняя | На первом этапе in-process; remote позже | Слишком chatty для раннего gRPC; важнее стабилизировать cart service port и snapshots |
| `order` | order write model, post-order operations | Средняя | In-process до зрелой saga; remote позже | Order должен стать ledger/snapshot owner; remote extraction требует outbox + compensation model |
| `payment` | payment collections/provider operations | Средняя/высокая | Хороший remote adapter candidate | Внешние provider calls уже похожи на service boundary; нужен строгий idempotency key contract |
| `fulfillment` | shipping options, fulfillment lifecycle | Средняя/высокая | Хороший remote adapter candidate | Подходит для provider/warehouse integrations; checkout должен работать через typed shipping quote port |
| `customer` | customer profile/accounts boundary | Средняя | Обычно in-process до auth/profile boundary maturity | Зависимость от `profiles`; sensitive data и auth linkage требуют осторожности |
| `region` | regions, currency/country config | Высокая для reads | Remote read service не нужен на старте | Лучше держать как in-process reference/config service и кэшировать snapshots |
| `tax` | tax calculation support crate | Средняя | Candidate для stateless tax engine adapter | Сначала formalize as service port; решать отдельно, должен ли стать platform module |

## Предлагаемые service boundaries для ecommerce

### 1. Commerce Orchestrator

`rustok-commerce` не должен становиться микросервисом в обычном смысле. Его целевая роль в
FBA — **orchestration/facade layer**:

```text
UI / GraphQL / REST / #[server]
        ↓
rustok-commerce facade
        ↓
CommerceOrchestrator
        ├─ CartPort
        ├─ PricingPort
        ├─ InventoryPort
        ├─ TaxPort
        ├─ PaymentPort
        ├─ FulfillmentPort
        └─ OrderPort
```

Этот слой:

- принимает public API request;
- нормализует tenant/auth/channel/locale context;
- выбирает in-process или gRPC adapter для каждого port-а;
- управляет checkout saga;
- публикует cross-module events через outbox;
- не читает напрямую storage соседних модулей, если модуль уже объявлен remote-owned.

### 2. Product Catalog Service

Целевая граница:

```text
ProductCatalogPort
├─ get_product_snapshot(product_id, locale, channel)
├─ get_variant_snapshot(variant_id, locale, channel)
├─ list_publishable_catalog_page(...)
└─ emit product/variant changed events
```

Рекомендация: не выносить product write path первым. Начать с read-side contract-а и
product snapshot DTO, который pricing/cart/search/order могут использовать без прямого
доступа к product internals.

### 3. Pricing Service

Целевая граница:

```text
PricingPort
├─ price_variant(variant_id, region_id, channel_id, customer_context)
├─ price_cart(lines, region_id, channel_id, customer_context)
└─ preview_discount(scope, adjustment_input)
```

Pricing можно вынести после того, как `product` предоставит typed catalog snapshot/read
port. Если pricing продолжит зависеть от product crate internals, remote mode станет
формальным, а не fluid.

### 4. Inventory Reservation Service

Целевая граница:

```text
InventoryPort
├─ check_availability(items, location/seller/channel)
├─ reserve(items, idempotency_key, expires_at)
├─ commit_reservation(reservation_id)
└─ release_reservation(reservation_id)
```

Это один из лучших кандидатов на `server-grpc`, потому что:

- availability checks часто масштабируются отдельно;
- reservation semantics естественно выражаются через idempotent commands;
- failures можно компенсировать release/cancel шагами.

Но до extraction нужны:

- reservation table/model;
- stable idempotency key;
- deadline/expiration;
- event contract `InventoryReserved`, `InventoryReservationReleased`, `InventoryAdjusted`.

### 5. Cart Service

Cart лучше не выносить первым: он chatty, сильно зависит от storefront context, shipping,
tax и pricing snapshots. Целевой шаг — сначала сделать CartPort transport-agnostic:

```text
CartPort
├─ get_cart(cart_id, context)
├─ add_line_item(cart_id, item, context)
├─ set_quantity(cart_id, line_id, quantity, context)
├─ apply_adjustment(cart_id, adjustment, context)
└─ start_checkout(cart_id, context)
```

Remote cart имеет смысл только после стабилизации snapshot contract-а для product, pricing,
tax и shipping.

### 6. Tax Service

Сейчас `rustok-tax` больше похож на support service, чем на platform module. Для FBA лучше
сначала оформить его как explicit port:

```text
TaxPort
├─ quote_cart_tax(cart_snapshot, region, shipping_address)
├─ quote_line_tax(line_snapshot, region, address)
└─ explain_tax_lines(...)
```

Потом можно решить, должен ли tax стать:

- in-process support crate;
- remote stateless calculation service;
- provider adapter layer для внешних tax engines;
- полноценный optional module с tenant-level enablement.

### 7. Payment Service

Payment — хороший кандидат на remote adapter, но не на преждевременное service-owned order
state. Целевой contract:

```text
PaymentPort
├─ create_payment_collection(order_or_cart_snapshot, idempotency_key)
├─ authorize(collection_id, payment_method, idempotency_key)
├─ capture(payment_id, amount, idempotency_key)
├─ refund(payment_id, amount, reason, idempotency_key)
└─ cancel_authorization(payment_id, idempotency_key)
```

Правило: payment service владеет payment lifecycle, но не order lifecycle. Order хранит
payment snapshot/status через events и explicit command results.

### 8. Fulfillment Service

Fulfillment подходит для remote profile из-за provider/warehouse integrations:

```text
FulfillmentPort
├─ quote_shipping(cart_or_order_snapshot, address, channel)
├─ create_fulfillment(order_snapshot, idempotency_key)
├─ cancel_fulfillment(fulfillment_id, reason)
└─ track(fulfillment_id)
```

Для checkout это означает: cart/order не вычисляют shipping напрямую, а получают shipping
quote/snapshot от fulfillment boundary.

### 9. Order Service

Order — критичный write model. Его можно готовить к FBA, но не выносить первым.

Целевая роль:

- владелец order ledger/snapshot;
- source of truth для order state machine;
- consumer payment/fulfillment/inventory events;
- publisher order lifecycle events.

Remote extraction order service требует зрелой saga model. До этого безопаснее держать
order in-process, но уже запретить соседям писать order tables напрямую.

## Cross-module orchestration

### Checkout saga как первый большой orchestration-кейс

Предлагаемый high-level flow:

```text
CheckoutStarted
  ↓
1. CartPort.get_cart_snapshot
  ↓
2. PricingPort.price_cart
  ↓
3. TaxPort.quote_cart_tax
  ↓
4. FulfillmentPort.quote_shipping
  ↓
5. InventoryPort.reserve
  ↓
6. OrderPort.create_pending_order
  ↓
7. PaymentPort.authorize
  ↓
8. OrderPort.mark_payment_authorized
  ↓
9. InventoryPort.commit_reservation
  ↓
10. FulfillmentPort.create_fulfillment_request
  ↓
OrderPlaced
```

Компенсации:

| Шаг, который упал | Компенсация |
|---|---|
| payment authorization после inventory reserve | `InventoryPort.release_reservation` |
| order create после inventory reserve | release reservation + cancel payment authorization, если был payment |
| fulfillment request после payment authorize | order остаётся paid/unfulfilled, fulfillment retry через outbox |
| commit reservation после payment authorize | manual review / compensation policy, потому что payment уже authorized |

Важное правило: не каждый шаг должен быть synchronous gRPC. Например, fulfillment retry,
email, analytics и search indexing должны идти через events/outbox.

### Read orchestration vs write orchestration

| Тип orchestration | Пример | Transport по умолчанию |
|---|---|---|
| Read composition | product page, cart summary, admin order detail | in-process или gRPC query с cache/read-model |
| Strong write command | checkout, refund, inventory adjustment | in-process сначала; gRPC только с idempotency/deadline |
| Async projection | search index, analytics, recommendations | outbox/events |
| External integration | payment provider, shipping provider, tax provider | service adapter, часто remote-capable |

## Event contract для FBA

FBA не должна строиться только на RPC. Минимальный event vocabulary для ecommerce:

| Событие | Publisher | Consumers |
|---|---|---|
| `ProductPublished` / `ProductUpdated` | product | search, index, pricing cache, storefront projections |
| `PriceListUpdated` / `PriceChanged` | pricing | cart repricing, search/index projections |
| `InventoryAdjusted` | inventory | storefront availability, admin alerts, index/search |
| `InventoryReserved` / `InventoryReservationReleased` | inventory | order, analytics, operations |
| `CartUpdated` | cart | analytics, abandoned cart workflows |
| `OrderPlaced` | order | payment, fulfillment, email, analytics, search/index |
| `PaymentAuthorized` / `PaymentCaptured` / `RefundCreated` | payment | order, accounting/reporting, email |
| `FulfillmentCreated` / `ShipmentDispatched` | fulfillment | order, email, customer UI |
| `TaxQuoted` / `TaxCommitted` | tax | order audit/reporting |

Для каждого event-а нужны:

- stable event type;
- tenant id;
- aggregate id;
- causation/correlation id;
- schema version;
- idempotency/replay semantics;
- no localized display text in payload, если это business event.

## Context envelope для gRPC и events

FBA требует единый context envelope. Минимальный состав:

```text
FbaContext
├─ tenant_id
├─ actor/principal/service_identity
├─ authz claims или policy snapshot reference
├─ channel_id / channel slug / resolution source, если применимо
├─ locale / effective locale, если операция locale-aware
├─ request_id
├─ correlation_id
├─ causation_id
├─ traceparent / tracestate
├─ idempotency_key, если command retry-safe
├─ deadline_ms или absolute deadline
└─ runtime profile: embedded | grpc | worker | test
```

Правило: этот envelope должен быть typed contract-ом в shared crate, а не набором
ad-hoc metadata keys внутри каждого gRPC adapter-а.

## Service registry и adapter selection

Целевая runtime-схема:

```text
apps/server
  ↓
FbaServiceRegistry
  ├─ product: InProcess(ProductService)
  ├─ pricing: Grpc(PricingClient)
  ├─ inventory: Grpc(InventoryClient)
  ├─ payment: InProcess(PaymentService)
  └─ fulfillment: Grpc(FulfillmentClient)
```

### Требования к registry

1. Adapter selection должен быть runtime/build profile, а не `if remote` внутри business logic.
2. Каждый port должен иметь in-process implementation для local/dev/test profile.
3. Remote client должен реализовывать тот же trait/port, что и in-process service.
4. Health/readiness host-а должны учитывать обязательность remote dependency.
5. Optional remote service должен иметь degradation policy: fail closed, cached read-only,
   queued command или disabled feature.
6. Version/capability negotiation нужна до принятия request path traffic.

### Manifest extension как будущий шаг

В будущем можно расширить `rustok-module.toml` описанием remote-capable profiles, например:

```toml
[provides.backend_service]
port = "inventory::InventoryPort"
default_topology = "in_process"
remote_transports = ["grpc"]
required_context = ["tenant", "actor", "trace", "idempotency"]

[provides.backend_service.grpc]
crate = "rustok-inventory-grpc"
service = "rustok.inventory.v1.InventoryService"
```

Это не нужно делать первым шагом. Сначала важнее стабилизировать code-level ports и
context envelope.

## План реализации

### Этап 0. Инвентаризация boundaries

Цель: понять, где business logic сейчас живёт в service layer, а где она размазана между
GraphQL/REST/controllers/host wiring.

Работы:

1. Для каждого ecommerce модуля описать local docs section `Service boundary`.
2. Выписать команды, запросы, события, owned tables и foreign references.
3. Отметить прямые cross-crate calls и прямой доступ к чужим tables/entities.
4. Выделить synchronous path и async/event path.
5. Пометить модуль как `embedded-only`, `fba-ready`, `grpc-candidate`, `event-only-candidate`.

Deliverables:

- matrix FBA readiness по модулям;
- список missing ports;
- список direct coupling, который блокирует remote mode.

### Этап 1. Shared FBA contracts

Цель: дать общий язык для embedded и remote mode.

Работы:

1. Создать или расширить shared contract crate для:
   - `FbaContext`;
   - `ServiceIdentity`;
   - `IdempotencyKey`;
   - `Deadline`;
   - `CorrelationContext`;
   - common service error envelope.
2. Зафиксировать mapping domain errors → GraphQL/REST/gRPC status.
3. Добавить tracing/correlation propagation helpers.
4. Описать policy: какие поля context обязательны для read, write и admin operations.

Deliverables:

- shared Rust types;
- docs по context propagation;
- targeted tests на error/status mapping.

### Этап 2. Ports before transports

Цель: вынести business boundary в traits/ports до появления gRPC.

Порядок для ecommerce:

1. `InventoryPort` — лучший первый write-side candidate.
2. `PricingPort` — query/compute boundary, но после product snapshot port.
3. `PaymentPort` — provider-oriented command boundary.
4. `FulfillmentPort` — provider/warehouse boundary.
5. `ProductCatalogPort` — сначала read snapshot.
6. `CartPort` и `OrderPort` — позже, после checkout saga design.

Правило: каждый port сначала должен работать in-process и иметь tests без gRPC.

Deliverables:

- module-owned port traits;
- in-process adapters;
- contract tests, которые затем можно запускать против gRPC client/server.

### Этап 3. Event vocabulary и outbox-first integration

Цель: отделить remote service extraction от synchronous RPC-only мышления.

Работы:

1. Формализовать ecommerce event vocabulary.
2. Добавить schema version и replay/idempotency правила.
3. Привязать event publishers к module-owned write transactions.
4. Проверить, что search/index/analytics/email/fulfillment retries идут через outbox, а не через host shortcuts.

Deliverables:

- event contract docs;
- outbox publishing tests;
- consumer registration map через module registry.

### Этап 4. Pilot 1: read/async service boundary

Рекомендуемый первый pilot: `search`/`index` или `media`/AI enrichment, а не checkout.

Почему:

- lower consistency risk;
- event-driven ingestion уже естественен;
- remote failure можно деградировать в cached/read-only/disabled UX;
- меньше вероятность сломать core checkout revenue path.

Deliverables:

- один service port;
- optional gRPC adapter;
- health/readiness;
- contract tests против in-process и gRPC mode.

### Этап 5. Pilot 2: inventory reservation service

Inventory — первый хороший ecommerce write-side pilot, но только после ports/events.

Работы:

1. Ввести reservation commands и idempotency.
2. Описать expiration/deadline semantics.
3. Добавить compensation commands.
4. Подключить checkout orchestrator к `InventoryPort`, а не к inventory internals.
5. Добавить `rustok-inventory-grpc` как adapter crate.

Deliverables:

- in-process inventory reservation;
- gRPC adapter с теми же contract tests;
- failure-mode tests: retry, timeout, duplicate idempotency key, release after failure.

### Этап 6. Commerce checkout saga

Цель: сделать orchestration explicit до выноса order/cart.

Работы:

1. Ввести `CommerceOrchestrator` как thin orchestration layer.
2. Перевести checkout на ports: cart, pricing, tax, fulfillment, inventory, order, payment.
3. Описать compensation table.
4. Разделить synchronous steps и async after-commit steps.
5. Добавить saga correlation id в events и logs.

Deliverables:

- checkout saga docs;
- integration tests для happy path и failure paths;
- metrics по каждому step latency/error.

### Этап 7. Payment/Fulfillment remote adapters

Payment и fulfillment можно выносить после orchestration, потому что они provider-oriented
и естественно работают через command/results.

Работы:

1. Idempotency для authorize/capture/refund/create fulfillment.
2. Provider error taxonomy.
3. Retry policy и DLQ для async follow-ups.
4. gRPC adapters как optional runtime profile.

Deliverables:

- `rustok-payment-grpc` и/или `rustok-fulfillment-grpc` adapter crates;
- contract tests;
- provider failure runbook.

### Этап 8. Product/Pricing remote read-side

Цель: вынести heavy catalog/pricing reads без выноса всех writes.

Работы:

1. Product snapshot DTO и product read port.
2. Pricing query port, который не лезет в product internals.
3. Cache/read-model invalidation через product/price events.
4. Optional remote query clients.

Deliverables:

- catalog/pricing read contracts;
- cache invalidation tests;
- GraphQL/REST parity через те же ports.

### Этап 9. Решение по service-owned storage

Только после предыдущих этапов можно решать, какие модули получают отдельную DB/storage
boundary.

Кандидаты:

- search/index projections;
- media processing metadata/blob provider boundary;
- inventory reservations/stock ledger;
- payment provider ledger;
- analytics/reporting.

Не начинать с:

- tenant/auth/RBAC;
- cart/order checkout write path;
- commerce umbrella;
- region/reference config.

Для каждого кандидата нужен отдельный ADR с rollback plan.

## Проверка и quality gates

Минимальные gates для каждого FBA candidate:

1. `cargo xtask module validate <slug>`.
2. `cargo xtask module test <slug>` или targeted module tests.
3. Contract tests для port-а, запускаемые против in-process и gRPC adapter.
4. Error mapping tests: domain error → gRPC status → public API error.
5. Context propagation tests: tenant/auth/locale/channel/trace/idempotency.
6. Retry/idempotency tests для remote commands.
7. Outbox/event replay tests для async consumers.
8. Health/readiness tests для required и optional remote dependencies.
9. Documentation updates: local docs модуля + central FBA docs/index.

## Риски и решения

| Риск | Почему опасно | Решение |
|---|---|---|
| Distributed monolith | Модули remote, но всё ещё читают чужие internals | Ports, anti-corruption DTOs, запрет direct SQL к remote-owned tables |
| Chatty gRPC | Cart/product UI может породить десятки RPC на request | batch query ports, read models, DataLoader/cache layer |
| Разные business rules | In-process и gRPC paths начинают расходиться | contract tests против обоих adapters |
| Потеря tenant isolation | Remote service доверяет raw headers | typed `FbaContext`, signed/internal service identity, policy checks на service side |
| Слом checkout при timeout | Sync RPC без compensation | deadline, idempotency, saga compensation, outbox retries |
| Слишком ранняя service-owned DB | Нет consistency story | staged extraction: shared DB remote → events/read-model → service-owned DB через ADR |
| Host становится god-service | Orchestrator начинает владеть domain rules | host/commerce facade только координирует ports, business rules остаются в modules |

## Рекомендуемая последовательность для ecommerce

Практический порядок, который минимизирует риск:

1. **Не трогать topology**: сначала зафиксировать ports и context envelope.
2. **Выделить ProductCatalogSnapshot**: убрать необходимость читать product internals из pricing/cart/order.
3. **Inventory reservation**: первый write-side FBA pilot.
4. **Payment/Fulfillment ports**: подготовить provider-oriented boundaries.
5. **Commerce checkout saga**: сделать orchestration явной и тестируемой.
6. **Pricing remote query**: после product snapshot и cache invalidation.
7. **Product remote read-side**: только read-heavy paths, write path оставить embedded.
8. **Cart/Order remote mode**: последний этап, после saga maturity и event replay.

## Что считать успехом

FBA можно считать реально реализованной, когда хотя бы один ecommerce module service:

- имеет module-owned port;
- имеет in-process implementation;
- имеет gRPC adapter без business logic;
- проходит одинаковые contract tests в обоих режимах;
- переносит `FbaContext` без ad-hoc headers;
- публикует/потребляет события через outbox;
- может быть включён remote в `apps/server` без изменения GraphQL/REST/Leptos UI contract-а.

## Связанные документы

- [Fluid Backend Architecture для RusTok](./fluid-backend-architecture.md)
- [Fluid Frontend Architecture для RusTok](./fluid-frontend-architecture.md)
- [Архитектура модулей](../architecture/modules.md)
- [API и surface-контракты](../architecture/api.md)
- [Контракт event flow](../architecture/event-flow-contract.md)
- [DataLoader](../architecture/dataloader.md)
- [Реестр модулей и приложений](../modules/registry.md)
- [Как писать модуль в RusToK](../modules/module-authoring.md)
- [Документация `rustok-commerce`](../../crates/rustok-commerce/docs/README.md)
- [Документация `rustok-cart`](../../crates/rustok-cart/docs/README.md)
- [Документация `rustok-product`](../../crates/rustok-product/docs/README.md)
- [Документация `rustok-pricing`](../../crates/rustok-pricing/docs/README.md)
- [Документация `rustok-inventory`](../../crates/rustok-inventory/docs/README.md)
- [Документация `rustok-order`](../../crates/rustok-order/docs/README.md)
- [Документация `rustok-payment`](../../crates/rustok-payment/docs/README.md)
- [Документация `rustok-fulfillment`](../../crates/rustok-fulfillment/docs/README.md)
