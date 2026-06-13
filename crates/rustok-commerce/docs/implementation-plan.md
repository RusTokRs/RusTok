# План реализации `rustok-commerce`

## Execution checkpoint

- Current phase: next-admin commerce operator templates and post-order parity hardening
- Last checkpoint: Moved the storefront cart handoff status into `rustok-cart/storefront`, payment collection card/create action presentation into `rustok-payment/storefront`, shipping handoff + seller-aware shipping selection presentation into `rustok-fulfillment/storefront`, and checkout result/order status and complete-checkout action presentation into `rustok-order/storefront`; `rustok-commerce/storefront` now consumes owner-module components for those UI fragments while retaining checkout orchestration transport and only a transitional selection callback.
- Next step: Continue by moving owner transports behind the owner storefront packages before deleting commerce compatibility fields or transport paths.
- Open blockers: None.
- Hand-off notes for next agent: After each post-order operator UI/page addition, update this checkpoint block and central registry evidence; keep the Next host route as a thin auth/options adapter only.
- Last updated at (UTC): 2026-06-13T23:20:00Z


## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress` (readiness hardening для уже готовых slices; remote transport/runtime profile ещё не считается включённым)
- Structural shape: `core_transport_ui`
- Evidence:
  - module plan синхронизирован с central FFA/FBA readiness board; UI surface уже опубликован и ведётся в migration/backlog ритме;
  - admin return decision tree теперь имеет transport parity (`/admin/orders/{id}/returns/decision` ↔ `createOrderReturnDecision`) над единым `PostOrderOrchestrationService`, включая completion semantics для `return_only/refund/exchange/claim`, без дублирования rules в host/UI adapters; live REST и GraphQL parity tests фиксируют claim → completed return + `order_change(change_type=claim)`;
  - module-owned admin UI получил native-first post-order change operator: операторы фильтруют order changes по `order_id/status` и вызывают `OrderService::apply_order_change` / `cancel_order_change` через module-owned `#[server]` functions с targeted SSR coverage, при этом GraphQL `orderChanges` / `applyOrderChange` / `cancelOrderChange` сохранены как fallback transport, когда native server-function transport недоступен;
  - exchange/claim return-decision helper metadata теперь помечает создаваемые order changes `return_decision_action` и `return_decision_source`, а admin operator workspace показывает resolution summary cards из preview/metadata через framework-agnostic `admin/src/core/` helper без переноса domain rules в host или Leptos render adapter;
  - FFA admin transport module split: `admin/src/lib.rs` больше не содержит Leptos render/business code и только wires modules + re-export `CommerceAdmin`; `admin/src/core/mod.rs` реэкспортирует subdomain files для form/command/view-model policy, а `admin/src/transport/mod.rs` реэкспортирует shipping-profile, cart-promotion и order-change transport operations over the existing native/GraphQL-capable `api` layer;
  - FFA storefront transport/core split: aggregate checkout route теперь строит `FetchCommerceRequest`, `CartCommandRequest`, `SelectShippingOptionRequest` и commerce-owned context fallback view-model (`tenant/channel/resolution`) в Leptos-free `storefront/src/core/` submodules (`requests`, `presentation`); cart totals/line-items/adjustments stay in `rustok-cart`, payment details stay in `rustok-payment`, order totals stay in `rustok-order`, fulfillment/shipping option details stay in `rustok-fulfillment`, and cart handoff status rendering is owned by `rustok-cart-storefront`, payment-collection card/create action rendering is owned by `rustok-payment-storefront`, fulfillment/shipping handoff and seller-aware selection rendering are owned by `rustok-fulfillment-storefront`, checkout result/order status and complete-checkout action presentation is owned by `rustok-order-storefront`, and commerce consumes those owner-module components while retaining checkout orchestration transport; `storefront/src/transport/mod.rs` owns единственную native-first + GraphQL fallback policy, duplicate combined fallback helpers удалены из `storefront/src/api.rs`, а `native_server_adapter.rs` / `graphql_adapter.rs` являются единственными местами storefront UI package, где вызываются raw native/GraphQL `api::*` functions;
  - дальнейшее повышение статуса выполняется только вместе с verification evidence и обновлением local+central docs;
  - FBA-readiness gate включён для уже готовых ecommerce slices до расширения roadmap новыми marketplace/provider модулями: проверяются service-contract ownership, typed request context/errors, explicit cross-module ports/events и отсутствие business logic в transport/UI adapters.
- Last verified at (UTC): 2026-06-12T00:00:00Z
- Owner: `rustok-commerce` module team

## Статус документа

Этот документ фиксирует актуальный roadmap umbrella-модуля `rustok-commerce` после отказа от legacy REST surface `/api/commerce/*` и после появления platform-level `rustok-channel`.

Актуализация этого roadmap выполнена на 8 апреля 2026 года: UI split ecommerce family
переведён из чисто планового статуса в активную execution-фазу, потому что `product`
уже получил собственный module-owned admin route, shipping options вынесены в
`fulfillment`, order operations вынесены в `order`, inventory visibility и targeted stock/reservation/availability actions вынесены
в `inventory`, pricing visibility вынесена в `pricing`, customer operations вынесены
в `customer`, region CRUD вынесен в `region`, а aggregate `commerce` UI очищен до
typed shipping-profile registry плюс aggregate cart-promotion operator surface.

Исходные предпосылки:

