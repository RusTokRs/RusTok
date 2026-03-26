# Документация `rustok-commerce`

В этой папке хранится документация модуля `crates/rustok-commerce`.

## Документы

- [План реализации](./implementation-plan.md) — подробный план миграции `rustok-commerce` на Medusa-подобную архитектуру, backlog противоречий и ссылки на актуальные Medusa v2 API.
- [Пакет админского UI](../admin/README.md)
- [Пакет storefront UI](../storefront/README.md)

## Статус распила

- `rustok-cart`, `rustok-customer`, `rustok-product`, `rustok-region`, `rustok-pricing`, `rustok-inventory`, `rustok-order`, `rustok-payment` и `rustok-fulfillment` уже выделены в отдельные crates и platform modules.
- `rustok-commerce` теперь играет роль `Ecommerce` umbrella/root module для всего ecommerce family и держит transport/API surface, orchestration и legacy-части, которые ещё не вынесены в отдельные модули.
- Внутри `rustok-commerce` теперь есть и живой orchestration-layer `CheckoutService`, который собирает flow `cart -> payment -> order -> fulfillment` поверх отдельных подмодулей и применяет базовую compensation policy до этапа capture.
- Внутри `rustok-commerce` теперь есть `StoreContextService`, который связывает `rustok-region` с tenant locales и резолвит storefront/checkout context по `region_id`, стране, locale и currency.
- `payment` и `fulfillment` пока работают в built-in manual/default режиме; внешний provider layer сознательно отложен.
- Общие DTO, entities, error surface и search helpers вынесены в `rustok-commerce-foundation`.

## Статус адаптеров

- GraphQL и REST адаптеры commerce теперь живут внутри `crates/rustok-commerce` (`src/graphql/*`, `src/controllers/*`).
- `apps/server` больше не содержит бизнес-логики commerce-адаптеров и использует только thin shim/re-export слой для маршрутов, OpenAPI и GraphQL schema composition.
- Общие transport-контракты (`AuthContext`, `TenantContext`, `RequestContext`, `require_module_enabled`, locale/pagination helpers) модуль получает из `rustok-api`.
- Store cart теперь является persisted storefront-context aggregate: корзина хранит `region_id`, `country_code`, `locale_code`, `selected_shipping_option_id`, `customer_id`, `email` и `currency_code`.
- Для Medusa-style storefront flow появился явный update-path `POST /store/carts/{id}`, который обновляет cart context и делает cart source of truth для последующих shipping/payment/checkout сценариев.
- `GET /store/shipping-options?cart_id=...`, `POST /store/payment-collections` и `POST /store/carts/{id}/complete` теперь читают storefront context из cart snapshot; legacy overrides в checkout сначала persist'ятся обратно в cart ради совместимости.
- Checkout flow теперь использует промежуточный cart status `checking_out`, чтобы защититься от racey double-submit сценариев.
- `POST /store/payment-collections` повторно использует уже активную collection для того же cart, а повторный checkout по уже завершённой корзине восстанавливает существующий `order/payment/fulfillment` response вместо создания дублей.
- Publishable Leptos admin UI для commerce теперь живёт в `crates/rustok-commerce/admin/`; host admin подключает пакет через manifest-driven `build.rs` и рендерит module-owned catalog control room на `/modules/commerce`.
- Publishable Leptos storefront UI для commerce теперь живёт в `crates/rustok-commerce/storefront/`; host storefront подключает пакет через manifest-driven `build.rs`, а public GraphQL read-path отдаёт published product catalog и selected product detail для `/modules/commerce`.

## Контракты событий

- [Event flow contract (central)](../../../docs/architecture/event-flow-contract.md)
