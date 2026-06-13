# План реализации `rustok-order`

Статус: order boundary выделен; модуль владеет order write-side lifecycle,
outbox publication и module-owned admin UI, а post-order и transport parity
дособираются umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: ffa_admin_storefront_handoff
- Last checkpoint: Order storefront FFA slice introduced `rustok-order/storefront`: order-owned checkout result handoff view-model plus complete-checkout action labels/Leptos adapter are now consumed by `rustok-commerce-storefront` instead of rendering order status/result presentation inline.
- Next step: Continue returns/refund/exchange/claim UI policy slices or move order-owned storefront transport behind `rustok-order/storefront` when host routing is ready, without changing the existing GraphQL order contract.
- Open blockers: серверный OpenAPI contract test под default features ранее упирался в существующие compile errors вне order/commerce (`rustok-pages-admin`, server build service/module lifecycle/graphql mutations); targeted order lifecycle и `rustok-commerce` check остаются основным gate для этого среза.
- Hand-off notes for next agent: После каждого returns/refund/exchange/claim инкремента обновлять FFA evidence и FBA placeholder, README/admin docs и central registry в том же PR.
- Last updated at (UTC): 2026-06-13T22:45:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `core_transport_ui`
- Evidence:
  - модуль ведётся в ускоренном FFA migration track; FBA остаётся `not_started` до закрытия FFA phase-gate как часть ecommerce family;
  - любые изменения UI/transport boundary должны фиксироваться с parity/boundary evidence в этом же инкременте;
  - admin FFA slice добавил framework-agnostic `admin/src/core/` list/filter request policy, module-owned `admin/src/transport/mod.rs` facade и явный Leptos render adapter `admin/src/ui/leptos.rs`; `admin/src/lib.rs` теперь только wires modules и re-export `OrderAdmin`, а Leptos adapter больше не вызывает raw `api::*` напрямую для covered order list/detail/lifecycle flows; latest admin slices moved mark-paid, ship, deliver and cancel action payload preparation into core-owned command helpers (`prepare_mark_paid_command`, `prepare_ship_order_command`, `prepare_deliver_order_command`, `prepare_cancel_order_command`) with unit-test evidence, then added fast boundary evidence via `scripts/verify/verify-order-admin-boundary.mjs` and the aggregate `npm run verify:ffa:ui:migration` pipeline; the presentation slices moved status labels/classes, order captions, detail summaries, timeline/action hints, optional display fallback and selected-detail form-state/default/fallback mapping into Leptos-free core while keeping signal setters in `helpers.rs`; the transport slice moved GraphQL code under `admin/src/transport/graphql_adapter.rs` behind `admin/src/transport/mod.rs`; the latest structure slice split the growing core into `admin/src/core/{requests,commands,detail_form,presentation}.rs`; storefront handoff slice added `storefront/src/core.rs` and `storefront/src/ui/leptos.rs` for order checkout result presentation and complete-checkout action presentation consumed by commerce orchestration.
- Last verified at (UTC): 2026-06-13T00:00:00Z
- Owner: `rustok-order` module team

## Область работ

- удерживать `rustok-order` как owner order lifecycle и order snapshots;
- синхронизировать order runtime contract, event flow, admin UI и local docs;
- не смешивать order write model с payment/fulfillment/provider orchestration.

## Текущее состояние

- `orders` и `order_line_items` уже module-owned;
- `order_adjustments` уже module-owned и фиксируют language-neutral promotion/discount snapshot без display labels;
- `order_tax_lines` теперь тоже несут typed `provider_id`, а checkout переносит provider-aware tax snapshot
  из cart без metadata-only fallback;
- write-side lifecycle и order events уже закреплены внутри модуля;
- product/variant связи хранятся как snapshot references, без cross-module FK;
- transport adapters по-прежнему публикуются фасадом `rustok-commerce`;
- `rustok-order/admin` публикует module-owned route для order list/detail/lifecycle с `admin/src/core/` request defaults, `admin/src/transport/mod.rs` facade и явным `admin/src/ui/leptos.rs` render adapter.

## Этапы

### 1. Contract stability

- [x] закрепить order-owned lifecycle и snapshot model;
- [x] добавить typed order adjustment snapshot с `subtotal_amount`, `adjustment_total` и net `total_amount`;
- [x] удерживать event publication частью module boundary;
- [x] вынести admin order UI в module-owned пакет `rustok-order/admin`;
- [ ] удерживать sync между order runtime contract, commerce transport и module metadata.

### 2. Post-order expansion

- [~] развивать returns, refunds, exchanges, claims и order changes как отдельный следующий слой; (started: `order_returns` + `order_return_items` storage, item validation, `OrderService::{create_return,get_return,list_returns,complete_return,cancel_return}` foundation and resolution-ссылки завершённого возврата for refund/exchange/claim/order-change orchestration)
- [x] покрывать lifecycle transitions и failure semantics targeted tests; (return lifecycle `pending -> completed|cancelled`, second-transition guard, tenant-scoped show)
- [~] удерживать compatibility с payment/fulfillment orchestration без размывания order ownership. (started: `order_changes` skeleton хранит preview/apply/cancel state без payment/fulfillment side effects)

### 3. Operability

- [~] документировать новые order guarantees одновременно с изменением runtime surface; (returns lifecycle, item-level lines, resolution-ссылки завершённого возврата и order-change skeleton checkpoint зафиксированы)
- [ ] удерживать local docs и `README.md` синхронизированными;
- [ ] обновлять umbrella commerce docs при изменении order/post-order scope.

## Проверка

- `cargo xtask module validate order`
- `cargo xtask module test order`
- targeted tests для order lifecycle, typed adjustments, outbox events и snapshot invariants

## Правила обновления

1. При изменении order runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md`, `admin/README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении order/payment/fulfillment orchestration обновлять umbrella docs.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
