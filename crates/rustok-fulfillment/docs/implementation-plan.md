# План реализации `rustok-fulfillment`

Статус: fulfillment boundary выделен; shipping options, fulfillments и typed
`fulfillment_items` уже служат основой для deliverability domain, а provider
SPI и post-order delivery changes ещё остаются в активном backlog umbrella
`rustok-commerce`.

## Execution checkpoint

- Current phase: ffa_admin_storefront_boundary
- Last checkpoint: Fulfillment storefront FFA slice now includes `storefront/src/transport.rs` request/selection normalization for shipping-option handoff in addition to the fulfillment-owned shipping handoff presentation consumed by commerce checkout orchestration.
- Next step: Continue by moving the async shipping-option selection native/GraphQL adapter from commerce compatibility into fulfillment-owned storefront when the host route is ready; compatibility code can only shrink after owner transport is wired.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-06-13T23:20:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `core_transport_ui`
- Evidence:
  - модуль ведётся в ускоренном FFA migration track; FBA остаётся `not_started` до закрытия FFA phase-gate как часть ecommerce family;
  - любые изменения UI/transport boundary должны фиксироваться с parity/boundary evidence в этом же инкременте;
  - admin FFA slice добавил framework-agnostic `admin/src/core.rs` request policy для списка и фильтров, module-owned `admin/src/transport.rs` facade и явный Leptos адаптер отрисовки `admin/src/ui/leptos.rs`; `admin/src/lib.rs` теперь только wires modules и re-export `FulfillmentAdmin`, а Leptos adapter больше не вызывает raw `api::*` напрямую для covered shipping-option flows; fast guardrail `scripts/verify/verify-fulfillment-admin-boundary.mjs` закрепляет boundary и docs sync без full-workspace compile;
  - storefront handoff slice lives in `storefront/src/ui/leptos.rs` as fulfillment-owned shipping/fulfillment presentation consumed by commerce checkout orchestration; `storefront/src/transport.rs` owns shipping selection request normalization for the next transport cutover.
- Last verified at (UTC): 2026-06-13T00:00:00Z
- Owner: `rustok-fulfillment` module team

## Область работ

- удерживать `rustok-fulfillment` как owner shipping-option/fulfillment boundary;
- синхронизировать shipping contracts, allowed profile bindings и local docs;
- не смешивать базовую shipping domain model с provider-specific delivery logic.

## Текущее состояние

- `shipping_options`, `fulfillments`, `FulfillmentModule` и `FulfillmentService` уже выделены;
- typed `fulfillment_items` уже фиксируют состав fulfillment поверх `order_line_item_id + quantity`;
- typed `fulfillment_items` уже фиксируют и progress-поля `shipped_quantity` / `delivered_quantity` для partial delivery path;
- first-class `allowed_shipping_profile_slugs` уже являются частью live contract;
- deliverability orchestration с `delivery_groups[]`, `shipping_selections[]` и multi-fulfillment checkout строится umbrella `rustok-commerce` поверх этого boundary;
- admin/post-order create fulfillment path в `rustok-commerce` уже использует typed `items[]` и валидирует order-line ownership + remaining quantity до вызова `FulfillmentService`;
- item-level `ship` / `deliver` adjustments уже работают поверх typed fulfillment items и пишут language-agnostic audit trail в metadata fulfillment/item'ов; `delivered_note` не дублируется в audit JSON;
- explicit `reopen` / `reship` recovery path уже работает поверх того же typed fulfillment boundary: delivered fulfillment можно вернуть в `shipped`, cancelled fulfillment можно вернуть в actionable state, а повторная shipment attempt фиксируется audit-safe без language-dependent metadata;
- admin/operator surface уже использует typed lifecycle для shipping options, а module-owned route `rustok-fulfillment/admin` забрал ownership shipping-option UI у umbrella `rustok-commerce-admin` и теперь держит `admin/src/core.rs` настройки request по умолчанию, `admin/src/transport.rs` facade и явный `admin/src/ui/leptos.rs` адаптер отрисовки; storefront handoff presentation и request normalization для shipping selection теперь живут в `rustok-fulfillment/storefront`.

## Этапы

### 1. Contract stability

- [x] закрепить shipping-option/fulfillment boundary;
- [x] встроить first-class `allowed_shipping_profile_slugs`;
- [x] удерживать compatibility shim для single-group carts только как переходный transport layer;
- [x] вынести shipping-option admin UI в module-owned пакет `rustok-fulfillment/admin`;
- [ ] удерживать sync между fulfillment runtime contract, commerce orchestration и module metadata.

### 2. Deliverability expansion

- [x] довести richer fulfillment-item model без размывания boundary;
- [x] расширить fulfillment-item model от уже живого manual post-order create path до item-level delivery changes и adjustments поверх seller-aware grouping;
- [x] добавить explicit post-order recovery semantics `reopen` / `reship` поверх typed fulfillment-item progress и language-agnostic audit trail;
- [ ] покрывать mixed-cart и multi-fulfillment edge-cases targeted tests;
- [ ] удерживать compatibility с payment/order orchestration и shipping-profile registry.

### 3. Operability

- [x] документировать новые fulfillment guarantees одновременно с изменением runtime surface;
- [ ] удерживать local docs и `README.md` синхронизированными;
- [ ] обновлять umbrella commerce docs при изменении deliverability/provider scope.

## Проверка

- `cargo xtask module validate fulfillment`
- `cargo xtask module test fulfillment`
- targeted tests для shipping options, fulfillments, delivery groups и multi-fulfillment invariants

## Правила обновления

1. При изменении fulfillment runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении deliverability/provider architecture обновлять umbrella docs.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
