# План реализации `rustok-inventory`

Статус: inventory boundary выделен; модуль держит stock/runtime baseline, backend
admin read-side service, native server-function read transport и module-owned admin UI, а dedicated inventory write transport
и channel-aware orchestration дособираются через umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: wave5_read_facade
- Last checkpoint: Подключён primary native `#[server]` read path в `crates/rustok-inventory/admin/src/native.rs`: bootstrap/list/detail извлекают `AuthContext`, `TenantContext`, `RequestContext`, проверяют inventory permissions и tenant match, затем вызывают backend `AdminInventoryReadService`. Transitional commerce GraphQL adapter остаётся package-private fallback-ом только для native-unavailable ошибок, а `admin/tests/boundary.rs` закрепляет GraphQL runtime boundary.
- Next step: Расширить parity coverage для native read path против transitional adapter и вынести dedicated inventory write/mutation transport из umbrella `rustok-commerce`.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-06-02T08:35:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - модуль ведётся в ускоренном FFA/FBA migration track как часть ecommerce family;
  - backend crate экспортирует `AdminInventoryReadService` и typed read DTO (`AdminInventoryProductList`, `AdminInventoryProductDetail`, variants/prices/translations) как inventory-owned read-side source для native server-function transport;
  - inventory admin UI вынесен в explicit `ui/leptos.rs` adapter, вызывает inventory-owned `core`/`api` facade, primary read path идёт через dedicated `admin/src/native.rs` native `#[server]` functions, а transport boundary держит transitional commerce GraphQL adapter внутри пакета только как native-unavailable fallback;
  - unit tests покрывают locale fallback, tags extraction, price sale mapping, search normalization и variant title fallback в backend read-side service;
  - compatibility tests фиксируют минимальные поля read model (`inventoryQuantity`, `inventoryPolicy`, `inStock`, variants/translations/feed paging), сериализацию normalized GraphQL variables, facade request builders и mapping `GraphqlHttpError` → inventory-owned `InventoryTransportError` до выделения dedicated inventory transport;
  - `admin/tests/boundary.rs` проверяет, что `leptos_graphql`, `GraphqlRequest`, `GraphqlHttpError`, `/api/graphql` и `RUSTOK_GRAPHQL_URL` не попадают в `api`, `core`, `model`, `native` или `ui`.
- Last verified at (UTC): 2026-06-02T08:35:00Z
- Owner: `rustok-inventory` module team

## Область работ

- удерживать `rustok-inventory` как owner inventory/stock boundary;
- синхронизировать inventory runtime contract, module-owned admin UI и local docs;
- не смешивать inventory logic с catalog, fulfillment или storefront transport.

## Текущее состояние

- `InventoryModule`, `InventoryService`, backend `AdminInventoryReadService` и stock-related migrations уже выделены;
- модуль зависит от `product`, не создавая цикла на umbrella `rustok-commerce`;
- backend admin read service уже возвращает inventory-owned DTO для product/variant/price/translations read-side;
- transport adapters по-прежнему публикуются фасадом `rustok-commerce`;
- `rustok-inventory/admin` уже публикует inventory-owned admin route для stock visibility,
  low-stock triage и variant-level health inspection;
- dedicated inventory mutations пока не вынесены: текущий inventory UI использует
  inventory-owned read facade, внутри которого commerce GraphQL остаётся transitional adapter-ом;
- dedicated native/server-function read transport подключён к backend `AdminInventoryReadService`; GraphQL остаётся transitional compatibility fallback-ом только когда native path недоступен.

## Этапы

### 1. Contract stability

- [x] закрепить inventory boundary как отдельный модуль;
- [x] удерживать product dependency без цикла на umbrella;
- [x] вынести inventory admin UI в module-owned пакет `rustok-inventory/admin`;
- [x] удерживать sync между inventory runtime contract, admin UI, commerce orchestration
  и module metadata через local docs + registry evidence.

### 2. Inventory transport split

- [x] добавить backend inventory-owned admin read service/read DTO для product/variant/price/translations read-side;
- [x] добавить inventory-owned core/read facade и explicit Leptos adapter для admin UI, изолировав текущий commerce GraphQL доступ в transitional adapter-е и закрепив это boundary test-ом;
- [x] подключить dedicated inventory read transport/native `#[server]` path к backend `AdminInventoryReadService`;
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

- [x] документировать backend admin read-side service одновременно с изменением runtime surface;
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


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
