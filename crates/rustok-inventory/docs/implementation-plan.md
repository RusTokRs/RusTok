# План реализации `rustok-inventory`

Статус: inventory boundary выделен; модуль держит stock/runtime baseline и module-owned
admin read-side UI, а dedicated inventory write transport и channel-aware orchestration
дособираются через umbrella `rustok-commerce`.

## Область работ

- удерживать `rustok-inventory` как owner inventory/stock boundary;
- синхронизировать inventory runtime contract, module-owned admin UI и local docs;
- не смешивать inventory logic с catalog, fulfillment или storefront transport.

## Текущее состояние

- `InventoryModule`, `InventoryService` и stock-related migrations уже выделены;
- модуль зависит от `product`, не создавая цикла на umbrella `rustok-commerce`;
- transport adapters по-прежнему публикуются фасадом `rustok-commerce`;
- `rustok-inventory/admin` уже публикует inventory-owned admin route для stock visibility,
  low-stock triage и variant-level health inspection;
- dedicated inventory mutations пока не вынесены: текущий inventory UI честно остаётся
  read-side поверх существующего product GraphQL контракта.

## Этапы

### 1. Contract stability

- [x] закрепить inventory boundary как отдельный модуль;
- [x] удерживать product dependency без цикла на umbrella;
- [x] вынести inventory admin UI в module-owned пакет `rustok-inventory/admin`;
- [ ] удерживать sync между inventory runtime contract, admin UI, commerce orchestration
  и module metadata.

### 2. Inventory transport split

- [ ] вынести dedicated inventory read/write transport из umbrella `rustok-commerce`;
- [ ] перевести inventory admin UI с read-only product-backed transport на inventory-owned
  mutations и targeted stock operations;
- [ ] покрывать transport parity и stock mutation semantics targeted tests.

### 3. Availability hardening

- [ ] развивать stock locations, reservations и availability semantics как module-owned contract;
- [ ] покрывать channel-aware availability edge-cases targeted tests через integration
  с umbrella;
- [ ] удерживать read/write paths совместимыми с checkout и catalog visibility flows.

### 4. Operability

- [ ] документировать новые inventory guarantees одновременно с изменением runtime surface;
- [ ] удерживать local docs и `README.md` синхронизированными;
- [ ] обновлять umbrella commerce docs при изменении availability semantics.

## Проверка

- `cargo xtask module validate inventory`
- `cargo xtask module test inventory`
- targeted tests для stock mutations, inventory transport и checkout-facing invariants

## Правила обновления

1. При изменении inventory runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md`, `admin/README.md`
   и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении inventory/checkout/channel-aware orchestration обновлять umbrella docs.
