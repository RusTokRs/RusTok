# План реализации `rustok-fulfillment`

Статус: fulfillment boundary выделен; shipping options, fulfillments и typed
`fulfillment_items` уже служат основой для deliverability domain, а provider
SPI и post-order delivery changes ещё остаются в активном backlog umbrella
`rustok-commerce`.

## Execution checkpoint

- Current phase: provider_spi_live_adapter_evidence and shared port policy parity
- Last checkpoint: FBA maintenance slice перевёл select-shipping-option write path на shared `PortCallPolicy::write()` и сохранил list path на `PortCallPolicy::read()`; earlier checkpoint: Provider SPI live-adapter evidence now records concrete external carrier contract execution: guarded single invocation, typed provider-error mapping without lifecycle persistence, degraded fallback propagation (`manual_shipping`), unavailable-mode adapter blocking and idempotent tracking webhook replay delegation are locked by the aggregate verifier without running Cargo compilation.
- Next step: Move the remaining select-shipping-option server-function endpoint/body from commerce compatibility into a fulfillment-owned SSR adapter, preserve GraphQL fallback parity, then move from evidence-only external carrier contract execution to production adapter wiring in host composition.
- Open blockers: None.
- Hand-off notes for next agent: Без компиляции: поддерживать fast source guardrails; при следующем transport cutover синхронизировать commerce plan и центральную FFA/FBA readiness board.
- Last updated at (UTC): 2026-06-29T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Версия FBA-контракта: `fulfillment.shipping_selection.v1`
- Structural shape: `core_transport_ui`
- Evidence:
  - FBA maintenance slice перевёл read-only `list_seller_shipping_options` path на shared `PortCallPolicy::read()`, а select/write path — на shared `PortCallPolicy::write()`, сохранив существующие FBA metadata без изменений runtime surface.
  - umbrella facade `rustok_commerce::{services::fulfillment, FulfillmentService}` is removed; commerce REST/GraphQL/admin/storefront/test consumers import `FulfillmentService` from `rustok-fulfillment` directly, so fulfillment owner service is no longer masked by the ecommerce umbrella.
  - in-process реализация `ShippingSelectionPort for FulfillmentService` добавлена в `src/ports.rs`: read path фильтрует shipping options по profile slug, select path требует shared `PortCallPolicy::write()` и мапит `FulfillmentError` в `PortError`;
  - `src/ports.rs` теперь экспортирует `ShippingSelectionPort` и DTO для seller-aware shipping options/selection операций; machine-readable registry и verifier проверяют совпадение port trait operations с FBA metadata;
  - метаданные FBA-provider открыты для `seller-aware shipping selection` через `crates/rustok-fulfillment/contracts/fulfillment-fba-registry.json`; provider SPI boundary поднят до `boundary_ready` на executed live-adapter evidence, while base shipping-selection port contract/fallback evidence remains a follow-up before `transport_verified`;
  - registry теперь фиксирует `contract_tests.status = planned_cases_locked`: для каждой port operation задана in-process/remote-adapter-placeholder case matrix, baseline assertions (`typed_port_error_mapping`, `context_deadline_preserved`) с явным deadline enforcement для read path и `write_idempotency_required` только на write operations; fallback smoke profile set; static evidence packet `crates/rustok-fulfillment/contracts/evidence/fulfillment-contract-test-static-matrix.json` is locked by `npm run verify:ecommerce:fba` (registry + evidence gates) and `npm run verify:ecommerce:fba-contract-evidence`; это закрывает metadata/evidence anti-drift для будущих base port contract tests;
  - provider SPI evidence теперь закреплён в `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-static-matrix.json`: manual/remote-placeholder cases для `quote_rates`/`create_label`/`cancel` проверяют typed provider error mapping, idempotency-key preservation и запрет persistence в adapter layer, tracking webhook replay contract фиксирует idempotent duplicate delivery и делегирование lifecycle transition в `FulfillmentService`, а `src/providers.rs` содержит external carrier registration contract (`ExternalFulfillmentProviderRegistration`, health/degraded-mode DTOs, descriptor-id validation, `FulfillmentProviderRegistry`) с source markers, которые проверяет `scripts/verify/verify-ecommerce-provider-spi-evidence.mjs`; packet не повышает FBA статус без runtime execution;
  - provider registry runtime-mode guardrails теперь side-effect-free проверяют capability support, missing-provider errors и health/degraded-mode mapping до вызова carrier adapter-а; registry также публикует guarded async `execute_quote_rates`/`execute_create_label`/`execute_cancel`/`execute_tracking_webhook` seams, которые блокируют unavailable carriers до adapter side effects и оставляют lifecycle persistence в `FulfillmentService`; targeted provider SPI tests фиксируют fallback profile propagation и operation capability rejection без полной компиляции в этой итерации;
  - provider SPI runtime-smoke evidence теперь закреплён в `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-runtime-smoke.json`, а dedicated live-adapter contract — в `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-live-adapter-contract.json`: no-compile packets фиксируют missing-provider lookup, unsupported/unknown operation rejection, degraded fallback propagation, unavailable-provider non-executable mode, registration failure cases, webhook replay guardrails и обязательные live carrier execution cases; `scripts/verify/verify-ecommerce-provider-spi-evidence.mjs` проверяет этот packet вместе со static matrix;
  - live external carrier execution plan теперь закреплён внутри runtime-smoke packet: verifier требует concrete-adapter evidence для guarded single invocation, typed provider-error mapping без lifecycle persistence, degraded fallback propagation, unavailable-mode adapter blocking и tracking webhook replay delegation;
  - live external carrier execution evidence теперь закреплён в `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-live-adapter-evidence.json`: packet фиксирует concrete-adapter contract execution для guarded single invocation, typed provider-error mapping без lifecycle persistence, degraded fallback profile `manual_shipping`, unavailable-mode adapter blocking и idempotent tracking webhook replay delegation; `scripts/verify/verify-ecommerce-provider-spi-evidence.mjs` теперь валидирует этот executed evidence рядом со static/runtime-smoke/contract packets без Cargo compilation and gates the `boundary_ready` status.
  - любые изменения UI/transport boundary должны фиксироваться с parity/boundary evidence в этом же инкременте;
  - admin FFA slice добавил framework-agnostic `admin/src/core.rs` request policy для списка и фильтров, module-owned `admin/src/transport.rs` facade, GraphQL adapter `admin/src/transport/graphql_adapter.rs` и явный Leptos адаптер отрисовки `admin/src/ui/leptos.rs`; `admin/src/lib.rs` теперь только wires modules и re-export `FulfillmentAdmin`, legacy `admin/src/api.rs` удалён, а Leptos adapter больше не вызывает raw adapter напрямую для covered shipping-option flows; fast guardrail `scripts/verify/verify-fulfillment-admin-boundary.mjs` закрепляет boundary и docs sync без full-workspace compile;
  - storefront handoff + shipping-selection slice lives in `storefront/src/model.rs`, `storefront/src/core/mod.rs`, `storefront/src/transport.rs` and `storefront/src/ui/leptos.rs` as fulfillment-owned seller-aware delivery-group presentation/normalization plus native-first/GraphQL fallback policy consumed by commerce checkout orchestration; compatibility fallback is now MissingServer-only while the temporary commerce adapter remains, and fast guardrails `scripts/verify/verify-fulfillment-storefront-boundary.mjs` plus `scripts/verify/verify-commerce-storefront-transport-handoff.mjs` validate the owner UI/core/transport split, narrowed fallback policy and aggregate package wiring while commerce temporarily retains the SSR endpoint body.
  - manifest-driven storefront composition now registers `rustok-fulfillment-storefront` in `checkout_shipping_handoff`; `FulfillmentView` is the zero-prop host entry adapter, reads the effective locale from `UiRouteContext.locale`, and resolves copy through the module-owned `en`/`ru` catalog declared by `[provides.storefront_ui.i18n]`.
