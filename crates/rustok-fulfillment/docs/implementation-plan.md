# План реализации `rustok-fulfillment`

Статус: fulfillment boundary выделен; shipping options и fulfillments уже
служат основой для deliverability domain, а provider SPI и seller-aware model
ещё остаются в активном backlog umbrella `rustok-commerce`.

## Область работ

- удерживать `rustok-fulfillment` как owner shipping-option/fulfillment boundary;
- синхронизировать shipping contracts, allowed profile bindings и local docs;
- не смешивать базовую shipping domain model с provider-specific delivery logic.

## Текущее состояние

- `shipping_options`, `fulfillments`, `FulfillmentModule` и `FulfillmentService` уже выделены;
- first-class `allowed_shipping_profile_slugs` уже являются частью live contract;
- deliverability orchestration с `delivery_groups[]`, `shipping_selections[]` и multi-fulfillment checkout строится umbrella `rustok-commerce` поверх этого boundary;
- admin/operator surface уже использует typed lifecycle для shipping options.

## Этапы

### 1. Contract stability

- [x] закрепить shipping-option/fulfillment boundary;
- [x] встроить first-class `allowed_shipping_profile_slugs`;
- [x] удерживать compatibility shim для single-group carts только как переходный transport layer;
- [ ] удерживать sync между fulfillment runtime contract, commerce orchestration и module metadata.

### 2. Deliverability expansion

- [ ] довести seller-aware grouping и richer fulfillment-item model без размывания boundary;
- [ ] покрывать mixed-cart и multi-fulfillment edge-cases targeted tests;
- [ ] удерживать compatibility с payment/order orchestration и shipping-profile registry.

### 3. Operability

- [ ] документировать новые fulfillment guarantees одновременно с изменением runtime surface;
- [ ] удерживать local docs и `README.md` синхронизированными;
- [ ] обновлять umbrella commerce docs при изменении deliverability/provider scope.

## Проверка

- `cargo xtask module validate fulfillment`
- `cargo xtask module test fulfillment`
- targeted tests для shipping options, fulfillments, delivery groups и multi-fulfillment invariants

## Правила обновления

1. При изменении fulfillment runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении deliverability/provider architecture обновлять umbrella docs.
