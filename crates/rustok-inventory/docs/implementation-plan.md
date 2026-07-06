# Implementation plan for `rustok-inventory`

Status: inventory boundary is separated; the module holds the stock/runtime baseline, backend
admin read-side service, native-only server-function read/write transport for current admin stock operations set-quantity/adjust-quantity/reserve-quantity/release-reservation/check-availability and module-owned admin UI; the remaining tail relates to non-admin/channel-aware availability semantics and compatibility evidence with umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: wave5_verification_evidence
- Latest checkpoint: commerce REST/GraphQL cart add/update availability paths now atomically call inventory-owned `check_variant_availability_for_public_channel`; generic `InventoryReservationPort::check_availability` and its tenant/channel/locale/deadline assembly removed from transport helpers, so backorder policy and channel-visible stock lookup again have a single owner. `npm run verify:inventory:admin-boundary`, full `npm run verify:ffa:ui:migration` and `npm run verify:ecommerce:fba` pass.
- Last checkpoint: FBA maintenance slice moved inventory reserve/release write paths to shared `PortCallPolicy::write()` and kept availability on `PortCallPolicy::read()`; earlier checkpoint: Inventory received its first FBA provider slice: `InventoryReservationPort` locks neutral availability/reserve/release boundary over owner `InventoryService`, `rustok-module.toml` and `contracts/inventory-fba-registry.json` publish provider metadata for commerce/product consumers, and the fast aggregate verifier now includes inventory registry anti-drift without full compilation.
- Previous checkpoint: Set-quantity semantics aligned with reservation-aware admin read model: `InventoryService::set_inventory` now treats requested quantity as target available quantity and preserves existing reserved units through `stocked_quantity_for_available(available, reserved)`, so optimistic UI and the following read-side snapshot do not diverge under active reservations; targeted unit test locks this calculation. Reservation write validation also moved to an explicit backend helper `validate_reservation_quantity`, which rejects negative reserve requests before opening transaction/DB lookup; a targeted unit test is locked symmetrically to the existing release-reservation and check-availability guardrails. Release-reservation is brought to consistent backend semantics: backend `InventoryService::release_reservation_quantity` returns typed `InventoryReservationReleaseWriteResult { released_quantity, available_quantity, in_stock }`, native/transport facade `inventory/variant/release-reservation` passes tenant/permission checks without GraphQL fallback, UI calls it through a targeted Release reservation action, applies `available_quantity/in_stock` to detail state, shows released quantity from typed result, and backend release path does not create inventory item/level on failed release, checks tracked `reservation_items` before mutation and debits release from existing reservation item rows together with reserved quantity in existing levels. Availability check remains native-only and is called from detail UI through a targeted Check availability action; reserve/set/adjust quantity also remain native-only, with typed `InventoryReservationWriteResult { reserved_quantity, available_quantity, in_stock }` and `InventoryQuantityWriteResult { quantity, in_stock }`. The next small UI-boundary slice separated client-side parse helpers and i18n copy for reservation and availability flows: availability action now uses domain-labeled `parse_availability_quantity` and... (line truncated to 2000 chars)
- Next step: verification/CI evidence slice for `InventoryReservationPort`: close contract tests/fallback smoke and then prepare promotion to `boundary_ready`; keep the iteration small and do not run long compilation.
- Latest slice: storefront product inventory projection became inventory-owned: `PublicChannelInventoryProjection { available_quantity, in_stock }` and `load_inventory_projection_by_variant_for_public_channel` centralize available quantity + backorder policy semantics, while `rustok-commerce::storefront_channel` only applies the ready projection to DTO; the fast verifier prohibits return of direct loader/backorder branching in the commerce projection adapter. Follow-up locked a pure projection-map regression test: missing inventory levels give `available_quantity=0`, but depleted `continue` policy keeps `in_stock=true`, so bulk storefront projection does not diverge from single-variant availability semantics.
- Open blockers: None.
- Hand-off notes for next agent: After each increment, update this block.
- Last updated at (UTC): 2026-06-30T11:05:01Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- FBA contract version: `inventory.reservation.v1`
- Structural shape: `core_transport_ui`
- Evidence:
  - batch no-compile FBA gate `scripts/verify/verify-commerce-domain-fba-runtime-smoke.mjs` and fixture-regression suite check `crates/rustok-inventory/contracts/evidence/inventory-runtime-contract-smoke.json`: read/write policy order, mandatory write-idempotency before owner service invocation, typed error mapping and registry parity for fallback/degraded modes. Status remains `in_progress` until live provider execution;
  - in-process implementation of `InventoryReservationPort for InventoryService` added in `src/ports.rs`: availability read path requires shared `PortCallPolicy::read()` and calls owner channel-aware `check_variant_availability_for_channel`, reservation/release write paths require shared `PortCallPolicy::write()` and map `CommerceError` to `PortError`;
  - commerce checkout consumer now calls only runtime-composed `InventoryReservationPort` for availability validation; direct inventory helper and old constructor path removed from `CheckoutService`, all callers moved to target signature without compatibility wrapper;
  - commerce REST/GraphQL cart add/update paths call inventory-owned `check_variant_availability_for_public_channel`; line-item helpers no longer accept generic port dependency and do not duplicate tenant/channel/locale/deadline assembly, backorder policy or channel-visible stock lookup;
  - legacy umbrella facade removed: `rustok-commerce` no longer re-exports `InventoryService` or `services::inventory`, remaining integration callers use owner crate directly;
  - `src/ports.rs` exports `InventoryReservationPort` and DTO for availability/reserve/release operations; machine-readable registry and verifier check port trait operations match FBA metadata;
  - FBA-provider metadata is open for `inventory reservation/availability` through `crates/rustok-inventory/contracts/inventory-fba-registry.json`; status remains `in_progress` until contract tests/remote transport evidence, which would allow promotion above embedded checkout/storefront compatibility;
  - registry now locks `contract_tests.status = planned_cases_locked`: for each port operation, an in-process/remote-adapter-placeholder case matrix is defined, baseline assertions (`typed_port_error_mapping`, `context_deadline_preserved`) with explicit deadline enforcement for read path and `write_idempotency_required` only on write operations; fallback smoke profile set; static evidence packet `crates/rustok-inventory/contracts/evidence/inventory-contract-test-static-matrix.json` is locked by `npm run verify:ecommerce:fba` (registry + evidence gates) and `npm run verify:ecommerce:fba-contract-evidence`; this closes metadata/evidence anti-drift for future contract tests, but does not raise status without runtime evidence;
  - backend crate exports `AdminInventoryReadService` and typed read DTO (`AdminInventoryProductList`, `AdminInventoryProductDetail`, variants/prices/translations) as inventory-owned read-side source for native server-function transport;
  - inventory admin UI moved to explicit `ui/leptos.rs` adapter, calls inventory-owned `core` + `admin/src/transport/mod.rs` facade, while `admin/src/transport/native_server_adapter.rs` as the sole adapter layer accesses dedicated `admin/src/native.rs` native `#[server]` functions, write split is represented by native `inventory/variant/set-quantity`, `inventory/variant/adjust-quantity`, `inventory/variant/reserve-quantity`, `inventory/variant/release-reservation` and `inventory/variant/check-availability` endpoints with typed `InventoryQuantityWriteResult` / `InventoryReservationWriteResult` / `InventoryReservationReleaseWriteResult` / `InventoryAvailabilityCheckResult`; UI targeted set-quantity, +/-1 adjustment, reserve, release-reservation and check-availability controls work without GraphQL fallback, apply quantity/in-stock or available-quantity/in-stock state from write result, and the former transitional commerce GraphQL adapter removed from the package;
  - unit tests cover locale fallback, tags extraction, price sale mapping, search normalization, variant title fallback in backend read-side service, service-level non-negative reservation/availability request invariants, reservation-aware set-quantity stocked/available calculation, policy-aware set/adjust quantity `in_stock` typed result semantics, no-create reservation release error semantics and tracked reservation item release guardrail;
  - compatibility tests lock minimum fields of read model (`inventoryQuantity`, `inventoryPolicy`, `inStock`, variants/translations/feed paging), model serde snapshots for product list/detail, source-level parity between backend DTO/native mapper and facade request builders after removal of GraphQL variable/error-mapping coverage;
  - `admin/tests/boundary.rs` checks that `leptos_graphql`, `GraphqlRequest`, `GraphqlHttpError`, `/api/graphql`, `RUSTOK_GRAPHQL_URL`, `CommerceGraphqlInventoryReadAdapter`, `transitional_read_transport`, `fallback_`, legacy `src/transport.rs` and pre-FFA `src/api.rs` are absent, and read/write boundary checks separate transport facade, explicit `native_server_adapter`, native read markers, native-only set/adjust/reserve/release quantity plus availability-check facades and set-quantity/+/-1/reserve/release/check-availability UI without GraphQL fallback;
  - `node scripts/verify/verify-inventory-admin-boundary.mjs` added as a fast Wave 6 source-level gate for the same inventory-owned admin write/read invariants and prohibition of returning GraphQL fallback without full Rust compilation;
  - public-channel inventory visibility/projection helpers are now exported from `rustok-inventory`, while `rustok-commerce::storefront_channel` keeps only request-context wiring and application of inventory-owned availability/projection to commerce DTO.
