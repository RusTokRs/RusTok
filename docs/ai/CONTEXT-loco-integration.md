# AI Context: Loco Integration

> Контекст для сессии по интеграции Loco с модулями RusToK.

## Читай в этом порядке

1. `apps/server/docs/LOCO_FEATURE_SUPPORT.md` — матрица: что используем из Loco, что намеренно нет
2. `apps/server/docs/loco-core-integration-plan.md` — фазовый план (Accepted)
3. `apps/server/docs/loco-integration-review.md` — ревью плана, gaps, открытые вопросы
4. `docs/modules/registry.md` — реестр всех модулей и crate-ов

## Ключевые точки входа

- `apps/server/src/app.rs` — Loco Hooks (boot, routes, etc.)
- `apps/server/src/common/settings.rs` — `RustokSettings`
- `apps/server/config/development.yaml` — конфигурация

## Обрати внимание

- **НЕ трогай** event pipeline (outbox, не Loco queue) — архитектурное решение
- **НЕ трогай** cache (наш мощнее Loco Cache) — намеренно
- **Mailer и Storage** — серверная инфраструктура, НЕ отдельные модули (ADR: `DECISIONS/2026-03-11-loco-mailer-storage-as-server-infra.md`)
- `graphql/schema.rs` содержит hard-coded imports доменных модулей — это известная проблема, Phase 4
- Зависимости фаз: Phase 0 (i18n) → {1, 1.5} → {2, 3, 4} параллельно → 5 → 6
