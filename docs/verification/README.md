# Планы верификации

Этот раздел содержит специализированные планы верификации по отдельным контурам платформы.

## Назначение

- Хранить все verification-планы в одном месте.
- Обеспечить weekly-проход агентами по фиксированному чеклисту.
- Упростить контроль статуса: что проверено, что в работе, где есть блокеры.
- Не хранить здесь активный remediation backlog: execution/remediation хвосты фиксируются в профильных live docs (`docs/architecture/*`, `docs/UI/*`, `apps/*/docs/*`), отдельно от periodic verification.

## Список планов

- [Главный план верификации платформы](./PLATFORM_VERIFICATION_PLAN.md) — reset-friendly master-checklist для периодических прогонов.
- [План foundation-верификации](./platform-foundation-verification-plan.md) — сборка, архитектура, ядро, auth, RBAC, tenancy.
- [План верификации событий, доменов и интеграций](./platform-domain-events-integrations-verification-plan.md) — события, доменные модули, интеграционные связи.
- [План верификации API-поверхностей](./platform-api-surfaces-verification-plan.md) — GraphQL и REST контракты.
- [План верификации frontend-поверхностей](./platform-frontend-surfaces-verification-plan.md) — Leptos, Next.js, UI libraries и shared packages.
- [План верификации качества и эксплуатационной готовности](./platform-quality-operations-verification-plan.md) — тесты, observability, документация, CI/CD, security, quality и phased rollout quality-tooling пакета.
- [План rolling-верификации RBAC для server и runtime-модулей](./rbac-server-modules-verification-plan.md) — прицельный rolling-план по RBAC-контрактам.
- [Верификация Leptos-библиотек](./leptos-libraries-verification-plan.md) — rolling-план библиотечного UI-контура.
- [План rolling-верификации целостности ядра платформы](./platform-core-integrity-verification-plan.md) — верификация server + обе admin-панели + core crates как самодостаточного целого.

## Регламент обновления

При изменении архитектуры, API, UI-контрактов, поведения библиотек или процесса верификации:

1. Обновить соответствующий план в этой папке.
2. Обновить профильные локальные документы в `apps/*` и `crates/*`.
3. Обновить центральные документы в `docs/` (включая `docs/index.md`).
4. Если изменение затрагивает Internal UI workspace (`docs/UI/`), синхронизировать и документы из `docs/UI/`.

## Формат статусов

- `⬜ Не начато`
- `🟡 В процессе`
- `✅ Завершено`
- `❌ Блокировано`
## Как использовать набор планов

1. Начинать с [главного плана](./PLATFORM_VERIFICATION_PLAN.md) как с orchestration-точки входа.
2. Проходить укрупнённые блоки через детальные платформенные планы.
3. Подключать rolling-планы (`RBAC`, `Leptos libraries`) только когда менялся соответствующий контур или нужен targeted-аудит.
4. Нерешённые блокеры фиксировать прямо в соответствующем verification-плане до их устранения.
