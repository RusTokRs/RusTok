# Распил `rustok-commerce` на `product`, `pricing` и `inventory`

- Date: 2026-03-25
- Status: Accepted & Implemented

## Context

Исследовательский migration plan для Medusa-подобной модели требовал перестать трактовать `rustok-commerce`
как единый модуль каталога, pricing и inventory. Это противоречило фактической структуре кода, где:

- `CatalogService`, `PricingService` и `InventoryService` жили в одном crate;
- runtime wiring и `modules.toml` не отражали отдельные platform modules;
- документация одновременно требовала split и описывала `commerce` как единый optional module.

Без устранения этого противоречия нельзя было честно двигаться дальше к cart/order/customer/payment slices и к
Medusa-compatible API surface.

## Decision

Принять и реализовать первый этап split прямо в коде:

- выделить общий support crate `rustok-commerce-foundation` для shared DTO, entities, errors и search helpers;
- выделить `rustok-product` как отдельный optional platform module для каталога, вариантов, опций и публикации;
- выделить `rustok-pricing` как отдельный optional platform module для pricing slice;
- выделить `rustok-inventory` как отдельный optional platform module для inventory slice;
- оставить `rustok-commerce` как переходный facade совместимости:
  - re-export shared contracts и extracted services;
  - держать legacy GraphQL/REST transport surface;
  - держать order state machine и legacy migrations, еще не вынесенные в отдельные модули.

Runtime manifest и server module registry должны регистрировать `product`, `pricing`, `inventory` и `commerce`
как отдельные optional modules, при этом `commerce` зависит от `product`, `pricing`, `inventory`.

## Consequences

Положительные:

- противоречие между исследовательским планом и фактической модульной топологией устранено;
- platform/runtime wiring теперь отражает реальную декомпозицию commerce-контура;
- следующий этап split можно продолжать из честной базы, а не из facade-монолита.

Отрицательные:

- `rustok-commerce` пока остается переходным фасадом и все еще несет transport/API surface;
- collections/categories/order-related части еще не выделены в отдельные модули;
- inventory schema/model все еще требует дальнейшей нормализации по backlog migration plan.

Follow-up:

- довести `cart`, `order`, `customer`, `payment`, `fulfillment` до отдельных модулей;
- вынести remaining legacy migrations и transport surfaces из `rustok-commerce`;
- продолжить schema hardening и Medusa-compatible API contract tests.
