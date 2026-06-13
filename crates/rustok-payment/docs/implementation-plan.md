# План реализации `rustok-payment`

Статус: payment boundary выделен; базовый manual/default flow уже есть, а
provider SPI и richer payment lifecycle остаются в backlog umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: storefront_ffa_slice
- Last checkpoint: Payment storefront FFA slice now owns payment collection create/reuse command normalization in `storefront/src/transport.rs`; `rustok-commerce-storefront` builds payment commands through the payment-owned transport request before invoking the temporary commerce orchestration adapter.
- Next step: Move the async native/GraphQL payment collection transport adapter behind `rustok-payment/storefront` when the host route can depend on the owner package without circular orchestration; keep commerce only as temporary checkout orchestration until that cutover.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-06-13T23:20:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `core_transport_ui`
- Evidence:
  - модуль ведётся в ускоренном FFA migration track; FBA остаётся `not_started` до закрытия FFA phase-gate как часть ecommerce family;
  - storefront UI slice now lives in `storefront/src/core.rs` + `storefront/src/ui/leptos.rs` and owns payment-collection card presentation/fallback policy plus create/reuse action button labels; `storefront/src/transport.rs` owns payment collection create/reuse request normalization consumed by commerce checkout orchestration during the compatibility window;
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
