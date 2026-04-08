# Документация `rustok-outbox`

`rustok-outbox` — core-модуль transactional event persistence и relay
infrastructure для event runtime платформы.

## Назначение

- публиковать канонический runtime entry type `OutboxModule`;
- держать write-side outbox contract и relay semantics вне host-слоя;
- давать платформе единый transactional publishing contract для событий.

## Зона ответственности

- `TransactionalEventBus` и atomic publish-with-transaction semantics;
- persistence в `sys_events` через transactional transport;
- relay, retry и DLQ semantics для event runtime;
- module-owned Leptos admin package `rustok-outbox-admin`.

## Интеграция

- используется `apps/server` для migrations, runtime relay bootstrap и event transport wiring;
- зависит от `rustok-core` для module contracts и event transport abstractions;
- может форвардить доставку в downstream transports вроде `rustok-iggy`, не владея provider-specific delivery semantics;
- остаётся `Core` module независимо от того, что часть bootstrap wiring живёт в host runtime.

## Проверка

- `cargo xtask module validate outbox`
- `cargo xtask module test outbox`
- targeted event-runtime tests для transactional publish, relay и backlog semantics

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)
