# Документация `rustok-fulfillment`

`rustok-fulfillment` — дефолтный fulfillment-подмодуль семейства `ecommerce`.

## Что сейчас внутри

- схема `shipping_options`;
- схема `fulfillments`;
- `FulfillmentModule` и `FulfillmentService`;
- shipping boundary для checkout-цепочки `cart -> payment -> order -> fulfillment`;
- встроенный manual/default fulfillment flow без внешних carrier providers на текущем этапе.

## Архитектурная граница

- модуль не зависит от `rustok-commerce` umbrella, чтобы не создавать цикл;
- модуль не владеет заказом или customer-профилем, а только ссылается на них по идентификаторам;
- provider-specific доставка отложена в backlog и должна жить как следующий вложенный подмодуль над fulfillment boundary, а не смешиваться с базовой shipping-моделью;
- GraphQL и REST transport пока остаются в фасаде `rustok-commerce`.

## Связанные документы

- [README crate](../README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
