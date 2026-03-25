# План миграции `rustok-commerce` на Medusa-подобную архитектуру

## Статус документа

Этот документ является **переносом готового исследовательского плана** в документацию
репозитория с дополнительной сверкой по текущему коду RusToK и официальной документации
Medusa v2.

Принцип документа:

- не придумывать новую целевую стратегию поверх исследования;
- не разрешать архитектурные противоречия прямо в тексте плана;
- фиксировать противоречия как **backlog согласования и внедрения**;
- держать целевую модель такой же, как в исследовании: распил `commerce` на Medusa-подобные модули.

Сверка Medusa docs выполнена **25 марта 2026 года** по официальным страницам `docs.medusajs.com`.

## Цель миграции

Цель миграции:

- трансформировать текущий `rustok-commerce` из одного крупного доменного модуля
  в набор отдельных модулей уровня `Product / Pricing / Inventory / Cart / Order / Customer /
  Payment / Fulfillment / Region / ...`;
- сохранить текущие RusToK-контракты на переходный период;
- добавить Medusa-подобный REST-слой под `/store/*` и `/admin/*`;
- сохранить модульный монолит RusToK, CQRS read-path через `rustok-index`,
  transactional outbox и tenant isolation.

## Что уже подтверждено в коде RusToK

Ниже не предположения, а факты, уже подтвержденные в репозитории:

- в `rustok-commerce` уже есть `CatalogService`, `PricingService`, `InventoryService`;
- есть REST-контроллеры внутри `crates/rustok-commerce/src/controllers/*`;
- есть GraphQL surface внутри `crates/rustok-commerce/src/graphql/*`;
- базовый write-side сегодня покрывает каталог, цены и простые остатки;
- order lifecycle частично представлен type-safe state machine, но не завершен как полноценный order module;
- read-path каталога уже завязан на `rustok-index`;
- тестовая поддержка commerce поднимает SQLite-таблицы из entities, а не проверяет production-путь миграций Postgres;
- inventory в runtime до сих пор завязан на `product_variants.inventory_quantity`,
  хотя в миграционной базе уже есть задел под нормализованную inventory-модель.

## Целевая модель из исследования

Целевой модульный состав переносится из исследования без изменения направления:

| Целевой модуль | Зона ответственности | Текущее состояние в RusToK | Целевая роль |
|---|---|---|---|
| `rustok-product` | каталог, варианты, опции, изображения, публикация | реализовано внутри `rustok-commerce` | отдельный модуль |
| `rustok-pricing` | цены, price sets, rules, price calculation | реализовано частично внутри `rustok-commerce` | отдельный модуль |
| `rustok-inventory` | остатки, availability, reservations, stock levels | реализовано частично внутри `rustok-commerce` | отдельный модуль |
| `rustok-cart` | корзина и line items | почти отсутствует | новый модуль |
| `rustok-order` | заказ, line items, lifecycle, returns | есть state machine без полного storage/API | новый модуль |
| `rustok-customer` | storefront customer domain | отсутствует как отдельный домен | новый модуль |
| `rustok-payment` | payment collections, payment providers | отсутствует | новый модуль |
| `rustok-fulfillment` | shipping options, fulfillment, tracking | отсутствует | новый модуль |
| `rustok-region` / related | регионы, валюты, locale policy | частично размазано по платформе и i18n | отдельный модуль или отдельный слой |
| `rustok-medusa-compat` | Medusa-compatible `/store/*` и `/admin/*` | отсутствует | integration layer |

Целевая композиция:

```mermaid
flowchart LR
  Client["UI / SDK / внешние клиенты"] --> Compat["rustok-medusa-compat\n/store + /admin"]
  Client --> Legacy["legacy GraphQL + /api/commerce"]

  subgraph Modules["Target commerce modules"]
    Product["rustok-product"]
    Pricing["rustok-pricing"]
    Inventory["rustok-inventory"]
    Cart["rustok-cart"]
    Order["rustok-order"]
    Customer["rustok-customer"]
    Payment["rustok-payment"]
    Fulfillment["rustok-fulfillment"]
  end

  Compat --> Product
  Compat --> Pricing
  Compat --> Inventory
  Compat --> Cart
  Compat --> Order
  Compat --> Customer
  Compat --> Payment
  Compat --> Fulfillment

  Legacy --> Product
  Legacy --> Pricing
  Legacy --> Inventory

  Product --> Outbox["rustok-outbox"]
  Pricing --> Outbox
  Inventory --> Outbox
  Cart --> Outbox
  Order --> Outbox
  Payment --> Outbox
  Fulfillment --> Outbox

  Outbox --> Index["rustok-index"]
```

