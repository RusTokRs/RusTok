# Документация `rustok-order`

`rustok-order` — дефолтный order-подмодуль семейства `ecommerce`.

## Назначение

- схема `orders`, `order_line_items`, `order_line_item_translations` и `order_adjustments` (localized line-item titles вынесены из base rows);
- `OrderModule` и `OrderService`;
- `order_returns` и `order_return_items` для order-owned post-order returns foundation с resolution-ссылками на refund/order-change orchestration;
- `order_changes` для draft/edit preview-apply skeleton без payment/fulfillment side effects;
- write-side lifecycle заказа: `pending -> confirmed -> paid -> shipped -> delivered/cancelled`;
- публикация order events через transactional outbox;
- module-owned admin UI пакет `rustok-order/admin` для order operations с разделением `admin/src/core/`, `admin/src/transport/mod.rs`, `admin/src/transport/graphql_adapter.rs` и `admin/src/ui/leptos.rs`.

## Зона ответственности

- модуль не зависит от `rustok-commerce` umbrella, чтобы не создавать цикл;
- product/variant ссылки в заказе хранятся как snapshot references, а не как
  обязательные cross-module foreign keys;
- order line items теперь тоже несут nullable `seller_id` как canonical multivendor snapshot key;
- order adjustments хранят promotion/discount snapshot как typed business data: `source_type/source_id`,
  `amount/currency_code`, optional line-item binding и metadata без localized display label;
- checkout snapshot переносит pricing repricing из cart в order так, что discounted line items сохраняют
  `base/compare_at unit_price`, а savings остаются в `order_adjustments`;
- GraphQL и REST transport пока остаются в фасаде `rustok-commerce`;
- admin UI ownership вынесен в `rustok-order/admin`;
- returns foundation хранит item-level lines с validation количества и принадлежности line-item к заказу, а `resolution_type/refund_id/order_change_id` связывают completed return с refund/exchange/claim orchestration без переноса payment logic в order boundary;
- order-change skeleton хранит `preview`, `change_type`, lifecycle `pending -> applied|cancelled` и metadata, но пока не применяет cross-domain effects.

## Контракты событий

- [Event flow contract (central)](../../../docs/architecture/event-flow-contract.md)

## Интеграция

- модуль входит в ecommerce family и должен сохранять собственную
  storage/runtime-границу без возврата ответственности в umbrella `rustok-commerce`;
- transport и GraphQL публикуются через `rustok-commerce`, а operator UX для
  order list/detail/lifecycle публикуется через `rustok-order/admin`;
- checkout/create-order snapshot передаёт typed adjustments в `rustok-order`, а `subtotal_amount`,
  `adjustment_total` и net `total_amount` остаются устойчивыми к смене default locale;
- checkout теперь так же передаёт first-class `shipping_total`, поэтому order snapshot и payment handoff
  живут по одному contract'у `subtotal - adjustments + shipping_total (+ tax при tax-exclusive region)`;
- shipping-scoped promotions тоже приходят в `rustok-order` через тот же typed adjustments contract,
  без отдельного order-side special case для discounts на доставку;
- tax snapshot теперь тоже provider-aware: checkout переносит в `order_tax_lines`
  first-class `provider_id`, а не прячет tax provider только в metadata;
- transport parity для этого snapshot уже подтверждён на storefront GraphQL checkout и admin
  order read-side: `shipping_total` и shipping-scoped adjustments доходят до order contract без
  схлопывания скидки в базовую сумму;
- payment collection до handoff в order продолжает использовать net `cart.total_amount`, поэтому order snapshot
  уже получает ту же net pricing semantics без повторного скрытого дисконта;
- изменения cross-module контракта нужно синхронизировать с `rustok-commerce`
  и соседними split-модулями.

## Разделение FFA для admin

Пакет admin теперь использует framework-agnostic defaults `admin/src/core/`, фасад `admin/src/transport/mod.rs` с GraphQL adapter `admin/src/transport/graphql_adapter.rs` и явный Leptos-адаптер отрисовки `admin/src/ui/leptos.rs`; корень crate только подключает слои модуля и повторно экспортирует `OrderAdmin`.

## Проверка

- `cargo xtask module validate order`
- `cargo xtask module test order`
- targeted commerce tests для order-домена при изменении runtime wiring

## Связанные документы

- [README crate](../README.md)
- [README admin package](../admin/README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
