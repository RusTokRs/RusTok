# Документация `rustok-inventory`

`rustok-inventory` — дефолтный inventory-подмодуль семейства `ecommerce`.

## Назначение

- inventory service logic;
- stock-related migrations;
- `InventoryModule`, `InventoryService`, backend `AdminInventoryReadService` и native admin stock write endpoints;
- module-owned admin UI пакет `rustok-inventory/admin` для inventory visibility,
  low-stock triage и variant-level stock inspection.

## Зона ответственности

- runtime dependency: `product`;
- модуль владеет inventory/stock boundary и операторской read-side UI-поверхностью
  для остатков;
- backend read-side для админки теперь имеет inventory-owned service/DTO в
  `src/services/admin_read.rs`, который отдаёт tenant-scoped product/variant/price/translations
  model для native server-function read transport;
- admin UI read-side теперь проходит через inventory-owned core/facade в `admin/src/core.rs`,
  `admin/src/api.rs`, `admin/src/native.rs`, native `#[server]` functions и explicit Leptos adapter
  `admin/src/ui/leptos.rs`; текущий доступ к commerce GraphQL изолирован в
  transitional adapter-е только как native-unavailable compatibility fallback до удаления umbrella read dependency;
- dedicated native inventory write endpoints `inventory/variant/set-quantity`,
  `inventory/variant/adjust-quantity` и `inventory/variant/reserve-quantity` уже вынесены в module-owned surface без GraphQL fallback и возвращают typed `InventoryQuantityWriteResult` / `InventoryReservationWriteResult`; set-quantity трактует requested quantity как целевую available quantity и сохраняет существующий reserved stock;
  remaining write parity ещё добирается из umbrella `rustok-commerce`;
- общие DTO, entities и error surface приходят из `rustok-commerce-foundation`.

## Интеграция

- модуль входит в ecommerce family и должен сохранять собственную storage/runtime-границу
  без возврата ответственности в umbrella `rustok-commerce`;
- inventory-owned backend admin read service экспортируется root crate-ом и является source
  для native server-function read transport;
- inventory-owned admin UX и read facade публикуются через `rustok-inventory/admin`;
  underlying commerce GraphQL adapter считается transitional read-only compatibility implementation detail, а native set/adjust/reserve quantity endpoints являются inventory-owned write surface для set-quantity, +/-1 operator и reservation flows;
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
