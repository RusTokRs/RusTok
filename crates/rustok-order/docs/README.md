# Документация `rustok-order`

`rustok-order` — дефолтный order-подмодуль семейства `ecommerce`.

## Назначение

- схема `orders` и `order_line_items`;
- `OrderModule` и `OrderService`;
- write-side lifecycle заказа: `pending -> confirmed -> paid -> shipped -> delivered/cancelled`;
- публикация order events через transactional outbox.

## Зона ответственности

- модуль не зависит от `rustok-commerce` umbrella, чтобы не создавать цикл;
- product/variant ссылки в заказе хранятся как snapshot references, а не как обязательные cross-module foreign keys;
- GraphQL и REST transport пока остаются в фасаде `rustok-commerce`.

## Контракты событий

- [Event flow contract (central)](../../../docs/architecture/event-flow-contract.md)

## Интеграция

- модуль входит в ecommerce family и должен сохранять собственную storage/runtime-границу без возврата ответственности в umbrella ustok-commerce;
- transport, GraphQL и UI-поверхности публикуются через ustok-commerce, пока для домена не зафиксирован отдельный module-owned surface;
- изменения cross-module контракта нужно синхронизировать с ustok-commerce и соседними split-модулями.

## Проверка

- cargo xtask module validate order
- cargo xtask module test order
- targeted commerce tests для order-домена при изменении runtime wiring
## Связанные документы

- [README crate](../README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
