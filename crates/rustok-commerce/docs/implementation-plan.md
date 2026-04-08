# План реализации `rustok-commerce`

## Статус документа

Этот документ фиксирует актуальный roadmap umbrella-модуля `rustok-commerce` после отказа от legacy REST surface `/api/commerce/*` и после появления platform-level `rustok-channel`.

Актуализация этого roadmap выполнена на 8 апреля 2026 года: UI split ecommerce family
переведён из чисто планового статуса в активную execution-фазу, потому что `product`
уже получил собственный module-owned admin route, shipping options вынесены в
`fulfillment`, order operations вынесены в `order`, inventory visibility вынесена
в `inventory`, pricing visibility вынесена в `pricing`, customer operations вынесены
в `customer`, region CRUD вынесен в `region`, а aggregate `commerce` UI очищен до
typed shipping-profile registry.

Исходные предпосылки:

- live REST-контракт для ecommerce живёт на `/store/*` и `/admin/*`;
- GraphQL остаётся поддерживаемым transport-слоем;
- `rustok-commerce` продолжает играть роль root umbrella module для ecommerce family;
- базовый split на `cart/customer/product/region/pricing/inventory/order/payment/fulfillment` уже выполнен и дальше углубляется;
- отдельный sales-channel домен в `commerce` не нужен: платформа уже имеет `rustok-channel`, и ecommerce должен стать channel-aware поверх него, а не дублировать его модель.

## Область работ

- удерживать `rustok-commerce` как umbrella/root layer для ecommerce family, а не как storage owner для уже выделенных bounded contexts;
- развивать cross-domain orchestration, transport parity и channel-aware commerce contract поверх `rustok-channel`;
- завершить UI split и дальнейший domain split без возврата ответственности в host-слой или aggregate UI.

## Цели

- довести Medusa-style ecommerce surface до production-grade состояния без локально выдуманных семантик;
- держать GraphQL и REST над одними и теми же application services;
- стабилизировать checkout, cart context и orchestration между cart/payment/order/fulfillment;
- сделать commerce channel-aware поверх `rustok-channel`;
- добрать недостающие bounded contexts для Medusa-паритета: merchandising availability, pricing/promotions, tax, post-order flows и provider extensibility;
- сохранять tenant isolation, outbox/event flow, index-backed read paths и thin-host роль `apps/server`.

## Текущий приоритет для Medusa JS clone

Порядок дальнейшей разработки фиксируется так:

1. выполнить UI split по module ownership: вынести domain UI из aggregate `rustok-commerce-admin` / `rustok-commerce-storefront` в соответствующие split-модули (`product`, `order`, `inventory`, `pricing`, `fulfillment`, `customer`, `region`), оставив `rustok-commerce` только orchestration/cross-domain surfaces;
2. завершить `Phase 7` до seller-aware deliverability model поверх уже работающего split-fulfillment baseline;
3. перевести `Phase 8` в Pricing 2.0 с channel-aware price resolution, price lists, rules и promotions;
4. вынести `Phase 9` в отдельный tax domain с tax lines и provider seam;
5. закрыть `Phase 10` post-order surface (`returns/refunds/exchanges/claims/order changes`);
6. стабилизировать `Phase 11` provider architecture для payment/fulfillment;
7. зафиксировать `Phase 12` как Medusa parity matrix и release discipline.

Ближайший execution slice:

- сначала продолжить уже начатый UI split: product admin route вынесен в `rustok-product/admin`, shipping-option admin route вынесен в `rustok-fulfillment/admin`, customer admin route вынесен в `rustok-customer/admin`, order admin route вынесен в `rustok-order/admin`, inventory admin route вынесен в `rustok-inventory/admin`, pricing admin route вынесен в `rustok-pricing/admin`, region admin route вынесен в `rustok-region/admin`, storefront split уже идёт через `rustok-region/storefront`, `rustok-product/storefront` и `rustok-pricing/storefront`, а aggregate `rustok-commerce-storefront` уже сжат до orchestration hub; следующий шаг — выносить только оставшиеся storefront flows с устойчивой ownership boundary поверх cart/checkout;
- затем закрыть seller-aware grouping поверх `delivery_groups[]`, чтобы split fulfillment перестал быть только profile-based;
- затем добавить fulfillment-item model и post-order delivery changes;
- и только после этого идти в channel-aware pricing.

## Текущее состояние

