# AI Context: Flex Development

> Контекст для сессии по разработке Flex.

## Читай в этом порядке

1. `docs/modules/flex.md` — спецификация (v2), там всё: типы, валидация, guardrails, API
2. `docs/architecture/flex.md` — план реализации (v4), фазы, паттерн интеграции модуля

## Текущий статус

**Кода нет.** Всё нужно писать с нуля, начиная с Phase 0.

Metadata columns уже есть в: users, products, nodes, tenants. Нет в: orders (нужна миграция).

## Куда писать

- `crates/rustok-core/src/field_schema.rs` — core types, traits, validation
- Экспорт добавить в `crates/rustok-core/src/lib.rs`
- Events добавить в `crates/rustok-events/src/types.rs`

## Обрати внимание

- Flex — **часть core**, НЕ отдельный модуль. Нет своих таблиц/данных/состояния
- Каждый модуль создаёт свою таблицу `{entity}_field_definitions` через migration helper
- Orders/payments **НЕ используют Flex** — только нормализованные поля
- Removal-safe: удали `field_schema.rs` → платформа работает
- Паттерн локализации: смотри `crates/rustok-core/src/i18n.rs`
- Пример entity с metadata: `crates/rustok-commerce/src/entities/product.rs`
