# План реализации `rustok-order`

Статус: order boundary выделен; модуль владеет order write-side lifecycle,
outbox publication и module-owned admin UI, а post-order и transport parity
дособираются umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: owner-owned dashboard order analytics
- Last checkpoint: `OrderStatsSnapshot` и `load_order_stats_snapshot` перенесены в `rustok-order`; `apps/server::RootQuery::dashboard_stats` только композирует owner helper за feature `mod-order` и больше не содержит SQL для событий `order.placed`. Граница закреплена `apps/server/tests/module_surface_boundary_guard.rs` без компиляции.
- Next step: удерживать parity публичного GraphQL order contract, пока post-order surfaces продолжают переезжать в owner admin/storefront packages; продолжать удалять оставшиеся module-specific server GraphQL artifacts малыми no-compile срезами.
- Open blockers: серверный OpenAPI contract test под default features ранее упирался в существующие compile errors вне order/commerce (`rustok-pages-admin`, server build service/module lifecycle/graphql mutations); targeted order lifecycle и `rustok-commerce` check остаются основным gate для этого среза.
- Hand-off notes for next agent: После каждого returns/refund/exchange/claim инкремента обновлять FFA evidence и FBA placeholder, README/admin docs и central registry в том же PR.
- Last updated at (UTC): 2026-07-02T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Версия FBA-контракта: `order.checkout_completion.v1`
- Structural shape: `core_transport_ui`
- Evidence:
  - Boundary readiness update: `crates/rustok-order/contracts/order-fba-registry.json` now has `runtime_evidence.checkout_completion_owner_path.status = "runtime_verified"`; `npm run verify:ecommerce:fba` gates the owner `OrderService` checkout-completion path, while remote/base fallback smoke remains a follow-up before `transport_verified`.
  - umbrella facade `rustok_commerce::{services::order, OrderService}` is removed; commerce REST/GraphQL/admin/storefront/test consumers import `OrderService` from `rustok-order` directly, so order owner service is no longer masked by the ecommerce umbrella.
  - FBA provider registry `crates/rustok-order/contracts/order-fba-registry.json` now also declares `ai-order` as an operator-context consumer of `CheckoutCompletionPort` / `order.checkout_completion.v1` `read_order_status`, with `generate_summary_without_live_status`, `require_operator_review`, and `skip_prefill_execution` degraded modes locked by `scripts/verify/verify-ai-fba-baseline.mjs`.
  - FBA maintenance slice перевёл read-only checkout result/status paths на shared `PortCallPolicy::read()`, а complete-checkout write path — на shared `PortCallPolicy::write()` без изменения temporary commerce transport handoff.
  - `src/ports.rs` теперь экспортирует `CheckoutCompletionPort` и DTO для complete/result/status операций; machine-readable registry и verifier проверяют совпадение port trait operations с FBA metadata;
  - метаданные FBA-provider открыты для `checkout completion/result` через `crates/rustok-order/contracts/order-fba-registry.json`; статус остаётся `in_progress` до появления contract tests/remote transport evidence, которые позволят подняться выше embedded checkout compatibility;
  - registry теперь фиксирует `contract_tests.status = planned_cases_locked`: для каждой port operation задана in-process/remote-adapter-placeholder case matrix, baseline assertions (`typed_port_error_mapping`, `context_deadline_preserved`) и исправленный `write_idempotency_required` только для `complete_checkout`; read-only result/status cases больше не требуют write idempotency; fallback smoke profile set; static evidence packet `crates/rustok-order/contracts/evidence/order-contract-test-static-matrix.json` is locked by `npm run verify:ecommerce:fba` (registry + evidence gates) and `npm run verify:ecommerce:fba-contract-evidence`; это закрывает metadata/evidence anti-drift для будущих contract tests, но не повышает статус без runtime evidence;
  - `in_process_provider_impl` теперь закрепляет `OrderService` как owner implementation для `CheckoutCompletionPort`: write-path вызывает `PortCallPolicy::write()` перед owner `create_order_with_channel`, подтверждает order lifecycle через `confirm_order` и reload-ит locale-aware snapshot при наличии locale context; read status вызывает `PortCallPolicy::read()` перед owner `get_order`, а cart-id result projection остаётся typed unavailable gap до появления storage projection; fast verifier проверяет эти semantics без полной компиляции;
  - любые изменения UI/transport boundary должны фиксироваться с parity/boundary evidence в этом же инкременте;
  - manifest-driven storefront composition now registers `rustok-order-storefront` in `checkout_result_handoff`; `OrderView` is the zero-prop host entry adapter, reads the effective locale from `UiRouteContext.locale`, and resolves copy through the module-owned `en`/`ru` catalog declared by `[provides.storefront_ui.i18n]`;
  - storefront native checkout completion is now owner-owned: `storefront/src/transport/native_server_adapter/raw_adapter.rs` publishes `order/complete-checkout` over the explicit `rustok_commerce::storefront_checkout_runtime` API, so commerce no longer keeps the native order owner-operation wrapper;
  - dashboard order analytics теперь owner-owned: `rustok-order::load_order_stats_snapshot` читает `order.placed` outbox events, а `apps/server::RootQuery` только композирует результат и проверяется boundary guard без компиляции;
  - admin FFA slice добавил framework-agnostic `admin/src/core/` list/filter request policy, module-owned `admin/src/transport/mod.rs` facade и явный Leptos render adapter `admin/src/ui/leptos.rs`, locked by `scripts/verify/verify-order-admin-boundary.mjs`; storefront owns `CompleteCheckoutRequest`, `CheckoutAdjustment`, `CheckoutCompletion`, the MissingServer-gated `complete_checkout` facade, `storefront/src/transport/graphql_adapter.rs` with the complete-checkout GraphQL mutation/mapping and `storefront/src/transport/native_server_adapter/raw_adapter.rs` with the `order/complete-checkout` server-function shell over the explicit commerce checkout runtime API; commerce no longer duplicates order GraphQL payload, response projection or native owner-operation wrapper; `scripts/verify/verify-order-storefront-boundary.mjs` and `scripts/verify/verify-commerce-storefront-transport-handoff.mjs` lock the owner boundary.
- Last verified at (UTC): 2026-07-02T00:00:00Z
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
- complete-checkout GraphQL execution, native server-function execution, result DTOs and fallback policy are order-owned; commerce exposes only the shared checkout runtime API for orchestration;
- dashboard order analytics (`OrderStatsSnapshot`, `load_order_stats_snapshot`) уже module-owned; server GraphQL не содержит SQL по `order.placed`;
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
