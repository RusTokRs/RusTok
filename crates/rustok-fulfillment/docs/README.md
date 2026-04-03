# Документация `rustok-fulfillment`

`rustok-fulfillment` — дефолтный fulfillment-подмодуль семейства `ecommerce`.

## Что сейчас внутри

- схема `shipping_options`;
- схема `fulfillments`;
- `FulfillmentModule` и `FulfillmentService`;
- shipping boundary для checkout-цепочки `cart -> payment -> order -> fulfillment`;
- first-class `allowed_shipping_profile_slugs` в shipping-option contract, который пока нормализуется в metadata-backed `shipping_profiles.allowed_slugs`;
- transport-level validation для `allowed_shipping_profile_slugs` теперь живёт в фасаде `rustok-commerce` и проверяет ссылки против active shipping profiles из typed registry `shipping_profiles`;
- admin REST/admin GraphQL и module-owned `rustok-commerce-admin` UI уже потребляют этот shipping-option contract как typed operator surface поверх `FulfillmentService`, включая deactivate/reactivate lifecycle поверх флага `active`;
- встроенный manual/default fulfillment flow без внешних carrier providers на текущем этапе.

## Архитектурная граница

- модуль не зависит от `rustok-commerce` umbrella, чтобы не создавать цикл;
- модуль не владеет заказом или customer-профилем, а только ссылается на них по идентификаторам;
- provider-specific доставка отложена в backlog и должна жить как следующий вложенный подмодуль над fulfillment boundary, а не смешиваться с базовой shipping-моделью;
- GraphQL и REST transport пока остаются в фасаде `rustok-commerce`.

## Связанные документы

- [README crate](../README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
