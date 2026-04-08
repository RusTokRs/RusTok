# Документация `rustok-iggy`

`rustok-iggy` — transport crate для streaming event delivery на базе Iggy. Он
реализует `EventTransport` и держит transport-level abstractions поверх
`rustok-iggy-connector`, не владея самим connection/mode lifecycle.

## Назначение

- публиковать канонический Iggy-based `EventTransport` surface для платформы;
- держать serialization, topology, DLQ, replay и consumer-group abstractions внутри transport crate;
- отделять transport behavior от connector-level connection management.

## Зона ответственности

- `IggyTransport` и transport-facing configuration;
- JSON/Postcard serialization;
- topology management, consumer groups, DLQ, replay и health abstractions;
- observability hooks для transport layer;
- отсутствие ownership над embedded/remote connection lifecycle.

## Интеграция

- зависит от `rustok-iggy-connector` для embedded/remote mode abstraction и low-level message I/O;
- реализует `EventTransport` для platform event system;
- должен оставаться transport crate, а не connector/runtime configuration bucket;
- любые изменения transport contracts должны синхронизироваться с outbox/event docs и connector docs.

## Проверка

- targeted compile/tests для transport configuration, serialization, topology и replay/DLQ abstractions;
- integration tests нужны при изменении реального Iggy SDK path;
- structural verification для local docs и connector/transport boundary.

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Документация `rustok-iggy-connector`](../../rustok-iggy-connector/docs/README.md)
