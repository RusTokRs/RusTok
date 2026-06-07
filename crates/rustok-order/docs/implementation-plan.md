# План реализации `rustok-order`

Статус: order boundary выделен; модуль владеет order write-side lifecycle,
outbox publication и module-owned admin UI, а post-order и transport parity
дособираются umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: ffa_admin_transport_ui_split
- Last checkpoint: Admin order получил FFA slice: `admin/src/core.rs` list/filter defaults, `admin/src/transport.rs` facade над existing GraphQL order transport и явный Leptos render adapter `admin/src/ui/leptos.rs`; crate root стал wiring/re-export boundary без изменения order lifecycle behavior.
- Next step: Продолжать сокращать `admin/src/api.rs` до transport adapter implementation и выносить remaining request/view policy в `core`, затем расширить umbrella `rustok-commerce` operator UX для refund/exchange/claim return decisions без host-owned logic.
- Open blockers: серверный OpenAPI contract test под default features ранее упирался в существующие compile errors вне order/commerce (`rustok-pages-admin`, server build service/module lifecycle/graphql mutations); targeted order lifecycle и `rustok-commerce` check остаются основным gate для этого среза.
- Hand-off notes for next agent: После каждого returns/refund/exchange/claim инкремента обновлять FFA evidence и FBA placeholder, README/admin docs и central registry в том же PR.
- Last updated at (UTC): 2026-06-02T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `core_transport_ui`
- Evidence:
  - модуль ведётся в ускоренном FFA migration track; FBA остаётся `not_started` до закрытия FFA phase-gate как часть ecommerce family;
  - любые изменения UI/transport boundary должны фиксироваться с parity/boundary evidence в этом же инкременте;
  - admin FFA slice добавил framework-agnostic `admin/src/core.rs` list/filter request policy, module-owned `admin/src/transport.rs` facade и явный Leptos render adapter `admin/src/ui/leptos.rs`; `admin/src/lib.rs` теперь только wires modules и re-export `OrderAdmin`, а Leptos adapter больше не вызывает raw `api::*` напрямую для covered order list/detail/lifecycle flows.
- Last verified at (UTC): 2026-06-02T00:00:00Z
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
- `rustok-order/admin` публикует module-owned route для order list/detail/lifecycle с `admin/src/core.rs` request defaults, `admin/src/transport.rs` facade и явным `admin/src/ui/leptos.rs` render adapter.

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