## Противоречия и backlog согласования

Этот раздел фиксирует именно **противоречия**, которые нельзя замазывать формулировками.
Они должны быть закрыты отдельными задачами до или во время ранних фаз миграции.

| ID | Противоречие | Что конфликтует | Что нужно сделать |
|---|---|---|---|
| `BL-01` | Root-module topology для ecommerce family vs исходный монолитный `commerce` | исследовательский split vs старая platform topology | закрыто: принят ADR, обновлены `modules.toml`, runtime wiring и docs; дальнейший шаг — provider/capability selection поверх новой topology |
| `BL-02` | Entities vs migrations vs indexer SQL | runtime entities и production schema не полностью синхронны | провести schema hardening и добавить Postgres migration tests |
| `BL-03` | `inventory_quantity` vs inventory levels / reservations | текущая runtime-модель vs целевая Medusa-like inventory-модель | завершить переход на `stock_locations`, `inventory_items`, `inventory_levels`, `reservation_items` |
| `BL-04` | Order state machine есть, order module как продуктового контура нет | lifecycle описан, но persistence/API не завершены | достроить `rustok-order` как полноценный модуль с storage, API и integrations |
| `BL-05` | `/store/*` и `/admin/*` могут конфликтовать со встроенными UI маршрутами | target API prefix vs текущий embedded routing | определить route precedence и покрыть smoke-тестами |
| `BL-06` | Medusa-compat нужен, но scope совместимости пока не закреплен как контракт | “хотим быть совместимы” vs отсутствие точной contract boundary | зафиксировать список обязательных endpoints/semantics и гонять их против официальных docs/OpenAPI |
| `BL-07` | Customer domain в Medusa отделен от admin user, в RusToK это не разведено как commerce-модуль | Medusa customer context vs platform users/sessions | описать и внедрить storefront-customer boundary |
| `BL-08` | Checkout flow требует идемпотентности и многошаговой транзакционной целостности | cart -> payment -> order цепочка vs текущий незавершенный runtime | зафиксировать orchestration и retry/compensation policy |

## Официальные ссылки Medusa v2 для сверки

Для реализации и проверки совместимости нужно использовать **только официальные страницы Medusa**.

