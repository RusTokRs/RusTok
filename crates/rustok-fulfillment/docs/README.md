# Документация `rustok-fulfillment`

`rustok-fulfillment` — дефолтный fulfillment-подмодуль семейства `ecommerce`.

## Назначение

- схема `shipping_options`;
- схема `fulfillments`;
- `FulfillmentModule` и `FulfillmentService`;
- shipping boundary для checkout-цепочки `cart -> payment -> order -> fulfillment`;
- first-class `allowed_shipping_profile_slugs` в shipping-option contract, который пока нормализуется в metadata-backed `shipping_profiles.allowed_slugs`;
- transport-level validation для `allowed_shipping_profile_slugs` теперь живёт в фасаде `rustok-commerce` и проверяет ссылки против active shipping profiles из typed registry `shipping_profiles`;
- storefront cart/checkout больше не опирается на один глобальный shipping option: `rustok-commerce` поверх этого boundary уже строит `delivery_groups[]`, typed `shipping_selections[]` и multi-fulfillment checkout, а singular shipping fields остаются только compatibility shim'ом для single-group cart'ов;
- admin REST/admin GraphQL и module-owned `rustok-commerce-admin` UI уже потребляют этот shipping-option contract как typed operator surface поверх `FulfillmentService`, включая deactivate/reactivate lifecycle поверх флага `active`;
- встроенный manual/default fulfillment flow без внешних carrier providers на текущем этапе.

## Зона ответственности

- модуль не зависит от `rustok-commerce` umbrella, чтобы не создавать цикл;
- модуль не владеет заказом или customer-профилем, а только ссылается на них по идентификаторам;
- provider-specific доставка отложена в backlog и должна жить как следующий вложенный подмодуль над fulfillment boundary, а не смешиваться с базовой shipping-моделью;
- GraphQL и REST transport пока остаются в фасаде `rustok-commerce`.

## Интеграция

- модуль входит в ecommerce family и должен сохранять собственную storage/runtime-границу без возврата ответственности в umbrella ustok-commerce;
- transport, GraphQL и UI-поверхности публикуются через ustok-commerce, пока для домена не зафиксирован отдельный module-owned surface;
- изменения cross-module контракта нужно синхронизировать с ustok-commerce и соседними split-модулями.

## Проверка

- cargo xtask module validate fulfillment
- cargo xtask module test fulfillment
- targeted commerce tests для fulfillment-домена при изменении runtime wiring
## Связанные документы

- [README crate](../README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
