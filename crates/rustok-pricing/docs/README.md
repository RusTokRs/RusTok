# Документация `rustok-pricing`

`rustok-pricing` — дефолтный pricing-подмодуль семейства `ecommerce`.

## Назначение

- price-related service logic;
- pricing migrations;
- `PricingModule` и `PricingService`;
- typed resolver foundation `PriceResolutionContext -> ResolvedPrice` для
  deterministic price selection по `currency_code`, optional `region_id` и optional
  `quantity`, а также explicit `price_list_id` overlay для активных price list
  без ввода полноценного promotions слоя; read-side теперь ещё и возвращает
  нормализованный `discount_percent` для sale rows и effective prices; текущий
  resolver уже также учитывает host-provided `channel_id` / `channel_slug` и
  умеет выбирать channel-scoped base rows / active price lists без переноса
  ownership channel identity в pricing boundary; contract validation требует
  трёхбуквенный ASCII `currency_code`, отклоняет `quantity < 1`, не позволяет
  передавать `region_id`, `price_list_id` или `quantity` без `currency_code` и
  также отклоняет malformed explicit `channel_id`; pricing UI wrappers при этом
  валидируют этот contract до fallback с native `#[server]` transport на GraphQL;
- typed percentage-adjustment contract в `PricingService`: preview/apply helper
  для percent-based sale mutation теперь живёт в pricing boundary, а legacy
  `apply_discount` остаётся compatibility wrapper поверх canonical base-price row;
  typed adjustment path уже умеет target'ить не только base row, но и active
  `price_list` override rows, включая channel-scoped canonical rows;
- pricing-owned read contract для active tenant-scoped price lists, чтобы
  admin/storefront surfaces не жили на raw UUID-only selector semantics; теперь
  этот read contract ещё и несёт typed rule metadata;
- first-class `price_list` percentage rules, чтобы active list мог давать
  promotion-ready sale semantics поверх base-price rows даже без явных override rows;
- transport parity для admin-side `price_list` rule/scope mutation paths:
  future/expired lists и channel-scope mismatch теперь должны отклоняться без
  hidden fallback и без побочной записи/мутации override rows;
- selector активных price lists не дрейфует после scope save/clear: channel-bound
  lists исчезают из чужого канала и возвращаются после снятия scope;
- module-owned admin UI пакет `rustok-pricing/admin` для price visibility,
  sale markers, currency coverage inspection и operator-side effective price preview по
  `currency + optional region_id + optional quantity` через native-first `#[server]`
  transport с GraphQL fallback, а также для authoring базовых variant price rows и
  active `price_list_id` override rows, включая quantity tiers по `min_quantity` /
  `max_quantity`, а теперь ещё и для typed percentage-discount preview/apply по
  canonical base row или выбранному active `price_list` override, плюс для editing
  selected active `price_list` rule и channel scope у variant price rows / active
  price lists; channel scope authoring при этом теперь берёт selector options из
  `rustok-channel` read model, а не из raw UUID/slug text inputs; active
  `price_list` selector в admin effective context при этом тоже уже
  пересчитывается от явно выбранного `channel_id` / `channel_slug`, а не
  живёт на bootstrap snapshot host context;
- module-owned storefront UI пакет `rustok-pricing/storefront` для public pricing
  discovery, currency coverage, sale-marker visibility и effective price preview по
  optional route context (`currency`, `region_id`, `price_list_id`, `channel_id`,
  `channel_slug`, `quantity`) через native server functions;
  effective context для channel-aware pricing при этом не строится из package-local
  fallback chain: locale остаётся host-owned, а channel override приходит только как
  explicit route/server-function input или из host `RequestContext`; GraphQL fallback
  при этом теперь тоже получает `available_channels` и channel-aware active
  `price_lists` через storefront facade поля `storefrontPricingChannels` и
  `storefrontActivePriceLists(channelId, channelSlug)`, а не деградирует до пустого
  selector state; pricing detail fallback при этом тоже больше не живёт на generic
  catalog product contract и использует dedicated facade roots `storefrontPricingProduct`
  и `adminPricingProduct`, чтобы сохранять `effective_price` parity для explicit
  `currency/price_list/channel/quantity` context; эти facade roots валидируют
  resolution context так же строго, как `PricingService`, поэтому context modifiers
  без `currencyCode` не игнорируются молча; generic `product` /
  `storefrontProduct` при этом следует трактовать только как catalog snapshot
  contract, даже если они по-прежнему несут `variants.prices` для compatibility;

## Зона ответственности

- runtime dependency: `product`;
- модуль владеет pricing boundary и операторской UI-поверхностью для цен, включая
  base-price write path для variant pricing;
- модуль теперь владеет и публичной storefront read-side pricing-поверхностью,
  которая строит pricing atlas поверх published catalog и variant-level prices;
- текущий active resolver использует deterministic precedence
  `explicit override row -> active price_list rule -> base prices`,
  затем `exact region -> global` и `higher min_quantity -> lower max_quantity`;
  promotions поверх нескольких list layers и вне price-list boundary по-прежнему
  остаются отдельным follow-up;
- GraphQL и REST transport для promotions/rules по-прежнему остаются в фасаде
  `rustok-commerce`, но базовый pricing write path и active price-list override authoring для admin уже вынесены в
  module-owned `rustok-pricing/admin` через native `#[server]` transport; туда же
  уже протянут typed base-row percentage adjustment path с preview/apply semantics;
  parallel GraphQL facade при этом теперь тоже держит admin pricing write surface
  для variant price updates, typed percentage-discount preview/apply и selected
  active `price_list` rule/scope updates, а не только pricing-authoritative
  read roots;
- общие DTO, entities и error surface приходят из `rustok-commerce-foundation`.

## Интеграция

- модуль входит в ecommerce family и должен сохранять собственную storage/runtime-границу
  без возврата ответственности в umbrella `rustok-commerce`;
- transport и GraphQL пока публикуются через `rustok-commerce`, а pricing-owned admin/storefront
  UX уже публикуется через `rustok-pricing/admin` и `rustok-pricing/storefront`,
  при этом admin surface уже переключился на native-first `#[server]`
  data layer с GraphQL fallback;
- изменения cross-module контракта нужно синхронизировать с `rustok-commerce`
  и соседними split-модулями.

## Проверка

- `cargo xtask module validate pricing`
- `cargo xtask module test pricing`
- targeted commerce tests для pricing-домена при изменении runtime wiring
- текущий широкий verification baseline для pricing slice включает
  `pricing_service_test`, полный `graphql_runtime_parity_test` и SSR suites
  `rustok-pricing-admin` / `rustok-pricing-storefront`

## Связанные документы

- [README crate](../README.md)
- [README admin package](../admin/README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)

## Разделение FFA для admin и storefront

Пакет admin теперь использует фасад `admin/src/transport.rs` и явный Leptos-адаптер отрисовки `admin/src/ui/leptos.rs`; корень crate только подключает слои модуля и повторно экспортирует `PricingAdmin`. Пакет storefront уже сохраняет разделение `storefront/src/core.rs`, `storefront/src/transport/` и `storefront/src/ui/leptos.rs` с паритетом native-first и GraphQL fallback.