- `rustok-commerce` уже содержит `CatalogService`, `PricingService`, `InventoryService`, `CheckoutService`, `StoreContextService`;
- storefront и admin REST routes живут внутри `crates/rustok-commerce/src/controllers/*`;
- GraphQL surface живёт внутри `crates/rustok-commerce/src/graphql/*`;
- cart snapshot уже хранит storefront context (`region_id`, `country_code`, `locale_code`, `selected_shipping_option_id`, `customer_id`, `email`, `currency_code`) и channel snapshot (`channel_id`, `channel_slug`);
- checkout path использует `checking_out`, reuse payment collection и recovery semantics;
- checkout reuse-ит pre-created cart payment collection, вместо создания дублирующего payment record на шаге `complete`;
- guest checkout разрешён для guest cart без обязательного auth context, при этом customer-owned cart остаётся auth-gated;
- admin surface уже имеет order/payment/fulfillment lifecycle transport и runtime parity с GraphQL;
- `apps/server` остаётся thin host-слоем для route/OpenAPI/schema composition;
- `rustok-api` и `apps/server` уже пробрасывают `ChannelContext` (`channel_id`, `channel_slug`, `channel_resolution_source`) в request pipeline, а commerce storefront transport уже начал использовать его для channel-aware gating, cart snapshot и order snapshot;
- legacy `/api/commerce/*` удалён из live router, OpenAPI и контрактных тестов.

## Что ещё явно отсутствует

- полноценные channel-aware publication и availability semantics для admin write-path, pricing/inventory/fulfillment и остальных commerce entities beyond storefront baseline;
- seller-aware deliverability model поверх уже введённых typed shipping profiles: сейчас delivery groups группируются по effective `shipping_profile_slug`, но ещё не учитывают будущий seller/vendor domain;
- channel-aware price resolution всё ещё не реализован и переносится в `Phase 8`, несмотря на уже закрытый channel-aware catalog/inventory baseline;
- полноценный promotion/discount domain поверх price rules, а не только `compare_at_amount` и service-level `apply_discount`;
- отдельный tax domain: сейчас tax фактически живёт в `region` как `tax_rate` / `tax_included`;
- post-order слой уровня Medusa: returns, exchanges, claims, order changes, draft/edit flows, refund transport;
- provider registry для payment/fulfillment, webhook ingestion и внешний gateway/carrier story.

## Backlog противоречий

| ID | Противоречие | Что нужно сделать |
| --- | --- | --- |
| `BL-01` | umbrella module vs дальнейший split | продолжать вынос устойчивых bounded contexts в отдельные crates, оставляя `rustok-commerce` orchestration/root layer |
| `BL-02` | entities vs migrations vs indexer SQL | держать schema hardening, migration smoke и Postgres-first tests обязательными |
| `BL-03` | inventory model hardening | выравнивать read/write path вокруг stock locations, levels, reservations и channel-aware availability |
| `BL-04` | transport parity vs domain completeness | не путать наличие `/store/*` и `/admin/*` transport с фактическим Medusa parity по домену |
| `BL-05` | `/admin/*` и `/store/*` vs embedded UI routes | держать route precedence, OpenAPI и router smoke tests под постоянной регрессией |
| `BL-06` | Medusa parity scope | расширять contract tests по официальным Medusa docs, не inventing local semantics |
| `BL-07` | platform `channel` уже есть, а commerce остаётся channel-blind | сделать catalog/cart/order/pricing/inventory/fulfillment channel-aware поверх `rustok-channel`, без второго sales-channel слоя |
| `BL-08` | pricing rows vs merchandising model | перейти от базовых цен и `compare_at_amount` к price lists, rules, tiers, adjustments и promotions |
| `BL-09` | region tax flags vs отдельный tax domain | вынести tax calculation/rules/providers из плоской `region`-модели в отдельный bounded context |
| `BL-10` | линейный order lifecycle vs post-order reality | добавить returns, refunds, exchanges, claims, order changes и draft/edit semantics |
| `BL-11` | manual/default providers vs extensibility | стабилизировать payment/fulfillment provider SPI вместо смешивания базовой модели с внешними интеграциями |
| `BL-12` | typed shipping profile registry, typed product/variant bindings, line-item snapshots, cart delivery groups и multi-fulfillment checkout уже есть, но deliverability model ещё не закрыта до seller-aware уровня | довести deliverability domain от profile-based split fulfillment до seller-aware grouping, fulfillment-item model и post-order delivery changes |
| `BL-13` | split backend уже есть, но storefront и часть admin ownership всё ещё агрегированы в `rustok-commerce-storefront` и оставшихся umbrella admin routes | разнести admin/storefront UI по split ecommerce-модулям и оставить umbrella UI только для cross-domain orchestration surfaces |

