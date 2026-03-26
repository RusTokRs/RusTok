# Документация `rustok-cart`

`rustok-cart` — дефолтный cart-подмодуль семейства `ecommerce`.

## Что сейчас внутри

- схема `carts` и `cart_line_items`;
- `CartModule` и `CartService`;
- persisted cart context snapshot: `region_id`, `country_code`, `locale_code`, `selected_shipping_option_id`,
  `customer_id`, `email`, `currency_code`;
- lifecycle корзины: `active -> completed/abandoned`;
- CRUD line items, расчёт cart totals и нормализация locale/country snapshot для storefront-контекста.

## Архитектурная граница

- модуль не зависит от `rustok-commerce` umbrella, чтобы не создавать цикл;
- product/variant ссылки в корзине хранятся как snapshot references, а не как обязательные cross-module foreign keys;
- cart хранит snapshot storefront context, но не владеет region/locale policy: tenant locale enablement и
  cross-module orchestration остаются на уровне `rustok-commerce` umbrella;
- GraphQL и REST transport пока остаются в фасаде `rustok-commerce`.

## Связанные документы

- [README crate](../README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
