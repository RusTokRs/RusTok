# Активный remediation-план платформы

- **Статус:** Активный execution plan
- **Контур:** текущая remediation-волна после закрытия native i18n/UI migration wave
- **Последнее обновление:** 2026-04-04

---

## Назначение

Этот документ описывает **текущий исполняемый план**, который команда должна довести до конца в ближайшей волне работ.

Он не относится к weekly verification-прогонам и не заменяет планы из `docs/verification/*`.

Разделение ролей такое:

- `docs/verification/*` — периодические повторяемые прогоны, чтобы функциональность не поплыла и проверки были в едином формате;
- этот документ — активный remediation backlog, который нужно **выполнить**, а не просто перепроверять по расписанию.

---

## Что уже закрыто и не должно планироваться заново

Следующие пункты считаются завершёнными и должны описываться как **live contract**, а не как pending migration:

- package-owned locale bundles для Leptos admin/storefront surfaces;
- `UiRouteContext.locale` как host-level locale contract для Leptos UI;
- capability-owned `rustok-ai-admin` на том же native i18n contract;
- машинная проверка parity через `npm run verify:i18n:ui`;
- locale-aware storefront route contract и проверка через `npm run verify:storefront:routes`;
- manifest-level `[provides.*_ui.i18n]` contract для module-owned UI packages, когда пакет реально поставляет свои translation bundles.

### Что нельзя переоткрывать в новой волне

Не нужно заново:

- писать, что `rustok-ai-admin` или package-owned Leptos UI migration ещё не завершены;
- предлагать новую массовую миграцию Leptos UI packages на locale contract;
- описывать bundle parity как ручную договорённость без `verify:i18n:ui`;
- смешивать уже закрытую native i18n/UI-фазу с future work по новым локалям или Fluent/FTL.

---

## Активный scope

### 1. GraphQL hardening

Нужно довести GraphQL security-модель до состояния, где чувствительные admin-операции контролируются только сервером.

В scope:

- убрать любые остатки security-логики, завязанные на `operationName`;
- не использовать client-supplied persisted metadata как security boundary;
- закрепить server-side AST/root-field policy для sensitive admin operations;
- сохранить depth/complexity limits как базовый защитный слой;
- выровнять shared transport contract для host apps и module-owned packages без app-specific исключений;
- добрать focused tests и docs evidence.

**Definition of Done:**

- sensitive admin operations режутся сервером независимо от имени операции;
- docs не описывают `operationName` как security boundary;
- есть точечные тесты и verification evidence для реального enforcement path.

### 2. Outbound locale propagation вне UI

Native i18n для UI уже закрыт, но не все outbound flows доведены до того же уровня.

В scope:

- добить locale propagation в остальных email/template flows за пределами password reset;
- убрать локальные ad hoc fallback на `"en"` в service/transport paths;
- зафиксировать единый runtime contract для user-facing outbound сообщений.

**Definition of Done:**

- outbound сообщения берут effective locale из request/runtime contract;
- в docs не остаётся формулировок, будто UI wave ещё блокирует этот этап;
- есть точечные проверки на локализованные email/template flows.

### 3. Stabilization и regression evidence для runtime/security

Часть security/runtime hardening уже внедрена, но должна быть закреплена как формально проверенный contract.

В scope:

- focused regression coverage для tenant strict resolution;
- trusted proxy / request-trust verification;
- disabled tenant rejection;
- CSP split для API/UI;
- single-policy rate limiting;
- authz semantics `403/500`.

**Definition of Done:**

- на каждый критичный runtime/security фикс есть узкий test или verification artifact;
- docs описывают эти части как закреплённый contract, а не как размытый WIP.

### 4. Docs closeout

После закрытия технических хвостов нужно довести документацию до состояния без двойных трактовок.

В scope:

- убрать stale-формулировки из `docs/`, `apps/*/docs/` и `crates/*/README.md`;
- чётко развести active remediation work и future work;
- поддерживать `docs/index.md` как каноническую карту: verification отдельно, execution plan отдельно.

**Definition of Done:**

- документация не переоткрывает уже закрытые migration steps;
- future work явно вынесен отдельно и не маскируется под незавершённую базовую платформенную интеграцию.

---

## Порядок выполнения

1. GraphQL hardening.
2. Outbound locale propagation вне UI.
3. Runtime/security stabilization evidence.
4. Финальный docs closeout для этой remediation-волны.

---

## Связанные документы

- [Карта документации](../index.md)
- [Главный verification README](../verification/README.md)
- [Frontend surfaces verification](../verification/platform-frontend-surfaces-verification-plan.md)
- [Архитектура i18n](../architecture/i18n.md)
- [GraphQL и Leptos server functions](../UI/graphql-architecture.md)
- [План интеграции Loco + Core](../../apps/server/docs/loco-core-integration-plan.md)