## Этапы

### Phase 1. Module topology и contracts

Статус: `done`

- `rustok-commerce` закреплён как umbrella/root module;
- базовый split на профильные crates выполнен;
- shared DTO/entities/errors вынесены в `rustok-commerce-foundation`.

### Phase 1.5. UI split по ownership boundaries

Статус: `in progress`

Что уже закрыто в текущем срезе:

- `rustok-product` уже публикует собственный module-owned admin UI package `rustok-product/admin`;
- `rustok-product/rustok-module.toml` уже экспортирует `[provides.admin_ui]`, а `apps/admin` подхватывает новый route через manifest-driven composition;
- `rustok-product` уже забрал product CRUD ownership;
- `rustok-product` уже публикует собственный module-owned storefront UI package `rustok-product/storefront` и использует native Leptos server functions как default internal data layer с GraphQL fallback;
- `rustok-pricing` уже публикует собственный module-owned storefront UI package `rustok-pricing/storefront` и использует native Leptos server functions как default internal data layer с GraphQL fallback;
- `rustok-fulfillment` уже публикует собственный module-owned admin UI package `rustok-fulfillment/admin`;
- `rustok-customer` уже публикует собственный module-owned admin UI package `rustok-customer/admin` и использует native Leptos server functions как default admin data layer;
- `rustok-region` уже публикует собственный module-owned admin UI package `rustok-region/admin` и использует native Leptos server functions как default admin data layer;
- `rustok-region` уже публикует собственный module-owned storefront UI package `rustok-region/storefront` и использует native Leptos server functions как default internal data layer с GraphQL fallback;
- `rustok-order` уже публикует собственный module-owned admin UI package `rustok-order/admin`;
- `rustok-inventory` уже публикует собственный module-owned admin UI package `rustok-inventory/admin` для stock visibility и low-stock triage;
- `rustok-pricing` уже публикует собственный module-owned admin UI package `rustok-pricing/admin` для price visibility, sale markers и currency coverage;
- `rustok-pricing` уже публикует собственный module-owned storefront UI package `rustok-pricing/storefront` для public pricing atlas, sale markers и currency coverage;
- aggregate `rustok-commerce-storefront` больше не держит published catalog/pricing read-side и сжат до orchestration hub с request/tenant/channel context summary;
- aggregate `rustok-commerce-admin` больше не дублирует product/shipping-option flows и оставлен только под shipping profiles.

Следующие шаги:

- [x] вынести region UI по ownership boundary модуля;
- [x] начать отдельный storefront split через `rustok-region/storefront`;
- [x] вынести product storefront read-side из aggregate `rustok-commerce-storefront`;
- [x] вынести pricing storefront read-side из aggregate `rustok-commerce-storefront`.
- [x] сжать `rustok-commerce-storefront` до orchestration hub без catalog/pricing ownership.

### Phase 2. Medusa-style transport baseline

Статус: `done`

- live REST surface поднят на `/store/*` и `/admin/*`;
- реализованы storefront routes `products`, `regions`, `shipping-options`, `carts`, `payment-collections`, `orders/{id}`, `customers/me`;
- реализованы admin routes для `products`;
- OpenAPI и route contract tests привязаны к live surface без legacy compatibility layer.

### Phase 3. Cart context и checkout hardening

Статус: `in progress`

Фокус:

- удерживать cart как source of truth для storefront context;
- развивать checkout recovery/idempotency semantics;
- закрывать race conditions на `payment-collections` и `complete checkout`;
- держать transport response shape стабильным;
- закрепить cart model как storefront source of truth, включая channel snapshot, без повторного слома API.

Обязательные проверки:

- migration tests для cart context schema;
- integration tests `create cart -> update context -> add line item -> shipping options -> payment collection -> complete`;
- negative tests на `currency_code` vs `region_id`;
- auth/customer ownership tests;
- contract tests store cart endpoints;
- regression tests на повторный `complete checkout` и reuse existing payment collection.

Что уже закрыто в текущем срезе:

- transport coverage подтверждает, что cart context остаётся source of truth для `shipping-options`, `payment-collections` и `checkout`;
- transport coverage закрывает `currency_code` vs `region_id`, guest/customer ownership и сквозной storefront checkout flow;
- service coverage подтверждает reuse уже существующего cart-bound payment collection во время `complete checkout`.
- cart/order transport теперь сохраняет channel snapshot и использует его как часть storefront context во время checkout.

