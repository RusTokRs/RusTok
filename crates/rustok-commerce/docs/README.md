# Документация `rustok-commerce`

В этой папке хранится документация umbrella-модуля `crates/rustok-commerce`.

## Назначение

- удерживать `rustok-commerce` как umbrella/root module для ecommerce family;
- держать orchestration, transport и cross-domain contracts, которые ещё не вынесены в split-модули;
- не возвращать domain ownership из split-модулей обратно в host-слой.

## Зона ответственности

- orchestration между `cart/customer/product/region/pricing/inventory/order/payment/fulfillment`;
- REST/GraphQL transport и переходные aggregate UI-поверхности, пока доменные surfaces не вынесены по ownership boundaries;
- channel-aware commerce contract поверх `rustok-channel`, checkout orchestration и cross-domain deliverability semantics;
- поддержание thin-host роли `apps/server` без возврата commerce business logic в host.

## Интеграция

- `apps/server` остаётся adapter/wiring слоем для route, OpenAPI и schema composition;
- split ecommerce-модули владеют своими persistence/runtime boundaries, а `rustok-commerce` координирует cross-domain flow;
- module-owned UI пакеты подключаются host-приложениями через manifest-driven composition;
- любые изменения cross-domain contract нужно синхронизировать с local docs split-модулей и central docs платформы.

## Проверка

- `cargo xtask module validate commerce`
- `cargo xtask module test commerce`
- targeted commerce tests для checkout, transport parity и split-module integration при изменении runtime wiring

## Связанные документы

- [План реализации](./implementation-plan.md) — актуальный roadmap по развитию ecommerce family, Medusa-style REST transport, channel-aware commerce поверх `rustok-channel` и выносу ответственности в отдельные модули.
- [Сравнение RusTok и Medusa](../../../docs/research/medusa-vs-rustok-architecture.md)
- [Пакет админского UI](../admin/README.md)
- [Пакет storefront UI](../storefront/README.md)

## Текущее состояние

- `rustok-commerce` остаётся umbrella/root module для ecommerce family и держит orchestration, transport и оставшиеся несрезанные части домена.
- Основной REST-контракт живёт на `/store/*` и `/admin/*`; legacy `/api/commerce/*` удалён из live route tree и OpenAPI.
- На admin surface кроме product management уже подняты paginated order transport (`GET /admin/orders`, `GET /admin/orders/{id}`), explicit order lifecycle routes (`mark-paid`, `ship`, `deliver`, `cancel`) и list/detail/lifecycle routes для `payment-collections` и `fulfillments`.
- GraphQL surface сохранён и использует те же application services, что и REST; для admin commerce уже есть parity по order/payment/fulfillment queries, включая list read-path для `paymentCollections` и `fulfillments`, и lifecycle mutations, а storefront surface теперь включает `storefrontRegions`, `storefrontShippingOptions`, `storefrontCart`, `createStorefrontCart`, `updateStorefrontCartContext`, cart line-item lifecycle, `storefrontMe`, customer-owned `storefrontOrder`, `createStorefrontPaymentCollection` и `completeStorefrontCheckout`.
- `apps/server` остаётся thin host-слоем: маршруты, OpenAPI и schema composition, без дублирования commerce business logic.
- Cart snapshot уже хранит storefront context (`region_id`, `country_code`, `locale_code`, `selected_shipping_option_id`, `customer_id`, `email`, `currency_code`) и channel snapshot (`channel_id`, `channel_slug`); тот же channel snapshot теперь переносится в order transport при checkout.
- Checkout flow использует `checking_out`, reuse payment collection и recovery semantics для повторных storefront запросов.
- Платформа уже пробрасывает `ChannelContext` через `rustok-api` и `apps/server`, а `commerce` начал использовать этот слой как реальный storefront input: `/store/*` и storefront GraphQL теперь уважают `channel_module_bindings`, а catalog/shipping visibility можно ограничивать metadata-based allowlist'ом по `channel_slug`.
- Storefront product detail, cart mutation path и checkout validation теперь учитывают не только channel-aware видимость товаров и shipping options, но и доступный inventory по stock locations, видимым для текущего `channel_slug`; stale cart больше не проходит checkout ни с hidden product, ни с уже недоступным для канала остатком.
- Для shipping profiles metadata-backed baseline больше не является единственным source of truth: в `commerce` появился typed registry `shipping_profiles` + `ShippingProfileService`, а `products.shipping_profile_slug` и `product_variants.shipping_profile_slug` теперь живут как typed persistence с backward-compatible нормализацией в metadata.
- Product catalog surface дополнительно экспонирует first-class `shipping_profile_slug`, shipping option surface экспонирует first-class `allowed_shipping_profile_slugs`, а admin/storefront write-path теперь валидирует эти ссылки против активного typed shipping-profile registry.
- Cart и checkout теперь тоже стали deliverability-aware: line items хранят effective `shipping_profile_slug`, cart response отдаёт `delivery_groups[]`, cart context/checkout принимают typed `shipping_selections[]`, а checkout создаёт `fulfillments[]` по одной записи на delivery group.
- Legacy single-group contract сохраняется только как compatibility shortcut: `selected_shipping_option_id`, singular `shipping_option_id` и singular `fulfillment` заполняются только для cart'ов с одной delivery group.
- Preflight validation в checkout теперь отрабатывает до side effects: stale shipping-profile snapshot, отсутствующая per-group selection или несовместимый shipping option отпускают `checking_out` lock и не создают payment/order artifacts.
- Admin REST и admin GraphQL теперь тоже имеют typed shipping-option management surface: `list/show/create/update/deactivate/reactivate` для shipping options поверх `FulfillmentService`, включая `allowed_shipping_profile_slugs` и lifecycle по `active`.
- Admin REST и admin GraphQL теперь имеют и typed shipping-profile management surface: `list/show/create/update/deactivate/reactivate` поверх `ShippingProfileService`, так что compatibility rules больше не живут только в metadata или service helper'ах.
- Module-owned admin UI пакет `rustok-commerce/admin` сейчас остаётся переходным aggregate surface: shipping profiles и shipping options остаются его прямой зоной ответственности, а product CRUD уже начал переезжать в `rustok-product/admin`.
- Publishable UI пакеты для admin/storefront живут внутри модуля и подключаются host-приложениями через manifest-driven composition.

## Ближайший roadmap

- UI split уже начат с вынесения product admin route в `rustok-product/admin`; следующий шаг — убрать product CRUD из aggregate `rustok-commerce-admin` и затем резать storefront surface.
- Затем закрываем seller-aware deliverability поверх уже внедрённого split-fulfillment baseline.
- Затем идём в Pricing 2.0: channel-aware price resolution, price lists, rules и promotions.
- После этого выносим tax, post-order flows и provider SPI.

## Контракты событий

- [Event flow contract (central)](../../../docs/architecture/event-flow-contract.md)