- live REST-контракт для ecommerce живёт на `/store/*` и `/admin/*`;
- GraphQL остаётся поддерживаемым transport-слоем;
- `rustok-commerce` продолжает играть роль root umbrella module для ecommerce family;
- базовый split на `cart/customer/product/region/pricing/inventory/order/payment/fulfillment` уже выполнен и дальше углубляется;
- отдельный sales-channel домен в `commerce` не нужен: платформа уже имеет `rustok-channel`, и ecommerce должен стать channel-aware поверх него, а не дублировать его модель.


## FFA transition (FBA deferred alignment track)

Статус: `in progress`

> **Жёсткий gate для новых модулей и крупных ecommerce slices:** новый ecommerce/marketplace модуль
> нельзя начинать как host-owned UI, ad-hoc REST/GraphQL handler или storage appendage внутри
> `rustok-commerce`. Сначала фиксируются module slug/ownership, canonical service contract,
> typed request context/errors, data ownership, explicit ports/events для cross-module зависимостей
> и FFA/FBA status block в local docs + central registry. Только после этого добавляются
> transport adapters (`#[server]`, GraphQL, REST/RPC) и module-owned UI как thin adapter.

С этого среза ecommerce roadmap официально синхронизирован с переходом платформы на
Fluid Frontend Architecture (FFA) и Fluid Backend Architecture (FBA):

- FFA: module-owned UI surfaces (`admin`/`storefront`) остаются default path, а transport
  (`#[server]` + GraphQL/REST fallback) обязан сохранять semantic parity без локальных
  divergence по доменной логике;
- FBA: `rustok-commerce` и split ecommerce modules удерживают service-boundary-ready
  contract, где application services остаются canonical бизнес-ядром независимо от
  topology исполнения (embedded vs remote);
- umbrella слой не возвращает ownership уже выделенных bounded contexts и продолжает
  выступать orchestration root для cross-domain сценариев checkout/post-order;
- все новые Phase 8/9/10/11 инкременты должны сразу проходить FFA check и FBA-readiness guardrail:
  transport-neutral service semantics, channel-aware boundaries, typed context/error mapping,
  explicit module ports/events и отсутствие duplicated business rules в UI/transport adapters;
- перед расширением roadmap новыми marketplace/provider modules нужно довести FBA evidence для уже
  готовых ecommerce slices до уровня `in_progress -> boundary_ready candidate`: service contract first,
  transport adapters second, без host-owned business semantics.

Обязательные действия в ближайших итерациях:

1. для каждого нового ecommerce endpoint фиксировать FFA parity (`#[server]` ↔ GraphQL/REST);
2. для каждого нового post-order сценария фиксировать FBA boundary evidence
   (service contract first, transport adapters second, typed context/errors, explicit ports/events);
3. для каждого нового ecommerce/marketplace модуля до первого UI/transport PR заводить
   module-local `docs/implementation-plan.md` с FFA/FBA status block и строку в central readiness board;
4. при обновлении execution checkpoint явно отмечать, какие FFA invariants и FBA guardrails были
   проверены в конкретном срезе.

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

0. восстановить FBA-first stabilization gate для уже готовых ecommerce slices: зафиксировать boundary evidence
   по `product/cart/order/checkout/fulfillment/pricing/inventory` и post-order orchestration, убрать
   host/transport-owned business semantics, описать explicit ports/events и только затем расширять
   функциональный roadmap;
1. выполнить UI split по module ownership: вынести domain UI из aggregate `rustok-commerce-admin` / `rustok-commerce-storefront` в соответствующие split-модули (`product`, `order`, `inventory`, `pricing`, `fulfillment`, `customer`, `region`), оставив `rustok-commerce` только orchestration/cross-domain surfaces;
2. продолжить `Phase 7` от уже внедрённого seller-aware grouping и typed fulfillment-item model к post-order delivery changes;
3. перевести `Phase 8` в Pricing 2.0 с channel-aware price resolution, price lists, rules и promotions;
4. вынести `Phase 9` в отдельный tax domain с tax lines и provider seam;
5. закрыть `Phase 10` post-order surface (`returns/refunds/exchanges/claims/order changes`);
6. стабилизировать `Phase 11` provider architecture для payment/fulfillment только после FBA-readiness evidence для уже готовых payment/fulfillment/order boundaries;
7. зафиксировать `Phase 12` как Medusa parity matrix и release discipline;
8. после foundation + backfill + FBA gate открыть отдельный marketplace/seller-platform phase, а не наращивать seller portal/RBAC/payouts внутри umbrella `commerce`.

Ближайший execution slice:

- сначала продолжить уже начатый UI split: product admin route вынесен в `rustok-product/admin`, shipping-option admin route вынесен в `rustok-fulfillment/admin`, customer admin route вынесен в `rustok-customer/admin`, order admin route вынесен в `rustok-order/admin`, inventory admin route вынесен в `rustok-inventory/admin` с native set/adjust/reserve/release quantity и check-availability actions без GraphQL fallback, pricing admin route вынесен в `rustok-pricing/admin`, region admin route вынесен в `rustok-region/admin`, storefront split уже идёт через `rustok-region/storefront`, `rustok-product/storefront`, `rustok-pricing/storefront` и `rustok-cart/storefront`, а aggregate `rustok-commerce-storefront` уже сжат до aggregate checkout workspace, где seller-aware delivery-group shipping selection UI принадлежит `rustok-fulfillment-storefront`, а commerce удерживает только временный transport callback;
- параллельно закрепить `Marketplace Foundations`: canonical `seller_id` в product/cart/order/checkout/fulfillment contract, seller-aware grouping по `seller_id`, transitional compatibility для legacy `seller_scope` и подготовку seller-owned read model без разворачивания полного marketplace feature set; marketplace/seller-platform surface открывается только как новый FFA/FBA-first module boundary после foundation/backfill;
- `Phase 7` теперь уже закрыт до explicit `reopen` / `reship` semantics поверх seller-aware grouping, typed fulfillment-item model, manual post-order create path и partial ship/deliver baseline;
- и теперь уже идти в channel-aware pricing.

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
- post-order delivery changes и item-level delivery recovery поверх уже введённого seller-aware deliverability baseline, typed fulfillment-item model, manual post-order create path и partial ship/deliver baseline;
- seller portal, merchant RBAC surfaces, commissions/payouts/settlement и disputes/returns marketplace-policy сознательно вынесены за рамки ближайшего scope и не входят в foundation slice;
- channel-aware price resolution больше не является чистым backlog: `Phase 8` уже получил host-channel-aware resolver foundation (`channel_id/channel_slug`, channel-scoped base rows и channel-filtered active price lists), но promotion/rule layering и полноценный authoring UX всё ещё остаются внутри `Phase 8`;
- полноценный promotion/discount domain поверх price rules, а не только `compare_at_amount` и service-level `apply_discount`;
- отдельный tax domain: foundation уже начат (tax lines + totals), `rustok-tax` уже вынес default `region_default` calculation в отдельный module boundary, а provider seam и typed `provider_id` tax-line contract уже введены; backlog теперь смещается в richer tax rules и внешние tax engines вместо продолжения hardcoded region-only runtime;
- post-order слой уровня Medusa: returns, exchanges, claims, order changes, draft/edit flows, refund transport;
- provider registry для payment/fulfillment, webhook ingestion и внешний gateway/carrier story.

## Backlog противоречий

| ID | Противоречие | Что нужно сделать |
| --- | --- | --- |
| `BL-01` | umbrella module vs дальнейший split | продолжать вынос устойчивых bounded contexts в отдельные crates, оставляя `rustok-commerce` orchestration/root layer |
| `BL-02` | entities vs migrations vs indexer SQL | держать schema hardening, migration smoke и Postgres-first tests обязательными |
| `BL-03` | inventory model hardening | выравнивать read/write path вокруг stock locations, levels, reservations, exported inventory-owned case-insensitive backorder policy helper и channel-aware availability |
| `BL-04` | transport parity vs domain completeness | не путать наличие `/store/*` и `/admin/*` transport с фактическим Medusa parity по домену |
| `BL-05` | `/admin/*` и `/store/*` vs embedded UI routes | держать route precedence, OpenAPI и router smoke tests под постоянной регрессией |
| `BL-06` | Medusa parity scope | расширять contract tests по официальным Medusa docs, не inventing local semantics |
| `BL-07` | platform `channel` уже есть, а commerce остаётся channel-blind | сделать catalog/cart/order/pricing/inventory/fulfillment channel-aware поверх `rustok-channel`, без второго sales-channel слоя |
| `BL-08` | pricing rows vs merchandising model | перейти от базовых цен и `compare_at_amount` к price lists, rules, tiers, adjustments и promotions |
| `BL-09` | region tax flags vs отдельный tax domain | вынести tax calculation/rules/providers из плоской `region`-модели в отдельный bounded context |
| `BL-10` | линейный order lifecycle vs post-order reality | добавить returns, refunds, exchanges, claims, order changes и draft/edit semantics |
| `BL-11` | manual/default providers vs extensibility | стабилизировать payment/fulfillment provider SPI вместо смешивания базовой модели с внешними интеграциями |
| `BL-12` | typed shipping profile registry, typed product/variant bindings, seller-aware line-item snapshots, cart delivery groups, multi-fulfillment checkout и typed fulfillment items уже есть, но deliverability model ещё не закрыта до post-order уровня | довести deliverability domain от seller-aware grouping и typed fulfillment items до post-order delivery changes |
| `BL-13` | split backend уже есть, но storefront и часть admin ownership всё ещё агрегированы в `rustok-commerce-storefront` и оставшихся umbrella admin routes | разнести admin/storefront UI по split ecommerce-модулям и оставить umbrella UI только для cross-domain orchestration surfaces |
| `BL-14` | seller-aware grouping уже есть, но marketplace identity boundary исторически опиралась на `seller_scope` вместо стабильного opaque key | закрепить `seller_id` как canonical multivendor boundary в product/cart/order/fulfillment contracts, оставив `seller_scope` только как transitional compatibility field до backfill |

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
- `rustok-cart` уже публикует собственный module-owned storefront UI package `rustok-cart/storefront` для storefront cart inspection, safe decrement/remove line-item actions и seller-aware delivery-group snapshot;
- aggregate `rustok-commerce-storefront` больше не держит published catalog/pricing read-side и сжат до aggregate checkout workspace с request/tenant/channel context summary, seller-aware delivery-group shipping selection, `payment collection` reuse и `complete checkout` actions по `?cart_id=`;
- aggregate `rustok-commerce-admin` больше не дублирует product/shipping-option flows и оставлен только под shipping profiles.

Следующие шаги:

