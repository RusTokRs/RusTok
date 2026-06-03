# План реализации `rustok-pricing`

Статус: pricing boundary выделен как отдельный модуль; модуль держит pricing runtime
baseline, module-owned admin UI уже включает base-row, active `price_list` override,
rule и scope write paths, а полный promotions engine и остальной `pricing 2.0`
остаются в активном backlog umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: ffa_admin_transport_ui_split
- Last checkpoint: Admin pricing получил FFA slice: `admin/src/transport.rs` facade над existing native/GraphQL API functions и явный Leptos render adapter `admin/src/ui/leptos.rs`; crate root стал wiring/re-export boundary, а storefront уже сохраняет `core/transport/ui` split.
- Next step: Продолжать сокращать admin/storefront `api.rs` до transport adapter implementation: вынести remaining GraphQL/native mapping policy в typed adapter helpers без изменения native-first + GraphQL fallback contract.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-06-02T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - модуль ведётся в ускоренном FFA/FBA migration track как часть ecommerce family;
  - storefront pricing route теперь использует framework-agnostic `storefront/src/core.rs` для summary/label/effective context formatting, query href building и shared `StorefrontPricingQuery`; Leptos `lib.rs` больше не владеет этой presentation/request policy;
  - storefront transport разделён на thin facade + explicit `native_server_adapter` и `graphql_adapter`, при этом fallback order (`native #[server]` first, GraphQL second) сохранён;
  - Leptos render/bind adapter выделен в `storefront/src/ui/leptos.rs`, а `storefront/src/lib.rs` стал crate-level composition/re-export boundary;
  - targeted facade tests подтверждают обе ветки orchestration: native success не вызывает GraphQL, native error передаёт исходный `StorefrontPricingQuery` в GraphQL fallback;
  - request normalization/validation перенесены в `storefront/src/core.rs`, включая typed `StorefrontPricingQueryError`; API layer конвертирует core validation errors в existing transport envelope без изменения public behavior;
  - parity evidence: `cargo test -p rustok-pricing-storefront --lib` подтверждает existing transport validation tests, pure-core route/channel formatting tests, core request validation tests и transport facade fallback tests без изменения native/GraphQL fallback contract;
  - admin FFA slice добавил module-owned `admin/src/transport.rs` facade и явный Leptos render adapter `admin/src/ui/leptos.rs`; `admin/src/lib.rs` теперь только wires modules и re-export `PricingAdmin`, а Leptos adapter больше не вызывает raw `api::*` напрямую для covered flows.
- Last verified at (UTC): 2026-06-02T00:00:00Z
- Owner: `rustok-pricing` module team

## Область работ

- удерживать `rustok-pricing` как owner pricing service boundary;
- синхронизировать pricing runtime contract, module-owned admin UI и local docs;
- не смешивать pricing storage с product catalog, promotions или tax orchestration.

## Текущее состояние

- `PricingModule`, `PricingService` и pricing migrations уже выделены;
- модуль зависит от `product`, не создавая цикла с umbrella `rustok-commerce`;
- transport adapters по-прежнему публикуются фасадом `rustok-commerce`;
- `rustok-pricing/admin` уже публикует pricing-owned admin route для price visibility,
  sale markers, currency coverage inspection, operator-side effective price context,
  selector активных price lists и write actions по base rows или active price-list
  overlays для variant prices, включая quantity tiers и typed percentage-discount
  preview/apply по canonical base row или выбранному active `price_list` override; туда
  же теперь вынесен selected active `price_list` rule editor;
- `rustok-pricing/storefront` уже публикует pricing-owned storefront route для public
  pricing atlas, currency coverage, sale-marker visibility и selector активных
  price lists поверх existing effective context; storefront presentation policy
  для summary, health/option labels, effective context и query href теперь вынесена
  в framework-agnostic `storefront/src/core.rs`, shared fetch request тоже живёт в
  `core`, transport orchestration вынесен в `storefront/src/transport/`, а
  Leptos render/bind слой живёт в `storefront/src/ui/leptos.rs`;
