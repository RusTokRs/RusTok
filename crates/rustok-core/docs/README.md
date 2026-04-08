# Документация `rustok-core`

`rustok-core` — базовый foundation crate платформы. Он задаёт shared typed
contracts, ошибки, security primitives, content helpers и прочие инварианты, на
которые опираются остальные модули RusToK.

## Назначение

- публиковать канонический shared foundation contract для платформенных и доменных модулей;
- держать typed primitives и базовые invariants вне host- и domain-specific кода;
- снижать дублирование cross-module contracts без превращения `rustok-core` в runtime bucket для любой логики.

## Зона ответственности

- typed primitives и shared value objects;
- базовые error/validation helpers и security contracts;
- content/rich-text вспомогательные контракты, которые используются несколькими модулями;
- compatibility re-exports и shared API surface для foundation layer;
- отсутствие domain-owned runtime orchestration и transport-specific logic.

## Интеграция

- используется практически всеми `rustok-*` crates как foundation dependency;
- `apps/server` и runtime-модули зависят от typed contracts, но не должны тащить обратно shared logic в host слой;
- `rustok-events`, `rustok-rbac`, `rustok-content` и другие foundation/domain crates должны оставаться поверх `rustok-core`, а не наоборот;
- любые новые cross-module primitives должны попадать сюда только если они реально shared и не принадлежат одному bounded context.

## Проверка

- `cargo xtask module validate core`
- `cargo xtask module test core`
- targeted tests для typed primitives, validation helpers, security contracts и compatibility exports

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