- [x] вынести region UI по ownership boundary модуля;
- [x] начать отдельный storefront split через `rustok-region/storefront`;
- [x] вынести product storefront read-side из aggregate `rustok-commerce-storefront`;
- [x] вынести pricing storefront read-side из aggregate `rustok-commerce-storefront`.
- [x] сжать `rustok-commerce-storefront` до aggregate checkout workspace без catalog/pricing ownership.
- [x] вынести cart storefront inspection read-side в `rustok-cart/storefront`.
- [x] оставить в `rustok-commerce-storefront` только aggregate checkout workspace для delivery-group shipping selection, `payment collection` и `complete checkout`.

### Phase 2. Medusa-style transport baseline

Статус: `done`

- live REST surface поднят на `/store/*` и `/admin/*`;
- реализованы storefront routes `products`, `regions`, `shipping-options`, `carts`, `payment-collections`, `orders/{id}`, `customers/me`;
- реализованы admin routes для `products`;
- OpenAPI и route contract tests привязаны к live surface без legacy compatibility layer.


### Inventory availability compatibility tail

Статус: `in progress`

Этот хвост относится к umbrella ecommerce orchestration, а не к inventory admin UI scope.
`rustok-inventory` уже владеет admin read/write facade и public-channel inventory
availability/projection helpers; `rustok-commerce` должен оставаться thin compatibility
layer для storefront/checkout flows.

Текущие правила:

- GraphQL cart line quantity mutations, checkout cart inventory validation и store REST cart
  validation должны вызывать `rustok_inventory::check_variant_availability_for_public_channel`
  вместо прямой связки backorder policy + channel-visible inventory loader.
- Storefront product DTO projection должна вызывать
  `rustok_inventory::load_inventory_projection_by_variant_for_public_channel` и только
  применять `PublicChannelInventoryProjection.available_quantity/in_stock` к commerce DTO.
- Commerce callers не должны напрямую вызывать
  `load_available_inventory_for_variant_in_public_channel`,
  `load_available_inventory_by_variant_for_public_channel` или
  `inventory_policy_allows_backorder` для storefront availability decisions.
- Дальнейшая работа по stock locations, reservations и channel-aware availability edge-cases
  фиксируется здесь как umbrella compatibility/parity work и должна сопровождаться
  integration tests для checkout/catalog visibility flows.

Быстрые guardrails:

```bash
node scripts/verify/verify-inventory-admin-boundary.mjs
./scripts/verify/verify-all.sh inventory-admin-boundary
```

Следующие шаги:

- [ ] добавить targeted integration coverage для channel-aware inventory visibility edge-cases
  через storefront catalog/cart/checkout path;
- [ ] удерживать REST/GraphQL/native parity для checkout-facing inventory availability
  после расширения stock locations/reservation semantics;
- [ ] при изменении public-channel inventory semantics синхронизировать
  `crates/rustok-inventory/docs/implementation-plan.md`, этот commerce roadmap и
  `docs/modules/registry.md`.

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
- storefront payment-collection и complete-checkout paths перепрайсят cart line items перед созданием payment collection и перед `complete checkout`, чтобы price-list/quantity-tier изменения не оставляли stale pricing snapshot: line items при скидке нормализуются в `base/compare_at unit_price` плюс pricing-owned `cart_adjustments`, а payment collection продолжает брать net `cart.total_amount`.
- cart/order totals теперь дополнены first-class `shipping_total`: выбранные shipping options входят
  в persisted `cart.total_amount`, checkout snapshot'ит `shipping_total` в order и payment collection
  больше не живёт на subtotal-minus-adjustments без доставки.
- поверх этого base contract `rustok-cart` уже начал typed shipping-promotion layer: percentage/fixed
  shipping discounts живут в `cart_adjustments` как `scope=shipping`, checkout snapshot'ит их в order,
  а payment collection остаётся привязанным к тому же net total без hidden fallback на старую семантику.
- cart checkout lifecycle guardrails дополнительно зафиксированы на service coverage: `checking_out` carts
  отклоняют mutation paths для typed promotions и generic adjustment writes, `release_checkout`
  восстанавливает допустимые мутации без выставления `completed_at`, а `complete_cart` оставляет
  cart в финальном `completed` состоянии без повторного checkout/release.
- umbrella-level regression expectations синхронизированы с cart boundary: checkout recovery в `rustok-commerce` обязан сохранять visibility этих guardrails на transport/service уровне (re-entry/release/complete invariants не могут «прятаться» только в unit coverage cart-модуля).

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
- storefront GraphQL discovery/read surface включает `storefrontRegions` и `storefrontShippingOptions`, включая cart-context precedence над конфликтующим query currency; дополнительно storefront facade теперь отдаёт `storefrontPricingChannels`, `storefrontActivePriceLists(channelId, channelSlug)`, `storefrontPricingProduct` и `adminPricingProduct`, чтобы module-owned pricing fallback не терял ни channel-aware selector parity, ни variant-level `effective_price` parity.
- admin GraphQL facade теперь также держит pricing write mutations для
  `updateAdminPricingVariantPrice`, `previewAdminPricingVariantDiscount` и
  `applyAdminPricingVariantDiscount`, чтобы pricing-owned admin write path имел
  не только native `#[server]`, но и parallel GraphQL transport поверх того же
  `PricingService`.