- storefront package по-прежнему остаётся read-side surface, но admin package уже
  использует `admin/src/transport.rs` facade поверх native-first `#[server]` transport не только для read-side, но и для
  base-row writes, active `price_list` overrides, typed percentage adjustments и
  `price_list` rule/scope editing, оставляя product GraphQL контракт как fallback
  для чтения.

## Этапы

### 1. Contract stability

- [x] закрепить pricing boundary как отдельный модуль;
- [x] удерживать зависимость `pricing -> product` без цикла на umbrella;
- [x] вынести pricing admin UI в module-owned пакет `rustok-pricing/admin`;
- [x] вынести pricing storefront UI в module-owned пакет `rustok-pricing/storefront`;
- [x] удерживать sync между pricing runtime contract, admin UI, commerce transport
  и module metadata.

### 1.1. FFA storefront decomposition

- [x] вынести pricing storefront presentation policy из Leptos компонента в
  framework-agnostic `storefront/src/core.rs`: summary, variant health, seller/channel
  labels, effective price/context formatting и route href builders;
- [x] добавить pure-core tests для query href/channel-scope formatting рядом с existing
  transport validation suite;
- [x] ввести storefront `transport/` facade с explicit `native_server_adapter` и
  `graphql_adapter`, сохранив native-first + GraphQL fallback contract;
- [x] выделить Leptos render/bind adapter в `storefront/src/ui/leptos.rs`, оставив
  crate root composition/re-export boundary;
- [x] добавить targeted tests для `transport` facade: native-success path и GraphQL
  fallback path с сохранением исходного `StorefrontPricingQuery`;
- [x] перенести request normalization/validation из `api.rs` в `core`: UUID,
  currency, quantity, channel slug и resolution context sanitization с typed error;
- [~] продолжить сокращать `api.rs` до transport adapter implementation, не меняя
  public route/transport contract.

### 2. Pricing transport split

- [~] вынести dedicated pricing read/write transport из umbrella `rustok-commerce`;
- [x] перевести pricing admin UI с read-only product-backed transport на targeted
  base-price mutations и operator workflows;
- [~] покрывать transport parity, money semantics и compare-at invariants targeted tests.

### 3. Pricing 2.0 rollout

- [~] перейти от базовых цен к rule-driven price resolution;
- [x] ввести typed resolver foundation по `currency_code + optional region_id + optional quantity`
  с deterministic precedence для base prices;
- [x] активировать explicit `price_list_id` overlay в resolver для active tenant-scoped
  price lists с base-price fallback;
- [x] добавить channel-aware foundation в resolver/read-side contract через
  host-provided `channel_id` / `channel_slug`, channel-scoped base rows и
  channel-filtered active price lists без ownership drift в `rustok-channel`;
- [x] протянуть этот же channel-aware contract в module-owned admin authoring для
  variant price rows, typed discount preview/apply и active price-list scope без
  отдельного seller/channel portal;
- [x] заменить raw `channel_id/channel_slug` authoring inputs в pricing admin на
  selector поверх `rustok-channel` read model с global fallback и legacy-scope
  compatibility option;
- [x] протянуть effective price context в module-owned storefront/admin read-side surfaces
  через native-first `#[server]` transport с GraphQL fallback;
- [x] выровнять validation contract для `PriceResolutionContext` между runtime,
  dedicated GraphQL facade roots и native `#[server]` transport: `currency_code`
  должен быть трёхбуквенным ASCII business code, `quantity < 1` отклоняется,
  а `region_id`, `price_list_id` или `quantity` без `currency_code` не
  игнорируются молча; malformed explicit `channel_id` тоже отклоняется, а не
  fallback'ится к host channel context;
- [x] вынести тот же validation step в pricing UI fetch wrappers до попытки
  native-first `#[server]` transport, чтобы invalid input не проваливался в
  бессмысленный GraphQL fallback и не размывал transport contract;
