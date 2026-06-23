# Документация `rustok-customer`

`rustok-customer` — дефолтный storefront-customer подмодуль семейства `ecommerce`.

## Назначение

- схема `customers`;
- `CustomerModule` и `CustomerService`;
- module-owned admin UI пакет `rustok-customer/admin`;
- customer profile boundary, отделённый от platform/admin user;
- optional linkage на `user_id` для сценариев `store/customers/me`;
- optional service-level bridge `customer -> user -> profile`, который может вернуть customer вместе с `ProfileSummary`;
- FBA provider boundary `CustomerReadPort` для read-projection сценариев commerce checkout и order customer snapshots.

## Зона ответственности

- модуль не зависит от `rustok-commerce` umbrella, чтобы не создавать цикл;
- customer profile хранится отдельно от auth/user домена;
- связь с пользователем опциональна, tenant-scoped и не отменяет самостоятельность customer-модели;
- bridge к `profiles` остаётся опциональным read-contract и не превращает customer в канонический public profile;
- admin UI ownership теперь живёт в `rustok-customer/admin`; list defaults вынесены в `admin/src/core.rs`, Leptos-отрисовка вынесена в `admin/src/ui/leptos.rs`, а CRUD-вызовы идут через `admin/src/transport.rs`; storefront GraphQL и REST transport пока остаются в фасаде `rustok-commerce`.

## Интеграция

- модуль входит в ecommerce family и должен сохранять собственную storage/runtime-границу без возврата ответственности в umbrella `rustok-commerce`;
- storefront transport и GraphQL по-прежнему публикуются через `rustok-commerce`, но admin UI-поверхность уже зафиксирована как отдельный module-owned surface в `rustok-customer/admin`;
- изменения cross-module контракта нужно синхронизировать с `rustok-commerce` и соседними split-модулями;
- `CustomerService` нормализует email до проверки уникальности и хранения, поэтому create/update не допускают trimmed-дубликаты внутри tenant; duplicate `user_id` linkage остаётся tenant-scoped и не превращает customer в auth/user domain.
- `CustomerReadPort` использует общий `PortContext`/`PortError`, требует read deadline semantics и мапит invalid tenant / not found в typed port errors; no-compile runtime smoke зафиксирован в `contracts/evidence/customer-read-projection-runtime-smoke.json`, но FBA status остаётся `in_progress` до фактического compiled runtime execution.

## Разделение FFA для admin

Пакет admin теперь использует framework-agnostic defaults `admin/src/core.rs`, фасад `admin/src/transport.rs` поверх native Leptos server functions и явный Leptos-адаптер отрисовки `admin/src/ui/leptos.rs`; корень crate только подключает слои модуля и повторно экспортирует `CustomerAdmin`.

## Проверка

- No-compile source/evidence gates для итераций без сборки:
  - `node scripts/verify/verify-customer-fba-no-compile.mjs`
  - `node scripts/verify/verify-ecommerce-fba-contract-evidence.mjs`
  - `node scripts/verify/verify-ecommerce-provider-spi-evidence.mjs`
- После снятия ограничения на компиляции:
  - `cargo xtask module validate customer`
  - `cargo xtask module test customer`
  - targeted commerce tests для customer-домена при изменении runtime wiring
  - targeted customer port tests для `CustomerReadPort` deadline/error/fallback smoke перед повышением FBA статуса

## Связанные документы

- [README crate](../README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
