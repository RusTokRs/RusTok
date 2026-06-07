# План реализации `rustok-inventory`

Статус: inventory boundary выделен; модуль держит stock/runtime baseline, backend
admin read-side service, native server-function read transport, первые native write endpoints set-quantity/adjust-quantity/reserve-quantity/release-reservation, dedicated availability check endpoint и module-owned admin UI, а оставшийся write parity
и channel-aware orchestration дособираются через umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: wave5_write_transport_split
- Last checkpoint: Set-quantity semantics выровнены с reservation-aware admin read model: `InventoryService::set_inventory` теперь трактует requested quantity как целевую available quantity и сохраняет existing reserved units через `stocked_quantity_for_available(available, reserved)`, чтобы optimistic UI и следующий read-side snapshot не расходились при активных reservations; targeted unit test фиксирует этот расчёт. Reservation write validation также вынесена в явный backend helper `validate_reservation_quantity`, который отклоняет negative reserve requests до открытия transaction/DB lookup; рядом закреплён targeted unit test, симметричный уже существующим release-reservation и check-availability guardrails. Release-reservation остаётся доведённым до согласованного backend semantics: backend `InventoryService::release_reservation_quantity` возвращает typed `InventoryReservationReleaseWriteResult { released_quantity, available_quantity, in_stock }`, native/API facade `inventory/variant/release-reservation` проходит tenant/permission checks без GraphQL fallback, UI вызывает его через targeted Release reservation action, применяет `available_quantity/in_stock` к detail state, показывает released quantity из typed result, а backend release path не создаёт inventory item/level при failed release, проверяет tracked `reservation_items` до мутации и списывает release из существующих reservation item rows вместе с reserved quantity в существующих levels. Availability check остаётся native-only и вызывается из detail UI через targeted Check availability action; reserve/set/adjust quantity также остаются native-only, с typed `InventoryReservationWriteResult { reserved_quantity, available_quantity, in_stock }` и `InventoryQuantityWriteResult { quantity, in_stock }`. Следующий малый UI-boundary slice разделил client-side parse helpers и i18n copy для reservation и availability flows: availability action теперь использует domain-labeled `parse_availability_quantity` и dedicated `inventory.error.invalidAvailabilityQuantity`, reservation/release actions используют `inventory.error.invalidReservationQuantity`, а boundary test дополнительно фиксирует эти markers и запрещает `releaseReservation` в transitional GraphQL adapter-е. Следующий availability semantics slice нормализовал backorder policy matching в общем inventory-owned helper-е: `continue` теперь сравнивается case-insensitive для write result `in_stock`, set/adjust/reserve/check-availability guardrails, admin read-side stock state, commerce checkout validation и storefront inventory projection, а helper экспортируется из `rustok-inventory`, чтобы split-module и umbrella compatibility paths не расходились. Текущий малый backend semantics slice закрыл фактическое применение этого helper-а в native set/adjust quantity result: `InventoryQuantityWriteResult::from_quantity_and_policy` теперь сохраняет `in_stock=true` для depleted backorderable variants, а `set_variant_quantity`/`adjust_variant_quantity` строят typed result с inventory policy из того же mutation path вместо дополнительного pre-read/bare quantity heuristic; unit test закрепляет case-insensitive `CONTINUE` для native write facades. Следующий Wave 6 guardrail slice добавил быстрый source-level `node scripts/verify/verify-inventory-admin-boundary.mjs`, который фиксирует policy-aware write result, отсутствие duplicate pre-read, native-only admin write facades и read-only transitional GraphQL adapter без полной Rust-компиляции. Текущий verification-hardening slice добавил fixture tests `node scripts/verify/verify-inventory-admin-boundary.test.mjs`, чтобы сам guardrail доказывал pass path и падение на duplicate pre-read / GraphQL mutation drift.
- Next step: Перевести следующий remaining inventory write mutation beyond set/adjust/reserve/release/check-availability из umbrella `rustok-commerce` на inventory-owned native/API facade, используя typed result contract, и добавить targeted mutation semantics test; при следующих write slices сохранять policy-aware `in_stock` semantics для typed results.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-06-07T02:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - модуль ведётся в ускоренном FFA/FBA migration track как часть ecommerce family;
  - backend crate экспортирует `AdminInventoryReadService` и typed read DTO (`AdminInventoryProductList`, `AdminInventoryProductDetail`, variants/prices/translations) как inventory-owned read-side source для native server-function transport;
  - inventory admin UI вынесен в explicit `ui/leptos.rs` adapter, вызывает inventory-owned `core`/`api` facade, primary read path идёт через dedicated `admin/src/native.rs` native `#[server]` functions, write split представлен native `inventory/variant/set-quantity`, `inventory/variant/adjust-quantity`, `inventory/variant/reserve-quantity`, `inventory/variant/release-reservation` и `inventory/variant/check-availability` endpoint-ами с typed `InventoryQuantityWriteResult` / `InventoryReservationWriteResult` / `InventoryReservationReleaseWriteResult` / `InventoryAvailabilityCheckResult`; UI targeted set-quantity, +/-1 adjustment, reserve, release-reservation и check-availability controls работают без GraphQL fallback, применяют quantity/in-stock или available-quantity/in-stock state из write result, а transport boundary держит transitional commerce GraphQL adapter внутри пакета только как native-unavailable read fallback;
  - unit tests покрывают locale fallback, tags extraction, price sale mapping, search normalization, variant title fallback в backend read-side service, service-level non-negative reservation/availability request invariants, reservation-aware set-quantity stocked/available calculation, policy-aware set/adjust quantity `in_stock` typed result semantics, no-create reservation release error semantics и tracked reservation item release guardrail;
  - compatibility tests фиксируют минимальные поля read model (`inventoryQuantity`, `inventoryPolicy`, `inStock`, variants/translations/feed paging), model serde snapshots для product list/detail, source-level parity между backend DTO/native mapper/transitional GraphQL adapter, сериализацию normalized GraphQL variables, facade request builders и mapping `GraphqlHttpError` → inventory-owned `InventoryTransportError` до выделения dedicated inventory transport;
  - `admin/tests/boundary.rs` проверяет, что `leptos_graphql`, `GraphqlRequest`, `GraphqlHttpError`, `/api/graphql` и `RUSTOK_GRAPHQL_URL` не попадают в `api`, `core`, `model`, `native` или `ui`, а read/write boundary checks разделяют native read markers, read-only transitional GraphQL adapter/removal criteria и native-only set/adjust/reserve/release quantity plus availability-check facades и set-quantity/+/-1/reserve/release/check-availability UI без transitional GraphQL fallback;
  - `node scripts/verify/verify-inventory-admin-boundary.mjs` добавлен как быстрый Wave 6 source-level gate для тех же inventory-owned admin write/read invariants без полной Rust-компиляции.
