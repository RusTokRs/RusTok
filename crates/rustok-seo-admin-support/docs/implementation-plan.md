# План реализации `rustok-seo-admin-support`

Статус: support crate стабилизирован как reusable owner-side SEO UI слой. D6.1/D6.3 закрыты; execution wave синхронизирован с оставшимся треком (`D6.2`, `D8`, `D9`).

## Execution checkpoint

- Current phase: `phase_d6_transport_parity_followup`
- Last checkpoint: Legacy `src/api.rs` file removed; shared SEO GraphQL helpers now live in `src/transport.rs`, and `SeoEntityPanel` consumes the internal transport module directly while keeping owner-module API unchanged.
- Next step: Закрыть D6.2 — transport helpers parity (REST-first + GraphQL fallback) для diagnostics/sitemap/bulk control-plane read surfaces.
- Open blockers:
  - Для D6.2 нужно согласовать минимальный REST surface в support crate без дублирования heavy client-кода из Next admin.
  - Для D8 integration coverage потребуется согласованный fixture-набор ошибок permission/validation для Leptos owner panels.
- Hand-off notes for next agent:
  - Не переносить ownership entity screens в central SEO hub.
  - Поддерживать host-locale contract без package-local fallback chains.
  - Все новые UI виджеты должны работать одинаково для `pages/product/blog/forum`.
- Last updated at (UTC): 2026-06-07T22:40:00Z

## Цель

- не дублировать SEO panel logic в `pages`, `product`, `blog`, `forum` и будущих content-модулях;
- не превращать `rustok-seo-admin` в universal entity editor;
- держать reusable UI/tooling слой отдельно от SEO runtime и от owner-module screen ownership.

## Выполнено

- [x] создан support crate с root README и local docs;
- [x] вынесены shared GraphQL helper-ы для `seoMeta`, `upsertSeoMeta`, `publishSeoRevision`;
- [x] legacy `src/api.rs` file removed; shared SEO GraphQL helpers now live in `src/transport.rs`, and `SeoEntityPanel` consumes the internal transport module directly;
- [x] реализован `SeoEntityPanel` для owner-side entity editors;
- [x] реализован `SeoCapabilityNotice` для capability-slot сценариев;
- [x] встроены owner-side SEO panels в `rustok-pages/admin`, `rustok-product/admin`, `rustok-blog/admin`, `rustok-forum/admin`;
- [x] убран package-local locale override: support crate читает host effective locale, canonicalizes его и не держит editable locale field;
- [x] вынесены reusable snippet preview/recommendation/summary widgets;
- [x] raw `structured_data` textarea заменён на typed schema input contract с сохранением GraphQL write parity.

## Phase D backlog (SEO integration parity)

- [x] **D6.1 — Observability/remediation widgets**
  - [x] Добавить reusable cards для event delivery status (pending/sent/retry/failed/dead_letter) без жёсткой привязки к конкретному owner module layout.
  - [x] Добавить remediation hints для diagnostics issue-кодов с явным action mapping (`open_entity_editor`, `open_bulk_job`, `run_reindex`).

- [ ] **D6.2 — Transport helpers parity**
  - [ ] Расширить shared transport layer под REST parity endpoints из SEO Batch D4 (diagnostics summary, bulk job detail/status, sitemap job detail).
  - [ ] Сохранить fallback на текущий GraphQL contract, пока rollout-флаг REST parity выключен.

- [x] **D6.3 — UX consistency gates**
  - [x] Выделить единый visual/state contract для loading/error/permission/empty состояний.
  - [x] Привязать permission hints к canonical SEO permission model (`seo:read`, `seo:manage`).

- [ ] **D8 — Verification matrix**
  - [x] Unit tests для scoring/remediation mapping и locale wiring.
  - [ ] Integration tests для transport fallback (GraphQL/REST) и error envelope mapping.
  - [ ] Snapshot/smoke tests для reusable cards в owner layouts.

- [ ] **D9 — Docs/DoD sync**
  - [x] Обновить crate README/docs с operational guidance для owner modules.
  - [ ] Зафиксировать Definition of Done для reusable widget additions.

## Проверка

- `cargo check -p rustok-seo-admin-support --tests --config profile.dev.debug=0`
- `cargo check -p rustok-pages-admin --config profile.dev.debug=0`
- `cargo check -p rustok-product-admin --config profile.dev.debug=0`
- `cargo check -p rustok-blog-admin --config profile.dev.debug=0`
- `cargo check -p rustok-forum-admin --config profile.dev.debug=0`
- `npm --prefix apps/next-admin run lint && npm --prefix apps/next-admin run typecheck`

## Quality backlog

- [ ] Довести integration/snapshot coverage для новых observability/remediation widgets.
- [ ] Поддерживать transport compatibility matrix GraphQL/REST.
- [ ] Синхронизировать docs после каждого D6/D8 инкремента.
