# План реализации `rustok-commerce`

## Статус документа

Этот документ фиксирует актуальный roadmap umbrella-модуля `rustok-commerce` после отказа от legacy REST surface `/api/commerce/*`.

Исходные предпосылки:

- live REST-контракт для ecommerce живёт на `/store/*` и `/admin/*`;
- GraphQL остаётся поддерживаемым transport-слоем;
- `rustok-commerce` продолжает играть роль root umbrella module для ecommerce family;
- постепенный split на `cart/customer/product/region/pricing/inventory/order/payment/fulfillment` продолжается.

## Цели

- довести Medusa-style REST transport до production-grade состояния;
- выносить устойчивые доменные области из umbrella-модуля в отдельные crates;
- держать GraphQL и REST над одними и теми же application services;
- стабилизировать checkout, cart context и order/payment/fulfillment orchestration;
- сохранять tenant isolation, outbox/event flow и index-backed read paths.

## Что уже подтверждено в коде

- `rustok-commerce` уже содержит `CatalogService`, `PricingService`, `InventoryService`, `CheckoutService`, `StoreContextService`;
- storefront и admin REST routes живут внутри `crates/rustok-commerce/src/controllers/*`;
- GraphQL surface живёт внутри `crates/rustok-commerce/src/graphql/*`;
- cart snapshot уже хранит storefront context;
- checkout path использует `checking_out`, reuse payment collection и recovery semantics;
- checkout reuse-ит pre-created cart payment collection, вместо создания дублирующего payment record на шаге `complete`;
- guest checkout разрешён для guest cart без обязательного auth context, при этом customer-owned cart остаётся auth-gated;
- `apps/server` остаётся thin host-слоем для route/OpenAPI/schema composition;
- legacy `/api/commerce/*` удалён из live router, OpenAPI и контрактных тестов.

## Backlog противоречий

| ID | Противоречие | Что нужно сделать |
| --- | --- | --- |
| `BL-01` | umbrella module vs дальнейший split | продолжать вынос устойчивых bounded contexts в отдельные crates |
| `BL-02` | entities vs migrations vs indexer SQL | держать schema hardening и Postgres-first tests обязательными |
| `BL-03` | inventory model hardening | продолжать выравнивание read/write path вокруг stock locations, levels и reservations |
| `BL-04` | order/payment/fulfillment transport | довести transport/API поверх уже выделенных модулей |
| `BL-05` | `/admin/*` и `/store/*` vs embedded UI routes | держать route precedence под smoke tests |
| `BL-06` | Medusa parity scope | расширять contract tests по официальным Medusa docs, не inventing local semantics |

## Фазы

### Phase 1. Module topology и contracts

Статус: `done`

- `rustok-commerce` закреплён как umbrella/root module;
- базовый split на профильные crates выполнен;
- shared DTO/entities/errors вынесены в `rustok-commerce-foundation`.

### Phase 2. Medusa-style transport baseline

Статус: `done`

- live REST surface поднят на `/store/*` и `/admin/*`;
- реализованы storefront routes `products`, `regions`, `shipping-options`, `carts`, `payment-collections`, `orders/{id}`, `customers/me`;
- реализованы admin routes для `products`;
- OpenAPI и route contract tests привязаны к live surface без legacy compatibility layer.

### Phase 3. Cart context и checkout hardening

Статус: `in progress`

Фокус:

- удерживать cart как source of truth для storefront context;
- развивать checkout recovery/idempotency semantics;
- закрывать race conditions на `payment-collections` и `complete checkout`;
- держать transport response shape стабильным.

Обязательные проверки:

- migration tests для cart context schema;
- integration tests `create cart -> update context -> add line item -> shipping options -> payment collection -> complete`;
- negative tests на `currency_code` vs `region_id`;
- auth/customer ownership tests;
- contract tests store cart endpoints.

Что уже закрыто в текущем срезе:

- transport coverage подтверждает, что cart context остаётся source of truth для `shipping-options`, `payment-collections` и `checkout`;
- transport coverage закрывает `currency_code` vs `region_id`, guest/customer ownership и сквозной storefront checkout flow;
- service coverage подтверждает reuse уже существующего cart-bound payment collection во время `complete checkout`.

### Phase 4. Order/payment/fulfillment transport

Статус: `in progress`

Фокус:

- расширить admin/store transport поверх уже выделенных модулей;
- зафиксировать response shape и lifecycle semantics;
- продолжить parity между REST и GraphQL над общими сервисами.

Что уже закрыто в текущем срезе:

- добавлен первый admin order transport endpoint `GET /admin/orders/{id}`;
- добавлен paginated admin orders list endpoint `GET /admin/orders` с базовыми filters `status` и `customer_id`;
- admin order detail отдаёт order вместе с latest payment collection и latest fulfillment;
- добавлены explicit admin order lifecycle endpoints: `mark-paid`, `ship`, `deliver`, `cancel`;
- добавлены admin detail/lifecycle endpoints для `payment-collections` (`show`, `authorize`, `capture`, `cancel`) и `fulfillments` (`show`, `ship`, `deliver`, `cancel`);
- transport/OpenAPI coverage фиксирует RBAC и schema contract для admin order detail и admin payment/fulfillment lifecycle surface.

### Phase 5. Упрощение umbrella-модуля

Статус: `in progress`

Фокус:

- удалять dead transport, compatibility remnants и дублирующий код без оглядки на несуществующий migration period;
- держать `rustok-commerce` как orchestration/root layer, а не как склад исторических adapter-ов;
- переносить оставшиеся устойчивые области в профильные crates.

Что уже сделано:

- удалён legacy REST surface `/api/commerce/*`;
- удалены rollout/deprecation middleware, settings, runtime guardrails и operator scripts, которые имели смысл только для legacy cutover;
- OpenAPI и route tests переведены на live `/store/*` + `/admin/*` contract.

## Тесты и release gates

Обязательный минимум:

- unit tests для product/pricing/inventory/cart/order;
- integration tests для event publication и `rustok-index`;
- Postgres migration tests;
- contract tests для `/store/*` и `/admin/*`;
- parity tests `REST <-> GraphQL`;
- router/OpenAPI smoke tests;
- tenant/RBAC regression tests.

Release gates:

- нельзя считать Medusa-style transport стабильным без contract tests против live `/store/*` и `/admin/*`;
- нельзя расширять checkout flow без migration/integration coverage;
- нельзя тащить обратно legacy compatibility surface ради удобства локальной разработки.

## Что обновлять вместе с кодом

- `crates/rustok-commerce/README.md`
- `crates/rustok-commerce/docs/README.md`
- `docs/architecture/api.md`
- `docs/index.md`
- модульные docs по вынесенным crates
- ADR, если меняется module topology или transport contract
