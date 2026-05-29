# План реализации `rustok-order`

Статус: order boundary выделен; модуль владеет order write-side lifecycle,
outbox publication и module-owned admin UI, а post-order и transport parity
дособираются umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: post_order_customer_returns_graphql_mutation
- Last checkpoint: Slice 10.2 дополнен: umbrella `rustok-commerce` добавил customer-safe storefront REST list/create, GraphQL list и GraphQL create-return boundary поверх order-owned `OrderService`, сохраняя item-level validation внутри `rustok-order`.
- Next step: Связать returns orchestration с refund/exchange decision tree без переноса payment logic в `rustok-order`; storefront GraphQL create/list parity уже закрывает текущий transport slice.
- Open blockers: серверный OpenAPI contract test под default features упирается в существующие compile errors вне order/commerce (`rustok-pages-admin`, server build service/module lifecycle/graphql mutations); targeted order lifecycle и `rustok-commerce` check проходят.
- Hand-off notes for next agent: После каждого returns/refund/exchange инкремента обновлять FFA/FBA evidence, README/admin docs и central registry в том же PR.
- Last updated at (UTC): 2026-05-29T18:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Evidence:
  - модуль ведётся в ускоренном FFA/FBA migration track как часть ecommerce family;
  - любые изменения UI/transport boundary должны фиксироваться с parity/boundary evidence в этом же инкременте.
- Last verified at (UTC): 2026-05-29T18:00:00Z
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
- `rustok-order/admin` публикует module-owned route для order list/detail/lifecycle.

## Этапы

### 1. Contract stability

- [x] закрепить order-owned lifecycle и snapshot model;
- [x] добавить typed order adjustment snapshot с `subtotal_amount`, `adjustment_total` и net `total_amount`;
- [x] удерживать event publication частью module boundary;
- [x] вынести admin order UI в module-owned пакет `rustok-order/admin`;
- [ ] удерживать sync между order runtime contract, commerce transport и module metadata.

### 2. Post-order expansion

- [~] развивать returns, refunds, exchanges и order changes как отдельный следующий слой; (started: `order_returns` + `order_return_items` storage, item validation + `OrderService::{create_return,get_return,list_returns,complete_return,cancel_return}` foundation)
- [x] покрывать lifecycle transitions и failure semantics targeted tests; (return lifecycle `pending -> completed|cancelled`, second-transition guard, tenant-scoped show)
- [ ] удерживать compatibility с payment/fulfillment orchestration без размывания order ownership.

### 3. Operability

- [~] документировать новые order guarantees одновременно с изменением runtime surface; (returns lifecycle и item-level lines checkpoint зафиксированы)
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
