# Flex — Custom Fields System

> Документация модуля: [`crates/flex/docs/README.md`](../../crates/flex/docs/README.md)
> Implementation plan: [`crates/flex/docs/implementation-plan.md`](../../crates/flex/docs/implementation-plan.md)
> Архитектурный обзор: [`docs/architecture/flex.md`](../architecture/flex.md)

Flex — модуль-библиотека для runtime-определяемых кастомных полей.
Типы и валидаторы живут в `rustok-core/src/field_schema.rs`.
Attached mode (Phases 0–4) реализован. Standalone mode (Phase 5) — не начат.
Server migration guide (Phase 4.5): [`apps/server/docs/flex-phase45-migration-guide.md`](../../apps/server/docs/flex-phase45-migration-guide.md).