- generic catalog roots `product` / `storefrontProduct` при этом зафиксированы как catalog-authoritative surface, а их `variants.prices` остаётся только compatibility snapshot без статуса pricing-authoritative контракта.

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

Что уже начато в текущем срезе:

- cart получил `cart_tax_lines`, `tax_total` и пересчёт налога поверх line items + выбранных shipping options;
- order получил `order_tax_lines`, `tax_total`, `tax_included` и snapshot tax lines при checkout/create-order;
- tax-inclusive vs tax-exclusive семантика фиксируется в metadata tax line (`tax_included`);
- REST/GraphQL/Leptos DTO для cart/order начали возвращать tax lines и totals.

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
- channel-aware price resolution сознательно не считается полностью закрытым в этой фазе: foundation уже уехал в `Phase 8`, но promotion/rule layering и authoring UX там ещё продолжаются, чтобы не смешивать storefront availability с pricing 2.0;
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
- product create/update/read contracts теперь тоже принимают nullable `seller_id`, чтобы seller identity приходила из catalog write-side, а не вычислялась из merchandising/display полей вроде `vendor`;
- cart line items теперь хранят effective `shipping_profile_slug` и canonical seller snapshot (`seller_id`), cart response отдаёт seller-aware `delivery_groups[]`, а store/GraphQL checkout input принимает typed `shipping_selections[]`;
- `cart_shipping_selections` теперь персистит seller-aware key по `(shipping_profile_slug, seller_id)` и fallback'ится к `seller_scope` только для legacy записей без `seller_id`;
- `CheckoutService` теперь валидирует stale shipping-profile snapshots, режет несовместимые selections по delivery groups и создаёт отдельный fulfillment на каждую delivery group;
- storefront REST/GraphQL и module-owned storefront UI packages (`rustok-cart/storefront`, `rustok-commerce/storefront`) теперь прокидывают seller-aware delivery-group contract end-to-end с canonical `seller_id`, а fulfillment metadata сохраняет language-agnostic seller identity без seller display text;
- fulfillment boundary теперь хранит typed `fulfillment_items` и checkout связывает delivery groups с `order_line_item_id`, так что item scope больше не держится только на `delivery_group.line_item_ids` внутри metadata;
- admin REST/GraphQL теперь тоже умеют manual post-order `create fulfillment` с typed `items[]`: create path валидирует `order_line_item_id` против заказа, remaining quantity против уже созданных non-cancelled fulfillments и не даёт смешивать разные seller-aware delivery groups в одном follow-up fulfillment;
- `FulfillmentService` теперь держит item-level `shipped_quantity` / `delivered_quantity`, а admin REST/GraphQL `ship` / `deliver` принимают optional quantity adjustments по `fulfillment_item_id`, так что partial post-order delivery progress и audit trail живут в typed fulfillment boundary;
- admin REST/GraphQL теперь уже умеют explicit `reopen` / `reship` для fulfilled/cancelled recovery path: delivered fulfillments можно возвращать в `shipped`, cancelled fulfillments можно возвращать в actionable state, а delivery corrections больше не требуют неявных status hacks;
- `CompleteCheckoutResponse` и storefront GraphQL checkout surface теперь возвращают `fulfillments[]`, а singular `fulfillment` остаётся только compatibility shim для single-group carts;
- прежний strict single-option mixed-cart invariant больше не является целевой архитектурой: он сохранён только как compatibility shortcut для cart'ов с одной delivery group;
- regression tests уже покрывают effective-profile resolution, mixed-cart delivery groups, missing per-group selection, multi-fulfillment checkout, stale snapshot reject path и GraphQL checkout parity для новых полей.

Обязательные проверки:

- contract tests на несовместимые товары и shipping options;
- migration tests для product/variant/cart/order shipping-profile schema;
- integration tests на mixed cart с разной fulfillment policy;
- regression tests на preflight checkout failures, которые должны отпускать `checking_out` lock и не создавать payment/order artifacts до side effects.

### Cross-cutting. Marketplace Foundations

Статус: `in progress`

Фокус:

- добавить минимальный multivendor foundation без разворачивания seller portal, payouts, commissions или disputes;
- закрепить `seller_id` как canonical seller identity key для ecommerce write-side и orchestration;
- не хранить seller display label в ecommerce storage и не использовать `vendor` как seller identity.

Что уже закрыто:

- product create/update/read contracts теперь включают nullable `seller_id`;
- cart line items, `cart_shipping_selections`, order line items и fulfillment delivery-group metadata теперь несут `seller_id` как canonical key;
- cart grouping и checkout/manual fulfillment validation теперь опираются на `(shipping_profile_slug, seller_id)` и fallback'ятся к `seller_scope` только для legacy записей без `seller_id`;
- typed `cart_adjustments` и `order_adjustments` закрепляют promotion/discount snapshot как language-neutral бизнес-данные: source identity хранится через `source_type/source_id`, суммы через `amount/currency_code`, а display label не попадает в ecommerce storage;
- REST, GraphQL и Leptos `#[server]` contracts для product/cart/checkout/manual fulfillment уже расширены полем `seller_id`;
- seller display label больше не персистится в ecommerce storage, а legacy `seller_scope` сохраняется только как transitional compatibility field.

Ближайшие шаги:

