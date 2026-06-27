---
id: doc://docs/architecture/performance-baseline.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Базовая производительность

Этот документ фиксирует repeatable evidence workflow для performance changes в
RusToK.

## Назначение

Перед query rewrite, новым индексом, read-model изменением или partitioning
нужно собрать повторяемый baseline, чтобы сравнивать эффект изменений.

Базовый performance baseline не заменяет оптимизацию, а даёт evidence bundle для
архитектурного решения.

## Что собирать

Минимальный baseline включает:

- top SQL statements из `pg_stat_statements`
- `EXPLAIN` для hot paths
- tenant-scoped snapshot, который можно сравнить во времени

## Где живёт реализация

Текущий task implementation:

- [db_baseline.rs](/C:/проекты/RusTok/apps/server/src/tasks/db_baseline.rs)

Для search hot path дополнительно используется live PostgreSQL gate
`crates/rustok-search/tests/postgres_query_plan.rs`. Он создаёт 100 000
временных tenant-scoped документов, снимает `EXPLAIN (ANALYZE, BUFFERS)` и
проверяет GIN FTS/trigram indexes. Baseline от 2026-06-27: FTS `6.627 ms`,
typo fallback `327.516 ms` на локальном PostgreSQL 16.

## Когда использовать

Этот workflow нужен, если меняется:

- тяжёлый query path
- индексная стратегия
- read-side projection
- caching decision
- storage layout, влияющий на latency

## Рекомендуемая последовательность

1. Прогреть целевой path репрезентативным трафиком.
2. Запустить baseline task для нужного tenant-а.
3. Сохранить JSON artifact для текущей даты.
4. Внести query/index/read-model change.
5. Повторить baseline и сравнить планы и top statements.

## Ограничения

- evidence полезен только если на PostgreSQL включён `pg_stat_statements`
- baseline task сам не принимает архитектурное решение
- read-only evidence workflow не должен менять доменное состояние

## Что не делать

- не оптимизировать query path без baseline, если это затрагивает общий hot path
- не сравнивать несопоставимые tenant snapshots
- не считать read-model rewrite успешным без повторного baseline

## Связанные документы

- [Схема данных платформы](./database.md)
- [Контракт потока доменных событий](./event-flow-contract.md)
- [Обзор архитектуры платформы](./overview.md)