- Last verified at (UTC): 2026-06-30T11:05:01Z
- Owner: `rustok-inventory` module team

## Scope of work

- keep `rustok-inventory` as the owner inventory/stock boundary;
- synchronize inventory runtime contract, module-owned admin UI and local docs;
- do not mix inventory logic with catalog, fulfillment or storefront transport.

## Current state

- `InventoryModule`, `InventoryService`, backend `AdminInventoryReadService` and stock-related migrations are already separated;
- module depends on `product`, without creating a cycle on umbrella `rustok-commerce`;
- backend admin read service already returns inventory-owned DTO for product/variant/price/translations read-side and reads available quantity from `inventory_items`/`inventory_levels`, if stock-level state already exists;
- admin read/write transport is now native-only through dedicated server functions: set-quantity/adjust-quantity/reserve-quantity/release-reservation endpoints and availability-check facade moved to inventory-owned native facade; public-channel availability/projection helpers also belong to `rustok-inventory`, and further non-admin/channel-aware parity is handled separately from the admin UI scope;
- `rustok-inventory/admin` already publishes inventory-owned admin route for stock visibility,
  low-stock triage and variant-level health inspection;
- current dedicated inventory admin mutations/validation (`set_variant_quantity`, `adjust_variant_quantity`, `reserve_variant_quantity`, `release_reservation_quantity` and `check_variant_availability`) go through inventory-owned native server functions without GraphQL fallback and are connected to UI targeted stock/availability operations; new admin operations should be added only through the module-owned facade;
- dedicated native/server-function read transport is connected to backend `AdminInventoryReadService`; GraphQL transitional compatibility fallback removed from the inventory admin package.