### Phase 4. Order/payment/fulfillment transport

Статус: `in progress`

Фокус:

- расширить admin/store transport поверх уже выделенных модулей;
- зафиксировать response shape и lifecycle semantics;
- продолжить parity между REST и GraphQL над общими сервисами;
- не считать phase закрытой, пока post-order сценарии всё ещё вынесены за скобки.

Что уже закрыто в текущем срезе:

- добавлен admin order transport endpoint `GET /admin/orders/{id}`;
- добавлен paginated admin orders list endpoint `GET /admin/orders` с базовыми filters `status` и `customer_id`;
- admin order detail отдаёт order вместе с latest payment collection и latest fulfillment;
- добавлены explicit admin order lifecycle endpoints: `mark-paid`, `ship`, `deliver`, `cancel`;
- добавлены admin list/detail/lifecycle endpoints для `payment-collections` (`list`, `show`, `authorize`, `capture`, `cancel`) и `fulfillments` (`list`, `show`, `ship`, `deliver`, `cancel`);
- transport/OpenAPI coverage фиксирует RBAC и schema contract для admin order detail и admin payment/fulfillment lifecycle surface;
- GraphQL parity расширен до admin order/payment/fulfillment surface: read queries (`order`, `orders`, `paymentCollection`, `paymentCollections`, `fulfillment`, `fulfillments`) и lifecycle mutations теперь работают поверх тех же `OrderService`/`PaymentService`/`FulfillmentService`, что и REST, и покрыты runtime parity test'ом;
- storefront GraphQL read parity покрывает `storefrontMe` и `storefrontOrder`, включая ownership guard для чужого заказа;
- storefront GraphQL mutation surface покрывает `createStorefrontPaymentCollection` и `completeStorefrontCheckout`, включая guest checkout и reuse уже созданного cart-bound payment collection;
- storefront GraphQL cart surface покрывает `storefrontCart`, `createStorefrontCart`, line-item lifecycle и tri-state patch semantics для cart context;
- storefront GraphQL discovery/read surface включает `storefrontRegions` и `storefrontShippingOptions`, включая cart-context precedence над конфликтующим query currency.

### Phase 5. Упрощение umbrella-модуля

Статус: `in progress`

Фокус:

- удалять dead transport, compatibility remnants и дублирующий код без оглядки на несуществующий migration period;
- держать `rustok-commerce` как orchestration/root layer, а не как склад исторических adapter-ов;
- переносить оставшиеся устойчивые области в профильные crates;
- не затаскивать обратно domain logic в `apps/server`.

Что уже сделано:

- удалён legacy REST surface `/api/commerce/*`;
- удалены rollout/deprecation middleware, settings, runtime guardrails и operator scripts, которые имели смысл только для legacy cutover;
- OpenAPI и route tests переведены на live `/store/*` + `/admin/*` contract.

### Phase 6. Commerce channel-awareness

Статус: `in progress`

Фокус:

- использовать существующий `rustok-channel` как platform-level delivery context;
- сделать catalog, cart, order, inventory и fulfillment channel-aware без создания второго sales-channel домена;
- связать publication/availability semantics commerce с channel bindings и `ChannelContext`.

Deliverables:

- channel-aware product publication и catalog visibility;
- `channel_id` как часть cart/order snapshot и read-model filtering там, где это нужно по домену;
- channel-aware selection для shipping options и stock availability;
- явные правила precedence между `channel`, `region`, `currency` и tenant locale policy.

Что уже закрыто в текущем срезе:

- storefront REST и storefront GraphQL теперь останавливаются на request channel, если для него модуль commerce не включён через `channel_module_bindings`;
- catalog read-path (`/store/products`, `storefrontProduct`, `storefrontProducts`) уже фильтрует товары по metadata-based allowlist на `channel_slug`, поверх базовой проверки `active + published`;
- shipping options в REST/GraphQL и checkout validation уже уважают ту же channel visibility semantics, причём cart `channel_slug` имеет precedence над конфликтующим request/query context;
- cart line-item mutations больше не принимают товары, скрытые для текущего storefront channel;
- storefront product detail и cart line-item quantity checks теперь считают доступный inventory только по stock locations, видимым для текущего storefront channel;
- checkout service теперь повторно валидирует cart line items против текущей product visibility и channel-visible inventory, чтобы stale cart не завершался в заказ с hidden product или уже недоступным остатком;
- channel-aware price resolution сознательно не считается закрытым в этой фазе и переносится в `Phase 8`, чтобы не смешивать storefront availability с pricing 2.0;
- transport и service tests уже покрывают disabled channel module, hidden products, hidden shipping options и checkout reject path для channel-hidden shipping option.

