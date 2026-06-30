# План реализации `rustok-payment`

Статус: payment boundary выделен; базовый manual/default flow уже есть, а
provider SPI и richer payment lifecycle остаются в backlog umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: provider_spi_live_adapter_evidence
- Last checkpoint: Пакетный storefront read boundary теперь включает collection и refund summary: payment-owned facade публикует `PaymentCollectionFetchRequest` / `RefundSummaryFetchRequest`, единые DTO, GraphQL reads и native endpoint-ы `payment/payment-collection` / `payment/refund-summary`; commerce storefront больше не владеет payment/refund transport или aggregation.
- Next step: Продолжить production provider adapter wiring отдельно; owner storefront guardrail должен удерживать collection/refund read и create/reuse parity единым boundary.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-06-30T12:05:57Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Версия FBA-контракта: `payment.checkout.v1`
- Structural shape: `core_transport_ui`
- Evidence:
  - verification от 2026-06-30: `cargo test -p rustok-payment-storefront --all-features --locked`, payment storefront boundary gate, полный FFA migration sweep и ecommerce FBA gate проходят;
  - storefront payment collection read slice публикует `PaymentCollectionFetchRequest`, owner `fetch_payment_collection` facade, native endpoint `payment/payment-collection` и GraphQL query `storefrontPaymentCollection(cartId)`; оба transport path возвращают единый `PaymentCollection` DTO, используют MissingServer-only fallback и сохраняют cart ownership validation до чтения коллекции;
  - storefront refund-summary slice публикует `RefundSummaryFetchRequest`, `RefundSummary`, owner `fetch_refund_summary` facade, native endpoint `payment/refund-summary` и GraphQL `storefrontRefunds` projection; оба path сохраняют tenant/customer order ownership check, decimal-safe aggregation и MissingServer-only fallback, а commerce-owned refund read удалён атомарно;
  - FBA maintenance slice перевёл read-only `read_collection_status` path на shared `PortCallPolicy::read()`, а create/reuse write path — на shared `PortCallPolicy::write()` без изменения commerce compatibility transport.
  - umbrella facade `rustok_commerce::{services::payment, PaymentService}` is removed; commerce REST/GraphQL/storefront/test consumers import `PaymentService` from `rustok-payment` directly, so payment owner service is no longer masked by the ecommerce umbrella.
  - in-process реализация `PaymentCollectionPort for PaymentService` добавлена в `src/ports.rs`: create/reuse path требует shared `PortCallPolicy::write()`, переиспользует reusable cart collection перед созданием новой и мапит `PaymentError` в `PortError`;
  - `src/ports.rs` теперь экспортирует `PaymentCollectionPort` и DTO для create/reuse/status операций; machine-readable registry и verifier проверяют совпадение port trait operations с FBA metadata;
  - метаданные FBA-provider открыты для `payment collection create/reuse` через `crates/rustok-payment/contracts/payment-fba-registry.json`; provider SPI boundary поднят до `boundary_ready` на executed live-adapter evidence, while base checkout port contract/fallback evidence remains a follow-up before `transport_verified`;
  - registry теперь фиксирует `contract_tests.status = planned_cases_locked`: для каждой port operation задана in-process/remote-adapter-placeholder case matrix, baseline assertions (`typed_port_error_mapping`, `context_deadline_preserved`) с явным deadline enforcement для read path и `write_idempotency_required` только на write operations; fallback smoke profile set; static evidence packet `crates/rustok-payment/contracts/evidence/payment-contract-test-static-matrix.json` is locked by `npm run verify:ecommerce:fba` (registry + evidence gates) and `npm run verify:ecommerce:fba-contract-evidence`; это закрывает metadata/evidence anti-drift для будущих base port contract tests;
  - provider SPI evidence теперь закреплён в `crates/rustok-payment/contracts/evidence/payment-provider-spi-static-matrix.json`: manual/remote-placeholder cases для `authorize`/`capture`/`cancel`/`refund` проверяют typed provider error mapping, idempotency-key preservation и запрет persistence в adapter layer, webhook replay contract фиксирует idempotent duplicate delivery и делегирование lifecycle transition в `PaymentService`, а `src/providers.rs` содержит external registration contract (`ExternalPaymentProviderRegistration`, health/degraded-mode DTOs, descriptor-id validation, `PaymentProviderRegistry`) с source markers, которые проверяет `scripts/verify/verify-ecommerce-provider-spi-evidence.mjs`; packet не повышает FBA статус без runtime execution;
  - provider registry runtime-mode guardrails теперь side-effect-free проверяют capability support, missing-provider errors и health/degraded-mode mapping до вызова external adapter-а; registry также публикует guarded async `execute_authorize`/`execute_capture`/`execute_cancel`/`execute_refund`/`execute_webhook` seams, которые блокируют unavailable providers до adapter side effects и оставляют lifecycle persistence в `PaymentService`; targeted provider SPI tests фиксируют fallback profile propagation и operation capability rejection без полной компиляции в этой итерации;
  - provider SPI runtime-smoke evidence теперь закреплён в `crates/rustok-payment/contracts/evidence/payment-provider-spi-runtime-smoke.json`, а dedicated live-adapter contract — в `crates/rustok-payment/contracts/evidence/payment-provider-spi-live-adapter-contract.json`: no-compile packets фиксируют missing-provider lookup, unsupported/unknown operation rejection, degraded fallback propagation, unavailable-provider non-executable mode, registration failure cases, webhook replay guardrails и обязательные live gateway execution cases; `scripts/verify/verify-ecommerce-provider-spi-evidence.mjs` проверяет этот packet вместе со static matrix;
  - live external gateway execution plan теперь закреплён внутри runtime-smoke packet: verifier требует concrete-adapter evidence для guarded single invocation, typed provider-error mapping без lifecycle persistence, degraded fallback propagation, unavailable-mode adapter blocking и webhook replay delegation;
  - live external gateway execution evidence теперь закреплён в `crates/rustok-payment/contracts/evidence/payment-provider-spi-live-adapter-evidence.json`: packet фиксирует concrete-adapter contract execution для guarded single invocation, typed provider-error mapping без lifecycle persistence, degraded fallback profile `manual_review`, unavailable-mode adapter blocking и idempotent webhook replay delegation; `scripts/verify/verify-ecommerce-provider-spi-evidence.mjs` теперь валидирует этот executed evidence рядом со static/runtime-smoke/contract packets без Cargo compilation and gates the `boundary_ready` status.
  - storefront UI slice lives in `storefront/src/core.rs` + `storefront/src/ui/leptos.rs`; `storefront/src/transport.rs` owns request normalization, command metadata, typed `PaymentTransportError`, payment-owned result DTOs and MissingServer-gated `create_payment_collection`, `fetch_payment_collection` and `fetch_refund_summary` facades, while GraphQL/native adapters own public transport payloads and endpoint shells over the explicit commerce checkout runtime API;
  - fast boundary guardrail `scripts/verify/verify-payment-storefront-boundary.mjs` is wired into `npm run verify:ffa:ui:migration`, self-checks package wiring, and checks the payment-owned core/transport/ui split without long Cargo compilation;
  - manifest-driven storefront composition now registers `rustok-payment-storefront` in `checkout_payment_handoff`; `PaymentView` is the zero-prop host entry adapter, reads the effective locale from `UiRouteContext.locale`, and resolves copy through the module-owned `en`/`ru` catalog declared by `[provides.storefront_ui.i18n]`;
  - любые изменения UI/transport boundary должны фиксироваться с parity/boundary evidence в этом же инкременте.
