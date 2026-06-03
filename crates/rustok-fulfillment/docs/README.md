# Документация `rustok-fulfillment`

`rustok-fulfillment` — дефолтный fulfillment-подмодуль семейства `ecommerce`.

## Назначение

- схема `shipping_options`;
- схема `fulfillments`;
- схема `fulfillment_items`;
- `FulfillmentModule` и `FulfillmentService`;
- shipping boundary для checkout-цепочки `cart -> payment -> order -> fulfillment`;
- first-class `allowed_shipping_profile_slugs` в shipping-option contract, который пока нормализуется в metadata-backed `shipping_profiles.allowed_slugs`;
- transport-level validation для `allowed_shipping_profile_slugs` теперь живёт в фасаде `rustok-commerce` и проверяет ссылки против active shipping profiles из typed registry `shipping_profiles`;
- storefront cart/checkout больше не опирается на один глобальный shipping option: `rustok-commerce` поверх этого boundary уже строит `delivery_groups[]`, typed `shipping_selections[]` и multi-fulfillment checkout, а singular shipping fields остаются только compatibility shim'ом для single-group cart'ов;
- typed `fulfillment_items[]` теперь фиксируют состав каждого fulfillment поверх `order_line_item_id + quantity`, так что post-order delivery path больше не обязан восстанавливать item scope только из metadata delivery group;
- typed `fulfillment_items[]` теперь также держат `shipped_quantity` и `delivered_quantity`, поэтому partial ship/deliver progress живёт в самом fulfillment boundary, а не в ad-hoc metadata outside модели;
- admin/manual post-order create path в фасаде `rustok-commerce` теперь строится поверх тех же typed `fulfillment_items[]` и валидирует order-line ownership + remaining quantity до вызова `FulfillmentService`;
- `ship_fulfillment` и `deliver_fulfillment` теперь принимают item-level quantity adjustments, сохраняют только language-agnostic audit events в metadata fulfillment/item'ов и поддерживают partial post-order delivery progress без отдельного OMS слоя; `delivered_note` остаётся typed-полем fulfillment;
- explicit `reopen_fulfillment` и `reship_fulfillment` теперь тоже живут в этом boundary, так что post-order delivery recovery не требует неявных status hacks и не возвращает language-dependent бизнес-текст в metadata;
- admin REST/admin GraphQL и module-owned `rustok-fulfillment/admin` UI уже потребляют этот shipping-option contract как typed operator surface поверх `FulfillmentService`, включая deactivate/reactivate lifecycle поверх флага `active`;
- встроенный manual/default fulfillment flow без внешних carrier providers на текущем этапе.

## Зона ответственности

- модуль не зависит от `rustok-commerce` umbrella, чтобы не создавать цикл;
- модуль не владеет заказом или customer-профилем, а только ссылается на них по идентификаторам;
- provider-specific доставка отложена в backlog и должна жить как следующий вложенный подмодуль над fulfillment boundary, а не смешиваться с базовой shipping-моделью;
- GraphQL и REST transport пока остаются в фасаде `rustok-commerce`.

## Интеграция

- модуль входит в ecommerce family и должен сохранять собственную storage/runtime-границу без возврата ответственности в umbrella `rustok-commerce`;
- transport и GraphQL surface пока публикуются через `rustok-commerce`, а admin UI ownership уже вынесен в module-owned пакет `rustok-fulfillment/admin`;
- изменения cross-module контракта нужно синхронизировать с `rustok-commerce` и соседними split-модулями.

## Разделение FFA для admin

Пакет admin теперь использует framework-agnostic настройки по умолчанию `admin/src/core.rs`, фасад `admin/src/transport.rs` поверх GraphQL shipping-option transport и явный Leptos-адаптер отрисовки `admin/src/ui/leptos.rs`; корень crate только подключает слои модуля и повторно экспортирует `FulfillmentAdmin`.

## Проверка

- cargo xtask module validate fulfillment
- cargo xtask module test fulfillment
- targeted commerce tests для fulfillment-домена при изменении runtime wiring

## Связанные документы

- [README crate](../README.md)
- [README admin UI](../admin/README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
