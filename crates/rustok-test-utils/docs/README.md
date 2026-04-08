# Документация `rustok-test-utils`

`rustok-test-utils` — shared support crate для тестовой инфраструктуры RusToK.
Он держит reusable fixtures, mocks и helpers, которые должны сокращать
локальное дублирование в unit/integration/contract tests.

## Назначение

- публиковать канонический shared testing helper surface;
- стандартизировать test setup patterns для платформенных и модульных тестов;
- снижать количество ad-hoc fixtures и локальных mock implementations в workspace.

## Зона ответственности

- database setup helpers;
- mock event bus/transport utilities;
- fixtures/builders для common domain entities;
- helper functions и test context shortcuts;
- отсутствие production runtime logic и domain-owned behavior.

## Интеграция

- используется как `dev-dependencies` в crates и app test targets;
- опирается на `rustok-core`/`rustok-events` contracts для test doubles и fixtures;
- testing guide и module-level verification docs должны оставаться синхронизированными с этим crate;
- расширение helpers должно идти через reusable patterns, а не через случайные одноразовые fixtures.

## Проверка

- structural verification для local docs и public test-utils surface;
- targeted self-tests нужны при изменении fixtures, mocks и helper contracts;
- consumer-module docs обновляются при изменении рекомендованных testing patterns.

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Platform documentation map](../../../docs/index.md)
- [Testing guide](../../../docs/guides/testing.md)