- [x] добавить explicit channel selector в storefront/admin effective-context controls,
  чтобы channel-aware resolution можно было переключать без raw query editing и без
  возврата к package-local fallback chain;
- [x] перевести admin active `price_list` selector на context-aware read path, чтобы
  список overlays и rule editor пересчитывались по явно выбранному `channel`, а не
  только по bootstrap host context;
- [x] дотянуть тот же selector metadata contract до GraphQL fallback для
  `rustok-pricing/admin` и `rustok-pricing/storefront`, чтобы degraded path не
  терял `available_channels` и channel-aware active `price_lists`;
- [x] перевести GraphQL fallback detail contract на dedicated pricing-facing facade
  roots `adminPricingProduct` / `storefrontPricingProduct`, чтобы degraded path
  сохранял variant-level `effective_price` parity для explicit resolution context;
- [x] отдать active tenant-scoped price lists как pricing-owned read contract,
  чтобы admin/storefront route выбирали overlays без raw UUID-only UX;
- [~] добавить tiers, adjustments и promotion-ready semantics;
- [~] покрывать deterministic price resolution и rounding targeted tests.

Что уже закрыто дополнительно:

- module-owned `rustok-pricing/admin` теперь имеет targeted SSR tests для native
  `update-variant-price` transport path, включая quantity-tier happy path, active
  `price_list_id` override happy path и permission gate;
- тот же admin transport теперь уже покрывает и typed `preview_percentage_discount` /
  `apply_percentage_discount` path по canonical base-price row, включая targeted SSR
  tests на happy path и permission gate; active `price_list` override adjustment path
  теперь покрыт тем же transport parity слоем;
- runtime tests уже покрывают `set_price_tier` для quantity windows, invalid tier ranges
  и normalized `discount_percent` в `ResolvedPrice`, а admin/storefront surfaces
  уже показывают sale math поверх typed read-side contract.
- targeted runtime/transport tests уже покрывают strict resolution validation:
  service-level resolver, GraphQL roots `adminPricingProduct` / `storefrontPricingProduct`
  и native `#[server]` helpers в `rustok-pricing/admin` / `rustok-pricing/storefront`
  отклоняют invalid `currency_code`, `quantity < 1`, malformed explicit `channel_id`
  и modifiers без currency.
- тот же runtime теперь ещё и покрывает channel-aware deterministic resolution:
  channel-scoped base row выигрывает у global только при совпавшем host channel,
  а active price list selector не отдаёт channel-scoped list вне его scope.
- targeted runtime tests теперь ещё и фиксируют tie-break по `max_quantity`
  при одинаковом `min_quantity`, slug-only channel matching без `channel_id`
  и fractional `discount_percent` rounding для обычных sale rows.
- service-level pricing tests теперь ещё и фиксируют channel-scope semantics у
  active `price_list`: inheritance в `set_price_list_tier_with_channel`,
  rejection mismatched explicit scope и propagation нового scope на existing
  override rows через `set_price_list_scope`.
- те же service-level tests теперь ещё и фиксируют active time-window invariants:
  future `starts_at` и expired `ends_at` отклоняются и в read-side resolution,
  и в write-side authoring / scope update paths, а не только скрываются из
  `list_active_price_lists`.
- service-level tests теперь ещё и фиксируют lifecycle typed percentage rule:
  снятие rule metadata через `set_price_list_percentage_rule(..., None)` очищает
  `rule_kind` / `adjustment_percent`, а resolver с explicit `price_list_id`
  после этого детерминированно fallback'ит к base row без stale discount state.
- promotion-ready preview path теперь тоже зафиксирован targeted tests: preview
  price-list percentage adjustment отклоняет future/expired `price_list` и
  channel-scope mismatch, а не только `draft` status.
- apply path для price-list percentage adjustment теперь тоже зафиксирован
  targeted tests: future/expired `price_list` и channel-scope mismatch
  отклоняются без побочной записи нового override row и без мутации существующего
  scoped override.
