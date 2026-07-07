# Implementation plan for `rustok-customer`

Status: customer boundary is separated; the module remains owner of storefront customer
profile, admin UI ownership is already moved to `rustok-customer/admin`, and storefront
transport and checkout orchestration remain with umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: customer_docs_and_no_compile_verification_slice
- Last checkpoint: Customer read-port policy cleanup removed redundant direct deadline checks; `CustomerReadPort` now relies on shared `PortCallPolicy::read()` as the single read gate while keeping no-compile FBA evidence and docs promotion blockers unchanged. Native admin customer CRUD server functions now consume `rustok_api::HostRuntimeContext` and no longer depend on the previous runtime crate.
- Next step: When compilation is allowed again, run targeted customer service/port tests for normalized identity guards and read-projection runtime smoke, including verification of `PortCallPolicy::read()` deadline semantics, then decide whether FBA can move above `in_progress`; until then, keep fast no-compile gates (`node scripts/verify/verify-customer-fba-no-compile.mjs`, `node scripts/verify/verify-ecommerce-fba-contract-evidence.mjs`, `node scripts/verify/verify-ecommerce-provider-spi-evidence.mjs`) green without long builds.
- Open blockers: None.
- Hand-off notes for next agent: After each increment, update this block and keep the central readiness board synchronized.
- Last updated at (UTC): 2026-06-23T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- FBA contract version: `customer.read_projection.v1`
- Structural shape: `core_transport_ui`
- Evidence:
  - batch no-compile FBA gate `scripts/verify/verify-commerce-domain-fba-runtime-smoke.mjs` and fixture-regression suite check `crates/rustok-customer/contracts/evidence/customer-runtime-contract-smoke.json`: read policy → owner `CustomerService` invocation → typed error mapping and registry parity for fallback/degraded modes; the existing requirement of compiled runtime execution before `boundary_ready` remains;
  - `src/ports.rs` exports `CustomerReadPort` and DTO for customer read/list projection operations; machine-readable registry and verifier check port trait operations match FBA metadata;
  - FBA-provider metadata is open for `customer read projection` through `crates/rustok-customer/contracts/customer-fba-registry.json`; status remains `in_progress` until contract tests/remote transport evidence;
  - static evidence packet `crates/rustok-customer/contracts/evidence/customer-contract-test-static-matrix.json` is locked by `npm run verify:ecommerce:fba` (registry + evidence gates); source-locked runtime/fallback packet `crates/rustok-customer/contracts/evidence/customer-read-projection-runtime-smoke.json` points to authored no-compile tests in `crates/rustok-customer/tests/customer_service_test.rs` for deadline enforcement, typed port errors and tenant-scoped fallback listing; status is not raised without actual compiled runtime execution;
  - any UI/transport boundary changes must be locked with parity/boundary evidence in the same increment; native admin transport is now source-locked by `scripts/verify/verify-customer-admin-boundary.mjs` to use `HostRuntimeContext` and avoid the previous runtime crate dependency;
  - legacy umbrella facade removed: `rustok-commerce` no longer re-exports `CustomerService` or `services::customer`, and all affected callers import the owner crate directly;
  - admin FFA slice added framework-agnostic `admin/src/core.rs` list request policy, submit-command validation/preparation, submit/transport error message mapping, form snapshot mapping, shell/list/detail header view-models, field placeholder DTOs, detail section/profile-empty copy, timestamp/user/locale/visibility display labels, list/detail row view-model policy, active row CSS policy, page-level list/detail empty/error/loading states, refresh/open action-state policy and editor action-state policy; `admin/src/transport/mod.rs` remains the module-owned facade over native-only `admin/src/transport/native_server_adapter.rs` `#[server]` endpoints; explicit Leptos render adapter `admin/src/ui/leptos.rs` consumes core view-models/snapshots/states and no longer owns covered shell/list/detail header copy, list/detail fallback strings, timestamp/profile display labels, submit/transport error copy/formatting, form placeholders, detail section/profile-empty copy, refresh/open disabled policy, active-row class decisions or editor mode/disabled policy; legacy `admin/src/api.rs` removed, `admin/src/lib.rs` only wires modules and re-exports `CustomerAdmin`.
