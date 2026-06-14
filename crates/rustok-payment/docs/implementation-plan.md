# План реализации `rustok-payment`

Статус: payment boundary выделен; базовый manual/default flow уже есть, а
provider SPI и richer payment lifecycle остаются в backlog umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: storefront_action_request_boundary
- Last checkpoint: Payment storefront action UI now emits `PaymentCollectionCreateRequest` with payment-owned create/reuse command metadata via the payment-owned `storefront/src/transport.rs` facade, and the compatibility host forwards the owner DTO into native/GraphQL orchestration payload metadata instead of creating anonymous commerce-side command metadata.
- Next step: Move the async native/GraphQL payment collection transport adapter behind `rustok-payment/storefront` when the host route can depend on the owner package without circular orchestration; keep commerce only as temporary checkout orchestration until that cutover.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-06-14T01:30:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - in-process реализация `PaymentCollectionPort for PaymentService` добавлена в `src/ports.rs`: create/reuse path требует `PortContext::require_write_semantics`, переиспользует reusable cart collection перед созданием новой и мапит `PaymentError` в `PortError`;
  - `src/ports.rs` теперь экспортирует `PaymentCollectionPort` и DTO для create/reuse/status операций; machine-readable registry и verifier проверяют совпадение port trait operations с FBA metadata;
  - метаданные FBA-provider открыты для `payment collection create/reuse` через `crates/rustok-payment/contracts/payment-fba-registry.json`; статус остаётся `in_progress` до появления contract tests/remote transport evidence, которые позволят подняться выше embedded checkout compatibility;
  - registry теперь фиксирует `contract_tests.status = planned_cases_locked`: для каждой port operation задана in-process/remote-adapter-placeholder case matrix, baseline assertions (`typed_port_error_mapping`, `context_deadline_preserved`) и fallback smoke profile set; это закрывает metadata anti-drift для будущих contract tests, но не повышает статус без runtime evidence;
  - storefront UI slice now lives in `storefront/src/core.rs` + `storefront/src/ui/leptos.rs` and owns payment-collection card presentation/fallback policy plus create/reuse action button labels; `storefront/src/transport.rs` owns payment collection create/reuse request normalization and command metadata, `PaymentCollectionActionButton` emits `PaymentCollectionCreateRequest` to the temporary commerce checkout orchestration callback during the compatibility window, and commerce maps the owner DTO metadata into native/GraphQL payloads instead of exposing a duplicate payment request builder;
  - fast boundary guardrail `scripts/verify/verify-payment-storefront-boundary.mjs` is wired into `npm run verify:ffa:ui:migration`, self-checks package wiring, and checks the payment-owned core/transport/ui split without long Cargo compilation;
  - любые изменения UI/transport boundary должны фиксироваться с parity/boundary evidence в этом же инкременте.
- Last verified at (UTC): 2026-05-24T00:00:00Z
- Owner: `rustok-payment` module team

## Область работ

- удерживать `rustok-payment` как owner payment/payment-collection boundary;
- синхронизировать payment runtime contract и local docs;
- не смешивать базовую payment domain model с provider-specific integrations.

## Текущее состояние

- `payment_collections`, `payments`, `PaymentModule` и `PaymentService` уже выделены;
- модуль не владеет cart/order/customer, а только ссылается на них по identifiers;
- базовый manual/default payment flow уже зафиксирован;
- async transport adapters по-прежнему публикуются фасадом `rustok-commerce`, но storefront payment presentation и create/reuse command normalization уже принадлежат `rustok-payment/storefront`.

## Этапы

### 1. Contract stability

- [x] закрепить payment/payment-collection boundary;
- [x] удерживать manual/default flow внутри базового доменного слоя;
- [ ] удерживать sync между payment runtime contract, commerce transport и module metadata.

### 2. Provider expansion

- [ ] сформировать provider SPI до подключения внешних gateway integrations;
- [x] покрывать authorize/capture/cancel/refund semantics targeted tests;
- [ ] не смешивать provider-specific webhook logic с базовым payment domain contract.

### 3. Operability

- [ ] документировать новые payment guarantees одновременно с изменением runtime surface;
- [ ] удерживать local docs и `README.md` синхронизированными;
- [ ] обновлять umbrella commerce docs при изменении payment/provider scope.

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
