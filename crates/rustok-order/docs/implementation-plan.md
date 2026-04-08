# План реализации `rustok-order`

Статус: order boundary выделен; модуль владеет order write-side lifecycle,
outbox publication и module-owned admin UI, а post-order и transport parity
дособираются umbrella `rustok-commerce`.

## Область работ

- удерживать `rustok-order` как owner order lifecycle и order snapshots;
- синхронизировать order runtime contract, event flow, admin UI и local docs;
- не смешивать order write model с payment/fulfillment/provider orchestration.

## Текущее состояние

- `orders` и `order_line_items` уже module-owned;
- write-side lifecycle и order events уже закреплены внутри модуля;
- product/variant связи хранятся как snapshot references, без cross-module FK;
- transport adapters по-прежнему публикуются фасадом `rustok-commerce`;
- `rustok-order/admin` публикует module-owned route для order list/detail/lifecycle.

## Этапы

### 1. Contract stability

- [x] закрепить order-owned lifecycle и snapshot model;
- [x] удерживать event publication частью module boundary;
- [x] вынести admin order UI в module-owned пакет `rustok-order/admin`;
- [ ] удерживать sync между order runtime contract, commerce transport и module metadata.

### 2. Post-order expansion

- [ ] развивать returns, refunds, exchanges и order changes как отдельный следующий слой;
- [ ] покрывать lifecycle transitions и failure semantics targeted tests;
- [ ] удерживать compatibility с payment/fulfillment orchestration без размывания order ownership.

### 3. Operability

- [ ] документировать новые order guarantees одновременно с изменением runtime surface;
- [ ] удерживать local docs и `README.md` синхронизированными;
- [ ] обновлять umbrella commerce docs при изменении order/post-order scope.

## Проверка

- `cargo xtask module validate order`
- `cargo xtask module test order`
- targeted tests для order lifecycle, outbox events и snapshot invariants

## Правила обновления

1. При изменении order runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md`, `admin/README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении order/payment/fulfillment orchestration обновлять umbrella docs.