- [Store API Reference](https://docs.medusajs.com/api/store)
- [Admin API Reference](https://docs.medusajs.com/api/admin)
- [Store JS SDK Methods](https://docs.medusajs.com/resources/commerce-modules/store/js-sdk)
- [Publishable API Keys in Storefront](https://docs.medusajs.com/resources/storefront-development/publishable-api-keys)
- [Storefront Localization / Translation](https://docs.medusajs.com/resources/commerce-modules/translation/storefront)
- [Medusa Core Workflows Reference](https://docs.medusajs.com/resources/medusa-workflows-reference)
- [Product Module Overview](https://docs.medusajs.com/resources/commerce-modules/product)
- [API Routes Fundamentals](https://docs.medusajs.com/learn/fundamentals/api-routes)

Что именно подтверждено по этим страницам:

- Store API действительно живет под префиксом `/store`;
- Admin API действительно живет под префиксом `/admin`;
- Store API использует `x-publishable-api-key`;
- Store API поддерживает `fields`, `limit`, `offset`, `count`, `order`;
- Store API поддерживает `locale` и `x-medusa-locale`;
- Admin API описывает metadata merge semantics и auth-модели;
- Store/Admin routes в Medusa опираются на workflows.

## API-совместимость: baseline для сверки

Ниже переносится baseline из исследования. Это не “все возможные endpoints Medusa”, а минимальный
пул маршрутов, с которого должна начинаться проверяемая совместимость.

| Medusa endpoint | Метод | Целевой маршрут в RusToK | Назначение |
|---|---:|---|---|
| `/store/products` | `GET` | `/store/products` | storefront list products |
| `/store/products/{id}` | `GET` | `/store/products/{id}` | storefront product details |
| `/admin/products` | `GET` | `/admin/products` | admin list products |
| `/admin/products` | `POST` | `/admin/products` | admin create product |
| `/admin/products/{id}` | `POST` | `/admin/products/{id}` | admin update product |
| `/admin/products/{id}` | `DELETE` | `/admin/products/{id}` | admin delete/archive product |
| `/admin/products/{id}/publish` | `POST` | `/admin/products/{id}/publish` | admin publish product |
| `/store/carts` | `POST` | `/store/carts` | create cart |
| `/store/carts/{id}` | `GET` | `/store/carts/{id}` | retrieve cart |
| `/store/carts/{id}/line-items` | `POST` | `/store/carts/{id}/line-items` | add line item |
| `/store/orders` | `POST` | `/store/orders` | place order |
| `/store/orders/{id}` | `GET` | `/store/orders/{id}` | retrieve order |
| `/admin/orders` | `GET` | `/admin/orders` | admin list orders |
| `/store/customers/me` | `GET` | `/store/customers/me` | retrieve authenticated customer |
| `/store/payment-collections` | `POST` | `/store/payment-collections` | payment collection flow |
| `/store/shipping-options` | `GET` | `/store/shipping-options` | shipping options |
| `/store/regions` | `GET` | `/store/regions` | region discovery |

Обязательные semantics для сверки:

- `limit`, `offset`, `count`
- `fields`
- `metadata`
- `locale` и `x-medusa-locale`
- predictable error mapping
- publishable API key behavior для `/store/*`

## Фазовый план

### Pre-phase

Статус: `done`

Что уже сделано и подтверждено:

- собран audit текущего `rustok-commerce`;
- подтверждены текущие сервисы, маршруты, GraphQL surface, события и основные таблицы;
- подтвержден факт schema drift риска;
- подтвержден разрыв между целевой Medusa-like моделью и текущей topology платформы.

### Phase 0. Разрешение противоречий

Статус: `done`

Цель:

- не выбирать обходной путь в тексте плана;
- явно снять архитектурные противоречия из backlog `BL-01 ... BL-08`.

Задачи:

- оформить ADR по module split;
- зафиксировать, как именно новые модули входят в platform topology;
- закрепить route policy для `/store/*` и `/admin/*`;
- утвердить scope Medusa-compat;
- зафиксировать schema hardening plan;
- описать customer boundary и checkout orchestration.

Что уже сделано:

- принят ADR по split `commerce -> product / pricing / inventory`;
- принята модель `commerce` как root umbrella module семейства `ecommerce`;
- обновлены `modules.toml`, server manifest wiring и реестр модулей;
- зафиксировано, что `rustok-commerce-foundation` остается internal support crate, а не platform module.

Критерий завершения:

- backlog противоречий разобран и превращен в утвержденные решения;
- можно начинать реальный split без двусмысленностей в docs и runtime.

### Phase 1. Распил `rustok-commerce` на модули

Статус: `done (wave 1)`

Цель:

- выполнить целевой split, описанный в исследовании.

Задачи:

- выделить `rustok-product`;
- выделить `rustok-pricing`;
- выделить `rustok-inventory`;
- подготовить каркасы `rustok-cart`, `rustok-order`, `rustok-customer`,
  `rustok-payment`, `rustok-fulfillment`;
- оставить `rustok-commerce` как root umbrella module, orchestration/compatibility facade и верхний вход в ecommerce family;
- обновить registry/runtime wiring в соответствии с решением Phase 0.

Что уже сделано:

- выделены `rustok-product`, `rustok-pricing`, `rustok-inventory`;
- общий shared слой вынесен в `rustok-commerce-foundation`;
- `rustok-commerce` оставлен как umbrella/root module с compatibility surface;
- обновлены manifests, server wiring, migration aggregation и базовые module tests.

Критерий завершения:

- базовая бизнес-логика каталога, pricing и inventory больше не живет как один плотный crate;
- split отражен в коде и документации.

### Phase 2. Schema hardening и доведение доменной модели

Статус: `not started`

Цель:

- выровнять модель данных под целевую архитектуру.

Задачи:

- синхронизировать entities, migrations и indexer SQL;
- завершить inventory normalization;
- вынести pricing в отдельную storage-модель;
- ввести links/snapshots для order line items;
- утвердить единый money contract;
- подготовить backfill и switch plan.

Критерий завершения:

- production schema поднимается и проверяется через миграции;
- текущие read-model остаются корректными;
- доменная схема готова к cart/order/payment flows.

### Phase 3. Checkout и доменные модули заказа

Статус: `not started`

Цель:

- закрыть недостающий Medusa-like commerce core поверх новых модулей.

Задачи:

- реализовать `rustok-cart`;
- реализовать `rustok-order`;
- реализовать `rustok-customer`;
- реализовать каркас `rustok-payment`;
- реализовать каркас `rustok-fulfillment`;
- закрыть region/currency/locale policy;
- определить retry/idempotency/compensation policy.

Критерий завершения:

- есть рабочий путь `cart -> payment -> order`;
- order persistence и API полноценны;
- storefront flow больше не зависит от старой “каталог-only” модели.

### Phase 4. Medusa-compatible transport layer

Статус: `not started`

Цель:

- поднять `/store/*` и `/admin/*` как проверяемый слой совместимости.

Задачи:

- реализовать `rustok-medusa-compat`;
- покрыть baseline endpoints;
- поддержать `fields`, `metadata`, `pagination`, `locale`, error mapping;
- повесить слой за feature flag;
- привязать contract tests к официальным docs/OpenAPI.

Критерий завершения:

- новый REST-слой работает параллельно legacy surface;
- ключевые store/admin flows проходят contract tests.

### Phase 5. Rollout и deprecation legacy surface

Статус: `not started`

Цель:

- ввести новый контракт без слома текущих клиентов.

Задачи:

- включить telemetry по новому и старому API;
- раскатывать через feature flag / canary;
- сохранить `/api/commerce/*` и GraphQL на переходный период;
- добавить deprecation headers и migration guide;
- убрать legacy только после подтвержденного снижения использования.

Критерий завершения:

- новая API-поверхность стабильна;
- legacy surface убирается управляемо, а не “одним коммитом”.

## Тесты и release gates

Обязательный тестовый минимум из исследования:

- unit tests для product/pricing/inventory/cart/order;
- integration tests для event publication и `rustok-index`;
- Postgres migration tests;
- contract tests для `/store/*` и `/admin/*`;
- smoke tests маршрутизации;
- tenant/RBAC regression tests.

Release gates:

- нельзя считать migration phase завершенной без Postgres-проверки;
- нельзя считать Medusa-compat завершенным без сверки по официальным docs;
- нельзя выключать legacy routes без telemetry usage evidence.

## Риски

Ключевые риски переносятся из исследования без изменения:

1. `migrations != entities != indexer SQL`
2. коллизии `/admin/*` и `/store/*` с embedded UI
3. неполная Medusa-совместимость без официальной contract-сверки
4. прямые межмодульные вызовы вместо links/events
5. частичные состояния в checkout flow

## Что нужно обновлять вместе с реализацией

- `crates/rustok-commerce/README.md`
- `crates/rustok-commerce/docs/README.md`
- `docs/architecture/api.md`
- `docs/architecture/database.md`
- `docs/architecture/modules.md`
- `docs/modules/overview.md`
- `docs/modules/registry.md`
- `docs/index.md`
- ADR по module split и routing/model changes

## Критерий готовности к старту реализации

Реализацию можно начинать, когда выполнены три условия:

1. Противоречия из backlog согласования перенесены в утвержденные решения.
2. Зафиксирован baseline Medusa-compat и список обязательных routes/semantics для сверки.
3. Есть согласованный plan по schema hardening и Postgres-first validation.
