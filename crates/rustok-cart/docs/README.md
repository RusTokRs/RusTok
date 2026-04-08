# Документация `rustok-cart`

`rustok-cart` — дефолтный cart-подмодуль семейства `ecommerce`.

## Назначение

- схема `carts` и `cart_line_items`;
- `CartModule` и `CartService`;
- persisted cart context snapshot: `region_id`, `country_code`, `locale_code`, `selected_shipping_option_id`,
  `customer_id`, `email`, `currency_code`;
- lifecycle корзины: `active -> checking_out -> completed` и `active -> abandoned`;
- CRUD line items, расчёт cart totals и нормализация locale/country snapshot для storefront-контекста.

## Зона ответственности

- модуль не зависит от `rustok-commerce` umbrella, чтобы не создавать цикл;
- product/variant ссылки в корзине хранятся как snapshot references, а не как обязательные cross-module foreign keys;
- cart хранит snapshot storefront context, но не владеет region/locale policy: tenant locale enablement и
  cross-module orchestration остаются на уровне `rustok-commerce` umbrella;
- GraphQL и REST transport пока остаются в фасаде `rustok-commerce`.

## Интеграция

- модуль входит в ecommerce family и должен сохранять собственную storage/runtime-границу без возврата ответственности в umbrella ustok-commerce;
- transport, GraphQL и UI-поверхности публикуются через ustok-commerce, пока для домена не зафиксирован отдельный module-owned surface;
- изменения cross-module контракта нужно синхронизировать с ustok-commerce и соседними split-модулями.

## Проверка

- cargo xtask module validate cart
- cargo xtask module test cart
- targeted commerce tests для cart-домена при изменении runtime wiring
## Связанные документы

- [README crate](../README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
