# Документация `rustok-payment`

`rustok-payment` — дефолтный payment-подмодуль семейства `ecommerce`.

## Что сейчас внутри

- схема `payment_collections`;
- схема `payments`;
- `PaymentModule` и `PaymentService`;
- payment boundary для checkout-цепочки `cart -> payment -> order`;
- встроенный manual/default payment flow без внешних провайдеров на текущем этапе.

## Архитектурная граница

- модуль не зависит от `rustok-commerce` umbrella, чтобы не создавать цикл;
- модуль не владеет корзиной, заказом или customer-профилем, а только ссылается на них по идентификаторам;
- provider-specific реализация вроде `stripe` отложена в backlog и должна жить как следующий вложенный подмодуль над payment boundary, а не смешиваться с базовой доменной моделью;
- GraphQL и REST transport пока остаются в фасаде `rustok-commerce`.

## Связанные документы

- [README crate](../README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
