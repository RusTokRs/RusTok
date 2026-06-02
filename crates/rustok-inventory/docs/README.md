# Документация `rustok-inventory`

`rustok-inventory` — дефолтный inventory-подмодуль семейства `ecommerce`.

## Назначение

- inventory service logic;
- stock-related migrations;
- `InventoryModule` и `InventoryService`;
- module-owned admin UI пакет `rustok-inventory/admin` для inventory visibility,
  low-stock triage и variant-level stock inspection.

## Зона ответственности

- runtime dependency: `product`;
- модуль владеет inventory/stock boundary и операторской read-side UI-поверхностью
  для остатков;
- admin read-side теперь проходит через inventory-owned core/facade в `admin/src/core.rs`,
  `admin/src/api.rs` и `admin/src/transport.rs`; текущий доступ к commerce GraphQL изолирован в
  transitional adapter-е до появления dedicated inventory transport;
- GraphQL и REST write transport пока остаются в фасаде `rustok-commerce`, а dedicated
  inventory write transport ещё не вынесен в отдельный module-owned surface;
- общие DTO, entities и error surface приходят из `rustok-commerce-foundation`.

## Интеграция

- модуль входит в ecommerce family и должен сохранять собственную storage/runtime-границу
  без возврата ответственности в umbrella `rustok-commerce`;
- inventory-owned admin UX и read facade публикуются через `rustok-inventory/admin`;
  underlying commerce GraphQL adapter считается transitional implementation detail;
- изменения cross-module контракта нужно синхронизировать с `rustok-commerce`
  и соседними split-модулями.

## Проверка

- `cargo xtask module validate inventory`
- `cargo xtask module test inventory`
- targeted commerce tests для inventory-домена при изменении runtime wiring

## Связанные документы

- [README crate](../README.md)
- [README admin package](../admin/README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