## Stages

### 1. Contract stability

- [x] lock inventory boundary as a separate module;
- [x] keep product dependency without cycle on umbrella;
- [x] move inventory admin UI to module-owned package `rustok-inventory/admin`;
- [x] maintain sync between inventory runtime contract, admin UI, commerce orchestration
  and module metadata through local docs + registry evidence.

### 2. Inventory transport split

- [x] add backend inventory-owned admin read service/read DTO for product/variant/price/translations read-side;
- [x] add inventory-owned core/read facade and explicit Leptos adapter for admin UI, isolating the current commerce GraphQL access in a transitional adapter and locking this with a boundary test;
- [x] connect dedicated inventory read transport/native `#[server]` path to backend `AdminInventoryReadService`;
- [x] move dedicated inventory admin read/write transport from umbrella `rustok-commerce` (admin read path native-only; current admin write/validation surface: native set-quantity/adjust-quantity/reserve-quantity/release-reservation endpoints plus check-availability);
- [x] connect initial inventory admin UI targeted stock operations to inventory-owned set/adjust/reserve/release quantity mutations and check-availability validation;
- [x] move current inventory admin UI stock operations to inventory-owned native/transport mutations (set/adjust/reserve/release/check-availability; new operations should be added only through module-owned `transport/` facade);
- [x] cover current admin transport parity and stock mutation semantics with targeted tests (facade/boundary checks, write-result serde snapshots and service-level negative reserve/availability request and reservation release error semantics tests added for typed set/adjust/reserve/release/check-availability endpoints; product list/detail serde snapshots, source-level backend DTO/native mapper parity and removed-GraphQL-adapter boundary check lock the current read-model shape and absence of GraphQL fallback).

### 3. Availability hardening

- [x] read reservation-aware available quantity from inventory levels in admin read-side, leaving legacy variant quantity only as compatibility fallback;
- [ ] evolve stock locations, reservations and availability semantics as module-owned contract;
- [ ] cover channel-aware availability edge-cases with targeted tests through integration
  with umbrella;
- [ ] keep read/write paths compatible with checkout and catalog visibility flows.

### 4. Operability

- [x] document backend admin read-side service simultaneously with changing runtime surface;
- [x] document new inventory guarantees simultaneously with changing runtime surface;
- [x] keep local docs and `README.md` synchronized;
- [x] update umbrella commerce docs when availability semantics change.

## Verification

- `cargo xtask module validate inventory`
- `cargo xtask module test inventory`
- targeted tests for stock mutations, inventory transport and checkout-facing invariants

## Update rules

1. When changing inventory runtime contract, update this file first.
2. When changing public/runtime surface, synchronize `README.md`, `admin/README.md`
   and `docs/README.md`.
3. When changing module metadata, synchronize `rustok-module.toml`.
4. When changing inventory/checkout/channel-aware orchestration, update umbrella docs.


## Quality backlog

- [x] Update test coverage for key module scenarios (targeted inventory admin boundary verifier fixtures, public-channel projection regression, typed write-result serde/semantics tests).
- [x] Verify completeness and currency of `README.md` and local docs for the current native admin/read/write + public-channel projection state.
- [x] Lock/update verification gates for current module state (`node scripts/verify/verify-inventory-admin-boundary.mjs`, `./scripts/verify/verify-all.sh inventory-admin-boundary`, `node scripts/verify/verify-inventory-admin-boundary.test.mjs`).
