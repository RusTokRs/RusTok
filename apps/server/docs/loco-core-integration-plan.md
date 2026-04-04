# План полной интеграции Loco RS + Core с управлением из админки

**Дата:** 2026-03-12
**Актуализировано:** 2026-03-19
**Статус:** Частично реализован; документ отражает текущее состояние и остаточный scope

## 1. Контекст и цель

RusToK использует Loco RS как server/runtime framework, а платформенные capability layers строит через `apps/server` и core/library crates.

Цель этого документа теперь не в том, чтобы заново спланировать уже сделанные шаги, а в том, чтобы честно разделить:

1. что уже интегрировано и стало частью live contract;
2. что реализовано частично;
3. что остаётся future scope.

> [!IMPORTANT]
> Архитектурный инвариант сохраняется: `apps/server` остаётся composition/integration layer и не становится владельцем доменной логики модулей.

---

## 2. Текущее состояние

### 2.1 Что уже зафиксировано в live runtime

| Capability | Текущее состояние |
|---|---|
| Application hooks / `Hooks` | Используются как основной runtime surface |
| Typed settings + YAML bootstrap | Работают |
| `platform_settings` + `SettingsService` | Реализованы |
| GraphQL settings API | Реализован (`platformSettings`, `allPlatformSettings`, `updatePlatformSettings`) |
| Auth lifecycle | Централизован через `AuthLifecycleService` |
| RBAC runtime | Живой path = `rustok-rbac` + Casbin-only |
| Mailer | Provider-based server service: `smtp | loco | none` |
| Storage | Shared runtime через `rustok-storage`; media domain через `rustok-media` |
| Event/outbox runtime | Реализован и остаётся source of truth |
| GraphQL module composition | Compile-time feature gating уже используется |
| Workflow runtime | Интегрирован в server |

### 2.2 Что больше нельзя описывать как “не внедрено”

- Loco Mailer уже участвует в live runtime через `EmailProvider::Loco`.
- Единый storage layer уже существует через `rustok-storage` и runtime bootstrap.
- `platform_settings` и schema version уже есть.
- GraphQL auth parity по ключевым операциям уже сильно продвинута: `logout`, `me`, `sessions`, revoke flows, invite acceptance.
- `schema.rs` уже не держит безусловные hard-coded доменные импорты: используется `#[cfg(feature = "mod-*")]`.

### 2.3 Что остаётся неполным

- admin UI покрывает не все platform settings / system observability сценарии;
- волна package-owned UI migration на native i18n contract закрыта; открытым остаётся только дальнейшее outbound locale propagation вне уже покрытых UI/built-in-auth-email путей;
- compile-time feature gating уже есть, но полностью runtime-dynamic schema registration как отдельная цель больше не является приоритетным current path;
- advanced scheduler/channels/graceful shutdown остаются отдельным future scope.

---

## 3. Статус по фазам

### Фаза 0 — i18n по умолчанию

**Статус:** Частично реализована.

Уже есть:

- request locale resolution chain в server runtime;
- `RequestContext.locale` как effective locale;
- locale fallback на read paths и GraphQL.
- locale-prefixed storefront routing (`/{locale}` и `/{locale}/modules/{route_segment}`) с backward-compatible fallback на legacy `?lang=`.
- built-in auth outbound email locale propagation: password reset для REST/GraphQL и email verification для REST; `smtp` и `loco` теперь используют один и тот же localized auth template path.

Осталось:

- более полный outbound locale propagation за пределами built-in auth email flows;
- расширение locale coverage для остальных outbound template flows и future work по новым локалям/форматерам.

### Фаза 1 — Settings API

**Статус:** Backend реализован, UI частично.

Уже есть:

- `platform_settings`;
- `schema_version`;
- `SettingsService`;
- built-in validators;
- категории включая `rate_limit`, `email`, `events`, `oauth`;
- GraphQL settings API;
- `PlatformSettingsChanged` через outbox path.

Осталось:

- более полный admin UX для platform settings в primary admin surfaces;
- выравнивание module settings UX там, где он ещё не оформлен.

