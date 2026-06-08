# План реализации `rustok-customer`

Статус: customer boundary выделен; модуль остаётся owner-ом storefront customer
profile, admin UI ownership уже вынесен в `rustok-customer/admin`, а storefront
transport и checkout orchestration остаются у umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: ffa_admin_native_adapter_split
- Last checkpoint: Admin customer core теперь владеет submit-command validation/preparation (`CustomerAdminDraftInput`, `CustomerAdminSubmitCommand`, `CustomerAdminSubmitCommandError`, `build_customer_admin_submit_command`), а Leptos adapter только мапит core error policy в локализованные сообщения и вызывает transport facade; native Leptos server functions остаются в `admin/src/transport/native_server_adapter.rs`.
- Next step: Продолжать выносить detail/list row view-model policy в `core` без изменения admin CRUD behavior; при появлении второго transport path добавить explicit adapter рядом с `native_server_adapter.rs`.
- Open blockers: None.
- Hand-off notes for next agent: После каждого инкремента обновлять этот блок.
- Last updated at (UTC): 2026-06-08T01:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `not_started`
- Structural shape: `core_transport_ui`
- Evidence:
  - модуль ведётся в ускоренном FFA migration track; FBA остаётся `not_started` до закрытия FFA phase-gate как часть ecommerce family;
  - любые изменения UI/transport boundary должны фиксироваться с parity/boundary evidence в этом же инкременте;
  - admin FFA slice добавил framework-agnostic `admin/src/core.rs` list request policy и submit-command validation/preparation, module-owned `admin/src/transport/mod.rs` facade, native-only `admin/src/transport/native_server_adapter.rs` с `#[server]` endpoints и явный Leptos render adapter `admin/src/ui/leptos.rs`; legacy `admin/src/api.rs` удалён, `admin/src/lib.rs` теперь только wires modules и re-export `CustomerAdmin`, а Leptos adapter больше не вызывает raw `api::*` напрямую для covered CRUD flows.
- Last verified at (UTC): 2026-06-08T01:00:00Z
- Owner: `rustok-customer` module team

## Область работ

- удерживать `rustok-customer` как отдельный customer domain module;
- синхронизировать customer contract, optional user/profile bridge и local docs;
- не смешивать customer profile с platform/admin user domain.

## Текущее состояние

- `customers` и `CustomerService` уже выделены в отдельный модуль;
- optional linkage на `user_id` и bridge к `profiles` уже существуют как integration contract;
- `rustok-customer` уже публикует собственный module-owned admin UI package `rustok-customer/admin` с `admin/src/core.rs` defaults для request и submit-command policy, `admin/src/transport/mod.rs` facade поверх `admin/src/transport/native_server_adapter.rs` native Leptos server functions для list/detail/create/update customer records и явным `admin/src/ui/leptos.rs` render adapter;
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