- Last verified at (UTC): 2026-06-06T00:00:00Z
- Owner: `rustok-inventory` module team

## Область работ

- удерживать `rustok-inventory` как owner inventory/stock boundary;
- синхронизировать inventory runtime contract, module-owned admin UI и local docs;
- не смешивать inventory logic с catalog, fulfillment или storefront transport.

## Текущее состояние

- `InventoryModule`, `InventoryService`, backend `AdminInventoryReadService` и stock-related migrations уже выделены;
- модуль зависит от `product`, не создавая цикла на umbrella `rustok-commerce`;
- backend admin read service уже возвращает inventory-owned DTO для product/variant/price/translations read-side и читает available quantity из `inventory_items`/`inventory_levels`, если stock-level state уже создан;
- read transport уже имеет dedicated native path, первые set-quantity/adjust-quantity/reserve-quantity/release-reservation write endpoints и availability-check facade вынесены в inventory-owned native facade, оставшийся mutation parity всё ещё дособирается из umbrella `rustok-commerce`;
- `rustok-inventory/admin` уже публикует inventory-owned admin route для stock visibility,
  low-stock triage и variant-level health inspection;
- dedicated inventory mutations/validation частично вынесены: `set_variant_quantity`, `adjust_variant_quantity`, `reserve_variant_quantity`, `release_reservation_quantity` и `check_variant_availability` уже идут через inventory-owned native server functions без GraphQL fallback и подключены к initial UI targeted stock/availability operations, но оставшийся mutation parity ещё не завершён;
- dedicated native/server-function read transport подключён к backend `AdminInventoryReadService`; GraphQL остаётся transitional compatibility fallback-ом только когда native read path недоступен.

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
- [ ] вынести dedicated inventory read/write transport из umbrella `rustok-commerce` (read path готов; первый write split: native set-quantity/adjust-quantity/reserve-quantity/release-reservation endpoints);
- [x] подключить initial inventory admin UI targeted stock operations к inventory-owned set/adjust/reserve/release quantity mutations и check-availability validation;
- [ ] перевести оставшиеся inventory admin UI stock operations beyond set/adjust/reserve/release на inventory-owned mutations;
- [ ] покрывать transport parity и stock mutation semantics targeted tests (facade/boundary checks, write-result serde snapshots и service-level negative reserve/availability request и reservation release error semantics tests добавлены для typed set/adjust/reserve/release/check-availability endpoints; product list/detail serde snapshots, source-level backend DTO/native mapper/transitional adapter parity и read-only transitional adapter/removal-criteria boundary check закрепляют текущий read-model shape и отсутствие GraphQL write fallback).

### 3. Availability hardening

- [x] читать reservation-aware available quantity из inventory levels в admin read-side, оставляя legacy variant quantity только compatibility fallback-ом;
- [ ] развивать stock locations, reservations и availability semantics как module-owned contract;
- [ ] покрывать channel-aware availability edge-cases targeted tests через integration
  с umbrella;
- [ ] удерживать read/write paths совместимыми с checkout и catalog visibility flows.

### 4. Operability

- [x] документировать backend admin read-side service одновременно с изменением runtime surface;
- [x] документировать новые inventory guarantees одновременно с изменением runtime surface;
- [x] удерживать local docs и `README.md` синхронизированными;
- [x] обновлять umbrella commerce docs при изменении availability semantics.

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
- [x] Зафиксировать/обновить verification gates для текущего состояния модуля (`node scripts/verify/verify-inventory-admin-boundary.mjs`, `./scripts/verify/verify-all.sh inventory-admin-boundary`, `node scripts/verify/verify-inventory-admin-boundary.test.mjs`).
