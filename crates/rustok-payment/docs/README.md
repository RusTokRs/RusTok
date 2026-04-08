# Документация `rustok-payment`

`rustok-payment` — дефолтный payment-подмодуль семейства `ecommerce`.

## Назначение

- схема `payment_collections`;
- схема `payments`;
- `PaymentModule` и `PaymentService`;
- payment boundary для checkout-цепочки `cart -> payment -> order`;
- встроенный manual/default payment flow без внешних провайдеров на текущем этапе.

## Зона ответственности

- модуль не зависит от `rustok-commerce` umbrella, чтобы не создавать цикл;
- модуль не владеет корзиной, заказом или customer-профилем, а только ссылается на них по идентификаторам;
- provider-specific реализация вроде `stripe` отложена в backlog и должна жить как следующий вложенный подмодуль над payment boundary, а не смешиваться с базовой доменной моделью;
- GraphQL и REST transport пока остаются в фасаде `rustok-commerce`.

## Интеграция

- модуль входит в ecommerce family и должен сохранять собственную storage/runtime-границу без возврата ответственности в umbrella ustok-commerce;
- transport, GraphQL и UI-поверхности публикуются через ustok-commerce, пока для домена не зафиксирован отдельный module-owned surface;
- изменения cross-module контракта нужно синхронизировать с ustok-commerce и соседними split-модулями.

## Проверка

- cargo xtask module validate payment
- cargo xtask module test payment
- targeted commerce tests для payment-домена при изменении runtime wiring
## Связанные документы

- [README crate](../README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
