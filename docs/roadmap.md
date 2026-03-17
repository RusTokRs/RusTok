# Roadmap

Это сводная страница дорожной карты платформы RusToK.

## Ближайшие задачи

### 1. Привести документацию в соответствие с новыми решениями (2026-03-17)

Принятые решения затронули несколько фундаментальных вещей, которые отражены
во многих документах. Нужен audit и обновление:

- **Структура модульного крейта** — UI (Leptos admin + storefront) живёт внутри
  `crates/rustok-<module>/src/`, не в отдельных крейтах `crates/leptos-<module>-admin/`
- **Режимы деплоя** — monolith / headless / любая комбинация через Cargo features,
  не фиксированный список сценариев
- **Next.js стратегия** — "batteries included", весь UI в `apps/next-*/src/features/`,
  без отдельных npm-пакетов в `crates/`

Затронутые документы (неполный список):
- `docs/modules/UI_PACKAGES_INDEX.md`
- `docs/modules/UI_PACKAGES_QUICKSTART.md`
- `docs/modules/module-system-plan.md`
- `docs/architecture/modules.md`
- `docs/modules/overview.md`
- `apps/next-admin/docs/implementation-plan.md`
- `apps/next-frontend/docs/implementation-plan.md`
- `crates/rustok-blog/docs/implementation-plan.md`

Связанные DECISIONS:
- `DECISIONS/2026-03-17-dual-ui-strategy-next-batteries-included.md`

---

## Текущее состояние

Платформа находится в состоянии **Production Ready** (v5.0). Все 4 спринта архитектурного улучшения завершены (17/17 задач).

| Метрика | До | После |
|---------|-----|-------|
| Architecture Score | 7.8/10 | 9.6/10 |
| Test Coverage | 31% | 80% |
| Security Score | 70% | 98% |

## История релизов

Релизные заметки хранятся в `docs/releases/`:

- [v4.1 (2026-02-17)](./releases/2026-02-17-v4.1.md) — консолидация документации и стабилизация платформы

Полная история изменений: [CHANGELOG.md](../CHANGELOG.md)

## Архитектурные рекомендации (живой документ)

Актуальный список рекомендаций по улучшению архитектуры:
→ [`docs/architecture/improvement-recommendations.md`](./architecture/improvement-recommendations.md)

Evidence workflow for `2.8`:
→ [`docs/architecture/performance-baseline.md`](./architecture/performance-baseline.md)

## Как обновлять roadmap

1. При закрытии значимого milestone обновляйте этот файл и `CHANGELOG.md`.
2. При выпуске новой версии создавайте файл в `docs/releases/YYYY-MM-DD-vX.Y.md`.
3. Архитектурные решения фиксируйте в `DECISIONS/` в формате ADR.
