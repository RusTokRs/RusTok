# План реализации `rustok-telemetry`

Статус: telemetry foundation crate уже есть, но локальная документация и
контракт границы нужно удерживать так же жёстко, как у остальных shared modules.

## Область работ

- удерживать `rustok-telemetry` как shared observability foundation layer;
- синхронизировать telemetry helpers, wiring expectations и local docs;
- не допускать втягивания domain-specific observability logic в foundation crate.

## Текущее состояние

- crate уже является общей зависимостью для observability-related wiring;
- shared telemetry helpers уже составляют часть platform baseline;
- host и модульные integrations должны опираться на единый foundation contract;
- local docs и root `README.md` должны оставаться частью module-standard path.

## Этапы

### 1. Contract stability

- [x] закрепить `rustok-telemetry` как shared observability foundation;
- [x] удерживать shared helpers отдельно от domain-specific metrics semantics;
- [ ] удерживать sync между public surface, host wiring и module metadata.

### 2. Boundary hardening

- [ ] продолжать выносить общие telemetry helpers из host-specific layers, если они реально shared;
- [ ] не тянуть сюда module-owned metrics/runbook semantics;
- [ ] покрывать новые foundation contracts targeted tests и compatibility checks.

### 3. Operability

- [ ] документировать изменения observability foundation одновременно с изменением runtime surface;
- [ ] удерживать local docs и `README.md` синхронизированными;
- [ ] обновлять host/verification docs, если меняются shared wiring expectations.

## Проверка

- `cargo xtask module validate telemetry`
- `cargo xtask module test telemetry`
- targeted tests для telemetry helpers, metrics/tracing wiring и compatibility contracts

## Правила обновления

1. При изменении telemetry foundation contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении shared observability wiring обновлять связанные host и verification docs.