- подготовить seller-owned read model/resolver для display label по `seller_id` и effective locale;
- сделать отдельный migration/backfill slice, после которого можно будет вычищать compatibility-path для legacy `seller_scope`;
- перед seller portal, merchant RBAC, commissions, payouts, settlement и disputes завести отдельный marketplace/seller-platform module plan с FFA/FBA status block, canonical service contract, data ownership, typed context/errors, explicit ports/events и transport parity DoD;
- не расширять текущий scope в merchant RBAC, seller portal, payouts, commissions и disputes до завершения foundation + backfill + FBA-readiness gate для уже готовых ecommerce boundaries.

### Phase 8. Pricing 2.0 и promotions

Статус: `in progress`

Фокус:

- выйти за рамки `prices.amount` / `compare_at_amount` / service-level `apply_discount`;
- добавить price lists, rules, tiers и adjustments;
- вынести promotions в отдельный bounded context вместо implicit price mutation.

Deliverables:

- pricing context `channel + region + currency + customer segment` там, где он действительно нужен;
- price lists и rule-driven resolution;
- typed cart/order adjustments как отдельный business snapshot слой, не смешанный с base price rows, price-list rows или localized display metadata;
- promotion engine для item/order/shipping discounts без смешивания с базовой price storage.

Что уже начато в текущем срезе:

- `rustok-pricing::PricingService` уже получил typed resolver foundation
  `PriceResolutionContext -> ResolvedPrice` поверх base-price rows;
- pricing resolution contract уже hardened: `currency_code` валидируется как
  трёхбуквенный ASCII business code, `quantity < 1` отклоняется, а GraphQL roots
  `adminPricingProduct` / `storefrontPricingProduct` не принимают `region_id`,
  `price_list_id` или `quantity` без явного `currencyCode`;
- active precedence уже deterministic: exact `region_id` имеет приоритет над global price,
  quantity tiers выбираются по более специфичному `min_quantity` и более узкому `max_quantity`;
- explicit active `price_list_id` overlay уже активирован в resolver, а
  module-owned pricing admin/storefront surfaces уже получили pricing-owned
  active price-list selector поверх этого read contract.
- `rustok-pricing/admin` больше не является чисто read-only surface: module-owned
  server-function transport уже покрывает base-price updates по variant prices,
  active `price_list` overrides и rule/scope editing для active price lists.
- quantity tiers теперь тоже получили минимальный write path в `rustok-pricing/admin`:
  оператор может задавать `min_quantity` / `max_quantity` для variant price rows, а
  resolver сразу использует эти окна при effective-price selection.
- тот же module-owned admin write path теперь уже умеет и active `price_list_id`
  overrides поверх base prices, а transport parity покрыт SSR tests на happy path и permission gate.
- поверх этого pricing runtime теперь уже возвращает typed `discount_percent` в resolved/effective
  price contract, а module-owned admin/storefront surfaces показывают sale math без ad-hoc
  вычисления только из `compare_at_amount`.
- parallel admin GraphQL transport теперь тоже закрывает не только base-row writes:
  `updateAdminPricingVariantPrice`, `previewAdminPricingVariantDiscount`,
  `applyAdminPricingVariantDiscount`, `updateAdminPricingPriceListRule` и
  `updateAdminPricingPriceListScope` работают поверх того же `PricingService`,
  сохраняя lifecycle/scope parity с native pricing admin transport.
- legacy service-level `apply_discount` тоже уже начал сжиматься до compatibility слоя:
  typed percentage-adjustment preview/apply path теперь живёт внутри `rustok-pricing` и
  работает по canonical base-price row или по выбранному active `price_list` override.
- targeted transport parity для этого admin write path уже заметно расширен: `rustok-pricing/admin`
  имеет SSR coverage не только на native `update-variant-price`, но и на
  rule/scope lifecycle, inactive time-window guards и channel mismatch без hidden fallback.
- поверх этого `rustok-pricing/admin` уже получил module-owned operator flow для typed
  percentage-discount preview/apply по canonical base row; теперь тот же flow уже умеет
  target'ить и selected active `price_list` override, а SSR tests покрывают не только raw
  price row updates, но и admin transport parity для adjustment path.
- следующий promotion-ready слой тоже уже начат внутри pricing boundary: active `price_list`
  теперь может держать typed percentage rule, `PricingService::resolve_variant_price`
  умеет fallback'иться к base row через это правило при отсутствии explicit override row,
  а `rustok-pricing/admin` уже умеет редактировать этот rule через module-owned server functions.
- pricing-focused GraphQL/runtime parity тоже уже расширен: `adminPricingProduct`,
  `storefrontPricingProduct` и active price-list selectors проходят полный parity sweep
  вместе с остальным `graphql_runtime_parity_test`, а clear/scope updates не оставляют
  stale selector metadata.
- cart/order promotion representation теперь тоже имеет typed foundation: `rustok-cart` хранит `cart_adjustments`,
  пересчитывает `subtotal_amount`, `adjustment_total` и net `total_amount`, а `rustok-order` snapshot'ит
  `order_adjustments` при checkout/create-order и отдаёт тот же summary в REST/GraphQL/Leptos-facing DTO;
  этот слой не хранит seller/product/promotion display labels и остаётся устойчивым к смене default locale;
  storefront repricing при наличии скидки теперь фиксирует в line item не effective sale price, а `base/compare_at`
  `unit_price`, пока discount savings живут в typed adjustment snapshot.
- storefront/admin GraphQL parity теперь тоже покрывает этот snapshot layer: storefront cart/query + checkout
  сохраняют typed `adjustments`, payment collection использует net `cart.total_amount`, а completed order
  переносит sanitized adjustment metadata без `display_label`.
