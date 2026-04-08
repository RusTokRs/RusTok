# План реализации `rustok-customer`

Статус: customer boundary выделен; модуль остаётся owner-ом storefront customer
profile, admin UI ownership уже вынесен в `rustok-customer/admin`, а storefront
transport и checkout orchestration остаются у umbrella `rustok-commerce`.

## Область работ

- удерживать `rustok-customer` как отдельный customer domain module;
- синхронизировать customer contract, optional user/profile bridge и local docs;
- не смешивать customer profile с platform/admin user domain.

## Текущее состояние

- `customers` и `CustomerService` уже выделены в отдельный модуль;
- optional linkage на `user_id` и bridge к `profiles` уже существуют как integration contract;
- `rustok-customer` уже публикует собственный module-owned admin UI package `rustok-customer/admin` с native Leptos server functions для list/detail/create/update customer records;
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
