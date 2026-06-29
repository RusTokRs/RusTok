# План реализации `rustok-order`

Статус: order boundary выделен; модуль владеет order write-side lifecycle,
outbox publication и module-owned admin UI, а post-order и transport parity
дособираются umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: ffa_storefront_complete_request_boundary
- Last checkpoint: Aggregate storefront complete-checkout transport handoff hardened the compatibility window: commerce still hosts the async native/GraphQL adapter, but the owner `CompleteCheckoutRequest` alias is preserved and `rustok-order/storefront` now owns typed `CheckoutCompletionTransportError` mapping plus the `complete_checkout_with_fallback` facade, so compatibility fallback is now MissingServer-only instead of retrying validation/domain failures through GraphQL. `scripts/verify/verify-commerce-storefront-transport-handoff.mjs` locks this until the adapter moves fully behind `rustok-order/storefront`.
- Next step: Continue returns/refund/exchange/claim UI policy slices or move the async complete-checkout native/GraphQL transport behind `rustok-order/storefront` when host routing is ready, without changing the existing GraphQL order contract.
- Open blockers: серверный OpenAPI contract test под default features ранее упирался в существующие compile errors вне order/commerce (`rustok-pages-admin`, server build service/module lifecycle/graphql mutations); targeted order lifecycle и `rustok-commerce` check остаются основным gate для этого среза.
- Hand-off notes for next agent: После каждого returns/refund/exchange/claim инкремента обновлять FFA evidence и FBA placeholder, README/admin docs и central registry в том же PR.
- Last updated at (UTC): 2026-06-29T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Версия FBA-контракта: `order.checkout_completion.v1`
- Structural shape: `core_transport_ui`
- Evidence:
  - FBA maintenance slice перевёл read-only checkout result/status paths на shared `PortCallPolicy::read()`, а complete-checkout write path — на shared `PortCallPolicy::write()` без изменения temporary commerce transport handoff.
  - `src/ports.rs` теперь экспортирует `CheckoutCompletionPort` и DTO для complete/result/status операций; machine-readable registry и verifier проверяют совпадение port trait operations с FBA metadata;
  - метаданные FBA-provider открыты для `checkout completion/result` через `crates/rustok-order/contracts/order-fba-registry.json`; статус остаётся `in_progress` до появления contract tests/remote transport evidence, которые позволят подняться выше embedded checkout compatibility;
  - registry теперь фиксирует `contract_tests.status = planned_cases_locked`: для каждой port operation задана in-process/remote-adapter-placeholder case matrix, baseline assertions (`typed_port_error_mapping`, `context_deadline_preserved`) и исправленный `write_idempotency_required` только для `complete_checkout`; read-only result/status cases больше не требуют write idempotency; fallback smoke profile set; static evidence packet `crates/rustok-order/contracts/evidence/order-contract-test-static-matrix.json` is locked by `npm run verify:ecommerce:fba` (registry + evidence gates) and `npm run verify:ecommerce:fba-contract-evidence`; это закрывает metadata/evidence anti-drift для будущих contract tests, но не повышает статус без runtime evidence;
  - `in_process_provider_impl` теперь закрепляет `OrderService` как owner implementation для `CheckoutCompletionPort`: write-path вызывает `PortCallPolicy::write()` перед owner `create_order_with_channel`, подтверждает order lifecycle через `confirm_order` и reload-ит locale-aware snapshot при наличии locale context; read status вызывает `PortCallPolicy::read()` перед owner `get_order`, а cart-id result projection остаётся typed unavailable gap до появления storage projection; fast verifier проверяет эти semantics без полной компиляции;
  - любые изменения UI/transport boundary должны фиксироваться с parity/boundary evidence в этом же инкременте;
  - manifest-driven storefront composition now registers `rustok-order-storefront` in `checkout_result_handoff`; `OrderView` is the zero-prop host entry adapter, reads the effective locale from `UiRouteContext.locale`, and resolves copy through the module-owned `en`/`ru` catalog declared by `[provides.storefront_ui.i18n]`;
  - admin FFA slice добавил framework-agnostic `admin/src/core/` list/filter request policy, module-owned `admin/src/transport/mod.rs` facade и явный Leptos render adapter `admin/src/ui/leptos.rs`; `admin/src/lib.rs` теперь только wires modules и re-export `OrderAdmin`, а Leptos adapter больше не вызывает raw `api::*` напрямую для covered order list/detail/lifecycle flows; latest admin slices moved mark-paid, ship, deliver and cancel action payload preparation into core-owned command helpers (`prepare_mark_paid_command`, `prepare_ship_order_command`, `prepare_deliver_order_command`, `prepare_cancel_order_command`) with unit-test evidence, then added fast boundary evidence via `scripts/verify/verify-order-admin-boundary.mjs` and the aggregate `npm run verify:ffa:ui:migration` pipeline; the presentation slices moved status labels/classes, order captions, detail summaries, timeline/action hints, optional display fallback and selected-detail form-state/default/fallback mapping into Leptos-free core while keeping signal setters in `helpers.rs`; the transport slice moved GraphQL code under `admin/src/transport/graphql_adapter.rs` behind `admin/src/transport/mod.rs`; the latest structure slice split the growing core into `admin/src/core/{requests,commands,detail_form,presentation}.rs`; storefront handoff slice added `storefront/src/core.rs`, `storefront/src/transport.rs` and `storefront/src/ui/leptos.rs` for order checkout result presentation, complete-checkout command normalization, completion command metadata and complete-checkout action presentation consumed by commerce orchestration; latest storefront slice makes `OrderCheckoutCompleteButton` emit `CompleteCheckoutRequest`, maps the owner DTO metadata through commerce native/GraphQL orchestration, removes the duplicate commerce complete-checkout request builder, and adds aggregate-wired fast boundary evidence via `scripts/verify/verify-order-storefront-boundary.mjs`.
- Last verified at (UTC): 2026-06-29T00:00:00Z
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
- async transport adapters по-прежнему публикуются фасадом `rustok-commerce`, while complete-checkout command normalization and typed native/GraphQL fallback facade are order-owned and compatibility fallback is now MissingServer-only during the temporary adapter window;
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
