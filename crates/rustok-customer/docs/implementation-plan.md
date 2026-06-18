# План реализации `rustok-customer`

Статус: customer boundary выделен; модуль остаётся owner-ом storefront customer
profile, admin UI ownership уже вынесен в `rustok-customer/admin`, а storefront
transport и checkout orchestration остаются у umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: fba_customer_read_projection_provider_slice
- Last checkpoint: Customer получил первый FBA provider slice: `CustomerReadPort` закрепляет нейтральный read/list projection boundary поверх owner `CustomerService`; `rustok-module.toml`, `contracts/customer-fba-registry.json` и static evidence packet публикуют customer read-projection provider metadata для commerce/order consumers без переноса customer rules в commerce transport.
- Next step: Закрыть runtime contract tests/fallback smoke для `CustomerReadPort`, затем готовить повышение выше `in_progress`; не запускать долгую компиляцию, использовать fast verifier `npm run verify:ecommerce:fba`.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок и держать central readiness board синхронизированным.
- Last updated at (UTC): 2026-06-18T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - `src/ports.rs` экспортирует `CustomerReadPort` и DTO для customer read/list projection операций; machine-readable registry и verifier проверяют совпадение port trait operations с FBA metadata;
  - метаданные FBA-provider открыты для `customer read projection` через `crates/rustok-customer/contracts/customer-fba-registry.json`; статус остаётся `in_progress` до contract tests/remote transport evidence;
  - static evidence packet `crates/rustok-customer/contracts/evidence/customer-contract-test-static-matrix.json` is locked by `npm run verify:ecommerce:fba` (registry + evidence gates); это закрывает metadata/evidence anti-drift для будущих contract tests, но не повышает статус без runtime evidence;
  - любые изменения UI/transport boundary должны фиксироваться с parity/boundary evidence в этом же инкременте;
  - admin FFA slice добавил framework-agnostic `admin/src/core.rs` list request policy, submit-command validation/preparation, submit/transport error message mapping, form snapshot mapping, shell/list/detail header view-models, field placeholder DTOs, detail section/profile-empty copy, timestamp/user/locale/visibility display labels, list/detail row view-model policy, active row CSS policy, page-level list/detail empty/error/loading states, refresh/open action-state policy and editor action-state policy; `admin/src/transport/mod.rs` remains the module-owned facade over native-only `admin/src/transport/native_server_adapter.rs` `#[server]` endpoints; explicit Leptos render adapter `admin/src/ui/leptos.rs` consumes core view-models/snapshots/states and no longer owns covered shell/list/detail header copy, list/detail fallback strings, timestamp/profile display labels, submit/transport error copy/formatting, form placeholders, detail section/profile-empty copy, refresh/open disabled policy, active-row class decisions or editor mode/disabled policy; legacy `admin/src/api.rs` удалён, `admin/src/lib.rs` только wires modules и re-export `CustomerAdmin`.
- Last verified at (UTC): 2026-06-18T00:00:00Z
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
- [ ] удерживать ownership guard и tenant isolation покрытыми targeted tests;
- [ ] не допускать размывания customer semantics в auth/user domain.

### 3. Operability

- [x] документировать новые customer guarantees одновременно с изменением runtime surface;
- [x] удерживать local docs и `README.md` синхронизированными;
- [ ] добавлять richer diagnostics только при реальном operational pressure.

## Проверка

- `cargo xtask module validate customer`
- `cargo xtask module test customer`
- targeted tests для customer CRUD/lookup, ownership guard и optional profile bridge

## Правила обновления

1. При изменении customer runtime contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении integration с `auth`/`profiles` обновлять связанные module docs.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
