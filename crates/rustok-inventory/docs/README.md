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
- admin UI read-side теперь проходит только через inventory-owned core/facade в `admin/src/core.rs`,
  `admin/src/api.rs`, `admin/src/native.rs`, native `#[server]` functions и explicit Leptos adapter
  `admin/src/ui/leptos.rs`; прежний commerce GraphQL transitional adapter удалён вместе с
  `admin/src/transport.rs`, `leptos-graphql` и token/tenant-slug fallback параметрами;
- dedicated native inventory write/validation endpoints `inventory/variant/set-quantity`,
  `inventory/variant/adjust-quantity`, `inventory/variant/reserve-quantity`,
  `inventory/variant/release-reservation` и `inventory/variant/check-availability` уже вынесены
  в module-owned surface без GraphQL fallback и возвращают typed write/validation results;
  set-quantity трактует requested quantity как целевую available quantity и сохраняет
  существующий reserved stock, а backorder policy `continue` нормализуется case-insensitive
  в service/read-side и commerce checkout/storefront compatibility semantics через exported
  inventory-owned policy helper; remaining write parity ещё добирается из umbrella `rustok-commerce`;
- public-channel inventory visibility/projection helpers (`normalize_public_channel_slug`, metadata allowlist parsing, channel-visible available quantity loaders, `PublicChannelInventoryProjection` и `load_inventory_projection_by_variant_for_public_channel`) принадлежат inventory crate-у и переиспользуются umbrella `rustok-commerce` для storefront/checkout compatibility без дублирования backorder policy branching в commerce DTO adapter-е;
- общие DTO, entities и error surface приходят из `rustok-commerce-foundation`.

## Интеграция

- модуль входит в ecommerce family и должен сохранять собственную storage/runtime-границу
  без возврата ответственности в umbrella `rustok-commerce`;
- inventory-owned backend admin read service экспортируется root crate-ом и является source
  для native server-function read transport;
- inventory-owned admin UX и read facade публикуются через `rustok-inventory/admin`;
  read-side и targeted set/adjust/reserve/release quantity plus check-availability flows идут через native inventory-owned server-function surface без commerce GraphQL fallback;
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