- Last verified at (UTC): 2026-06-20T00:00:00Z
- Owner: `rustok-customer` module team

## Scope of work

- keep `rustok-customer` as a separate customer domain module;
- synchronize customer contract, optional user/profile bridge and local docs;
- do not mix customer profile with platform/admin user domain.

## Current state

- `customers` and `CustomerService` are already separated into their own module;
- optional linkage to `user_id` and bridge to `profiles` already exist as integration contract;
- `rustok-customer` already publishes its own module-owned admin UI package `rustok-customer/admin` with `admin/src/core.rs` defaults for request, submit-command policy, submit/transport error message mapping, form snapshots, shell/list/detail header view-models, field placeholder DTOs, detail section/profile-empty copy, timestamp/user/locale/visibility display labels, list/detail view-model policy, page-state policy, refresh/open action-state policy and editor action-state policy, `admin/src/transport/mod.rs` facade over Loco-free `admin/src/transport/native_server_adapter.rs` native Leptos server functions for list/detail/create/update customer records and explicit `admin/src/ui/leptos.rs` render adapter;
- transport adapters are still published through `rustok-commerce` facade;
- customer read/write contract does not turn customer into a canonical public profile surface.

## Stages

### 1. Contract stability

- [x] lock separate customer profile boundary;
- [x] keep optional linkage to `user` and `profiles` as integration-only contract;
- [x] maintain sync between customer runtime contract, commerce transport and module metadata.

### 2. Domain expansion

- [ ] expand customer-owned settings/profile flows only inside the module;
- [x] keep ownership guard and tenant isolation covered by targeted tests;
- [x] prevent blurring of customer semantics into auth/user domain (tenant-scoped duplicate `user_id` guard covered by no-compile test).

### 3. Operability

- [x] document new customer guarantees simultaneously with changing runtime surface;
- [x] keep local docs and `README.md` synchronized;
- [ ] add richer diagnostics only when real operational pressure demands it.

## Verification

- `cargo xtask module validate customer`
- `cargo xtask module test customer`
- targeted tests for customer CRUD/lookup, ownership guard and optional profile bridge

## No-compile verification gates

While compilation is prohibited, customer increments are checked by fast source/evidence gates:

- `node scripts/verify/verify-customer-admin-boundary.mjs` - checks the customer admin core/transport/ui split and Loco-free native server-function runtime boundary;

- `node scripts/verify/verify-customer-fba-no-compile.mjs` — checks `CustomerReadPort`, `rustok-module.toml`, `Cargo.toml`, local plan and central readiness board against `customer-fba-registry.json`;
- `node scripts/verify/verify-ecommerce-fba-contract-evidence.mjs` — checks static contract-test matrix against registry contract cases/profiles/assertions;
- `node scripts/verify/verify-ecommerce-provider-spi-evidence.mjs` — maintains provider/evidence surface family-wide without running Rust compilation;
- compiled gates (`cargo xtask module validate customer`, `cargo xtask module test customer`, targeted `cargo test -p rustok-customer ...`) remain mandatory before raising FBA above `in_progress`, but are not run in this iteration due to explicit constraint.

## Update rules

1. When changing customer runtime contract, update this file first.
2. When changing public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata, synchronize `rustok-module.toml`.
4. When changing integration with `auth`/`profiles`, update related module docs.


## Quality backlog

- [x] Update test coverage for key module scenarios: normalized email uniqueness, update duplicate checks, tenant-scoped user linkage and read-projection smoke are source-locked; compiled execution pending by request.
- [x] Verify completeness and currency of `README.md` and local docs.
- [x] Lock/update verification gates for current module state.
