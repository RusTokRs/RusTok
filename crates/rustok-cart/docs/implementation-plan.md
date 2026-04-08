# План реализации `rustok-cart`

Статус: cart boundary выделен; модуль остаётся owner-ом cart state и storefront
context snapshot, а orchestration над checkout живёт в umbrella `rustok-commerce`.

## Область работ

- удерживать `rustok-cart` как owner cart lifecycle и line-item state;
- синхронизировать cart snapshot contract, runtime dependencies и local docs;
- не допускать возврата cart domain logic обратно в umbrella или host слой.

## Текущее состояние

- `carts` и `cart_line_items` уже module-owned;
- cart lifecycle и persisted storefront context snapshot уже встроены в базовый contract;
- transport adapters по-прежнему публикуются фасадом `rustok-commerce`, без цикла зависимостей;
- channel/context/deliverability orchestration поверх cart выполняется на уровне umbrella-модуля.

## Этапы

### 1. Contract stability

- [x] зафиксировать cart lifecycle и storefront context snapshot;
- [x] удерживать line-item CRUD и totals внутри `rustok-cart`;
- [ ] удерживать sync между cart runtime contract, commerce orchestration и module metadata.

### 2. Checkout hardening

- [ ] удерживать `checking_out`/recovery semantics совместимыми с payment/order orchestration;
- [ ] покрывать stale snapshot, shipping selection и multi-group edge-cases targeted tests;
- [ ] развивать cart state только через explicit snapshot/versioning semantics.

### 3. Operability

- [ ] документировать новые cart guarantees одновременно с изменением checkout flows;
- [ ] удерживать local docs и `README.md` синхронизированными с storefront contract;
- [ ] расширять diagnostics только при реальном runtime pressure.

## Проверка

- `cargo xtask module validate cart`
- `cargo xtask module test cart`
- targeted tests для cart lifecycle, line items, snapshot context и checkout-preflight semantics

## Правила обновления

1. При изменении cart runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении checkout orchestration expectations обновлять umbrella docs в `rustok-commerce`.
