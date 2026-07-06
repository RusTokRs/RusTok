# Complete Loco RS + Core Integration Plan with Admin Management

> Status: deprecated historical context. Active roadmap: [Loco RS Exit Plan](../../../docs/architecture/loco-exit-plan.md).

**Date:** 2026-03-12
**Updated:** 2026-04-05
**Status:** Partially implemented; document reflects current state and residual scope

## 1. Context and goal

RusToK uses Loco RS as server/runtime framework, and builds platform capability layers through `apps/server` and core/library crates.

The goal of this document now is not to re-plan already completed steps, but to honestly separate:

1. what is already integrated and became part of live contract;
2. what is partially implemented;
3. what remains future scope.

> [!IMPORTANT]
> Architectural invariant is preserved: `apps/server` remains composition/integration layer and does not become owner of domain logic for modules.

---

## 2. Current state

### 2.1 What is already established in live runtime

| Capability | Current state |
|---|---|
| Application hooks / `Hooks` | Used as main runtime surface |
| Typed settings + YAML bootstrap | Working |
| `platform_settings` + `SettingsService` | Implemented |
| GraphQL settings API | Implemented (`platformSettings`, `allPlatformSettings`, `updatePlatformSettings`) |
| Auth lifecycle | Centralized via `AuthLifecycleService` |
| RBAC runtime | Live path = `rustok-rbac` + tenant policy runtime |
| Mailer | Provider-based server service: `smtp | loco | none` |
| Storage | Shared runtime via `rustok-storage`; media domain via `rustok-media` |
| Event/outbox runtime | Implemented and remains source of truth |
| GraphQL module composition | Compile-time feature gating already used |
| Workflow runtime | Integrated in server |

### 2.2 What can no longer be described as "not implemented"

- Loco Mailer already participates in live runtime via `EmailProvider::Loco`.
- Unified storage layer already exists via `rustok-storage` and runtime bootstrap.
- `platform_settings` and schema version already exist.
- GraphQL auth parity for key operations already significantly advanced: `logout`, `me`, `sessions`, revoke flows, invite acceptance.
- `schema.rs` no longer holds unconditional hard-coded domain imports: uses `#[cfg(feature = "mod-*")]`.

### 2.3 What remains incomplete

- admin UI does not cover all platform settings / system observability scenarios;
- package-owned UI migration wave to native i18n contract is closed; only further outbound locale propagation outside already covered UI/built-in-auth-email paths remains open;
- compile-time feature gating already exists, but fully runtime-dynamic schema registration as separate goal is no longer prioritized current path;
- advanced scheduler/channels/graceful shutdown remain separate future scope.

---

## 3. Status by phases

### Phase 0 — i18n by default

**Status:** Live contract for server/runtime closed; residual scope moved to future work.

Already exists:

- request locale resolution chain in server runtime;
- `RequestContext.locale` as effective locale;
- locale fallback on read paths and GraphQL.
- locale-prefixed storefront routing (`/{locale}` and `/{locale}/modules/{route_segment}`) with backward-compatible fallback to legacy `?lang=`.
- built-in auth outbound email locale propagation: password reset for REST/GraphQL and email verification for REST; `smtp` and `loco` now use same localized auth template path.

Residual future scope:

- more complete outbound locale propagation beyond built-in auth email flows;
- expanded locale coverage for remaining outbound template flows and future work on new locales/formatters.

### Phase 1 — Settings API

**Status:** Backend implemented, UI partial.

Already exists:

- `platform_settings`;
- `schema_version`;
- `SettingsService`;
- built-in validators;
- categories including `rate_limit`, `email`, `events`, `oauth`;
- GraphQL settings API;
- `PlatformSettingsChanged` via outbox path.

Remains:

- more complete admin UX for platform settings in primary admin surfaces;
- alignment of module settings UX where it is not yet formalized.

### Phase 1.5 — API parity

**Status:** Mostly closed for key auth scenarios.

Confirmed in code:

- GraphQL: `logout`, `me`, `sessions`, `revoke_session`, `revoke_all_sessions`, `accept_invite`;
- REST: session management and auth lifecycle coverage;
- backend flows consolidated to common application service.

Residual scope for this phase is no longer main blocker for architectural integrity.

### Phase 2 — Mailer

**Status:** Core integration implemented.

Already exists:

- provider-based email runtime;
- `EmailProvider::{Smtp,Loco,None}`;
- `LocoMailerAdapter`;
- template-based built-in auth emails.

Remains:

- if needed, more general modular email template contract;
- further alignment of observability and locale propagation for outbound mail.

### Phase 3 — Storage + Media

**Status:** Core architecture shift implemented.

Already exists:

- `rustok-storage` as shared storage contract;
- runtime bootstrap `StorageService`;
- `rustok-media` as core media module;
- media cleanup task and storage usage in server runtime.

Remains:

- further development of media/admin UX;
- possible expansion of background lifecycle around storage GC/policies.

### Phase 4 — Module settings + GraphQL composition

**Status:** Partially implemented.

Already exists:

- compile-time feature flags in `schema.rs`;
- runtime guards and module toggle model;
- `tenant_modules.settings` as persisted module setting payload.

Remains:

- if needed, further development of module settings schema/UI contracts;
- not runtime-dynamic GraphQL "at any cost", but consistent development of current feature-gated path.

### Phase 5 — Observability dashboard

**Status:** Partially implemented.

Already exists:

- `systemHealth` GraphQL surface;
- DLQ REST/admin flows;
- metrics and health endpoints;
- build/module UI pieces in admin.

Remains:

- more cohesive admin observability dashboard;
- pagination/UX around additional system stats;
- consolidated alerting UX, if it remains in scope.

### Phase 6 — Advanced runtime features

**Status:** Future scope.

Still includes:

- channels/websocket scenarios;
- more formal scheduler governance;
- graceful shutdown protocol hardening;
- additional advanced runtime contracts beyond current baseline.

---

## 4. What is no longer an active plan

The following items should no longer be treated as open migration goals:

- "migrate to Loco Mailer" as if mailer integration is still absent;
- "introduce storage layer" as if shared storage does not yet exist;
- "add platform settings table" as if DB/config split is not yet formalized;
- "remove hard-coded imports from `schema.rs`" as if feature-gated composition is not yet implemented;
- "enable separate RBAC runtime" as if server still holds separate custom live engine.

---

## 5. Residual roadmap

Real residual scope at current moment:

1. Completion of platform/admin UX for settings, media and observability.
2. Further i18n formalization beyond current request locale chain.
3. Advanced runtime features: channels, scheduler governance, graceful shutdown.
4. Additional cleanup/consistency work around module settings contracts and operator dashboards.

---

## 6. Definition of Done for residual scope

Remaining plan can be considered closed when:

- platform settings and system surfaces have consistent admin UX;
- live docs do not describe already closed migration steps as pending;
- i18n/runtime/platform contracts are aligned between server code and docs;
- future items are consolidated to separate roadmap/ADR, not masked as incomplete basic integration.

---

## Related documents

- [LOCO_FEATURE_SUPPORT.md](./LOCO_FEATURE_SUPPORT.md)
- [README.md](./README.md)
- [api.md](../../docs/architecture/api.md)
- [i18n.md](../../docs/architecture/i18n.md)
- [modules.md](../../docs/architecture/modules.md)
- [rustok-outbox docs](../../crates/rustok-outbox/docs/README.md)
- [overview.md](../../docs/architecture/overview.md)