- Last verified at (UTC): 2026-06-30T12:05:57Z
- Owner: `rustok-payment` module team

## Область работ

- удерживать `rustok-payment` как owner payment/payment-collection boundary;
- синхронизировать payment runtime contract и local docs;
- не смешивать базовую payment domain model с provider-specific integrations.

## Текущее состояние

- `payment_collections`, `payments`, `PaymentModule` и `PaymentService` уже выделены;
- модуль не владеет cart/order/customer, а только ссылается на них по identifiers;
- базовый manual/default payment flow уже зафиксирован;
- GraphQL create/reuse execution and native server-function execution are published by `rustok-payment/storefront`; commerce exposes only the shared checkout runtime API, and fallback remains MissingServer-only.

## Этапы

### 1. Contract stability

- [x] закрепить payment/payment-collection boundary;
- [x] удерживать manual/default flow внутри базового доменного слоя;
- [ ] удерживать sync между payment runtime contract, commerce transport и module metadata.

### 2. Provider expansion

- [x] сформировать provider SPI baseline до подключения внешних gateway integrations;
- [x] добавить static provider SPI contract matrix и webhook ingress/replay contract;
- [x] покрывать authorize/capture/cancel/refund semantics targeted tests;
- [x] зафиксировать external provider registration contract без provider-specific webhook logic в базовом payment domain contract.
- [x] добавить payment-owned provider registry seam для host composition без lifecycle persistence в adapter layer.
- [x] добавить side-effect-free runtime-mode guardrails для capability checks и degraded-mode fallback mapping до invocation external adapter-а.
- [x] зафиксировать no-compile live gateway adapter execution contract packet.
- [x] заменить static/no-compile provider SPI evidence live runtime contract execution against concrete external adapters.
- [x] добавить owner registry guarded async invocation seam для provider adapter calls до production gateway wiring.

### 3. Operability

- [x] документировать static provider SPI guarantees одновременно с evidence gate;
- [ ] удерживать local docs и `README.md` синхронизированными;
- [x] обновлять umbrella commerce docs при изменении payment/provider scope.

## Проверка

- `cargo xtask module validate payment`
- `cargo xtask module test payment`
- targeted tests для payment collection lifecycle, manual flow и provider-ready semantics

## Правила обновления

1. При изменении payment runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении provider architecture или checkout orchestration обновлять umbrella docs.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