- admin SSR transport parity теперь тоже закрывает `price_list` rule/scope mutation
  lifecycle: clear rule metadata не оставляет stale `rule_kind` / `adjustment_percent`,
  update rule path режет inactive/draft, future/expired lists тем же contract, а
  scope clear возвращает active option и existing override rows к global boundary.
- pricing-focused GraphQL parity теперь ещё и фиксирует rule-driven effective
  price resolution без explicit override, precedence explicit override над rule,
  selector lifecycle после clear/scope update и channel-mismatch validation для
  `adminPricingProduct` / `storefrontPricingProduct`.
- GraphQL facade теперь уже закрывает не только pricing-authoritative reads:
  admin write-side mutations `updateAdminPricingVariantPrice`,
  `previewAdminPricingVariantDiscount`, `applyAdminPricingVariantDiscount`,
  `updateAdminPricingPriceListRule` и `updateAdminPricingPriceListScope`
  тоже работают поверх `PricingService`, сохраняя parallel transport contract
  рядом с native `#[server]` path и active-option parity для rule/scope editing.
- compare-at invariants теперь тоже зафиксированы шире, чем только service-level
  runtime: admin SSR transport режет invalid `compare_at < amount` без мутации
  base row или existing `price_list` override, а GraphQL parity отдельно
  подтверждает, что `compare_at == amount` не протекает в ложный sale state
  (`discount_percent = null`, `on_sale = false`).
- storage-level money sync теперь тоже покрыт targeted tests: `set_price` держит
  decimal fields и legacy cents fields в одном состоянии даже на дробных значениях,
  а clearing `compare_at` обнуляет и decimal, и legacy compare-at representation
  как в service write path, так и в admin native transport; admin `update-variant-price`
  отдельно фиксирует тот же decimal-to-cents sync и для дробных operator-side inputs.
- bulk write path `set_prices` теперь тоже зафиксирован отдельно: channel-scoped
  rows нормализуют `channel_slug`, синхронно пишут decimal + legacy cents
  representation, а невалидный один input откатывает всю пачку без partial update
  уже существующих rows.
- event payload parity для pricing write paths теперь тоже покрыт service-level
  tests: `PriceUpdated.old_amount/new_amount` публикуются в rounded cents contract
  и для `set_price`, и для bulk `set_prices`, включая кейсы `update existing row`
  против `insert new row`.
- текущий широкий verification baseline теперь уже проходит и полным
  `graphql_runtime_parity_test`, а не только pricing-focused subset.
- legacy `apply_discount` больше не живёт как отдельная ad-hoc mutation: pricing runtime
  теперь держит typed `preview_percentage_discount` / `apply_percentage_discount` поверх
  canonical base-price row, а старый helper остаётся compatibility wrapper.
- promotion-ready semantics тоже уже сдвинулись вперёд: active `price_list` теперь может
  держать typed percentage rule, resolver умеет fallback'иться к base row через это правило,
  а module-owned admin transport уже даёт first-class write path для rule authoring.

### 4. Operability

- [x] документировать новые pricing guarantees одновременно с изменением runtime surface;
- [x] удерживать local docs и `README.md` синхронизированными;
- [x] обновлять umbrella commerce docs при изменении pricing/promotion scope.

## Проверка

- `cargo xtask module validate pricing`
- `cargo xtask module test pricing`
- targeted tests для price resolution, pricing transport и money semantics
- текущий широкий verification baseline для этого slice:
  `cargo test -p rustok-commerce --test pricing_service_test`,
  `cargo test -p rustok-commerce --test graphql_runtime_parity_test`,
  и SSR/lib sweeps для `rustok-pricing-admin` / `rustok-pricing-storefront`

## Правила обновления

1. При изменении pricing runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md`, `admin/README.md`
   и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении pricing/promotion boundary обновлять umbrella commerce docs.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