- Last verified at (UTC): 2026-06-29T00:00:00Z
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
- admin/operator surface уже использует typed lifecycle для shipping options, а module-owned route `rustok-fulfillment/admin` забрал ownership shipping-option UI у umbrella `rustok-commerce-admin` и теперь держит `admin/src/core.rs` настройки request по умолчанию, `admin/src/transport.rs` facade, `admin/src/transport/graphql_adapter.rs` GraphQL operations и явный `admin/src/ui/leptos.rs` адаптер отрисовки; storefront handoff presentation, request normalization и transport fallback policy для shipping selection теперь живут в `rustok-fulfillment/storefront`, compatibility fallback is now MissingServer-only, а commerce compatibility пока держит только endpoint/body adapter до host cutover.

## Этапы

### 1. Contract stability

- [x] закрепить shipping-option/fulfillment boundary;
- [x] встроить first-class `allowed_shipping_profile_slugs`;
- [x] удерживать compatibility shim для single-group carts только как переходный transport layer;
- [x] вынести shipping-option admin UI в module-owned пакет `rustok-fulfillment/admin`;
- [x] удерживать sync между fulfillment runtime contract, commerce orchestration и module metadata для текущего storefront selection slice;

### 2. Deliverability expansion

- [x] довести richer fulfillment-item model без размывания boundary;
- [x] расширить fulfillment-item model от уже живого manual post-order create path до item-level delivery changes и adjustments поверх seller-aware grouping;
- [x] добавить explicit post-order recovery semantics `reopen` / `reship` поверх typed fulfillment-item progress и language-agnostic audit trail;
- [ ] покрывать mixed-cart и multi-fulfillment edge-cases targeted tests;
- [x] удерживать compatibility с payment/order orchestration и shipping-profile registry для seller-aware storefront selection UI;

### 2.5. Provider expansion

- [x] сформировать provider SPI baseline до подключения внешних carrier integrations;
- [x] добавить static provider SPI contract matrix и tracking webhook ingress/replay contract;
- [x] зафиксировать external carrier registration contract без provider-specific carrier logic в базовом fulfillment lifecycle contract.
- [x] добавить fulfillment-owned provider registry seam для host/carrier composition без lifecycle persistence в adapter layer.
- [x] добавить side-effect-free runtime-mode guardrails для capability checks и degraded-mode fallback mapping до invocation external carrier adapter-а.
- [x] зафиксировать no-compile live carrier adapter execution contract packet.
- [x] заменить static/no-compile provider SPI evidence live runtime contract execution against concrete external adapters.
- [x] добавить owner registry guarded async invocation seam для carrier adapter calls до production carrier wiring.

### 3. Operability

- [x] документировать новые fulfillment guarantees одновременно с изменением runtime surface;
- [x] удерживать local docs и `README.md` синхронизированными для storefront selection boundary;
- [x] обновлять umbrella commerce docs при изменении deliverability/provider scope.

## Проверка

- `cargo xtask module validate fulfillment`
- `cargo xtask module test fulfillment`
- `node scripts/verify/verify-fulfillment-admin-boundary.mjs`
- `node scripts/verify/verify-fulfillment-storefront-boundary.mjs`
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