### Фаза 1.5 — API parity

**Статус:** По ключевым auth сценариям в основном закрыта.

Подтверждено в коде:

- GraphQL: `logout`, `me`, `sessions`, `revoke_session`, `revoke_all_sessions`, `accept_invite`;
- REST: session management и auth lifecycle coverage;
- backend flows сведены к общему application service.

Остаточный scope по этой фазе больше не является главным блокером архитектурной целостности.

### Фаза 2 — Mailer

**Статус:** Основная интеграция реализована.

Уже есть:

- provider-based email runtime;
- `EmailProvider::{Smtp,Loco,None}`;
- `LocoMailerAdapter`;
- template-based built-in auth emails.

Осталось:

- если потребуется, более общий модульный email template contract;
- дальнейшее выравнивание observability и locale propagation для outbound mail.

### Фаза 3 — Storage + Media

**Статус:** Основной architecture shift реализован.

Уже есть:

- `rustok-storage` как shared storage contract;
- runtime bootstrap `StorageService`;
- `rustok-media` как core media module;
- media cleanup task и storage usage в server runtime.

Осталось:

- дальнейшее развитие media/admin UX;
- возможное расширение background lifecycle around storage GC/policies.

### Фаза 4 — Module settings + GraphQL composition

**Статус:** Частично реализована.

Уже есть:

- compile-time feature flags в `schema.rs`;
- runtime guards и module toggle model;
- `tenant_modules.settings` как persisted module setting payload.

Осталось:

- если нужно, дальнейшее развитие module settings schema/UI contracts;
- не runtime-dynamic GraphQL “любой ценой”, а согласованное развитие текущего feature-gated пути.

### Фаза 5 — Observability dashboard

**Статус:** Частично реализована.

Уже есть:

- `systemHealth` GraphQL surface;
- DLQ REST/admin flows;
- metrics and health endpoints;
- build/module UI pieces в admin.

Осталось:

- более цельный admin observability dashboard;
- pagination/UX вокруг additional system stats;
- consolidated alerting UX, если это останется в scope.

### Фаза 6 — Advanced runtime features

**Статус:** Future scope.

Сюда по-прежнему относятся:

- channels/websocket scenarios;
- more formal scheduler governance;
- graceful shutdown protocol hardening;
- дополнительные advanced runtime contracts beyond current baseline.

---

## 4. Что уже не является активным планом

Следующие пункты больше не должны трактоваться как открытые migration цели:

- “перейти на Loco Mailer” как будто mailer integration ещё отсутствует;
- “ввести storage layer” как будто shared storage ещё не существует;
- “добавить platform settings table” как будто DB/config split ещё не оформлен;
- “убрать hard-coded imports из `schema.rs`” как будто feature-gated composition ещё не внедрена;
- “включить Casbin/RBAC module runtime” как будто server всё ещё держит отдельный самописный живой engine.

---

## 5. Остаточный roadmap

Реальный остаточный scope на текущий момент:

1. Доводка platform/admin UX для settings, media и observability.
2. Дальнейшая i18n formalization beyond current request locale chain.
3. Advanced runtime features: channels, scheduler governance, graceful shutdown.
4. Дополнительная cleanup/consistency работа вокруг module settings contracts и operator dashboards.

---

## 6. Definition of Done для остаточного scope

Оставшийся план можно считать закрытым, когда:

- platform settings и system surfaces имеют согласованный admin UX;
- live docs не описывают уже закрытые migration steps как pending;
- i18n/runtime/platform contracts выровнены между server code и docs;
- future items сведены к отдельным roadmap/ADR, а не маскируются под незавершённую базовую интеграцию.

---

## Связанные документы

- [LOCO_FEATURE_SUPPORT.md](./LOCO_FEATURE_SUPPORT.md)
- [README.md](./README.md)
- [api.md](../../docs/architecture/api.md)
- [i18n.md](../../docs/architecture/i18n.md)
- [modules.md](../../docs/architecture/modules.md)
- [rustok-outbox docs](../../crates/rustok-outbox/docs/README.md)
- [overview.md](../../docs/architecture/overview.md)
