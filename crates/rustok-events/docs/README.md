# Документация `rustok-events`

`rustok-events` — канонический shared import surface для event contracts
платформы. Он владеет `DomainEvent`, `EventEnvelope`, schema metadata и
validation rules, а `rustok-core` сохраняет только compatibility re-export path.

## Назначение

- публиковать единый event-contract layer для платформы;
- удерживать schema metadata, envelope shape и validation rules внутри отдельного модуля;
- оторвать потребителей событий от прямой зависимости на `rustok-core::events`.

## Зона ответственности

- `DomainEvent`, `EventEnvelope`, `EventSchema`, `FieldSchema` и schema registry;
- validation rules и versioning policy для event payloads;
- compatibility aliases и non-breaking migration path для consumers;
- contract tests и release-gate expectations для event-schema changes;
- отсутствие transport-specific event delivery logic.

## Интеграция

- `rustok-core::events` остаётся compatibility adapter поверх канонического surface из `rustok-events`;
- доменные модули, outbox/runtime crates и test utilities должны импортировать event contracts напрямую из `rustok-events`;
- изменения event contracts должны быть синхронизированы с outbox, replay, DLQ и reindex guidance;
- breaking payload changes требуют version bump и explicit dual-read/migration plan.

## Проверка

- `cargo xtask module validate events`
- `cargo xtask module test events`
- targeted tests для schema coverage, validation, versioning и envelope JSON roundtrip

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Platform documentation map](../../../docs/index.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
