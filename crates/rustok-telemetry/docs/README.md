# Документация `rustok-telemetry`

`rustok-telemetry` — foundation-модуль наблюдаемости платформы. Он держит
shared telemetry primitives и wiring contracts, которые должны использоваться
модулями и host-слоем единообразно.

## Назначение

- публиковать канонический telemetry/observability foundation contract;
- держать shared telemetry helpers и wiring expectations вне `apps/server`;
- снижать дрейф метрик, traces и logging conventions между модулями.

## Зона ответственности

- shared telemetry primitives и instrumentation helpers;
- базовые observability contracts для metrics, tracing и related runtime wiring;
- foundation surface для consumer modules и host integrations;
- отсутствие domain-owned metrics semantics и transport/business logic.

## Интеграция

- используется `apps/server` и runtime-модулями как shared observability dependency;
- module-specific metrics остаются внутри owning modules, но строятся поверх общих foundation contracts;
- любые изменения shared telemetry wiring должны синхронизироваться с host docs и verification docs;
- `rustok-telemetry` не должен поглощать domain-specific observability runbooks.

## Проверка

- `cargo xtask module validate telemetry`
- `cargo xtask module test telemetry`
- targeted tests для telemetry helpers, wiring contracts и compatibility expectations

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Observability quickstart](../../../docs/guides/observability-quickstart.md)
