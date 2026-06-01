# План реализации `rustok-product`

Статус: product boundary выделен; модуль владеет каталогом и typed product data,
а transport и часть orchestration остаются у umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: ffa_storefront_core_slice
- Last checkpoint: Storefront shell copy and fetch request shape now live in framework-agnostic `ProductStorefrontShellViewModel` / `ProductStorefrontFetchRequest`; Leptos `ProductView` supplies host route context and passes the prepared request to the transport facade.
- Next step: Continue FFA-first sequencing by moving the next storefront/admin render fragment or route/query writer smoke into core without changing native/GraphQL transport parity.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-05-31T00:00:00Z


## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_only`
- Evidence:
  - module plan синхронизирован с central FFA/FBA readiness board; UI surface уже опубликован и ведётся в migration/backlog ритме;
  - FFA slice: storefront catalog rail title/total/empty/open labels, item fallback labels, seller boundary text, published timestamp fallback and handle links now live in framework-agnostic `ProductCatalogRailViewModel` with unit-test evidence;
  - FFA slice: selected-product card empty state, pricing context label, ownership note, metric labels and pricing action label now live in `SelectedProductEmptyViewModel` / `SelectedProductViewModel` with unit-test evidence;
  - FFA slice: storefront shell badge/title/subtitle/load-error copy and typed fetch request shape now live in `ProductStorefrontShellViewModel` / `ProductStorefrontFetchRequest` with unit-test evidence;
  - дальнейшее повышение статуса выполняется только вместе с verification evidence и обновлением local+central docs.
- Last verified at (UTC): 2026-05-31T00:00:00Z
- Owner: `rustok-product` module team

## Область работ

- удерживать `rustok-product` как owner product/variant/catalog domain;
- закрепить product-owned admin UI как первый UI slice распила ecommerce family;
- синхронизировать product tags, shipping profile bindings и local docs;
- не смешивать catalog runtime с pricing/inventory/order orchestration.

## Текущее состояние

- product catalog, variants, options, translations и publication contract уже живут в модуле;
- taxonomy-backed `product_tags` уже служат first-class product tag surface;
- typed `shipping_profile_slug` уже закреплён в product/variant persistence и DTO;
- module-owned admin UI пакет `rustok-product/admin` уже поднят и подключён в
  manifest-driven admin composition как первый шаг UI split;
- module-owned storefront UI пакет `rustok-product/storefront` уже поднят и
  подключён в manifest-driven storefront composition для published catalog
  discovery через native Leptos server functions с GraphQL fallback;
- storefront UI продолжает FFA-декомпозицию: route/query normalization, typed fetch
  request shape, shell copy, selected-product view-model composition, selected-card
  labels/empty state, catalog rail view-model, pricing/seller labels и pricing
  deep-link state вынесены в framework-agnostic `storefront/src/core.rs`, а Leptos
  слой остаётся thin render/host-context adapter поверх transport;
- transport-level validation и public transport по-прежнему публикуются фасадом `rustok-commerce`.

## Этапы

### 1. Contract stability

- [x] зафиксировать product-owned catalog boundary;
- [x] перевести tags на taxonomy-backed first-class contract;
- [x] зафиксировать typed `shipping_profile_slug` для product/variant;
- [ ] удерживать sync между product runtime contract, commerce transport и module metadata.

### 2. Catalog hardening

- [ ] покрывать publication, tags и shipping-profile edge-cases targeted tests;
- [ ] развивать product-specific semantics без возврата к metadata-only contract;
- [ ] удерживать deliverability-facing bindings совместимыми с fulfillment/pricing flows.

### 3. Operability

- [x] поднять module-owned admin UI пакет для product catalog surface;
- [x] документировать новые catalog guarantees одновременно с изменением runtime surface;
- [ ] удерживать local docs и `README.md` синхронизированными;
- [x] вынести storefront FFA core slice для route/query state, selected-product view-model и pricing/seller helpers;
- [x] вынести storefront catalog rail presentation в core view-model без Leptos runtime;
- [x] вынести selected-product card labels и empty state в core view-model без Leptos runtime;
- [x] вынести storefront shell copy и typed fetch request shape в core без Leptos runtime;
- [ ] обновлять consumer-module docs при изменении tag/deliverability integration rules.

## Проверка

- `cargo xtask module validate product`
- `cargo xtask module test product`
- targeted tests для catalog CRUD, tags, publication и shipping-profile bindings

## Правила обновления

1. При изменении product runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata или UI wiring синхронизировать `rustok-module.toml`.
4. При изменении shipping-profile или taxonomy integration обновлять связанные commerce docs.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