- storefront/admin REST transport теперь тоже покрывает этот snapshot layer: controller tests для
  `/store/carts/{id}` и `/admin/orders/{id}` фиксируют typed `adjustments`, sanitized metadata и
  текущую shipping-selection semantics, где incompatible selection может soft-clear'иться до `null`,
  а verification baseline для umbrella-модуля снова включает полный `cargo test -p rustok-commerce --lib`.
- storefront GraphQL add-to-cart теперь резолвит `unit_price` через `PricingService` с тем же
  `PriceResolutionContext` (currency + region + channel + quantity), а не через raw `price` row;
  это выравнивает pricing semantics между REST и GraphQL storefront cart path и даёт общий
  `base unit_price + pricing adjustment` snapshot contract; add-to-cart write path теперь
  пишет этот snapshot атомарно в одной cart-транзакции, а не через отдельный follow-up repricing step.
- storefront cart quantity update теперь переоценивает line items через pricing resolver,
  чтобы quantity tiers и channel-aware pricing применялись при изменении количества без записи
  effective sale price прямо в persisted `unit_price`.
- storefront cart context update (region/country/locale/shipping selections) теперь
  перепрайсит все line items через pricing resolver, чтобы смена контекста не оставляла
  stale pricing snapshot и пересобирала `base unit_price + adjustments` под новый storefront context.
- typed promotion runtime поверх snapshot layer тоже уже начат в `rustok-cart`: cart service умеет
  preview/apply percentage/fixed promotions на cart-level и line-item scope, не перетирая pricing-owned
  adjustments и сохраняя order/payment snapshot parity через существующий checkout flow.
- operator-side GraphQL transport над этим runtime тоже уже есть: admin mutations умеют preview/apply
  typed cart promotions для `cart`, `line_item` и `shipping` scope, используя тот же `CartService`
  вместо отдельного promotion-specific storage или ad hoc adjustment writer.
- native-first operator transport поверх того же runtime теперь тоже есть в `rustok-commerce-admin`:
  package-level `#[server]` functions умеют preview/apply typed cart promotions для `cart`,
  `line_item` и `shipping` scope, используют тот же `CartService`, держат тот же permission contract
  (`orders:read` для preview, `orders:update` для apply) и покрыты SSR tests на shipping scope,
  target validation и permission gate.

Обязательные проверки:

- deterministic price-resolution tests;
- contract tests на priority/override semantics;
- cart/order adjustment snapshot tests: net total, source identity, line-item binding и отсутствие localized display labels в storage;
- checkout regression tests: cart adjustments должны snapshot'иться в order adjustments, а payment collection должен использовать net `cart.total_amount`;
- regression tests на rounding и decimal money contract;
- transport tests на price + promotion representation в `/store/*`, `/admin/*` и GraphQL,
  включая storefront parity для `shipping_total` и shipping-scoped promotion snapshot.

### Phase 9. Tax domain

Статус: `in progress`

Фокус:

- перестать считать `region.tax_rate` и `region.tax_included` достаточной tax-моделью;
- ввести отдельный tax bounded context с tax lines, rules и provider seam;
- не ломать текущий checkout flow при постепенном переходе.

Deliverables:

- tax calculation context поверх cart/order/shipping;
- tax lines для line items и shipping;
- provider seam для внешних tax engines;
- migration path от плоской region tax policy к более реалистичной модели.

Что уже начато в текущем срезе:

- `rustok-tax` введён как отдельный bounded context для tax calculation contract вместо продолжения hardcoded tax runtime внутри `rustok-cart`;
- default provider `region_default` сохраняет текущую семантику `region.tax_rate` / `tax_included`, но теперь живёт за provider seam;
- текущий provider selection hook проходит через `regions.tax_provider_id`; неизвестный provider режется как validation error ещё в cart runtime вместо скрытого fallback;
- `rustok-cart` больше не считает tax lines напрямую из region helper-кода: cart runtime вызывает `TaxService` и snapshot'ит provider-aware tax lines;
- `cart_tax_lines` и `order_tax_lines` теперь несут first-class `provider_id`, а checkout переносит этот snapshot в order без hidden metadata-only fallback;
- targeted regression уже фиксирует, что complete checkout сохраняет `provider_id=region_default` в cart/order tax lines вместе с `tax_included` metadata.
- cart runtime теперь уже учитывает channel-aware provider mapping из region metadata key `channel_tax_provider_ids`: при наличии `cart.channel_id` tax pipeline передаёт `channel_provider_id` в `TaxService` с precedence поверх region `tax_provider_id`.

Обязательные проверки:

- integration tests `cart -> taxes -> payment -> order`;
- negative tests на конфликт tax-inclusive/exclusive semantics;
- contract tests на transport shape tax lines.

Ближайший execution slice (продолжение coding-плана):

- [x] добавить channel-aware provider mapping (`regions.tax_provider_id` + `channel_id`) без hidden fallback на `region_default`;
- [x] расширить `rustok-tax` до typed rule input (`item class`, `shipping class`, `customer tax-exempt`) без возврата налоговой логики в `rustok-cart`;
- [x] закрепить admin/store read-side tax breakdown contract (line-item vs shipping vs order aggregate) в REST и GraphQL parity тестах;
- [x] добавить migration/contract smoke для backfill `provider_id` в legacy `order_tax_lines` snapshots.