Обязательные проверки:

- integration tests на `ChannelContext -> catalog/cart/checkout`;
- negative tests на неактивный или несвязанный канал;
- regression tests на отсутствие второго локального sales-channel layer;
- docs sync с `rustok-channel`, если меняются contracts между модулями.

### Phase 7. Deliverability domain и split fulfillment

Статус: `in progress`

Фокус:

- закрыть gap между catalog и fulfillment boundary уже не только на уровне compatibility rules, но и на уровне cart/order/fulfillment model;
- закрепить effective shipping profile как typed domain concept: `variant -> product -> default`;
- отделить deliverability domain от старой single-option cart semantics.

Deliverables:

- typed `shipping_profile_slug` для product и variant + effective-profile resolution;
- typed line-item snapshot `shipping_profile_slug` в cart/order;
- typed cart `shipping_selections`, `delivery_groups[]` и multi-fulfillment checkout;
- compatibility shims для single-group carts через legacy `selected_shipping_option_id`, `shipping_option_id` и `fulfillment`.

Что уже закрыто в текущем срезе:

- metadata-backed baseline больше не единственный source of truth: введены schema/migration `shipping_profiles`, typed `ShippingProfileService` и admin-facing registry для shipping profiles;
- product create/update/read contracts уже экспонируют first-class `shipping_profile_slug`, shipping option read/create contracts экспонируют first-class `allowed_shipping_profile_slugs`, а `products.shipping_profile_slug` и `product_variants.shipping_profile_slug` теперь существуют как typed persistence;
- admin REST/GraphQL surface уже умеет `list/show/create/update/deactivate/reactivate` shipping options с typed `allowed_shipping_profile_slugs`, так что shipping profile compatibility и lifecycle больше не живут только в service/tests;
- admin REST/GraphQL surface теперь так же умеет `list/show/create/update/deactivate/reactivate` shipping profiles, а product/shipping-option write-path валидирует ссылки против active registry;
- module-owned `rustok-commerce/admin` UI уже потребляет этот control plane напрямую и может показывать inactive shipping options вместе с explicit lifecycle actions;
- `CatalogService` и `FulfillmentService` по-прежнему нормализуют эти поля в metadata-backed storage shape для backward compatibility, но source of truth для deliverability decisions уже живёт в typed product/variant fields, typed registry и line-item snapshots;
- cart line items теперь хранят effective `shipping_profile_slug`, cart response отдаёт `delivery_groups[]`, а store/GraphQL checkout input принимает typed `shipping_selections[]`;
- `CheckoutService` теперь валидирует stale shipping-profile snapshots, режет несовместимые selections по delivery groups и создаёт отдельный fulfillment на каждую delivery group;
- `CompleteCheckoutResponse` и storefront GraphQL checkout surface теперь возвращают `fulfillments[]`, а singular `fulfillment` остаётся только compatibility shim для single-group carts;
- прежний strict single-option mixed-cart invariant больше не является целевой архитектурой: он сохранён только как compatibility shortcut для cart'ов с одной delivery group;
- regression tests уже покрывают effective-profile resolution, mixed-cart delivery groups, missing per-group selection, multi-fulfillment checkout, stale snapshot reject path и GraphQL checkout parity для новых полей.

Обязательные проверки:

- contract tests на несовместимые товары и shipping options;
- migration tests для product/variant/cart/order shipping-profile schema;
- integration tests на mixed cart с разной fulfillment policy;
- regression tests на preflight checkout failures, которые должны отпускать `checking_out` lock и не создавать payment/order artifacts до side effects.

### Phase 8. Pricing 2.0 и promotions

Статус: `planned`

Фокус:

- выйти за рамки `prices.amount` / `compare_at_amount` / service-level `apply_discount`;
- добавить price lists, rules, tiers и adjustments;
- вынести promotions в отдельный bounded context вместо implicit price mutation.

Deliverables:

- pricing context `channel + region + currency + customer segment` там, где он действительно нужен;
- price lists и rule-driven resolution;
- cart/order adjustments;
- promotion engine для item/order/shipping discounts без смешивания с базовой price storage.

