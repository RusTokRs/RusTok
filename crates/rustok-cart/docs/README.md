# Документация `rustok-cart`

`rustok-cart` — дефолтный cart-подмодуль семейства `ecommerce`.

## Назначение

- схема `carts`, `cart_line_items` и `cart_line_item_translations` (localized line-item titles вынесены из base rows);
- `CartModule` и `CartService`;
- persisted cart context snapshot: `region_id`, `country_code`, `locale_code`, `selected_shipping_option_id`,
  `customer_id`, `email`, `currency_code`;
- typed `cart_adjustments` для promotion/discount snapshot: `source_type/source_id`, `amount/currency_code`,
  optional line-item binding и language-neutral metadata без display label;
- lifecycle корзины: `active -> checking_out -> completed` и `active -> abandoned`;
- CRUD line items, расчёт totals, seller-aware delivery-group snapshot с canonical `seller_id` и нормализация locale/country snapshot для storefront-контекста;
- перепрайс line items при изменении количества или storefront context (region/channel), причём pricing discount
  нормализуется в `base/compare_at unit_price` плюс pricing-owned `cart_adjustments`, чтобы persisted `unit_price`
  не дрейфовал между effective sale price и base row;
- module-owned storefront пакет `rustok-cart/storefront` для cart inspection и безопасных line-item decrement/remove действий.

## Зона ответственности

- модуль не зависит от `rustok-commerce` umbrella, чтобы не создавать цикл;
- product/variant ссылки в корзине хранятся как snapshot references, а не как обязательные cross-module foreign keys;
- cart хранит snapshot storefront context, но не владеет region/locale policy: tenant locale enablement и
  cross-module orchestration остаются на уровне `rustok-commerce` umbrella;
- GraphQL и REST transport по-прежнему остаются в фасаде `rustok-commerce`;
- storefront cart UI теперь живёт внутри самого модуля и не возвращает cart ownership обратно в host или umbrella UI.

## Интеграция

- модуль входит в ecommerce family и должен сохранять собственную storage/runtime-границу без возврата ответственности в umbrella `rustok-commerce`;
- transport и GraphQL по-прежнему публикуются через `rustok-commerce`, но storefront cart read-side, seller-aware delivery-group snapshot и безопасный line-item write-side уже вынесены в отдельный module-owned surface `rustok-cart/storefront`;
- cart delivery grouping and shipping selection use only canonical `seller_id` plus shipping profile; `seller_scope` is not read as a grouping or selection fallback.
- `cart_adjustments` являются source of truth для скидочного snapshot в cart: `subtotal_amount`, `adjustment_total`
  и net `total_amount` не зависят от default locale или localized promotion label.
- выбранные shipping options теперь materialize'ятся в first-class `shipping_total`: persisted cart total
  считается как `subtotal - adjustments + shipping_total (+ tax при tax-exclusive region)` вместо
  implicit legacy shortcut через `selected_shipping_option_id`.
- typed promotion runtime теперь покрывает не только cart/line-item scope, но и shipping scope:
  shipping discounts живут как `cart_adjustments` с `scope=shipping`, а не как скрытая мутация
  `shipping_total` или отдельный не-snapshot'ящийся side effect.
- tax calculation больше не зашит напрямую в `CartService`: cart runtime теперь вызывает
  `rustok-tax::TaxService`, а `cart_tax_lines` несут typed `provider_id`, чтобы будущие
  внешние tax engines не ломали cart/order transport contract вторым миграционным срезом.
- storefront transport parity для этого слоя уже подтверждён: `/store/carts/{id}` и storefront
  GraphQL checkout сохраняют `shipping_total`, `adjustment_total` и shipping-scoped promotion
  metadata без скрытого fallback или схлопывания скидки в базовую цену.
- pricing-driven repricing переписывает только pricing-owned adjustments для затронутых line items и не смешивает
  скидочный snapshot с manual/non-pricing adjustments.
- storefront add-to-cart при наличии скидки тоже пишет pricing snapshot атомарно: line item и pricing-owned
  adjustment создаются в одной cart-транзакции без промежуточного persisted sale-only состояния.
- typed promotion runtime поверх этого слоя уже поддерживает preview/apply для percentage/fixed discounts
  на cart-level и line-item scope, причём application path не требует raw full-replace всех adjustments.
- aggregate admin GraphQL transport уже поднимает этот runtime как operator-side preview/apply path
  для `cart`, `line_item` и `shipping` scope, поэтому cart promotions больше не живут только на
  service/test уровне.
- изменения cross-module контракта нужно синхронизировать с `rustok-commerce` и соседними split-модулями;
- storefront package использует native Leptos `#[server]` functions как default data layer и сохраняет GraphQL storefront contract как fallback.

## Проверка

- `cargo xtask module validate cart`
- `cargo xtask module test cart`
- targeted commerce tests для cart-домена при изменении runtime wiring

## Связанные документы

- [README crate](../README.md)
- [План развития `rustok-cart`](./implementation-plan.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