### Phase 10. Post-order flows: returns, refunds, exchanges, claims, order changes

Статус: `in progress`

Фокус:

- выйти за рамки линейного `pending -> confirmed -> paid -> shipped -> delivered/cancelled`;
- сделать refund/return semantics частью домена, а не только state-machine helper;
- добавить order-change/draft-edit слой, нужный для Medusa-style OMS behavior.

Deliverables:

- return/refund records и lifecycle;
- exchanges / claims как order-change-backed post-order decisions в целевом Medusa parity scope;
- order change / draft order / preview-apply semantics;
- admin/store transport для post-order сценариев.

Текущее состояние:

- стартовый refund slice уже поднят поверх `payment-collections`: `rustok-payment` теперь хранит first-class `refunds`, `PaymentService` умеет `create/list/show/complete/cancel`, а aggregate `PaymentCollectionResponse` возвращает `refunded_amount` и `refunds[]`;
- admin REST/GraphQL уже публикуют первый post-order refund transport (`/admin/payment-collections/{id}/refunds`, `/admin/refunds*`, `createRefund`, `completeRefund`, `cancelRefund`, `refunds`), так что Phase 10 больше не начинается с нуля;
- следующий объём внутри Phase 10 остаётся шире refund-only baseline: returns, exchanges/claims и order-change/draft-edit semantics;
- claims scope decision зафиксирован без отдельного storage owner в `rustok-commerce`: claim decision создаёт order-owned `order_change` с `change_type=claim`, завершает return как `resolution_type=claim` с `order_change_id` и оставляет дальнейший lifecycle в `rustok-order`.


Execution slices (Phase 10):

- [x] Slice 10.1: returns foundation (`rustok-order` storage + service lifecycle + admin REST/GraphQL read/write transport). Storage/read baseline was started earlier; this slice added show/read, complete/cancel lifecycle, REST routes `/admin/returns/{id}`, `/admin/returns/{id}/complete`, `/admin/returns/{id}/cancel`, GraphQL `orderReturn(s)` + `create/complete/cancelOrderReturn`, OpenAPI registration and targeted lifecycle tests. Item-level return lines closed in this slice via `order_return_items`; added resolution references of completed return (`resolution_type/refund_id/order_change_id`), and the umbrella complete-return REST/GraphQL helper creates/optionally completes refund via `PaymentService` and passes `refund_id`; exchange and claim helpers are also automated.
- [x] Slice 10.2: refund transport parity expansion (store/customer-safe read-side + ownership/RBAC contract tests).
- [x] Slice 10.3: order-change groundwork (draft edit snapshot + preview/apply contract skeleton without host-owned logic). Started in `rustok-order`: `order_changes` storage/service skeleton with `pending -> applied|cancelled` lifecycle and service tests. This slice added umbrella admin REST routes `/admin/orders/{id}/changes`, `/admin/order-changes*`, lifecycle routes `apply/cancel`, OpenAPI contract registration and GraphQL parity roots `orderChange(s)` + mutations `create/apply/cancelOrderChange`; further: storefront customer-facing read-side `GET /store/orders/{id}/changes` + GraphQL `storefrontOrderChanges` with customer ownership guard closed; linking of changes with refund/exchange orchestration closed via `PostOrderOrchestrationService.apply_exchange_order_change` / `apply_claim_order_change`.
- [x] Slice 10.4: exchanges/claims scope decision + parity matrix update in this plan and module docs. Decision tree brought to transport-level parity: admin REST `POST /admin/orders/{id}/returns/decision` and admin GraphQL `createOrderReturnDecision` use the same `PostOrderOrchestrationService` and publish unified `ReturnDecisionResponse` (`return_only/refund/exchange/claim`) without host-owned logic. Claims scope decision fixed as order-change-backed claim (`change_type=claim`) with `order_return_id` in preview/metadata and completed return `resolution_type=claim/order_change_id`; live REST and GraphQL runtime parity additionally check claim response output (`order_return/orderReturn`, `order_change/orderChange`, `refund=null`) against runtime service semantics. Dedicated claim storage/API remains out of scope until a dedicated bounded context is introduced. The UX slice added in `rustok-commerce-admin` a post-order change operator for `orderChanges` with `apply/cancel` actions, and the next increment transitioned this operator to native-first `#[server]` API over `OrderService` with SSR tests on pending filter, apply/cancel lifecycle, and permission gates; GraphQL fallback is kept for unavailable native transport. Exchange/claim helper metadata also marks created order changes `return_decision_action` / `return_decision_source`, and operator UI displays resolution summary cards from order-change preview/metadata without moving domain rules to host.

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
- контрактные тесты покрывают все публичные use-case;
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
5. Любые изменения схемы проходят i18n-аудит: локализованные строки не храним в base-таблицах, display-поля живут только в `*_translations`.
6. Module-owned UI пакеты не вводят package-local locale override: write-side использует host-provided effective locale, а edit/detail hydration резолвит переводы по нему же, с fallback только после попытки точного locale match.
7. Read-side/runtime helpers не сравнивают locale raw-строкой: резолв локализованных данных идёт через shared locale normalization и одну цепочку fallback `requested -> tenant default -> first available`.


## Quality backlog

- [x] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [x] Проверить полноту и актуальность `README.md` и локальных docs.
- [x] Зафиксировать/обновить verification gates для текущего состояния модуля.