Обязательные проверки:

- deterministic price-resolution tests;
- contract tests на priority/override semantics;
- regression tests на rounding и decimal money contract;
- transport tests на price + promotion representation в `/store/*`, `/admin/*` и GraphQL.

### Phase 9. Tax domain

Статус: `planned`

Фокус:

- перестать считать `region.tax_rate` и `region.tax_included` достаточной tax-моделью;
- ввести отдельный tax bounded context с tax lines, rules и provider seam;
- не ломать текущий checkout flow при постепенном переходе.

Deliverables:

- tax calculation context поверх cart/order/shipping;
- tax lines для line items и shipping;
- provider seam для внешних tax engines;
- migration path от плоской region tax policy к более реалистичной модели.

Обязательные проверки:

- integration tests `cart -> taxes -> payment -> order`;
- negative tests на конфликт tax-inclusive/exclusive semantics;
- contract tests на transport shape tax lines.

### Phase 10. Post-order flows: returns, refunds, exchanges, claims, order changes

Статус: `planned`

Фокус:

- выйти за рамки линейного `pending -> confirmed -> paid -> shipped -> delivered/cancelled`;
- сделать refund/return semantics частью домена, а не только state-machine helper;
- добавить order-change/draft-edit слой, нужный для Medusa-style OMS behavior.

Deliverables:

- return/refund records и lifecycle;
- exchanges / claims, если остаются в целевом Medusa parity scope;
- order change / draft order / preview-apply semantics;
- admin/store transport для post-order сценариев.

Обязательные проверки:

- state-machine и property tests для refund/return/order-change transitions;
- RBAC/ownership tests для customer/admin post-order flows;
- contract tests против live transport для refund/return/order-change surface.

### Phase 11. Provider architecture

Статус: `planned`

Фокус:

- не смешивать manual/default payment/fulfillment domain model с provider-specific кодом;
- сначала стабилизировать SPI, потом подключать конкретные gateway/carrier integrations;
- сохранить `rustok-commerce` orchestration слоем, а не местом для vendor-specific adapters.

Deliverables:

- payment provider registry и webhook ingress contracts;
- fulfillment provider registry и carrier abstraction;
- provider capability model для authorize/capture/refund, rate-quote/ship/cancel;
- явные fallback semantics для manual/default providers.

Обязательные проверки:

- contract tests для provider SPI;
- replay/idempotency tests для webhooks;
- negative tests на частично успешные внешние операции.

### Phase 12. Parity matrix и release discipline

Статус: `planned`

Фокус:

- перевести roadmap из набора локальных фич в явную Medusa parity matrix;
- фиксировать `feature -> module -> transport -> tests -> status`;
- не выпускать transport как "готовый", если доменный слой под ним ещё неполон.

Deliverables:

- parity matrix по официальным Medusa docs;
- release checklist для `/store/*`, `/admin/*` и GraphQL parity;
- список сознательно отложенных фич с явным объяснением, почему они вне текущего scope.

## Проверка

Обязательный минимум:

- unit tests для product/pricing/inventory/cart/order/payment/fulfillment;
- integration tests для event publication и `rustok-index`;
- Postgres migration tests;
- contract tests для `/store/*` и `/admin/*`;
- parity tests `REST <-> GraphQL`;
- router/OpenAPI smoke tests;
- tenant/RBAC regression tests;
- channel-aware regression tests после начала Phase 6.

Release gates:

- нельзя считать Medusa-style transport стабильным без contract tests против live `/store/*` и `/admin/*`;
- нельзя расширять checkout flow без migration/integration coverage;
- нельзя внедрять provider-specific integration до стабилизации provider SPI;
- нельзя заводить внутри `commerce` отдельную sales-channel taxonomy, пока platform-level `rustok-channel` остаётся каноническим channel layer;
- нельзя тащить обратно legacy compatibility surface ради удобства локальной разработки.

## Правила обновления

1. При изменении umbrella/runtime contract сначала обновлять этот файл.
2. При изменении public surface синхронизировать `crates/rustok-commerce/README.md` и `crates/rustok-commerce/docs/README.md`.
3. При изменении контракта между `channel` и `commerce` синхронизировать `rustok-channel` docs и central `docs/architecture/api.md`.
4. При изменении module topology, transport contract или границы `channel` vs `commerce` обновлять `docs/index.md`, локальные docs вынесенных crates и ADR при необходимости.
