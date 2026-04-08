# Документация `rustok-iggy-connector`

`rustok-iggy-connector` — connection abstraction layer для Iggy transport
стека. Он владеет embedded/remote mode switching, connection lifecycle и
low-level message I/O, не забирая у `rustok-iggy` transport-level semantics.

## Назначение

- публиковать канонический connector contract для Iggy-based transport stack;
- держать embedded/remote mode switching и lifecycle management в отдельном crate;
- давать `rustok-iggy` и другим возможным consumers единый low-level connector surface.

## Зона ответственности

- `IggyConnector`, `RemoteConnector`, `EmbeddedConnector`;
- `ConnectorConfig`, `PublishRequest`, `MessageSubscriber`, `ConnectorError`;
- connection lifecycle, mode abstraction и low-level publish/subscribe contracts;
- optional Iggy SDK integration через feature flag;
- отсутствие ownership над transport-level serialization, DLQ, replay и topology policy.

## Интеграция

- используется `rustok-iggy` как low-level connection layer;
- должен оставаться отдельным connector crate без transport/business semantics;
- любые изменения connector contracts должны синхронизироваться с `rustok-iggy` docs и runtime expectations;
- simulation mode без feature flag должен оставаться явно задокументированной compatibility surface.

## Проверка

- targeted compile/tests для connector configuration, mode switching, request building и error handling;
- integration tests нужны при изменении реального SDK/lifecycle path;
- structural verification для boundary между connector и transport crate.

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Документация `rustok-iggy`](../../rustok-iggy/docs/README.md)
