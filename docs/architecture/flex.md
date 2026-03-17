# Flex — Architecture

> Документация модуля: [`docs/modules/flex.md`](../modules/flex.md)
> Implementation plan: [`crates/flex/docs/implementation-plan.md`](../../crates/flex/docs/implementation-plan.md)

---

## Философия

**Flex — это катана, а не склад мечей.**

Flex существует рядом со стандартными модулями, а не вместо них. Это «запасной выход» для edge-cases там, где стандартные модули (content, commerce, blog) недостаточны, но создавать отдельный доменный модуль нецелесообразно.

Flex — модуль-библиотека: как `serde` — предоставляет типы и трейт, а реализация у каждого потребителя своя.

## Два режима

**Attached mode** — кастомные поля к существующим сущностям через JSONB `metadata`. Реализован (Phases 0–4). Библиотека типов живёт в `rustok-core/src/field_schema.rs`.

**Standalone mode** — произвольные схемы и записи (`flex_schemas`, `flex_entries`). Не реализован (Phase 5).

## Архитектурные законы

| # | Правило |
|---|---------|
| 1 | Standard modules NEVER depend on Flex |
| 2 | Flex зависит только от `rustok-core` |
| 3 | Removal-safe: отключение Flex не ломает платформу |
| 4 | Данные остаются в модуле-потребителе |
| 5 | Schema-first: валидация при каждой записи |
| 6 | Tenant isolation: определения per-tenant |
| 7 | No Flex in critical domains (orders, payments, inventory) |
