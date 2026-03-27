# Документация `rustok-commerce`

В этой папке хранится документация umbrella-модуля `crates/rustok-commerce`.

## Документы

- [План реализации](./implementation-plan.md) — актуальный roadmap по развитию ecommerce family, Medusa-style REST transport и выносу ответственности в отдельные модули.
- [Пакет админского UI](../admin/README.md)
- [Пакет storefront UI](../storefront/README.md)

## Текущее состояние

- `rustok-commerce` остаётся umbrella/root module для ecommerce family и держит orchestration, transport и оставшиеся несрезанные части домена.
- Основной REST-контракт живёт на `/store/*` и `/admin/*`; legacy `/api/commerce/*` удалён из live route tree и OpenAPI.
- На admin surface кроме product management уже подняты paginated order transport (`GET /admin/orders`, `GET /admin/orders/{id}`), explicit order lifecycle routes (`mark-paid`, `ship`, `deliver`, `cancel`) и detail/lifecycle routes для `payment-collections` и `fulfillments`.
- GraphQL surface сохранён и должен использовать те же application services, что и REST.
- `apps/server` остаётся thin host-слоем: маршруты, OpenAPI и schema composition, без дублирования commerce business logic.
- Cart snapshot уже хранит storefront context (`region_id`, `country_code`, `locale_code`, `selected_shipping_option_id`, `customer_id`, `email`, `currency_code`).
- Checkout flow использует `checking_out`, reuse payment collection и recovery semantics для повторных storefront запросов.
- Publishable UI пакеты для admin/storefront живут внутри модуля и подключаются host-приложениями через manifest-driven composition.

## Контракты событий

- [Event flow contract (central)](../../../docs/architecture/event-flow-contract.md)
