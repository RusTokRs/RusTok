# План реализации `rustok-customer`

Статус: customer boundary выделен; модуль остаётся owner-ом storefront customer
profile, admin UI ownership уже вынесен в `rustok-customer/admin`, а storefront
transport и checkout orchestration остаются у umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: customer_docs_and_no_compile_verification_slice
- Last checkpoint: Customer read-port policy cleanup removed redundant direct deadline checks; `CustomerReadPort` now relies on shared `PortCallPolicy::read()` as the single read gate while keeping no-compile FBA evidence and docs promotion blockers unchanged.
- Next step: Когда компиляции снова разрешены, выполнить targeted customer service/port tests for normalized identity guards and read-projection runtime smoke, включая проверку `PortCallPolicy::read()` deadline semantics, then decide whether FBA can move above `in_progress`; до этого держать fast no-compile gates (`node scripts/verify/verify-customer-fba-no-compile.mjs`, `node scripts/verify/verify-ecommerce-fba-contract-evidence.mjs`, `node scripts/verify/verify-ecommerce-provider-spi-evidence.mjs`) зелёными без долгих сборок.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок и держать central readiness board синхронизированным.
- Last updated at (UTC): 2026-06-23T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Версия FBA-контракта: `customer.read_projection.v1`
- Structural shape: `core_transport_ui`
- Evidence:
  - пакетный no-compile FBA gate `scripts/verify/verify-commerce-domain-fba-runtime-smoke.mjs` и fixture-regression suite проверяют `crates/rustok-customer/contracts/evidence/customer-runtime-contract-smoke.json`: read policy → owner `CustomerService` invocation → typed error mapping и registry parity для fallback/degraded modes; существующее требование compiled runtime execution перед `boundary_ready` сохраняется;
  - `src/ports.rs` экспортирует `CustomerReadPort` и DTO для customer read/list projection операций; machine-readable registry и verifier проверяют совпадение port trait operations с FBA metadata;
  - метаданные FBA-provider открыты для `customer read projection` через `crates/rustok-customer/contracts/customer-fba-registry.json`; статус остаётся `in_progress` до contract tests/remote transport evidence;
  - static evidence packet `crates/rustok-customer/contracts/evidence/customer-contract-test-static-matrix.json` is locked by `npm run verify:ecommerce:fba` (registry + evidence gates); source-locked runtime/fallback packet `crates/rustok-customer/contracts/evidence/customer-read-projection-runtime-smoke.json` points to authored no-compile tests in `crates/rustok-customer/tests/customer_service_test.rs` for deadline enforcement, typed port errors and tenant-scoped fallback listing; статус не повышается без фактического compiled runtime execution;
  - любые изменения UI/transport boundary должны фиксироваться с parity/boundary evidence в этом же инкременте;
  - legacy umbrella facade удалён: `rustok-commerce` больше не реэкспортирует `CustomerService` или `services::customer`, а все затронутые callers импортируют owner crate напрямую;
  - admin FFA slice добавил framework-agnostic `admin/src/core.rs` list request policy, submit-command validation/preparation, submit/transport error message mapping, form snapshot mapping, shell/list/detail header view-models, field placeholder DTOs, detail section/profile-empty copy, timestamp/user/locale/visibility display labels, list/detail row view-model policy, active row CSS policy, page-level list/detail empty/error/loading states, refresh/open action-state policy and editor action-state policy; `admin/src/transport/mod.rs` remains the module-owned facade over native-only `admin/src/transport/native_server_adapter.rs` `#[server]` endpoints; explicit Leptos render adapter `admin/src/ui/leptos.rs` consumes core view-models/snapshots/states and no longer owns covered shell/list/detail header copy, list/detail fallback strings, timestamp/profile display labels, submit/transport error copy/formatting, form placeholders, detail section/profile-empty copy, refresh/open disabled policy, active-row class decisions or editor mode/disabled policy; legacy `admin/src/api.rs` удалён, `admin/src/lib.rs` только wires modules и re-export `CustomerAdmin`.
- Last verified at (UTC): 2026-06-20T00:00:00Z
- Owner: `rustok-customer` module team

## Область работ

- удерживать `rustok-customer` как отдельный customer domain module;
- синхронизировать customer contract, optional user/profile bridge и local docs;
- не смешивать customer profile с platform/admin user domain.

## Текущее состояние

- `customers` и `CustomerService` уже выделены в отдельный модуль;
- optional linkage на `user_id` и bridge к `profiles` уже существуют как integration contract;
- `rustok-customer` уже публикует собственный module-owned admin UI package `rustok-customer/admin` с `admin/src/core.rs` defaults для request, submit-command policy, submit/transport error message mapping, form snapshots, shell/list/detail header view-models, field placeholder DTOs, detail section/profile-empty copy, timestamp/user/locale/visibility display labels, list/detail view-model policy, page-state policy, refresh/open action-state policy и editor action-state policy, `admin/src/transport/mod.rs` facade поверх `admin/src/transport/native_server_adapter.rs` native Leptos server functions для list/detail/create/update customer records и явным `admin/src/ui/leptos.rs` render adapter;
- transport adapters по-прежнему публикуются фасадом `rustok-commerce`;
- customer read/write contract не превращает customer в canonical public profile surface.

## Этапы

### 1. Contract stability

- [x] зафиксировать отдельный customer profile boundary;
- [x] удерживать optional linkage к `user` и `profiles` как integration-only contract;
- [x] удерживать sync между customer runtime contract, commerce transport и module metadata.

### 2. Domain expansion

- [ ] расширять customer-owned settings/profile flows только внутри модуля;
- [x] удерживать ownership guard и tenant isolation покрытыми targeted tests;
- [x] не допускать размывания customer semantics в auth/user domain (tenant-scoped duplicate `user_id` guard covered by no-compile test).

### 3. Operability

- [x] документировать новые customer guarantees одновременно с изменением runtime surface;
- [x] удерживать local docs и `README.md` синхронизированными;
- [ ] добавлять richer diagnostics только при реальном operational pressure.

## Проверка

- `cargo xtask module validate customer`
- `cargo xtask module test customer`
- targeted tests для customer CRUD/lookup, ownership guard и optional profile bridge

## No-compile verification gates

Пока компиляции запрещены, customer-инкременты проверяются быстрыми source/evidence gates:

- `node scripts/verify/verify-customer-fba-no-compile.mjs` — сверяет `CustomerReadPort`, `rustok-module.toml`, `Cargo.toml`, local plan и central readiness board с `customer-fba-registry.json`;
- `node scripts/verify/verify-ecommerce-fba-contract-evidence.mjs` — сверяет static contract-test matrix с registry contract cases/profiles/assertions;
- `node scripts/verify/verify-ecommerce-provider-spi-evidence.mjs` — удерживает provider/evidence surface family-wide без запуска Rust-компиляции;
- compiled gates (`cargo xtask module validate customer`, `cargo xtask module test customer`, targeted `cargo test -p rustok-customer ...`) остаются обязательными перед повышением FBA выше `in_progress`, но не запускаются в этой итерации по явному ограничению.

## Правила обновления

1. При изменении customer runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении integration с `auth`/`profiles` обновлять связанные module docs.


## Quality backlog

- [x] Актуализировать покрытие тестами по ключевым сценариям модуля: normalized email uniqueness, update duplicate checks, tenant-scoped user linkage and read-projection smoke are source-locked; compiled execution pending by request.
- [x] Проверить полноту и актуальность `README.md` и локальных docs.
- [x] Зафиксировать/обновить verification gates для текущего состояния модуля.
